//! 거래·계좌 도메인 (HMAC 서명 필수) — 주문/취소/조회·포지션·잔고·레버리지.
//!
//! **실거래 경고:** 이 모듈의 모든 호출은 실제 자금을 움직인다. 개발·검증은
//! 반드시 [`crate::global::binance::BinanceConfig::testnet`]에서 한다.

use serde::Deserialize;

use crate::global::binance::client::{ApiCall, BinanceClient};
use crate::global::binance::error::Result;

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

/// 주문 유형. (지정가·시장가만 우선 지원; STOP 등은 `raw_call`로.)
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

/// 주문 요청. 수량·가격은 정밀도 보존 위해 String (심볼 `quantityPrecision`/
/// `pricePrecision`에 맞춰 호출부가 포맷).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    /// 주문 수량 (계약 수).
    pub quantity: String,
    /// 지정가. LIMIT 필수, MARKET이면 None.
    pub price: Option<String>,
    /// 체결 조건. LIMIT 필수.
    pub time_in_force: Option<TimeInForce>,
    /// 포지션 축소 전용 여부.
    pub reduce_only: Option<bool>,
    /// 사용자 지정 주문 ID (멱등 추적).
    pub client_order_id: Option<String>,
}

impl OrderRequest {
    /// 시장가 주문.
    pub fn market(symbol: impl Into<String>, side: Side, quantity: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            order_type: OrderType::Market,
            quantity: quantity.into(),
            price: None,
            time_in_force: None,
            reduce_only: None,
            client_order_id: None,
        }
    }

    /// 지정가 주문 (기본 GTC).
    pub fn limit(
        symbol: impl Into<String>,
        side: Side,
        quantity: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            order_type: OrderType::Limit,
            quantity: quantity.into(),
            price: Some(price.into()),
            time_in_force: Some(TimeInForce::Gtc),
            reduce_only: None,
            client_order_id: None,
        }
    }

    /// 포지션 축소 전용으로 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = Some(v);
        self
    }

    /// 사용자 주문 ID 부여 (builder).
    pub fn client_order_id(mut self, id: impl Into<String>) -> Self {
        self.client_order_id = Some(id.into());
        self
    }

    fn to_params(&self) -> Vec<(String, String)> {
        let mut p = vec![
            ("symbol".to_string(), self.symbol.clone()),
            ("side".to_string(), self.side.as_str().to_string()),
            ("type".to_string(), self.order_type.as_str().to_string()),
            ("quantity".to_string(), self.quantity.clone()),
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
        if let Some(id) = &self.client_order_id {
            p.push(("newClientOrderId".into(), id.clone()));
        }
        p
    }
}

/// 주문 응답 / 조회 결과.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    pub order_id: i64,
    pub symbol: String,
    /// NEW / PARTIALLY_FILLED / FILLED / CANCELED / REJECTED / EXPIRED.
    pub status: String,
    pub client_order_id: String,
    pub price: String,
    /// 평균 체결가 (미체결이면 "0").
    pub avg_price: String,
    pub orig_qty: String,
    pub executed_qty: String,
    /// 누적 체결대금 (USDT).
    #[serde(default)]
    pub cum_quote: String,
    #[serde(rename = "type")]
    pub order_type: String,
    pub side: String,
    #[serde(default)]
    pub time_in_force: String,
    #[serde(default)]
    pub reduce_only: bool,
    #[serde(default)]
    pub update_time: i64,
}

/// 포지션 1건 (`GET /fapi/v2/positionRisk`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub symbol: String,
    /// 포지션 수량 (부호 = 방향; 음수 short). "0"이면 무포지션.
    pub position_amt: String,
    pub entry_price: String,
    pub mark_price: String,
    /// 미실현손익 (USDT).
    #[serde(rename = "unRealizedProfit")]
    pub unrealized_profit: String,
    pub liquidation_price: String,
    pub leverage: String,
    pub position_side: String,
}

/// 잔고 1건 (`GET /fapi/v2/balance`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub asset: String,
    /// 지갑 잔고.
    pub balance: String,
    /// 주문 가능 잔고.
    pub available_balance: String,
    /// 교차마진 미실현손익.
    #[serde(default)]
    pub cross_un_pnl: String,
}

