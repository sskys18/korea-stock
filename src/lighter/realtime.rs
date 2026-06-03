//! 실시간 시세 (WS 오더북) — `wss://{host}/stream`.
//!
//! Lighter는 마켓당 단일 WS 채널 `order_book/{market_id}`만 제공한다. 구독 시 전체
//! 스냅샷(`subscribed/order_book`)을 받고, 이후 가격대(level) 증분 델타
//! (`update/order_book`)가 흐른다. Hyperliquid의 `bbo`처럼 top-of-book 전용 스트림은
//! 없으므로, 본 어댑터가 마켓별 호가장 상태를 직접 유지하고 BBO(최우선 매수/매도)를
//! 파생한다.
//!
//! 엔드포인트(메인넷, 2026-06 확인): `wss://mainnet.zklighter.elliot.ai/stream`.
//! [`crate::lighter::DEFAULT_BASE_URL`]의 `https`를 `wss`로 치환해 유도한다.
//!
//! 프레임 형태는 kimp `tests/fixtures/lighter_ws_*.json`(BTC market_id=1, 2026-05 캡처)을
//! 그대로 #[cfg(test)]에 반영했다 — 이 부분은 **검증 가능**하다(픽스처 존재).
//!
//! 설계: 본 어댑터는 시세 표시·BBO 파생에 쓰는 **연결 스트림**을 제공한다. 가격·수량은
//! 정밀도 보존 위해 String 원문을 유지하되, 최우선가 비교를 위해 f64 파싱을 병행한다
//! (Lighter는 마켓별 고정 `supported_price_decimals`로 호가를 보내므로 같은 수치 레벨은
//! 항상 같은 문자열로 도착한다 — 문자열 키로 안전하게 dedup된다).
//!
//! ```ignore
//! use korea_stock::lighter::realtime::{LighterRealtime, BookEvent};
//!
//! let rt = LighterRealtime::mainnet();
//! let mut rx = rt.subscribe(&[162]).await?;  // 삼성전자 USD market_id
//! while let Some(ev) = rx.recv().await {
//!     if let BookEvent::Bbo { market_id, bid, ask, .. } = ev {
//!         println!("{market_id}: {bid:?} / {ask:?}");
//!     }
//! }
//! ```

use std::collections::HashMap;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::lighter::error::{LighterError, Result};

/// 메인넷 WS 스트림 URL.
pub const MAINNET_WS_URL: &str = "wss://mainnet.zklighter.elliot.ai/stream";
/// 테스트넷 WS 스트림 URL.
pub const TESTNET_WS_URL: &str = "wss://testnet.zklighter.elliot.ai/stream";

/// 호가 1단계 (가격·수량 모두 String 원문 보존).
#[derive(Debug, Clone, Deserialize)]
pub struct Level {
    pub price: String,
    pub size: String,
}

/// WS 오더북 페이로드. 스냅샷·델타 동일 형태.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct WsOrderBook {
    #[serde(default)]
    pub asks: Vec<Level>,
    #[serde(default)]
    pub bids: Vec<Level>,
}

/// WS 최상위 envelope. `subscribed/order_book`·`update/order_book` 공용.
///
/// `channel`은 `"order_book:1"`처럼 콜론 뒤 market_id를 싣는다(구독 시엔 슬래시
/// `order_book/{id}`로 보내지만, 서버 프레임의 channel은 콜론 형태다 — kimp 픽스처 확인).
#[derive(Debug, Clone, Deserialize)]
pub struct WsMessage {
    #[serde(default, rename = "type")]
    pub msg_type: Option<String>,
    #[serde(default)]
    pub channel: Option<String>,
    #[serde(default)]
    pub order_book: Option<WsOrderBook>,
    #[serde(default)]
    pub timestamp: Option<i64>,
}

/// 마켓별 호가장 상태. 가격 문자열 → 수량 f64.
#[derive(Default, Debug)]
pub struct BookState {
    bids: HashMap<String, f64>,
    asks: HashMap<String, f64>,
}

