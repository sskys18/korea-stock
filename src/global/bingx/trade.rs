//! 거래·계좌 도메인 (HMAC 서명 필수) — 주문/취소/조회·포지션·잔고·레버리지.
//!
//! **실거래 경고:** 이 모듈의 모든 호출은 실제 자금을 움직인다. BingX는 별도
//! 테스트넷을 제공하지 않으므로 소액·reduce-only로 운영에서 신중히 검증한다.
//!
//! BingX Perpetual Swap V2(`/openApi/swap/v2/trade/*`). 파라미터는 (서명 대상으로)
//! 삽입순 queryString에 실린다. 주문 응답은 envelope `data.order`에 들어온다
//! ([`OrderResponse`]). `positionSide`는 단방향(one-way) 계정이면 생략 가능, 헤지
//! 모드면 LONG/SHORT 필수.
//!
//! **미검증 주의:** 시세는 라이브로 증명되나(키 불필요), 이 거래 경로는 운영 키가
//! 없어 실주문으로 검증하지 못했다. 서명기 자체는 공식 문서 테스트 벡터로 검증된다
//! (`client` 단위테스트). 필드명(특히 `clientOrderID` 대문자 ID)은 BingX Swap V2
//! 규약을 따른다.

use serde::Deserialize;

use crate::global::bingx::client::{ApiCall, BingxClient};
use crate::global::bingx::error::Result;

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

/// 포지션 방향. 헤지 모드 필수, 단방향(one-way) 계정이면 `Both`(생략).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionSide {
    Long,
    Short,
    /// 단방향 계정 — `positionSide` 파라미터를 보내지 않는다.
    Both,
}

impl PositionSide {
    /// 전송할 값. `Both`는 파라미터 생략(None).
    fn as_param(self) -> Option<&'static str> {
        match self {
            PositionSide::Long => Some("LONG"),
            PositionSide::Short => Some("SHORT"),
            PositionSide::Both => None,
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

/// 주문 요청. 수량·가격은 정밀도 보존 위해 String (계약 `quantityPrecision`/
/// `pricePrecision`에 맞춰 호출부가 포맷).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub position_side: PositionSide,
    pub order_type: OrderType,
    /// 주문 수량 (기초자산 단위).
    pub quantity: String,
    /// 지정가. LIMIT 필수, MARKET이면 None.
    pub price: Option<String>,
    /// 포지션 축소 전용 여부.
    pub reduce_only: Option<bool>,
    /// 사용자 지정 주문 ID (멱등 추적). BingX Swap V2 필드명 `clientOrderID`(대문자 ID).
    pub client_order_id: Option<String>,
}

impl OrderRequest {
    /// 시장가 주문 (단방향 계정 기본).
    pub fn market(symbol: impl Into<String>, side: Side, quantity: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            position_side: PositionSide::Both,
            order_type: OrderType::Market,
            quantity: quantity.into(),
            price: None,
            reduce_only: None,
            client_order_id: None,
        }
    }

    /// 지정가 주문 (단방향 계정 기본).
    pub fn limit(
        symbol: impl Into<String>,
        side: Side,
        quantity: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            position_side: PositionSide::Both,
            order_type: OrderType::Limit,
            quantity: quantity.into(),
            price: Some(price.into()),
            reduce_only: None,
            client_order_id: None,
        }
    }

    /// 포지션 방향 지정 (헤지 모드 LONG/SHORT) (builder).
    pub fn position_side(mut self, ps: PositionSide) -> Self {
        self.position_side = ps;
        self
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
        if let Some(ps) = self.position_side.as_param() {
            p.push(("positionSide".into(), ps.to_string()));
        }
        if let Some(price) = &self.price {
            p.push(("price".into(), price.clone()));
        }
        if let Some(ro) = self.reduce_only {
            p.push(("reduceOnly".into(), ro.to_string()));
        }
        if let Some(id) = &self.client_order_id {
            p.push(("clientOrderID".into(), id.clone()));
        }
        p
    }
}

/// 주문 응답 — envelope `data.order`. BingX Swap V2 주문 객체.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    pub order_id: i64,
    pub symbol: String,
    pub side: String,
    #[serde(default)]
    pub position_side: String,
    #[serde(rename = "type")]
    pub order_type: String,
    /// 사용자 주문 ID (`clientOrderID`).
    #[serde(default, rename = "clientOrderID")]
    pub client_order_id: String,
    #[serde(default)]
    pub price: String,
    /// 주문 수량.
    #[serde(default)]
    pub quantity: String,
}

/// 주문 응답 envelope 래퍼 (`data.order`).
#[derive(Deserialize)]
struct OrderEnvelope {
    order: OrderResponse,
}

/// 포지션 1건 (`GET /openApi/swap/v2/user/positions`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub symbol: String,
    /// LONG / SHORT.
    #[serde(default)]
    pub position_side: String,
    /// 포지션 수량.
    #[serde(default)]
    pub position_amt: String,
    #[serde(default)]
    pub avg_price: String,
    /// 미실현손익 (USDT).
    #[serde(default)]
    pub unrealized_profit: String,
    #[serde(default)]
    pub leverage: String,
}

