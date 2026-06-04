//! 거래·계좌 도메인 (HMAC 서명 필수) — 주문/취소·포지션·잔고.
//!
//! **실거래 경고:** 이 모듈의 모든 호출은 실제 자금을 움직인다. WEEX는 공개
//! 테스트넷이 없어(데모 엔드포인트 `/capi/v3/sim/order`만 존재) 운영 키로
//! 소액 검증해야 한다.
//!
//! **심볼 표기 미검증:** 시세는 `cmt_samsungusdt`를 쓰지만, WEEX 거래 문서
//! (`POST /capi/v3/order`)의 예시 심볼은 접두어 없는 `BTCUSDT` 형태다. KR 종목
//! 거래 심볼이 `cmt_samsungusdt`인지 `SAMSUNGUSDT`인지 **키 없이 확정 불가**.
//! 호출부가 [`OrderRequest::symbol`]에 venue가 받아들이는 정확한 문자열을 넣어야
//! 한다(미검증).

use serde::Deserialize;
use serde_json::{json, Value};

use crate::global::weex::client::{ApiCall, WeexClient};
use crate::global::weex::error::Result;

/// 주문 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }
}

/// 포지션 방향 (헤지/원웨이 — WEEX `positionSide` 필수 필드).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionSide {
    Long,
    Short,
}

impl PositionSide {
    fn as_str(self) -> &'static str {
        match self {
            PositionSide::Long => "LONG",
            PositionSide::Short => "SHORT",
        }
    }
}

/// 주문 유형 (지정가·시장가).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

impl OrderType {
    fn as_str(self) -> &'static str {
        match self {
            OrderType::Limit => "LIMIT",
            OrderType::Market => "MARKET",
        }
    }
}

/// 체결 조건. LIMIT 주문 필수.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// 취소 전까지 유효.
    Gtc,
    /// 즉시 체결·잔량 취소.
    Ioc,
    /// 전량 즉시 체결 아니면 취소.
    Fok,
}

impl TimeInForce {
    fn as_str(self) -> &'static str {
        match self {
            TimeInForce::Gtc => "GTC",
            TimeInForce::Ioc => "IOC",
            TimeInForce::Fok => "FOK",
        }
    }
}

/// 주문 요청 (`POST /capi/v3/order` body). 수량·가격은 정밀도 보존 위해 String
/// (계약의 `tick_size`/`size_increment`에 맞춰 호출부가 포맷).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub position_side: PositionSide,
    pub order_type: OrderType,
    /// 주문 수량 (계약 수).
    pub quantity: String,
    /// 지정가. LIMIT 필수, MARKET이면 None.
    pub price: Option<String>,
    /// 체결 조건. LIMIT 필수.
    pub time_in_force: Option<TimeInForce>,
    /// 사용자 지정 주문 ID (1~36자, 멱등 추적). `newClientOrderId`로 전송.
    pub client_order_id: Option<String>,
}

impl OrderRequest {
    /// 시장가 주문.
    pub fn market(
        symbol: impl Into<String>,
        side: Side,
        position_side: PositionSide,
        quantity: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            position_side,
            order_type: OrderType::Market,
            quantity: quantity.into(),
            price: None,
            time_in_force: None,
            client_order_id: None,
        }
    }

    /// 지정가 주문 (기본 GTC).
    pub fn limit(
        symbol: impl Into<String>,
        side: Side,
        position_side: PositionSide,
        quantity: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            position_side,
            order_type: OrderType::Limit,
            quantity: quantity.into(),
            price: Some(price.into()),
            time_in_force: Some(TimeInForce::Gtc),
            client_order_id: None,
        }
    }

    /// 사용자 주문 ID 부여 (builder).
    pub fn client_order_id(mut self, id: impl Into<String>) -> Self {
        self.client_order_id = Some(id.into());
        self
    }

    /// 서명·전송 대상 JSON body. 필드명은 WEEX `POST /capi/v3/order` 규격.
    fn to_body(&self) -> Value {
        let mut m = serde_json::Map::new();
        m.insert("symbol".into(), json!(self.symbol));
        m.insert("side".into(), json!(self.side.as_str()));
        m.insert("positionSide".into(), json!(self.position_side.as_str()));
        m.insert("type".into(), json!(self.order_type.as_str()));
        m.insert("quantity".into(), json!(self.quantity));
        if let Some(price) = &self.price {
            m.insert("price".into(), json!(price));
        }
        if let Some(tif) = self.time_in_force {
            m.insert("timeInForce".into(), json!(tif.as_str()));
        }
        if let Some(id) = &self.client_order_id {
            m.insert("newClientOrderId".into(), json!(id));
        }
        Value::Object(m)
    }
}