impl BookState {
    /// 스냅샷으로 전체 교체.
    pub fn replace(&mut self, book: &WsOrderBook) {
        self.bids.clear();
        self.asks.clear();
        for lvl in &book.bids {
            if let Ok(sz) = lvl.size.parse::<f64>() {
                if sz > 0.0 {
                    self.bids.insert(lvl.price.clone(), sz);
                }
            }
        }
        for lvl in &book.asks {
            if let Ok(sz) = lvl.size.parse::<f64>() {
                if sz > 0.0 {
                    self.asks.insert(lvl.price.clone(), sz);
                }
            }
        }
    }

    /// 델타 적용 — size=0 레벨은 삭제, 그 외는 갱신/삽입.
    pub fn apply_delta(&mut self, book: &WsOrderBook) {
        for lvl in &book.bids {
            apply_level(&mut self.bids, &lvl.price, &lvl.size);
        }
        for lvl in &book.asks {
            apply_level(&mut self.asks, &lvl.price, &lvl.size);
        }
    }

    /// 최우선 매수가 (최댓값).
    pub fn best_bid(&self) -> Option<f64> {
        best_price(&self.bids, true)
    }

    /// 최우선 매도가 (최솟값).
    pub fn best_ask(&self) -> Option<f64> {
        best_price(&self.asks, false)
    }
}

fn apply_level(side: &mut HashMap<String, f64>, price: &str, size: &str) {
    let Ok(parsed) = size.parse::<f64>() else {
        return;
    };
    if parsed == 0.0 {
        side.remove(price);
    } else {
        side.insert(price.to_string(), parsed);
    }
}

fn best_price(side: &HashMap<String, f64>, highest: bool) -> Option<f64> {
    side.keys()
        .filter_map(|p| p.parse::<f64>().ok())
        .fold(None, |acc, px| match acc {
            None => Some(px),
            Some(cur) if highest && px > cur => Some(px),
            Some(cur) if !highest && px < cur => Some(px),
            other => other,
        })
}

/// 소비자에게 전달되는 이벤트.
#[derive(Debug, Clone)]
pub enum BookEvent {
    /// 갱신된 최우선 매수/매도가. 한쪽이라도 비면 None.
    Bbo {
        market_id: u32,
        bid: Option<f64>,
        ask: Option<f64>,
        /// 서버 timestamp(ms). 없으면 0.
        ts: i64,
    },
    /// 연결 끊김 — 재연결 시도 중.
    Reconnecting,
}

/// Lighter 실시간 오더북 어댑터.
pub struct LighterRealtime {
    ws_url: String,
}

impl LighterRealtime {
    /// 메인넷 어댑터.
    pub fn mainnet() -> Self {
        Self {
            ws_url: MAINNET_WS_URL.to_string(),
        }
    }

    /// 테스트넷 어댑터.
    pub fn testnet() -> Self {
        Self {
            ws_url: TESTNET_WS_URL.to_string(),
        }
    }

    /// 임의 WS URL.
    pub fn with_url(ws_url: impl Into<String>) -> Self {
        Self {
            ws_url: ws_url.into(),
        }
    }

    /// 주어진 market_id들을 구독하고 BBO 이벤트 스트림을 반환한다.
    ///
    /// 백그라운드 태스크가 연결·구독·델타 적용·재연결을 담당한다. 반환된 receiver가
    /// drop되면 태스크는 다음 전송 실패 시 종료한다. 연결이 끊기면 지수 백오프
    /// (1→30초)로 재연결하며 그 사이 [`BookEvent::Reconnecting`]을 1회 보낸다.
    pub async fn subscribe(&self, market_ids: &[u32]) -> Result<mpsc::Receiver<BookEvent>> {
        if market_ids.is_empty() {
            return Err(LighterError::Auth("subscribe: market_ids empty".into()));
        }
        let (tx, rx) = mpsc::channel(256);
        let url = self.ws_url.clone();
        let ids: Vec<u32> = market_ids.to_vec();
        tokio::spawn(async move {
            connection_loop(url, ids, tx).await;
        });
        Ok(rx)
    }
}