/// 레버리지 설정 결과 (`POST /fapi/v1/leverage`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeverageResult {
    pub symbol: String,
    pub leverage: u32,
    pub max_notional_value: String,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a BinanceClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a BinanceClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /fapi/v1/order`). **실체결.**
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::POST,
                "/fapi/v1/order",
                req.to_params(),
            ))
            .await?
            .parse()
    }

    /// 주문 취소 (`DELETE /fapi/v1/order`).
    pub async fn cancel(&self, symbol: &str, order_id: i64) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::DELETE,
                "/fapi/v1/order",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("orderId".into(), order_id.to_string()),
                ],
            ))
            .await?
            .parse()
    }

    /// 심볼 전체 미체결 주문 취소 (`DELETE /fapi/v1/allOpenOrders`).
    pub async fn cancel_all(&self, symbol: &str) -> Result<serde_json::Value> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::DELETE,
                "/fapi/v1/allOpenOrders",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 단일 주문 조회 (`GET /fapi/v1/order`).
    pub async fn get_order(&self, symbol: &str, order_id: i64) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                "/fapi/v1/order",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("orderId".into(), order_id.to_string()),
                ],
            ))
            .await?
            .parse()
    }

    /// 미체결 주문 목록 (`GET /fapi/v1/openOrders`). `symbol=None`이면 전체.
    pub async fn open_orders(&self, symbol: Option<&str>) -> Result<Vec<OrderResponse>> {
        let params = symbol
            .map(|s| vec![("symbol".to_string(), s.to_string())])
            .unwrap_or_default();
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                "/fapi/v1/openOrders",
                params,
            ))
            .await?
            .parse()
    }

    /// 포지션 조회 (`GET /fapi/v2/positionRisk`). `symbol=None`이면 전체.
    pub async fn positions(&self, symbol: Option<&str>) -> Result<Vec<Position>> {
        let params = symbol
            .map(|s| vec![("symbol".to_string(), s.to_string())])
            .unwrap_or_default();
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                "/fapi/v2/positionRisk",
                params,
            ))
            .await?
            .parse()
    }

    /// 선물 지갑 잔고 (`GET /fapi/v2/balance`).
    pub async fn balances(&self) -> Result<Vec<Balance>> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                "/fapi/v2/balance",
                vec![],
            ))
            .await?
            .parse()
    }

    /// 레버리지 설정 (`POST /fapi/v1/leverage`). 1~심볼별 상한.
    pub async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<LeverageResult> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::POST,
                "/fapi/v1/leverage",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("leverage".into(), leverage.to_string()),
                ],
            ))
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_order_params() {
        let req = OrderRequest::market("SAMSUNGUSDT", Side::Buy, "3");
        let p = req.to_params();
        assert!(p.contains(&("type".to_string(), "MARKET".to_string())));
        assert!(p.contains(&("side".to_string(), "BUY".to_string())));
        assert!(p.contains(&("quantity".to_string(), "3".to_string())));
        // MARKET은 price/timeInForce 없음.
        assert!(!p.iter().any(|(k, _)| k == "price"));
        assert!(!p.iter().any(|(k, _)| k == "timeInForce"));
    }

    #[test]
    fn limit_order_params_default_gtc() {
        let req = OrderRequest::limit("SKHYNIXUSDT", Side::Sell, "2", "120.50")
            .reduce_only(true)
            .client_order_id("kr-001");
        let p = req.to_params();
        assert!(p.contains(&("type".to_string(), "LIMIT".to_string())));
        assert!(p.contains(&("price".to_string(), "120.50".to_string())));
        assert!(p.contains(&("timeInForce".to_string(), "GTC".to_string())));
        assert!(p.contains(&("reduceOnly".to_string(), "true".to_string())));
        assert!(p.contains(&("newClientOrderId".to_string(), "kr-001".to_string())));
    }

    #[test]
    fn order_response_parses_with_type_rename() {
        let v = serde_json::json!({
            "orderId": 280001,
            "symbol": "SAMSUNGUSDT",
            "status": "NEW",
            "clientOrderId": "kr-001",
            "price": "65.00",
            "avgPrice": "0",
            "origQty": "3",
            "executedQty": "0",
            "cumQuote": "0",
            "type": "LIMIT",
            "side": "BUY",
            "timeInForce": "GTC",
            "reduceOnly": false,
            "updateTime": 1717398000000i64
        });
        let o: OrderResponse = serde_json::from_value(v).unwrap();
        assert_eq!(o.order_id, 280001);
        assert_eq!(o.order_type, "LIMIT");
        assert_eq!(o.status, "NEW");
    }

    #[test]
    fn position_parses_unrealized_field() {
        let v = serde_json::json!({
            "symbol": "HYUNDAIUSDT",
            "positionAmt": "-2.000",
            "entryPrice": "190.50",
            "markPrice": "188.00",
            "unRealizedProfit": "5.00000000",
            "liquidationPrice": "250.00",
            "leverage": "10",
            "positionSide": "BOTH"
        });
        let pos: Position = serde_json::from_value(v).unwrap();
        assert_eq!(pos.position_amt, "-2.000");
        assert_eq!(pos.unrealized_profit, "5.00000000");
        assert_eq!(pos.leverage, "10");
    }

    #[test]
    fn balance_parses() {
        let v = serde_json::json!({
            "accountAlias": "abc",
            "asset": "USDT",
            "balance": "1000.00000000",
            "availableBalance": "850.00000000",
            "crossUnPnl": "5.00000000"
        });
        let b: Balance = serde_json::from_value(v).unwrap();
        assert_eq!(b.asset, "USDT");
        assert_eq!(b.available_balance, "850.00000000");
        assert_eq!(b.cross_un_pnl, "5.00000000");
    }
}
