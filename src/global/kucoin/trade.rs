//! 거래·계좌 도메인 (HMAC 서명 필수) — 주문/취소·포지션·잔고.
//!
//! KuCoin Futures(USDT-마진 무기한). 주문 생성은 **POST + JSON 본문**, 취소는 **DELETE**,
//! 조회는 GET이다. 서명은 [`crate::global::kucoin::client`]가 `KC-API-*` 헤더로 부착한다.
//!
//! **실거래 경고:** 이 모듈의 변경 호출은 실제 자금을 움직인다. KuCoin Futures 샌드박스는
//! 폐지되어 테스트넷 검증이 불가하다 — **소액·reduceOnly로 운영 환경에서 신중히 검증**하라.
//!
//! KuCoin 주문 스키마 특이점(Bybit와 다름):
//!  - `side`/`type`은 **소문자**(`buy`/`sell`, `limit`/`market`).
//!  - `size`는 **계약 수(lots) 정수**이며 JSON **숫자**로 보낸다.
//!  - `clientOid`는 **필수**(멱등 추적). 미지정 시 32자 hex 난수를 자동 생성한다.
//!  - `reduceOnly`/`postOnly`는 JSON 불리언, `leverage`는 숫자.

use serde::de::{self, Deserializer};
use serde::Deserialize;
use serde_json::Value;

use crate::global::kucoin::client::{ApiCall, KucoinClient};
use crate::global::kucoin::error::{KucoinError, Result};

/// JSON 숫자 또는 문자열을 정밀도 보존 String으로 받는다(포지션/잔고 응답 혼재 대응).
fn num_or_str<'de, D>(de: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(de)? {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        Value::Null => Ok(String::new()),
        Value::Bool(b) => Ok(b.to_string()),
        other => Err(de::Error::custom(format!("expected scalar, got {other}"))),
    }
}

/// 주문 방향. KuCoin은 **소문자** `buy`/`sell`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Side::Buy => "buy",
            Side::Sell => "sell",
        }
    }
}

/// 주문 유형. KuCoin은 **소문자** `limit`/`market`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

impl OrderType {
    fn as_str(self) -> &'static str {
        match self {
            OrderType::Limit => "limit",
            OrderType::Market => "market",
        }
    }
}

/// 체결 조건 (limit 전용).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// 취소 전까지 유효.
    Gtc,
    /// 즉시 체결·잔량 취소.
    Ioc,
}

impl TimeInForce {
    fn as_str(self) -> &'static str {
        match self {
            TimeInForce::Gtc => "GTC",
            TimeInForce::Ioc => "IOC",
        }
    }
}

/// 주문 요청.
///
/// `size`는 계약 수(lots) **정수**(KuCoin 본문에서 JSON 숫자로 직렬화). `price`는 정밀도
/// 보존 위해 String(심볼 `tickSize`에 맞춰 호출부가 포맷). `client_oid` 미지정 시 전송
/// 직전 난수로 채운다(멱등 추적).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    /// 주문 수량 (계약 lots, 정수).
    pub size: u64,
    /// 지정가. Limit 필수, Market이면 None.
    pub price: Option<String>,
    /// 레버리지 배수 (None이면 계정 기본).
    pub leverage: Option<u32>,
    /// 체결 조건 (limit 전용).
    pub time_in_force: Option<TimeInForce>,
    /// 포지션 축소 전용 여부.
    pub reduce_only: Option<bool>,
    /// 메이커 전용(테이커면 거부).
    pub post_only: Option<bool>,
    /// 사용자 지정 주문 ID (멱등). None이면 클라이언트가 난수 생성.
    pub client_oid: Option<String>,
}

