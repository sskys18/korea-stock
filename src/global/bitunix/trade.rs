//! 거래·계좌 도메인 (이중 SHA256 서명 필수) — 주문/취소·계좌·포지션·미체결조회.
//!
//! **실거래 경고:** 이 모듈의 모든 변경 호출은 실제 자금을 움직인다. 서명 알고리즘은
//! 공식 문서로 검증됐으나 **어떤 주문도 라이브/테스트넷으로 실제 전송된 적 없다**(키
//! 미보유). 실자금 투입 전 사용자 키로 왕복 확인 필수.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::global::bitunix::client::{ApiCall, BitunixClient};
use crate::global::bitunix::error::Result;

/// 주문 방향. open long/close short = BUY, open short/close long = SELL은 [`TradeSide`]로 결정.
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

/// 포지션 방향 지정. Bitunix는 side(BUY/SELL)와 tradeSide(OPEN/CLOSE) 조합으로
/// 롱·숏 개시·청산을 표현한다.
/// - 롱 개시: side=BUY,  tradeSide=OPEN
/// - 숏 개시: side=SELL, tradeSide=OPEN
/// - 롱 청산: side=BUY,  tradeSide=CLOSE
/// - 숏 청산: side=SELL, tradeSide=CLOSE
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeSide {
    Open,
    Close,
}

impl TradeSide {
    fn as_str(self) -> &'static str {
        match self {
            TradeSide::Open => "OPEN",
            TradeSide::Close => "CLOSE",
        }
    }
}

/// 주문 유형.
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

/// 체결 조건(`effect`). LIMIT 주문에서 의미 있음. 기본 GTC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    /// 취소 전까지 유효(기본).
    Gtc,
    /// 즉시 체결·잔량 취소.
    Ioc,
    /// 전량 즉시 체결 아니면 취소.
    Fok,
    /// 메이커 전용.
    PostOnly,
}

impl Effect {
    fn as_str(self) -> &'static str {
        match self {
            Effect::Gtc => "GTC",
            Effect::Ioc => "IOC",
            Effect::Fok => "FOK",
            Effect::PostOnly => "POST_ONLY",
        }
    }
}

/// 주문 요청. 수량·가격은 정밀도 보존 위해 String(거래쌍 `basePrecision`/
/// `quotePrecision`에 맞춰 호출부가 포맷).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub trade_side: TradeSide,
    pub order_type: OrderType,
    /// 주문 수량.
    pub qty: String,
    /// 지정가. LIMIT 필수, MARKET이면 None.
    pub price: Option<String>,
    /// 체결 조건. LIMIT에서 의미.
    pub effect: Option<Effect>,
    /// 포지션 축소 전용 여부.
    pub reduce_only: Option<bool>,
    /// 사용자 지정 주문 ID (멱등 추적).
    pub client_id: Option<String>,
}

impl OrderRequest {
    /// 시장가 주문.
    pub fn market(
        symbol: impl Into<String>,
        side: Side,
        trade_side: TradeSide,
        qty: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            trade_side,
            order_type: OrderType::Market,
            qty: qty.into(),
            price: None,
            effect: None,
            reduce_only: None,
            client_id: None,
        }
    }

    /// 지정가 주문 (기본 GTC).
    pub fn limit(
        symbol: impl Into<String>,
        side: Side,
        trade_side: TradeSide,
        qty: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            trade_side,
            order_type: OrderType::Limit,
            qty: qty.into(),
            price: Some(price.into()),
            effect: Some(Effect::Gtc),
            reduce_only: None,
            client_id: None,
        }
    }

    /// 체결 조건 지정 (builder).
    pub fn effect(mut self, e: Effect) -> Self {
        self.effect = Some(e);
        self
    }

    /// 포지션 축소 전용으로 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = Some(v);
        self
    }

    /// 사용자 주문 ID 부여 (builder).
    pub fn client_id(mut self, id: impl Into<String>) -> Self {
        self.client_id = Some(id.into());
        self
    }

    /// POST JSON 본문으로 직렬화. null 키는 넣지 않는다(서명 본문에 포함되므로).
    fn to_body(&self) -> Value {
        let mut obj = serde_json::Map::new();
        obj.insert("symbol".into(), json!(self.symbol));
        obj.insert("side".into(), json!(self.side.as_str()));
        obj.insert("tradeSide".into(), json!(self.trade_side.as_str()));
        obj.insert("orderType".into(), json!(self.order_type.as_str()));
        obj.insert("qty".into(), json!(self.qty));
        if let Some(price) = &self.price {
            obj.insert("price".into(), json!(price));
        }
        if let Some(effect) = self.effect {
            obj.insert("effect".into(), json!(effect.as_str()));
        }
        if let Some(ro) = self.reduce_only {
            obj.insert("reduceOnly".into(), json!(ro));
        }
        if let Some(id) = &self.client_id {
            obj.insert("clientId".into(), json!(id));
        }
        Value::Object(obj)
    }
}

