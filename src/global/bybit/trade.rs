//! 거래·계좌 도메인 (HMAC 서명 필수) — 주문/취소/조회·포지션·잔고.
//!
//! Bybit v5 `category=linear`(USDT 무기한). 모든 변경 호출은 **POST + JSON 본문**,
//! 조회는 GET이다. 서명은 [`crate::global::bybit::client`]가 `X-BAPI-*` 헤더로 부착한다.
//!
//! **실거래 경고:** 이 모듈의 모든 변경 호출은 실제 자금을 움직인다. 개발·검증은
//! 반드시 [`crate::global::bybit::BybitConfig::testnet`]에서 한다.

use serde::Deserialize;

use crate::global::bybit::client::{ApiCall, BybitClient};
use crate::global::bybit::error::{BybitError, Result};

/// 주문 방향. Bybit는 `Buy`/`Sell`(대문자 첫 글자).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Side::Buy => "Buy",
            Side::Sell => "Sell",
        }
    }
}

/// 주문 유형. (지정가·시장가만 우선 지원.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

impl OrderType {
    fn as_str(self) -> &'static str {
        match self {
            OrderType::Limit => "Limit",
            OrderType::Market => "Market",
        }
    }
}

/// 체결 조건.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// 취소 전까지 유효.
    Gtc,
    /// 즉시 체결·잔량 취소.
    Ioc,
    /// 전량 즉시 체결 아니면 취소.
    Fok,
    /// 메이커 전용(테이커면 거부).
    PostOnly,
}

impl TimeInForce {
    fn as_str(self) -> &'static str {
        match self {
            TimeInForce::Gtc => "GTC",
            TimeInForce::Ioc => "IOC",
            TimeInForce::Fok => "FOK",
            TimeInForce::PostOnly => "PostOnly",
        }
    }
}

/// 주문 요청. 수량·가격은 정밀도 보존 위해 String (심볼 `qtyStep`/`tickSize`에 맞춰
/// 호출부가 포맷).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    /// 주문 수량 (계약 수).
    pub qty: String,
    /// 지정가. Limit 필수, Market이면 None.
    pub price: Option<String>,
    /// 체결 조건.
    pub time_in_force: Option<TimeInForce>,
    /// 포지션 축소 전용 여부.
    pub reduce_only: Option<bool>,
    /// 사용자 지정 주문 ID (멱등 추적, 최대 36자).
    pub order_link_id: Option<String>,
}

impl OrderRequest {
    /// 시장가 주문.
    pub fn market(symbol: impl Into<String>, side: Side, qty: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            order_type: OrderType::Market,
            qty: qty.into(),
            price: None,
            time_in_force: None,
            reduce_only: None,
            order_link_id: None,
        }
    }

    /// 지정가 주문 (기본 GTC).
    pub fn limit(
        symbol: impl Into<String>,
        side: Side,
        qty: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            order_type: OrderType::Limit,
            qty: qty.into(),
            price: Some(price.into()),
            time_in_force: Some(TimeInForce::Gtc),
            reduce_only: None,
            order_link_id: None,
        }
    }

    /// 포지션 축소 전용으로 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = Some(v);
        self
    }

    /// 사용자 주문 ID 부여 (builder).
    pub fn order_link_id(mut self, id: impl Into<String>) -> Self {
        self.order_link_id = Some(id.into());
        self
    }

    fn to_params(&self) -> Vec<(String, String)> {
        let mut p = vec![
            ("category".to_string(), "linear".to_string()),
            ("symbol".to_string(), self.symbol.clone()),
            ("side".to_string(), self.side.as_str().to_string()),
            ("orderType".to_string(), self.order_type.as_str().to_string()),
            ("qty".to_string(), self.qty.clone()),
        ];
        if let Some(price) = &self.price {
            p.push(("price".into(), price.clone()));
        }
        if let Some(tif) = self.time_in_force {
            p.push(("timeInForce".into(), tif.as_str().to_string()));
        }
        if let Some(ro) = self.reduce_only {
            p.push(("reduceOnly".into(), ro.to_string()));
        }
        if let Some(id) = &self.order_link_id {
            p.push(("orderLinkId".into(), id.clone()));
        }
        p
    }
}

/// 주문 생성/취소 ACK (`result`: `{orderId, orderLinkId}`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderAck {
    pub order_id: String,
    #[serde(default)]
    pub order_link_id: String,
}

