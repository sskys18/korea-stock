//! 실시간 WebSocket 클라이언트.
//!
//! 설계: docs/specs/2026-05-22-kis-adapter-design.md §3.7.

mod approval;
mod crypto;
mod decode;
mod run;
mod subscribe;

pub use decode::{
    MarketOperation, MemberTrade, OrderNotice, OverseasTrade, ProgramTrade, StockAsking, StockTrade,
};
pub use subscribe::{SubscriptionHandle, SubscriptionKind};

use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};

use crate::kis::config::KisConfig;
use crate::kis::error::{KisError, Result};
use crate::kis::realtime::subscribe::ControlMsg;

/// 이벤트 채널 용량 (D1).
const EVENT_CHANNEL_CAP: usize = 1024;
/// 동시 구독 한도 (스펙 §3.7, §8 — 약 41건).
const MAX_SUBSCRIPTIONS: usize = 41;

/// 호출자에게 전달되는 실시간 이벤트.
///
/// 모든 데이터 이벤트에 `tr_id`+`tr_key`가 포함되어 호출자가 분기 가능
/// (스펙 §3.7). `tr_id`는 수신 프레임 `[1]`에서 추출 — 체결통보의 실전
/// (`H0STCNI0`)/모의(`H0STCNI9`) 구분도 이 값으로 가능.
// 실시간 핫패스 — variant마다 큰 레코드이나 per-tick 힙 할당 회피 위해 박싱 안 함.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum RealtimeEvent {
    /// 국내주식 체결가 (H0STCNT0).
    DomesticTrade {
        tr_id: String,
        tr_key: String,
        data: decode::StockTrade,
    },
    /// 국내주식 호가 (H0{ST,NX,UN}ASP0). 시장은 tr_id로 구분.
    DomesticAsking {
        tr_id: String,
        tr_key: String,
        data: decode::StockAsking,
    },
    /// 국내주식 예상체결 (H0{ST,NX,UN}ANC0). 시장은 tr_id로 구분.
    ExpectedConclusion {
        tr_id: String,
        tr_key: String,
        data: decode::StockTrade,
    },
    /// 국내주식 장운영 (H0{ST,NX,UN}MKO0). 통합(UN)은 tr_key 빈값.
    MarketOperation {
        tr_id: String,
        tr_key: String,
        data: decode::MarketOperation,
    },
    /// 국내주식 회원사 (H0{ST,NX,UN}MBC0).
    MemberTrade {
        tr_id: String,
        tr_key: String,
        data: decode::MemberTrade,
    },
    /// 국내주식 프로그램매매 (H0{ST,NX,UN}PGM0).
    ProgramTrade {
        tr_id: String,
        tr_key: String,
        data: decode::ProgramTrade,
    },
    /// 체결통보 (H0STCNI0/9).
    OrderNotice {
        tr_id: String,
        tr_key: String,
        data: decode::OrderNotice,
    },
    /// 해외주식 체결가 (HDFSCNT0).
    OverseasTrade {
        tr_id: String,
        tr_key: String,
        data: decode::OverseasTrade,
    },
    /// 채널 포화로 n건 유실 (D1 — drop-newest + 이 통지).
    Lagged(u64),
    /// 연결 끊김 — 자동 재연결·재구독 진행 중.
    Reconnecting,
    /// 재연결·재구독 완료.
    Reconnected,
}

/// 구독 1건의 상태. 재연결 시 재전송에 사용.
#[derive(Debug, Clone)]
pub(crate) struct SubState {
    pub kind: SubscriptionKind,
    pub tr_key: String,
}

/// 구독 상태 공유 — 재연결 중 subscribe/unsubscribe 직렬화 (스펙 §3.7).
pub(crate) type SubMap = Arc<Mutex<HashMap<(String, String), SubState>>>;

/// 실시간 WebSocket 클라이언트.
///
/// `KisClient::realtime()`으로 생성. 생성 시 자격증명 검증 + 연결 루프
/// 태스크 spawn (WS 연결·approval_key 발급은 루프 태스크가 수행). 이벤트는
/// `take_events()`로 한 번 꺼내는 단일 `mpsc::Receiver`로 흐른다.
pub struct RealtimeClient {
    config: KisConfig,
    subs: SubMap,
    control_tx: mpsc::UnboundedSender<ControlMsg>,
    /// 이벤트 수신단 — `take_events()`로 한 번만 꺼낼 수 있음.
    events: Option<mpsc::Receiver<RealtimeEvent>>,
}

impl RealtimeClient {
    /// 실시간 클라이언트 생성. 검증용 approval_key 1회 발급(자격증명 사전
    /// 확인) → 연결 루프 태스크 spawn. 실제 연결마다 approval_key를 재발급
    /// 하므로(D7), `connect`의 발급은 자격증명 유효성 조기 검증 목적이다.
    /// 내부 호출: `KisClient::realtime()`.
    pub(crate) async fn connect(config: KisConfig, http: reqwest::Client) -> Result<Self> {
        let _ = approval::issue_approval_key(&http, &config).await?;
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAP);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let subs: SubMap = Arc::new(Mutex::new(HashMap::new()));

        run::spawn_connection_loop(run::ConnectionCtx {
            config: config.clone(),
            http,
            subs: subs.clone(),
            event_tx,
            control_rx,
            lag: Arc::new(AtomicU64::new(0)),
        });

        Ok(Self {
            config,
            subs,
            control_tx,
            events: Some(event_rx),
        })
    }

    /// 이벤트 수신 채널을 꺼낸다. 최초 1회만 `Some` — 이후 `None`.
    pub fn take_events(&mut self) -> Option<mpsc::Receiver<RealtimeEvent>> {
        self.events.take()
    }

    /// 실시간 데이터 구독. 한도(41건) 초과 시 `KisError::Ws`.
    pub async fn subscribe(&self, kind: SubscriptionKind, key: &str) -> Result<SubscriptionHandle> {
        let tr_id = kind.tr_id(self.config.environment).to_string();
        let tr_key = key.to_string();
        {
            let mut map = self.subs.lock().await;
            if map.len() >= MAX_SUBSCRIPTIONS && !map.contains_key(&(tr_id.clone(), tr_key.clone()))
            {
                return Err(KisError::Ws(format!(
                    "subscription limit {MAX_SUBSCRIPTIONS} reached"
                )));
            }
            map.insert(
                (tr_id.clone(), tr_key.clone()),
                SubState {
                    kind,
                    tr_key: tr_key.clone(),
                },
            );
        }
        self.control_tx
            .send(ControlMsg::Subscribe {
                tr_id: tr_id.clone(),
                tr_key: tr_key.clone(),
            })
            .map_err(|_| KisError::Ws("connection task gone".into()))?;
        Ok(SubscriptionHandle::new(
            tr_id,
            tr_key,
            self.control_tx.clone(),
        ))
    }
}