impl OrderRequest {
    /// 시장가 주문.
    pub fn market(symbol: impl Into<String>, side: Side, size: u64) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            order_type: OrderType::Market,
            size,
            price: None,
            leverage: None,
            time_in_force: None,
            reduce_only: None,
            post_only: None,
            client_oid: None,
        }
    }

    /// 지정가 주문 (기본 GTC).
    pub fn limit(
        symbol: impl Into<String>,
        side: Side,
        size: u64,
        price: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            order_type: OrderType::Limit,
            size,
            price: Some(price.into()),
            leverage: None,
            time_in_force: Some(TimeInForce::Gtc),
            reduce_only: None,
            post_only: None,
            client_oid: None,
        }
    }

    /// 레버리지 지정 (builder).
    pub fn leverage(mut self, lev: u32) -> Self {
        self.leverage = Some(lev);
        self
    }

    /// 포지션 축소 전용으로 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = Some(v);
        self
    }

    /// 메이커 전용으로 표시 (builder).
    pub fn post_only(mut self, v: bool) -> Self {
        self.post_only = Some(v);
        self
    }

    /// 사용자 주문 ID 부여 (builder).
    pub fn client_oid(mut self, id: impl Into<String>) -> Self {
        self.client_oid = Some(id.into());
        self
    }

    /// 전송 파라미터로 변환. `client_oid`는 호출부가 채워 넣은 값을 받는다(난수 생성은
    /// `place`에서 수행하므로 여기선 이미 채워졌다고 가정).
    fn to_params(&self, client_oid: &str) -> Vec<(String, String)> {
        let mut p = vec![
            ("clientOid".to_string(), client_oid.to_string()),
            ("symbol".to_string(), self.symbol.clone()),
            ("side".to_string(), self.side.as_str().to_string()),
            ("type".to_string(), self.order_type.as_str().to_string()),
            // size는 client::json_object_from_pairs의 NUMBER_KEYS로 JSON 숫자화된다.
            ("size".to_string(), self.size.to_string()),
        ];
        if let Some(price) = &self.price {
            p.push(("price".into(), price.clone()));
        }
        if let Some(lev) = self.leverage {
            p.push(("leverage".into(), lev.to_string()));
        }
        if let Some(tif) = self.time_in_force {
            p.push(("timeInForce".into(), tif.as_str().to_string()));
        }
        if let Some(ro) = self.reduce_only {
            p.push(("reduceOnly".into(), ro.to_string()));
        }
        if let Some(po) = self.post_only {
            p.push(("postOnly".into(), po.to_string()));
        }
        p
    }
}

/// 주문 생성 ACK (`data`: `{orderId, clientOid}`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderAck {
    pub order_id: String,
    #[serde(default)]
    pub client_oid: String,
}

/// 주문 취소 ACK (`data`: `{cancelledOrderIds: [..]}`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAck {
    #[serde(default)]
    pub cancelled_order_ids: Vec<String>,
}

/// 포지션 1건 (`GET /api/v1/positions` → data[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub symbol: String,
    /// 포지션 존재 여부.
    #[serde(default)]
    pub is_open: bool,
    /// 현재 수량 (부호: 양수=롱, 음수=숏). 0이면 무포지션.
    #[serde(deserialize_with = "num_or_str", default)]
    pub current_qty: String,
    /// 평균 진입가.
    #[serde(deserialize_with = "num_or_str", default)]
    pub avg_entry_price: String,
    /// 마크가.
    #[serde(deserialize_with = "num_or_str", default)]
    pub mark_price: String,
    /// 청산가.
    #[serde(deserialize_with = "num_or_str", default)]
    pub liquidation_price: String,
    /// 미실현손익 (USDT).
    #[serde(deserialize_with = "num_or_str", default)]
    pub unrealised_pnl: String,
    /// 실효 레버리지.
    #[serde(deserialize_with = "num_or_str", default)]
    pub real_leverage: String,
}