/// 미체결/주문 조회 1건 (`GET /v5/order/realtime` → result.list[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderDetail {
    pub order_id: String,
    #[serde(default)]
    pub order_link_id: String,
    pub symbol: String,
    pub side: String,
    pub order_type: String,
    /// New / PartiallyFilled / Filled / Cancelled / Rejected 등.
    pub order_status: String,
    #[serde(default)]
    pub price: String,
    pub qty: String,
    #[serde(default)]
    pub cum_exec_qty: String,
    #[serde(default)]
    pub avg_price: String,
    #[serde(default)]
    pub time_in_force: String,
    #[serde(default)]
    pub reduce_only: bool,
}

/// 포지션 1건 (`GET /v5/position/list` → result.list[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub symbol: String,
    /// "Buy"(롱)/"Sell"(숏); 무포지션이면 빈 문자열.
    #[serde(default)]
    pub side: String,
    /// 포지션 수량 (항상 양수). "0"이면 무포지션.
    pub size: String,
    pub avg_price: String,
    #[serde(default)]
    pub position_value: String,
    #[serde(default)]
    pub leverage: String,
    #[serde(default)]
    pub mark_price: String,
    #[serde(default)]
    pub liq_price: String,
    /// 미실현손익 (USDT).
    #[serde(default)]
    pub unrealised_pnl: String,
    #[serde(default)]
    pub position_idx: i64,
}

/// 코인별 잔고 1건 (`result.list[].coin[]`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoinBalance {
    pub coin: String,
    /// 지갑 잔고.
    pub wallet_balance: String,
    #[serde(default)]
    pub equity: String,
    #[serde(default)]
    pub usd_value: String,
    #[serde(default)]
    pub available_to_withdraw: String,
    #[serde(default)]
    pub unrealised_pnl: String,
}

/// 계정 잔고 (`GET /v5/account/wallet-balance` → result.list[0]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WalletBalance {
    pub account_type: String,
    #[serde(default)]
    pub total_equity: String,
    #[serde(default)]
    pub total_available_balance: String,
    #[serde(default)]
    pub total_wallet_balance: String,
    #[serde(default)]
    pub coin: Vec<CoinBalance>,
}

