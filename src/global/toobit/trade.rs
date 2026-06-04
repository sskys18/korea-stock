//! 거래·계좌 도메인 (HMAC 서명 필수) — 주문/취소/조회·포지션·잔고·레버리지.
//!
//! **실거래 경고:** 이 모듈의 모든 호출은 실제 자금을 움직인다. Toobit은 공개
//! testnet 호스트가 없어(공식 문서 미기재) 본 어댑터에는 testnet 빌더가 없다.
//! 따라서 **거래 경로는 본 작업에서 라이브 키 없이 미검증**이다. 필드·엔드포인트는
//! Toobit 공식 USDT-M 문서 + CCXT 레퍼런스 구현으로 교차 확인했다.
//!
//! Toobit 선물 주문 모델은 Binance와 달리 **방향+개시/청산이 한 필드**(`side`):
//! `BUY_OPEN`/`SELL_OPEN`/`BUY_CLOSE`/`SELL_CLOSE`. 시장가는 `type=LIMIT` +
//! `priceType=MARKET`로 보낸다(거래소가 이 형태를 시장가로 처리).

use serde::Deserialize;

use crate::global::toobit::client::{ApiCall, ToobitClient};
use crate::global::toobit::error::Result;

/// 주문 방향 + 개시/청산. Toobit `side` 단일 필드에 대응.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// 롱 진입.
    BuyOpen,
    /// 숏 진입.
    SellOpen,
    /// 숏 청산(매수로 닫음).
    BuyClose,
    /// 롱 청산(매도로 닫음).
    SellClose,
}

impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Side::BuyOpen => "BUY_OPEN",
            Side::SellOpen => "SELL_OPEN",
            Side::BuyClose => "BUY_CLOSE",
            Side::SellClose => "SELL_CLOSE",
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

/// 가격 유형. 지정가=`INPUT`, 시장가=`MARKET`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceType {
    Input,
    Market,
}

impl PriceType {
    fn as_str(self) -> &'static str {
        match self {
            PriceType::Input => "INPUT",
            PriceType::Market => "MARKET",
        }
    }
}

/// 주문 요청. 수량·가격은 정밀도 보존 위해 String.
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    /// Toobit 선물은 `type=LIMIT` 고정, 시장가 여부는 `price_type`로 구분.
    pub price_type: PriceType,
    /// 주문 수량 (계약 수).
    pub quantity: String,
    /// 지정가. INPUT 필수, MARKET이면 None.
    pub price: Option<String>,
    /// 체결 조건. 지정가 권장.
    pub time_in_force: Option<TimeInForce>,
    /// 사용자 지정 주문 ID (멱등 추적).
    pub client_order_id: Option<String>,
}