/// 계정 개요/잔고 (`GET /api/v1/account-overview?currency=USDT` → data).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountOverview {
    #[serde(default)]
    pub currency: String,
    /// 계정 자산(USDT).
    #[serde(deserialize_with = "num_or_str", default)]
    pub account_equity: String,
    /// 사용 가능 잔고.
    #[serde(deserialize_with = "num_or_str", default)]
    pub available_balance: String,
    /// 미실현손익.
    #[serde(deserialize_with = "num_or_str", default)]
    pub unrealised_pnl: String,
    /// 포지션 마진.
    #[serde(deserialize_with = "num_or_str", default)]
    pub position_margin: String,
    /// 주문 마진.
    #[serde(deserialize_with = "num_or_str", default)]
    pub order_margin: String,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a KucoinClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a KucoinClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /api/v1/orders`). **실체결.**
    ///
    /// `req.client_oid`가 None이면 32자 hex 난수를 생성해 채운다(KuCoin은 clientOid 필수).
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderAck> {
        let oid = match &req.client_oid {
            Some(o) => o.clone(),
            None => KucoinClient::client_oid()?,
        };
        self.client
            .call(ApiCall::signed_post("/api/v1/orders", req.to_params(&oid)))
            .await?
            .parse()
    }

    /// 주문 취소 (`DELETE /api/v1/orders/{orderId}`).
    pub async fn cancel(&self, order_id: &str) -> Result<CancelAck> {
        self.client
            .call(ApiCall::signed_delete(
                format!("/api/v1/orders/{order_id}"),
                vec![],
            ))
            .await?
            .parse()
    }

    /// 심볼 전체 미체결 주문 취소 (`DELETE /api/v1/orders?symbol=...`).
    pub async fn cancel_all(&self, symbol: &str) -> Result<CancelAck> {
        self.client
            .call(ApiCall::signed_delete(
                "/api/v1/orders",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 전체 포지션 조회 (`GET /api/v1/positions`).
    pub async fn positions(&self) -> Result<Vec<Position>> {
        self.client
            .call(ApiCall::signed_get("/api/v1/positions", vec![]))
            .await?
            .parse()
    }

    /// 단일 심볼 포지션 조회 (`GET /api/v1/position?symbol=...`).
    pub async fn position(&self, symbol: &str) -> Result<Position> {
        self.client
            .call(ApiCall::signed_get(
                "/api/v1/position",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 계정 개요/잔고 (`GET /api/v1/account-overview?currency=USDT`).
    pub async fn account_overview(&self, currency: &str) -> Result<AccountOverview> {
        self.client
            .call(ApiCall::signed_get(
                "/api/v1/account-overview",
                vec![("currency".into(), currency.into())],
            ))
            .await?
            .parse()
            .map_err(|e| match e {
                KucoinError::Json(j) => KucoinError::Decode(format!("account-overview decode: {j}")),
                other => other,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_order_params_lowercase_and_oid() {
        let req = OrderRequest::market("SAMSUNGUSDTM", Side::Buy, 3);
        let p = req.to_params("fixed-oid");
        assert!(p.contains(&("clientOid".to_string(), "fixed-oid".to_string())));
        assert!(p.contains(&("type".to_string(), "market".to_string())));
        assert!(p.contains(&("side".to_string(), "buy".to_string())));
        assert!(p.contains(&("size".to_string(), "3".to_string())));
        // Market은 price/timeInForce 없음.
        assert!(!p.iter().any(|(k, _)| k == "price"));
        assert!(!p.iter().any(|(k, _)| k == "timeInForce"));
    }

    #[test]
    fn limit_order_params_default_gtc_with_builders() {
        let req = OrderRequest::limit("SKHYNIXUSDTM", Side::Sell, 2, "1510.00")
            .leverage(5)
            .reduce_only(true)
            .post_only(true)
            .client_oid("kr-001");
        let p = req.to_params("kr-001");
        assert!(p.contains(&("type".to_string(), "limit".to_string())));
        assert!(p.contains(&("side".to_string(), "sell".to_string())));
        assert!(p.contains(&("price".to_string(), "1510.00".to_string())));
        assert!(p.contains(&("leverage".to_string(), "5".to_string())));
        assert!(p.contains(&("timeInForce".to_string(), "GTC".to_string())));
        assert!(p.contains(&("reduceOnly".to_string(), "true".to_string())));
        assert!(p.contains(&("postOnly".to_string(), "true".to_string())));
    }

    #[test]
    fn order_ack_parses() {
        let v = serde_json::json!({
            "orderId": "234125150956625920",
            "clientOid": "kr-001"
        });
        let a: OrderAck = serde_json::from_value(v).unwrap();
        assert_eq!(a.order_id, "234125150956625920");
        assert_eq!(a.client_oid, "kr-001");
    }

    #[test]
    fn position_parses_numeric_fields() {
        let v = serde_json::json!({
            "symbol": "HYUNDAIUSDTM",
            "isOpen": true,
            "currentQty": -5,
            "avgEntryPrice": 463.0,
            "markPrice": 463.39,
            "liquidationPrice": 600.0,
            "unrealisedPnl": 1.25,
            "realLeverage": 5
        });
        let pos: Position = serde_json::from_value(v).unwrap();
        assert!(pos.is_open);
        assert_eq!(pos.current_qty, "-5");
        assert_eq!(pos.mark_price, "463.39");
        assert_eq!(pos.unrealised_pnl, "1.25");
        assert_eq!(pos.real_leverage, "5");
    }

    #[test]
    fn account_overview_parses() {
        let v = serde_json::json!({
            "currency": "USDT",
            "accountEquity": 1000.0,
            "availableBalance": 850.5,
            "unrealisedPnl": 5.0,
            "positionMargin": 100.0,
            "orderMargin": 44.5
        });
        let a: AccountOverview = serde_json::from_value(v).unwrap();
        assert_eq!(a.currency, "USDT");
        // num_or_str는 JSON 숫자를 그대로 String화한다(1000.0 → "1000.0", 손실 없음).
        assert_eq!(a.account_equity, "1000.0");
        assert_eq!(a.available_balance, "850.5");
    }
}