/// `result.list[]` 래퍼.
#[derive(Debug, Clone, Deserialize)]
struct ListWrap<T> {
    #[serde(default = "Vec::new")]
    list: Vec<T>,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a BybitClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a BybitClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /v5/order/create`). **실체결.**
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderAck> {
        self.client
            .call(ApiCall::signed_post("/v5/order/create", req.to_params()))
            .await?
            .parse()
    }

    /// 주문 취소 (`POST /v5/order/cancel`).
    pub async fn cancel(&self, symbol: &str, order_id: &str) -> Result<OrderAck> {
        self.client
            .call(ApiCall::signed_post(
                "/v5/order/cancel",
                vec![
                    ("category".into(), "linear".into()),
                    ("symbol".into(), symbol.into()),
                    ("orderId".into(), order_id.into()),
                ],
            ))
            .await?
            .parse()
    }

    /// 심볼 전체 미체결 주문 취소 (`POST /v5/order/cancel-all`).
    pub async fn cancel_all(&self, symbol: &str) -> Result<serde_json::Value> {
        self.client
            .call(ApiCall::signed_post(
                "/v5/order/cancel-all",
                vec![
                    ("category".into(), "linear".into()),
                    ("symbol".into(), symbol.into()),
                ],
            ))
            .await?
            .parse()
    }

    /// 미체결 주문 목록 (`GET /v5/order/realtime`). `symbol=None`이면 settleCoin=USDT 전체.
    pub async fn open_orders(&self, symbol: Option<&str>) -> Result<Vec<OrderDetail>> {
        let mut params = vec![("category".to_string(), "linear".to_string())];
        match symbol {
            Some(s) => params.push(("symbol".into(), s.to_string())),
            None => params.push(("settleCoin".into(), "USDT".to_string())),
        }
        let wrap: ListWrap<OrderDetail> = self
            .client
            .call(ApiCall::signed_get("/v5/order/realtime", params))
            .await?
            .parse()?;
        Ok(wrap.list)
    }

    /// 포지션 조회 (`GET /v5/position/list`). `symbol=None`이면 settleCoin=USDT 전체.
    pub async fn positions(&self, symbol: Option<&str>) -> Result<Vec<Position>> {
        let mut params = vec![("category".to_string(), "linear".to_string())];
        match symbol {
            Some(s) => params.push(("symbol".into(), s.to_string())),
            None => params.push(("settleCoin".into(), "USDT".to_string())),
        }
        let wrap: ListWrap<Position> = self
            .client
            .call(ApiCall::signed_get("/v5/position/list", params))
            .await?
            .parse()?;
        Ok(wrap.list)
    }

    /// 통합 계정 잔고 (`GET /v5/account/wallet-balance`, accountType=UNIFIED).
    pub async fn wallet_balance(&self) -> Result<WalletBalance> {
        let wrap: ListWrap<WalletBalance> = self
            .client
            .call(ApiCall::signed_get(
                "/v5/account/wallet-balance",
                vec![("accountType".into(), "UNIFIED".into())],
            ))
            .await?
            .parse()?;
        wrap.list
            .into_iter()
            .next()
            .ok_or_else(|| BybitError::Decode("wallet-balance: empty list".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_order_params() {
        let req = OrderRequest::market("SAMSUNGUSDT", Side::Buy, "3");
        let p = req.to_params();
        assert!(p.contains(&("category".to_string(), "linear".to_string())));
        assert!(p.contains(&("orderType".to_string(), "Market".to_string())));
        assert!(p.contains(&("side".to_string(), "Buy".to_string())));
        assert!(p.contains(&("qty".to_string(), "3".to_string())));
        // Market은 price/timeInForce 없음.
        assert!(!p.iter().any(|(k, _)| k == "price"));
        assert!(!p.iter().any(|(k, _)| k == "timeInForce"));
    }

    #[test]
    fn limit_order_params_default_gtc() {
        let req = OrderRequest::limit("SKHYNIXUSDT", Side::Sell, "2", "120.50")
            .reduce_only(true)
            .order_link_id("kr-001");
        let p = req.to_params();
        assert!(p.contains(&("orderType".to_string(), "Limit".to_string())));
        assert!(p.contains(&("price".to_string(), "120.50".to_string())));
        assert!(p.contains(&("timeInForce".to_string(), "GTC".to_string())));
        assert!(p.contains(&("reduceOnly".to_string(), "true".to_string())));
        assert!(p.contains(&("orderLinkId".to_string(), "kr-001".to_string())));
    }

    #[test]
    fn order_ack_parses() {
        let v = serde_json::json!({
            "orderId": "1736012345678",
            "orderLinkId": "kr-001"
        });
        let a: OrderAck = serde_json::from_value(v).unwrap();
        assert_eq!(a.order_id, "1736012345678");
        assert_eq!(a.order_link_id, "kr-001");
    }

    #[test]
    fn position_parses_unrealised_field() {
        let v = serde_json::json!({
            "symbol": "HYUNDAIUSDT",
            "side": "Sell",
            "size": "2",
            "avgPrice": "190.50",
            "positionValue": "381.00",
            "leverage": "10",
            "markPrice": "188.00",
            "liqPrice": "250.00",
            "unrealisedPnl": "5.00",
            "positionIdx": 0
        });
        let pos: Position = serde_json::from_value(v).unwrap();
        assert_eq!(pos.side, "Sell");
        assert_eq!(pos.size, "2");
        assert_eq!(pos.unrealised_pnl, "5.00");
        assert_eq!(pos.leverage, "10");
    }

    #[test]
    fn wallet_balance_parses_nested_coin() {
        let v = serde_json::json!({
            "accountType": "UNIFIED",
            "totalEquity": "1000.00",
            "totalAvailableBalance": "850.00",
            "totalWalletBalance": "1000.00",
            "coin": [{
                "coin": "USDT",
                "walletBalance": "1000.00",
                "equity": "1000.00",
                "usdValue": "1000.00",
                "availableToWithdraw": "850.00",
                "unrealisedPnl": "5.00"
            }]
        });
        let w: WalletBalance = serde_json::from_value(v).unwrap();
        assert_eq!(w.account_type, "UNIFIED");
        assert_eq!(w.total_available_balance, "850.00");
        assert_eq!(w.coin[0].coin, "USDT");
        assert_eq!(w.coin[0].wallet_balance, "1000.00");
    }
}