/// 잔고 (`GET /openApi/swap/v2/user/balance` → data.balance).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Balance {
    pub asset: String,
    /// 지갑 잔고.
    #[serde(default)]
    pub balance: String,
    /// 주문 가능 잔고.
    #[serde(default)]
    pub available_margin: String,
    /// 미실현손익.
    #[serde(default)]
    pub unrealized_profit: String,
}

/// 잔고 응답 envelope 래퍼 (`data.balance`).
#[derive(Deserialize)]
struct BalanceEnvelope {
    balance: Balance,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a BingxClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a BingxClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /openApi/swap/v2/trade/order`). **실체결.**
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderResponse> {
        let env: OrderEnvelope = self
            .client
            .call(ApiCall::signed(
                reqwest::Method::POST,
                "/openApi/swap/v2/trade/order",
                req.to_params(),
            ))
            .await?
            .parse()?;
        Ok(env.order)
    }

    /// 주문 취소 (`DELETE /openApi/swap/v2/trade/order`).
    pub async fn cancel(&self, symbol: &str, order_id: i64) -> Result<OrderResponse> {
        let env: OrderEnvelope = self
            .client
            .call(ApiCall::signed(
                reqwest::Method::DELETE,
                "/openApi/swap/v2/trade/order",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("orderId".into(), order_id.to_string()),
                ],
            ))
            .await?
            .parse()?;
        Ok(env.order)
    }

    /// 미체결 주문 목록 (`GET /openApi/swap/v2/trade/openOrders`). `symbol=None`이면 전체.
    pub async fn open_orders(&self, symbol: Option<&str>) -> Result<serde_json::Value> {
        let params = symbol
            .map(|s| vec![("symbol".to_string(), s.to_string())])
            .unwrap_or_default();
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                "/openApi/swap/v2/trade/openOrders",
                params,
            ))
            .await?
            .parse()
    }

    /// 포지션 조회 (`GET /openApi/swap/v2/user/positions`). `symbol=None`이면 전체.
    pub async fn positions(&self, symbol: Option<&str>) -> Result<Vec<Position>> {
        let params = symbol
            .map(|s| vec![("symbol".to_string(), s.to_string())])
            .unwrap_or_default();
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                "/openApi/swap/v2/user/positions",
                params,
            ))
            .await?
            .parse()
    }

    /// 선물 계좌 잔고 (`GET /openApi/swap/v2/user/balance`).
    pub async fn balance(&self) -> Result<Balance> {
        let env: BalanceEnvelope = self
            .client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                "/openApi/swap/v2/user/balance",
                vec![],
            ))
            .await?
            .parse()?;
        Ok(env.balance)
    }

    /// 레버리지 설정 (`POST /openApi/swap/v2/trade/leverage`). `side` ∈ LONG/SHORT.
    pub async fn set_leverage(
        &self,
        symbol: &str,
        side: PositionSide,
        leverage: u32,
    ) -> Result<serde_json::Value> {
        let side_str = side.as_param().unwrap_or("LONG");
        self.client
            .call(ApiCall::signed(
                reqwest::Method::POST,
                "/openApi/swap/v2/trade/leverage",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("side".into(), side_str.to_string()),
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
    fn market_order_params_oneway_omits_position_side() {
        let req = OrderRequest::market("NCSKSAMSUNG2USD-USDT", Side::Buy, "0.01");
        let p = req.to_params();
        assert!(p.contains(&("type".to_string(), "MARKET".to_string())));
        assert!(p.contains(&("side".to_string(), "BUY".to_string())));
        assert!(p.contains(&("quantity".to_string(), "0.01".to_string())));
        // MARKET은 price 없음, 단방향은 positionSide 생략.
        assert!(!p.iter().any(|(k, _)| k == "price"));
        assert!(!p.iter().any(|(k, _)| k == "positionSide"));
    }

    #[test]
    fn limit_order_params_with_hedge_and_client_id() {
        let req = OrderRequest::limit("NCSKSKHYNIX2USD-USDT", Side::Sell, "0.02", "1510.00")
            .position_side(PositionSide::Short)
            .reduce_only(true)
            .client_order_id("kr-bingx-001");
        let p = req.to_params();
        assert!(p.contains(&("type".to_string(), "LIMIT".to_string())));
        assert!(p.contains(&("price".to_string(), "1510.00".to_string())));
        assert!(p.contains(&("positionSide".to_string(), "SHORT".to_string())));
        assert!(p.contains(&("reduceOnly".to_string(), "true".to_string())));
        // BingX Swap V2 필드명: clientOrderID (대문자 ID).
        assert!(p.contains(&("clientOrderID".to_string(), "kr-bingx-001".to_string())));
    }

    #[test]
    fn order_response_parses_under_order_envelope() {
        let v = serde_json::json!({
            "order": {
                "orderId": 1234567890i64,
                "symbol": "NCSKSAMSUNG2USD-USDT",
                "side": "BUY",
                "positionSide": "LONG",
                "type": "LIMIT",
                "clientOrderID": "kr-bingx-001",
                "price": "230.00",
                "quantity": "0.01"
            }
        });
        let env: OrderEnvelope = serde_json::from_value(v).unwrap();
        let o = env.order;
        assert_eq!(o.order_id, 1234567890);
        assert_eq!(o.symbol, "NCSKSAMSUNG2USD-USDT");
        assert_eq!(o.order_type, "LIMIT");
        assert_eq!(o.client_order_id, "kr-bingx-001");
    }
}
