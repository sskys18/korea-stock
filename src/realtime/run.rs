//! WS 연결 수명 관리 — 연결, 수신·송신, PINGPONG, 재연결.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::config::KisConfig;
use crate::realtime::decode::{self, ControlMessage, DecodedRecord};
use crate::realtime::subscribe::{build_frame, ControlMsg};
use crate::realtime::{RealtimeEvent, SubMap};

/// 연결 루프 컨텍스트.
pub(crate) struct ConnectionCtx {
    pub config: KisConfig,
    /// approval_key 재발급용 (D7 — 연결마다 새로 발급).
    pub http: reqwest::Client,
    pub subs: SubMap,
    pub event_tx: mpsc::Sender<RealtimeEvent>,
    pub control_rx: mpsc::UnboundedReceiver<ControlMsg>,
    pub lag: Arc<AtomicU64>,
}

/// 연결+재연결 루프를 백그라운드 태스크로 spawn.
pub(crate) fn spawn_connection_loop(ctx: ConnectionCtx) {
    tokio::spawn(async move { connection_loop(ctx).await });
}

async fn connection_loop(mut ctx: ConnectionCtx) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);
    let url = ctx.config.environment.ws_base().to_string();

    loop {
        match run_one_connection(&mut ctx, &url).await {
            ConnEnd::ClientGone => return,
            ConnEnd::Disconnected => {
                let _ = ctx.event_tx.send(RealtimeEvent::Reconnecting).await;
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}

enum ConnEnd {
    /// control_rx가 닫힘 — 클라이언트 drop, 영구 종료.
    ClientGone,
    /// 연결 끊김 — 재연결 대상.
    Disconnected,
}

/// 단일 연결의 수명. 끊기면 반환, control_rx 종료면 ClientGone.
async fn run_one_connection(ctx: &mut ConnectionCtx, url: &str) -> ConnEnd {
    let approval_key =
        match crate::realtime::approval::issue_approval_key(&ctx.http, &ctx.config).await {
            Ok(k) => k,
            Err(e) => {
                tracing::warn!("approval_key reissue failed: {e}");
                return ConnEnd::Disconnected;
            }
        };

    let (ws, _) = match tokio_tungstenite::connect_async(url).await {
        Ok(ok) => ok,
        Err(e) => {
            tracing::warn!("ws connect failed: {e}");
            return ConnEnd::Disconnected;
        }
    };
    let (mut sink, mut stream) = ws.split();

    // 죽은 연결 동안 큐에 쌓인 control 메시지를 resubscribe 전에 선반영 —
    // drop된 핸들의 Unsubscribe가 먼저 처리돼 해지된 구독이 재전송(replay)되지 않도록.
    {
        let mut map = ctx.subs.lock().await;
        while let Ok(msg) = ctx.control_rx.try_recv() {
            if let ControlMsg::Unsubscribe { tr_id, tr_key } = msg {
                map.remove(&(tr_id, tr_key));
            }
        }
    }

    {
        let map = ctx.subs.lock().await;
        for ((tr_id, tr_key), state) in map.iter() {
            let expected_tr_id = state.kind.tr_id(ctx.config.environment);
            if expected_tr_id != tr_id.as_str() {
                tracing::warn!("subscription state mismatch: {tr_id} != {expected_tr_id}");
            }
            if state.tr_key.as_str() != tr_key.as_str() {
                tracing::warn!("subscription key mismatch: {tr_key} != {}", state.tr_key);
            }
            let frame = build_frame(&approval_key, tr_id, tr_key, true);
            if sink.send(Message::Text(frame)).await.is_err() {
                return ConnEnd::Disconnected;
            }
        }
        if !map.is_empty() {
            let _ = ctx.event_tx.send(RealtimeEvent::Reconnected).await;
        }
    }

    // tr_id별 AES key/iv 캐시 (D4).
    let mut creds: std::collections::HashMap<String, crate::realtime::crypto::AesCreds> =
        std::collections::HashMap::new();

    loop {
        tokio::select! {
            ctl = ctx.control_rx.recv() => {
                match ctl {
                    None => return ConnEnd::ClientGone,
                    Some(ControlMsg::Subscribe { tr_id, tr_key }) => {
                        let f = build_frame(&approval_key, &tr_id, &tr_key, true);
                        if sink.send(Message::Text(f)).await.is_err() {
                            return ConnEnd::Disconnected;
                        }
                    }
                    Some(ControlMsg::Unsubscribe { tr_id, tr_key }) => {
                        ctx.subs.lock().await.remove(&(tr_id.clone(), tr_key.clone()));
                        let f = build_frame(&approval_key, &tr_id, &tr_key, false);
                        let _ = sink.send(Message::Text(f)).await;
                    }
                }
            }
            msg = stream.next() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => {
                        tracing::warn!("ws recv error: {e}");
                        return ConnEnd::Disconnected;
                    }
                    None => return ConnEnd::Disconnected,
                };
                match msg {
                    Message::Text(text) => {
                        if handle_text(ctx, &mut sink, &mut creds, text.as_ref())
                            .await
                            .is_err()
                        {
                            return ConnEnd::Disconnected;
                        }
                    }
                    Message::Ping(p) => {
                        let _ = sink.send(Message::Pong(p)).await;
                    }
                    Message::Close(_) => return ConnEnd::Disconnected,
                    _ => {}
                }
            }
        }
    }
}