/// 주문 제출/취소 응답 (`{orderId, clientId}`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderAck {
    pub order_id: String,
    #[serde(default)]
    pub client_id: String,
}

/// 계좌 자산 (`GET /api/v1/futures/account`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub margin_coin: String,
    /// 주문 가능 수량. `available + crossUnrealizedPNL`이 실 최대 개설액.
    pub available: String,
    /// 미체결 주문에 잠긴 수량.
    #[serde(default)]
    pub frozen: String,
    /// 포지션에 잠긴 증거금.
    #[serde(default)]
    pub margin: String,
    /// 교차 미실현손익. API 필드명은 대문자 PNL(`crossUnrealizedPNL`).
    #[serde(rename = "crossUnrealizedPNL", default)]
    pub cross_unrealized_pnl: String,
    /// 격리 미실현손익. API 필드명은 대문자 PNL(`isolationUnrealizedPNL`).
    #[serde(rename = "isolationUnrealizedPNL", default)]
    pub isolation_unrealized_pnl: String,
}

/// 포지션 1건 (`GET /api/v1/futures/position/get_pending_positions`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub position_id: String,
    pub symbol: String,
    /// 포지션 수량.
    pub qty: String,
    /// 방향: "LONG" / "SHORT".
    pub side: String,
    pub avg_open_price: String,
    /// 미실현손익. API 필드명은 대문자 PNL(`unrealizedPNL`).
    #[serde(rename = "unrealizedPNL")]
    pub unrealized_pnl: String,
    pub leverage: String,
    /// "ISOLATION" / "CROSS".
    pub margin_mode: String,
}

/// 미체결 주문 1건 (`GET /api/v1/futures/trade/get_pending_orders` → orderList[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingOrder {
    pub order_id: String,
    pub symbol: String,
    pub qty: String,
    #[serde(default)]
    pub trade_qty: String,
    pub price: String,
    pub side: String,
    pub order_type: String,
    /// "NEW" / "PART_FILLED".
    pub status: String,
    #[serde(default)]
    pub ctime: i64,
    #[serde(default)]
    pub mtime: i64,
}