/// 연결+재연결 루프.
async fn connection_loop(url: String, market_ids: Vec<u32>, tx: mpsc::Sender<BookEvent>) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);

    loop {
        match run_one_connection(&url, &market_ids, &tx).await {
            ConnEnd::ConsumerGone => return,
            ConnEnd::Disconnected => {
                if tx.send(BookEvent::Reconnecting).await.is_err() {
                    return;
                }
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}

enum ConnEnd {
    /// 소비자 receiver drop — 영구 종료.
    ConsumerGone,
    /// 연결 끊김 — 재연결 대상.
    Disconnected,
}

async fn run_one_connection(
    url: &str,
    market_ids: &[u32],
    tx: &mpsc::Sender<BookEvent>,
) -> ConnEnd {
    let (mut ws, _) = match tokio_tungstenite::connect_async(url).await {
        Ok(ok) => ok,
        Err(e) => {
            tracing::warn!("lighter ws connect failed: {e}");
            return ConnEnd::Disconnected;
        }
    };

    for id in market_ids {
        let sub = serde_json::json!({
            "type": "subscribe",
            "channel": format!("order_book/{id}"),
        });
        if ws.send(Message::Text(sub.to_string())).await.is_err() {
            return ConnEnd::Disconnected;
        }
    }

    let mut books: HashMap<u32, BookState> = HashMap::new();

    // 서버는 120초 무프레임 시 연결을 닫는다 — 30초마다 app-level ping.
    let mut ping = tokio::time::interval(Duration::from_secs(30));
    ping.tick().await; // 즉시 만료되는 첫 tick 소비.

    loop {
        tokio::select! {
            _ = ping.tick() => {
                let frame = serde_json::json!({ "type": "ping" });
                if ws.send(Message::Text(frame.to_string())).await.is_err() {
                    return ConnEnd::Disconnected;
                }
            }
            msg = ws.next() => {
                let Some(msg) = msg else { return ConnEnd::Disconnected };
                let msg = match msg {
                    Ok(m) => m,
                    Err(e) => {
                        tracing::warn!("lighter ws read error: {e}");
                        return ConnEnd::Disconnected;
                    }
                };
                match msg {
                    Message::Text(text) => {
                        match serde_json::from_str::<WsMessage>(&text) {
                            Ok(parsed) => {
                                // JSON으로 실린 서버 ping → pong.
                                if parsed.msg_type.as_deref() == Some("ping") {
                                    let pong = serde_json::json!({ "type": "pong" });
                                    let _ = ws.send(Message::Text(pong.to_string())).await;
                                    continue;
                                }
                                if let Some(ev) = process_message(&mut books, parsed) {
                                    if tx.send(ev).await.is_err() {
                                        return ConnEnd::ConsumerGone;
                                    }
                                }
                            }
                            Err(e) => tracing::debug!("lighter ws parse skip: {e}"),
                        }
                    }
                    Message::Ping(data) => {
                        let _ = ws.send(Message::Pong(data)).await;
                    }
                    Message::Close(_) => return ConnEnd::Disconnected,
                    _ => {}
                }
            }
        }
    }
}

/// 프레임 1개 처리 → BBO 이벤트(있으면). 스냅샷=replace, 델타=apply_delta.
fn process_message(books: &mut HashMap<u32, BookState>, msg: WsMessage) -> Option<BookEvent> {
    let kind = msg.msg_type.as_deref()?;
    let channel = msg.channel.as_deref()?;
    // 서버 프레임 channel은 "order_book:1" — 콜론 뒤 market_id.
    let id_str = channel.strip_prefix("order_book:")?;
    let market_id: u32 = id_str.parse().ok()?;
    let book = msg.order_book?;

    let state = books.entry(market_id).or_default();
    match kind {
        "subscribed/order_book" => state.replace(&book),
        "update/order_book" => state.apply_delta(&book),
        _ => return None,
    }

    Some(BookEvent::Bbo {
        market_id,
        bid: state.best_bid(),
        ask: state.best_ask(),
        ts: msg.timestamp.unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delta_applies_remove_and_insert() {
        let mut state = BookState::default();
        state.replace(&WsOrderBook {
            bids: vec![
                Level { price: "100.0".into(), size: "1".into() },
                Level { price: "99.0".into(), size: "2".into() },
            ],
            asks: vec![
                Level { price: "101.0".into(), size: "1".into() },
                Level { price: "102.0".into(), size: "2".into() },
            ],
        });
        assert_eq!(state.best_bid(), Some(100.0));
        assert_eq!(state.best_ask(), Some(101.0));

        // 최우선 매수 삭제 + 더 높은 매수 추가, 최우선 매도 삭제.
        state.apply_delta(&WsOrderBook {
            bids: vec![
                Level { price: "100.0".into(), size: "0".into() },
                Level { price: "100.5".into(), size: "3".into() },
            ],
            asks: vec![Level { price: "101.0".into(), size: "0".into() }],
        });
        assert_eq!(state.best_bid(), Some(100.5));
        assert_eq!(state.best_ask(), Some(102.0));
    }

    // kimp 픽스처(BTC market_id=1)와 동일한 프레임 형태로 스냅샷→델타 검증.
    // 스냅샷 best bid=75023.7, best ask=75023.9.
    #[test]
    fn snapshot_frame_emits_bbo() {
        let snapshot = r#"{
            "channel": "order_book:1",
            "order_book": {
                "code": 0,
                "asks": [
                    {"price":"75023.9","size":"0.00020"},
                    {"price":"75024.0","size":"0.00020"}
                ],
                "bids": [
                    {"price":"75023.7","size":"0.04114"},
                    {"price":"75023.0","size":"0.00160"}
                ]
            },
            "timestamp": 1779893750117,
            "type": "subscribed/order_book"
        }"#;
        let mut books = HashMap::new();
        let parsed: WsMessage = serde_json::from_str(snapshot).unwrap();
        let ev = process_message(&mut books, parsed).expect("snapshot must emit bbo");
        let BookEvent::Bbo { market_id, bid, ask, ts } = ev else {
            panic!("expected Bbo");
        };
        assert_eq!(market_id, 1);
        assert_eq!(bid, Some(75023.7));
        assert_eq!(ask, Some(75023.9));
        assert_eq!(ts, 1779893750117);
    }

    #[test]
    fn update_frame_applies_delta() {
        let snapshot = r#"{
            "channel": "order_book:1",
            "order_book": {
                "asks": [{"price":"75023.9","size":"0.00020"}],
                "bids": [{"price":"75023.7","size":"0.04114"}]
            },
            "type": "subscribed/order_book"
        }"#;
        // 델타: 매수 75017.1·75012.9를 size=0으로(삭제), 75018.1 추가; 매도 갱신.
        let update = r#"{
            "channel": "order_book:1",
            "order_book": {
                "asks": [{"price":"75030.9","size":"0.36332"}],
                "bids": [
                    {"price":"75018.1","size":"0.01652"},
                    {"price":"75023.7","size":"0.00000"}
                ]
            },
            "type": "update/order_book"
        }"#;
        let mut books = HashMap::new();
        let s: WsMessage = serde_json::from_str(snapshot).unwrap();
        process_message(&mut books, s).unwrap();
        let u: WsMessage = serde_json::from_str(update).unwrap();
        let ev = process_message(&mut books, u).expect("delta emits bbo");
        let BookEvent::Bbo { bid, ask, .. } = ev else {
            panic!("expected Bbo");
        };
        // 75023.7 삭제됨 → 최우선 매수는 75018.1.
        assert_eq!(bid, Some(75018.1));
        // 매도 75023.9 유지 + 75030.9 추가 → 최우선 75023.9.
        assert_eq!(ask, Some(75023.9));
    }

    #[test]
    fn non_book_frame_ignored() {
        let mut books = HashMap::new();
        let ping: WsMessage = serde_json::from_str(r#"{"type":"ping"}"#).unwrap();
        assert!(process_message(&mut books, ping).is_none());
    }
}