/// 텍스트 프레임 처리. Err = 송신 실패(연결 끊김).
async fn handle_text<S>(
    ctx: &ConnectionCtx,
    sink: &mut S,
    creds: &mut std::collections::HashMap<String, crate::realtime::crypto::AesCreds>,
    text: &str,
) -> std::result::Result<(), ()>
where
    S: futures_util::Sink<Message> + Unpin,
{
    let first = text.as_bytes().first().copied();
    match first {
        Some(b'0') | Some(b'1') => {
            let tr_id = text.split('|').nth(1).unwrap_or_default();
            let cred = creds.get(tr_id);
            match decode::decode_frame(text, cred) {
                Ok(records) => {
                    if records.is_empty() && first == Some(b'1') && cred.is_none() {
                        tracing::warn!(
                            "encrypted frame dropped before aes key/iv ack for tr_id={tr_id}"
                        );
                    }
                    for rec in records {
                        emit_record(ctx, tr_id, rec).await;
                    }
                }
                Err(e) => tracing::warn!("frame decode failed: {e}"),
            }
        }
        _ => {
            match decode::decode_control(text) {
                Ok(ControlMessage::PingPong) => {
                    // realtime.md §A-4 — 받은 데이터를 WebSocket Pong 프레임으로 그대로 되돌림.
                    sink.send(Message::Pong(text.as_bytes().to_vec()))
                        .await
                        .map_err(|_| ())?;
                }
                Ok(ControlMessage::SubscribeAck {
                    tr_id,
                    tr_key,
                    rt_cd,
                    msg,
                    creds: ack_creds,
                }) => {
                    if rt_cd != "0" && !rt_cd.is_empty() {
                        tracing::warn!(
                            "subscribe ack tr_id={tr_id} tr_key={tr_key} rt_cd={rt_cd}: {msg}"
                        );
                    }
                    if let Some(c) = ack_creds {
                        creds.insert(tr_id, c);
                    }
                }
                Err(e) => tracing::warn!("control decode failed: {e}"),
            }
        }
    }
    Ok(())
}

/// 디코드된 레코드를 이벤트로 변환해 채널에 송신. 포화 시 drop-newest + lag (D1).
/// `tr_id`는 수신 프레임 `[1]`에서 추출한 값.
async fn emit_record(ctx: &ConnectionCtx, tr_id: &str, rec: DecodedRecord) {
    let tr_id = tr_id.to_string();
    let event = match rec {
        DecodedRecord::StockTrade(d) => RealtimeEvent::DomesticTrade {
            tr_id,
            tr_key: d.mksc_shrn_iscd.clone(),
            data: d,
        },
        DecodedRecord::ExpectedConclusion(d) => RealtimeEvent::ExpectedConclusion {
            tr_id,
            tr_key: d.mksc_shrn_iscd.clone(),
            data: d,
        },
        DecodedRecord::StockAsking(d) => RealtimeEvent::DomesticAsking {
            tr_id,
            tr_key: d.mksc_shrn_iscd.clone(),
            data: d,
        },
        DecodedRecord::MarketOperation(d) => RealtimeEvent::MarketOperation {
            tr_id,
            tr_key: d.mksc_shrn_iscd.clone(),
            data: d,
        },
        DecodedRecord::MemberTrade(d) => RealtimeEvent::MemberTrade {
            tr_id,
            tr_key: d.mksc_shrn_iscd.clone(),
            data: d,
        },
        DecodedRecord::ProgramTrade(d) => RealtimeEvent::ProgramTrade {
            tr_id,
            tr_key: d.mksc_shrn_iscd.clone(),
            data: d,
        },
        DecodedRecord::OrderNotice(d) => RealtimeEvent::OrderNotice {
            tr_id,
            tr_key: d.cust_id.clone(),
            data: d,
        },
        DecodedRecord::OverseasTrade(d) => RealtimeEvent::OverseasTrade {
            tr_id,
            // 구독 키는 실시간 종목코드(rsym) — symb(단축코드)가 아님.
            tr_key: d.rsym.clone(),
            data: d,
        },
    };

    let pending = ctx.lag.swap(0, Ordering::Relaxed);
    if pending > 0
        && ctx
            .event_tx
            .try_send(RealtimeEvent::Lagged(pending))
            .is_err()
    {
        ctx.lag.fetch_add(pending, Ordering::Relaxed);
    }

    if ctx.event_tx.try_send(event).is_err() {
        ctx.lag.fetch_add(1, Ordering::Relaxed);
    }
}