#[derive(Deserialize)]
struct PendingOrdersPage {
    #[serde(rename = "orderList", default)]
    order_list: Vec<PendingOrder>,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a BitunixClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a BitunixClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /api/v1/futures/trade/place_order`). **실체결.**
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderAck> {
        self.client
            .call(ApiCall::signed_post(
                "/api/v1/futures/trade/place_order",
                req.to_body(),
            ))
            .await?
            .parse()
    }

    /// 주문 취소 (`POST /api/v1/futures/trade/cancel_orders`). `order_id`별 일괄 취소.
    pub async fn cancel(&self, symbol: &str, order_ids: &[&str]) -> Result<Value> {
        let order_list: Vec<Value> = order_ids.iter().map(|id| json!({ "orderId": id })).collect();
        self.client
            .call(ApiCall::signed_post(
                "/api/v1/futures/trade/cancel_orders",
                json!({ "symbol": symbol, "orderList": order_list }),
            ))
            .await?
            .parse()
    }

    /// 계좌 자산 조회 (`GET /api/v1/futures/account`). `margin_coin` 예: "USDT".
    pub async fn account(&self, margin_coin: &str) -> Result<Account> {
        self.client
            .call(ApiCall::signed_get(
                "/api/v1/futures/account",
                json!({ "marginCoin": margin_coin }),
            ))
            .await?
            .parse()
    }

    /// 포지션 조회 (`GET /api/v1/futures/position/get_pending_positions`).
    /// `symbol=None`이면 전체.
    pub async fn positions(&self, symbol: Option<&str>) -> Result<Vec<Position>> {
        let params = match symbol {
            Some(s) => json!({ "symbol": s }),
            None => json!({}),
        };
        self.client
            .call(ApiCall::signed_get(
                "/api/v1/futures/position/get_pending_positions",
                params,
            ))
            .await?
            .parse()
    }

    /// 미체결 주문 조회 (`GET /api/v1/futures/trade/get_pending_orders`).
    /// `symbol=None`이면 전체. 응답 envelope의 `orderList`를 펼쳐 반환.
    pub async fn pending_orders(&self, symbol: Option<&str>) -> Result<Vec<PendingOrder>> {
        let params = match symbol {
            Some(s) => json!({ "symbol": s }),
            None => json!({}),
        };
        let page: PendingOrdersPage = self
            .client
            .call(ApiCall::signed_get(
                "/api/v1/futures/trade/get_pending_orders",
                params,
            ))
            .await?
            .parse()?;
        Ok(page.order_list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_order_body_open_long() {
        let req = OrderRequest::market("SAMSUNGUSDT", Side::Buy, TradeSide::Open, "3");
        let b = req.to_body();
        assert_eq!(b["symbol"], "SAMSUNGUSDT");
        assert_eq!(b["side"], "BUY");
        assert_eq!(b["tradeSide"], "OPEN");
        assert_eq!(b["orderType"], "MARKET");
        assert_eq!(b["qty"], "3");
        // MARKET은 price/effect 없음.
        assert!(b.get("price").is_none());
        assert!(b.get("effect").is_none());
    }

    #[test]
    fn limit_order_body_default_gtc_with_builders() {
        let req = OrderRequest::limit("SKHYNIXUSDT", Side::Sell, TradeSide::Close, "2", "120.50")
            .reduce_only(true)
            .client_id("kr-001");
        let b = req.to_body();
        assert_eq!(b["orderType"], "LIMIT");
        assert_eq!(b["side"], "SELL");
        assert_eq!(b["tradeSide"], "CLOSE");
        assert_eq!(b["price"], "120.50");
        assert_eq!(b["effect"], "GTC");
        assert_eq!(b["reduceOnly"], true);
        assert_eq!(b["clientId"], "kr-001");
    }

    #[test]
    fn limit_order_post_only_effect() {
        let req = OrderRequest::limit("HYUNDAIUSDT", Side::Buy, TradeSide::Open, "1", "450")
            .effect(Effect::PostOnly);
        let b = req.to_body();
        assert_eq!(b["effect"], "POST_ONLY");
    }

    #[test]
    fn order_body_serializes_compact_no_spaces() {
        // 서명 본문이 될 JSON은 compact(공백 없음)여야 한다.
        let req = OrderRequest::limit("SAMSUNGUSDT", Side::Buy, TradeSide::Open, "3", "235.1");
        let s = serde_json::to_string(&req.to_body()).unwrap();
        assert!(!s.contains(' '));
    }

    #[test]
    fn order_ack_parses() {
        let v = serde_json::json!({ "orderId": "11111", "clientId": "kr-001" });
        let a: OrderAck = serde_json::from_value(v).unwrap();
        assert_eq!(a.order_id, "11111");
        assert_eq!(a.client_id, "kr-001");
    }

    #[test]
    fn account_parses() {
        let v = serde_json::json!({
            "marginCoin": "USDT",
            "available": "1000.00",
            "frozen": "50.00",
            "margin": "100.00",
            "crossUnrealizedPNL": "5.00",
            "isolationUnrealizedPNL": "0"
        });
        let a: Account = serde_json::from_value(v).unwrap();
        assert_eq!(a.margin_coin, "USDT");
        assert_eq!(a.available, "1000.00");
        assert_eq!(a.cross_unrealized_pnl, "5.00");
    }

    #[test]
    fn position_parses() {
        let v = serde_json::json!({
            "positionId": "p1",
            "symbol": "HYUNDAIUSDT",
            "qty": "2",
            "side": "LONG",
            "avgOpenPrice": "450.0",
            "unrealizedPNL": "10.0",
            "leverage": "20",
            "marginMode": "CROSS"
        });
        let p: Position = serde_json::from_value(v).unwrap();
        assert_eq!(p.position_id, "p1");
        assert_eq!(p.side, "LONG");
        assert_eq!(p.margin_mode, "CROSS");
    }

    #[test]
    fn pending_orders_unwraps_order_list() {
        let v = serde_json::json!({
            "total": 1,
            "orderList": [{
                "orderId": "o1",
                "symbol": "SAMSUNGUSDT",
                "qty": "3",
                "tradeQty": "0",
                "price": "235.0",
                "side": "BUY",
                "orderType": "LIMIT",
                "status": "NEW",
                "ctime": 1780547730000i64,
                "mtime": 1780547730000i64
            }]
        });
        let page: PendingOrdersPage = serde_json::from_value(v).unwrap();
        assert_eq!(page.order_list.len(), 1);
        assert_eq!(page.order_list[0].order_id, "o1");
        assert_eq!(page.order_list[0].status, "NEW");
    }
}