/// 주문 응답 (`POST /capi/v3/order`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    #[serde(default)]
    pub order_id: String,
    #[serde(default)]
    pub client_order_id: String,
    /// 수락 여부.
    #[serde(default)]
    pub success: bool,
    #[serde(default)]
    pub error_code: String,
    #[serde(default)]
    pub error_message: String,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a WeexClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a WeexClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /capi/v3/order`). **실체결.**
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed_post("/capi/v3/order", req.to_body()))
            .await?
            .parse()
    }

    /// 주문 취소 (`POST /capi/v3/order/cancel`). symbol+orderId.
    pub async fn cancel(&self, symbol: &str, order_id: &str) -> Result<Value> {
        let body = json!({ "symbol": symbol, "orderId": order_id });
        self.client
            .call(ApiCall::signed_post("/capi/v3/order/cancel", body))
            .await
            .map(|r| r.body)
    }

    /// 미체결 주문 목록 (`GET /capi/v3/openOrders`). 서명 GET.
    pub async fn open_orders(&self, symbol: &str) -> Result<Value> {
        self.client
            .call(ApiCall::signed_get(
                "/capi/v3/openOrders",
                vec![("symbol".into(), symbol.into())],
            ))
            .await
            .map(|r| r.body)
    }

    /// 포지션 조회 (`GET /capi/v3/position`). 서명 GET.
    pub async fn positions(&self, symbol: &str) -> Result<Value> {
        self.client
            .call(ApiCall::signed_get(
                "/capi/v3/position",
                vec![("symbol".into(), symbol.into())],
            ))
            .await
            .map(|r| r.body)
    }

    /// 계좌 잔고 (`GET /capi/v3/account`). 서명 GET.
    pub async fn account(&self) -> Result<Value> {
        self.client
            .call(ApiCall::signed_get("/capi/v3/account", vec![]))
            .await
            .map(|r| r.body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_order_body() {
        let req = OrderRequest::market("cmt_samsungusdt", Side::Buy, PositionSide::Long, "3");
        let b = req.to_body();
        assert_eq!(b["type"], "MARKET");
        assert_eq!(b["side"], "BUY");
        assert_eq!(b["positionSide"], "LONG");
        assert_eq!(b["quantity"], "3");
        // MARKET은 price/timeInForce 없음.
        assert!(b.get("price").is_none());
        assert!(b.get("timeInForce").is_none());
    }

    #[test]
    fn limit_order_body_default_gtc() {
        let req = OrderRequest::limit(
            "cmt_skhynixusdt",
            Side::Sell,
            PositionSide::Short,
            "2",
            "120.50",
        )
        .client_order_id("kr-001");
        let b = req.to_body();
        assert_eq!(b["type"], "LIMIT");
        assert_eq!(b["price"], "120.50");
        assert_eq!(b["timeInForce"], "GTC");
        assert_eq!(b["newClientOrderId"], "kr-001");
    }

    #[test]
    fn order_response_parses() {
        let v = serde_json::json!({
            "orderId": "9001",
            "clientOrderId": "kr-001",
            "success": true
        });
        let o: OrderResponse = serde_json::from_value(v).unwrap();
        assert_eq!(o.order_id, "9001");
        assert!(o.success);
    }
}