impl OrderRequest {
    /// 시장가 주문 (`type=LIMIT` + `priceType=MARKET`).
    pub fn market(symbol: impl Into<String>, side: Side, quantity: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            price_type: PriceType::Market,
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
        quantity: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            price_type: PriceType::Input,
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

    fn to_params(&self) -> Vec<(String, String)> {
        let mut p = vec![
            ("symbol".to_string(), self.symbol.clone()),
            ("side".to_string(), self.side.as_str().to_string()),
            // Toobit 선물 주문 type은 LIMIT 고정(시장가는 priceType=MARKET).
            ("type".to_string(), "LIMIT".to_string()),
            ("priceType".to_string(), self.price_type.as_str().to_string()),
            ("quantity".to_string(), self.quantity.clone()),
        ];
        if let Some(price) = &self.price {
            p.push(("price".into(), price.clone()));
        }
        if let Some(tif) = self.time_in_force {
            p.push(("timeInForce".into(), tif.as_str().to_string()));
        }
        if let Some(id) = &self.client_order_id {
            p.push(("newClientOrderId".into(), id.clone()));
        }
        p
    }
}

/// 주문 응답 / 조회 결과 (선물).
#[derive(Debug, Clone, Deserialize)]
pub struct OrderResponse {
    #[serde(rename = "orderId")]
    pub order_id: String,
    pub symbol: String,
    /// NEW / PENDING_NEW / PARTIALLY_FILLED / FILLED / CANCELED / REJECTED.
    pub status: String,
    #[serde(rename = "clientOrderId", default)]
    pub client_order_id: String,
    #[serde(default)]
    pub price: String,
    /// 평균 체결가 (미체결이면 "0").
    #[serde(rename = "avgPrice", default)]
    pub avg_price: String,
    #[serde(rename = "origQty", default)]
    pub orig_qty: String,
    #[serde(rename = "executedQty", default)]
    pub executed_qty: String,
    #[serde(rename = "type", default)]
    pub order_type: String,
    #[serde(default)]
    pub side: String,
    #[serde(rename = "timeInForce", default)]
    pub time_in_force: String,
    /// INPUT / MARKET.
    #[serde(rename = "priceType", default)]
    pub price_type: String,
    #[serde(default)]
    pub leverage: String,
    /// 생성 시각 (epoch ms, **문자열** — 선물 응답).
    #[serde(rename = "time", default)]
    pub time: String,
    /// 갱신 시각 (epoch ms, **문자열**).
    #[serde(rename = "updateTime", default)]
    pub update_time: String,
}

/// 포지션 1건 (`GET /api/v1/futures/positions`).
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub symbol: String,
    /// LONG / SHORT (Toobit `side`).
    #[serde(default)]
    pub side: String,
    /// 포지션 수량 (계약 수).
    #[serde(default)]
    pub position: String,
    /// 평균 진입가.
    #[serde(rename = "avgPrice", default)]
    pub avg_price: String,
    #[serde(rename = "markPrice", default)]
    pub mark_price: String,
    /// 미실현손익 (USDT).
    #[serde(rename = "unrealizedPnL", default)]
    pub unrealized_pnl: String,
    /// 포지션 명목가치.
    #[serde(rename = "positionValue", default)]
    pub position_value: String,
    /// 점유 마진.
    #[serde(default)]
    pub margin: String,
    #[serde(default)]
    pub leverage: String,
}

/// 잔고 1건 (`GET /api/v1/futures/balance`).
#[derive(Debug, Clone, Deserialize)]
pub struct Balance {
    pub asset: String,
    /// 총 잔고.
    #[serde(default)]
    pub balance: String,
    /// 주문 가능 잔고.
    #[serde(rename = "availableBalance", default)]
    pub available_balance: String,
    /// 포지션 점유 마진.
    #[serde(rename = "positionMargin", default)]
    pub position_margin: String,
}

/// 레버리지 설정 결과 (`POST /api/v1/futures/leverage`).
/// 응답: `{"code":200,"symbolId":"...","leverage":"19"}`.
#[derive(Debug, Clone, Deserialize)]
pub struct LeverageResult {
    #[serde(rename = "symbolId")]
    pub symbol: String,
    pub leverage: String,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a ToobitClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a ToobitClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /api/v1/futures/order`). **실체결.**
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::POST,
                "/api/v1/futures/order",
                req.to_params(),
            ))
            .await?
            .parse()
    }

    /// 주문 취소 (`DELETE /api/v1/futures/order`).
    pub async fn cancel(&self, symbol: &str, order_id: &str) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::DELETE,
                "/api/v1/futures/order",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("orderId".into(), order_id.to_string()),
                ],
            ))
            .await?
            .parse()
    }

    /// 심볼 전체 미체결 주문 취소 (`DELETE /api/v1/futures/batchOrders`).
    pub async fn cancel_all(&self, symbol: &str) -> Result<serde_json::Value> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::DELETE,
                "/api/v1/futures/batchOrders",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 미체결 주문 목록 (`GET /api/v1/futures/openOrders`). `symbol=None`이면 전체.
    pub async fn open_orders(&self, symbol: Option<&str>) -> Result<Vec<OrderResponse>> {
        let params = symbol
            .map(|s| vec![("symbol".to_string(), s.to_string())])
            .unwrap_or_default();
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                "/api/v1/futures/openOrders",
                params,
            ))
            .await?
            .parse()
    }

    /// 포지션 조회 (`GET /api/v1/futures/positions`). `symbol=None`이면 전체.
    pub async fn positions(&self, symbol: Option<&str>) -> Result<Vec<Position>> {
        let params = symbol
            .map(|s| vec![("symbol".to_string(), s.to_string())])
            .unwrap_or_default();
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                "/api/v1/futures/positions",
                params,
            ))
            .await?
            .parse()
    }

    /// 선물 지갑 잔고 (`GET /api/v1/futures/balance`).
    pub async fn balances(&self) -> Result<Vec<Balance>> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                "/api/v1/futures/balance",
                vec![],
            ))
            .await?
            .parse()
    }

    /// 레버리지 설정 (`POST /api/v1/futures/leverage`).
    pub async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<LeverageResult> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::POST,
                "/api/v1/futures/leverage",
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
        let req = OrderRequest::market("SAMSUNG-SWAP-USDT", Side::BuyOpen, "3");
        let p = req.to_params();
        // 시장가 = type LIMIT + priceType MARKET, price 없음.
        assert!(p.contains(&("type".to_string(), "LIMIT".to_string())));
        assert!(p.contains(&("priceType".to_string(), "MARKET".to_string())));
        assert!(p.contains(&("side".to_string(), "BUY_OPEN".to_string())));
        assert!(p.contains(&("quantity".to_string(), "3".to_string())));
        assert!(!p.iter().any(|(k, _)| k == "price"));
        assert!(!p.iter().any(|(k, _)| k == "timeInForce"));
    }

    #[test]
    fn limit_order_params_default_gtc() {
        let req = OrderRequest::limit("SKHYNIX-SWAP-USDT", Side::SellClose, "2", "120.50")
            .client_order_id("kr-001");
        let p = req.to_params();
        assert!(p.contains(&("type".to_string(), "LIMIT".to_string())));
        assert!(p.contains(&("priceType".to_string(), "INPUT".to_string())));
        assert!(p.contains(&("side".to_string(), "SELL_CLOSE".to_string())));
        assert!(p.contains(&("price".to_string(), "120.50".to_string())));
        assert!(p.contains(&("timeInForce".to_string(), "GTC".to_string())));
        assert!(p.contains(&("newClientOrderId".to_string(), "kr-001".to_string())));
    }

    #[test]
    fn all_four_sides_map() {
        assert_eq!(Side::BuyOpen.as_str(), "BUY_OPEN");
        assert_eq!(Side::SellOpen.as_str(), "SELL_OPEN");
        assert_eq!(Side::BuyClose.as_str(), "BUY_CLOSE");
        assert_eq!(Side::SellClose.as_str(), "SELL_CLOSE");
    }

    #[test]
    fn order_response_parses_string_order_id() {
        let v = serde_json::json!({
            "symbol": "SAMSUNG-SWAP-USDT",
            "price": "230.00",
            "origQty": "1",
            "orderId": "2024837825254460160",
            "clientOrderId": "kr-001",
            "executedQty": "0",
            "status": "NEW",
            "timeInForce": "GTC",
            "type": "LIMIT",
            "side": "BUY_OPEN",
            "time": "1668418485058",
            "leverage": "2",
            "avgPrice": "0",
            "priceType": "INPUT"
        });
        let o: OrderResponse = serde_json::from_value(v).unwrap();
        // orderId는 64bit 초과 가능 → String.
        assert_eq!(o.order_id, "2024837825254460160");
        assert_eq!(o.order_type, "LIMIT");
        assert_eq!(o.side, "BUY_OPEN");
        assert_eq!(o.price_type, "INPUT");
    }

    #[test]
    fn position_parses() {
        let v = serde_json::json!({
            "symbol": "HYUNDAI-SWAP-USDT",
            "side": "SHORT",
            "position": "2",
            "avgPrice": "190.50",
            "markPrice": "188.00",
            "unrealizedPnL": "5.00",
            "positionValue": "376.00",
            "margin": "37.6",
            "leverage": "10"
        });
        let pos: Position = serde_json::from_value(v).unwrap();
        assert_eq!(pos.side, "SHORT");
        assert_eq!(pos.position, "2");
        assert_eq!(pos.unrealized_pnl, "5.00");
        assert_eq!(pos.leverage, "10");
    }

    #[test]
    fn leverage_result_parses() {
        let v = serde_json::json!({
            "code": 200,
            "symbolId": "SAMSUNG-SWAP-USDT",
            "leverage": "19"
        });
        let l: LeverageResult = serde_json::from_value(v).unwrap();
        assert_eq!(l.symbol, "SAMSUNG-SWAP-USDT");
        assert_eq!(l.leverage, "19");
    }
}
