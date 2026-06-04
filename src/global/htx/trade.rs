//! 거래·계좌 도메인 (HMAC 서명 필수) — 주문/취소·포지션·잔고.
//!
//! HTX USDT-M cross(교차마진) 엔드포인트(`/linear-swap-api/v1/swap_cross_*`)를
//! 우선 지원한다. 모든 호출은 POST + JSON 본문이며 서명 파라미터는 쿼리로 간다.
//!
//! **실거래 경고:** 이 모듈의 모든 주문 호출은 실제 자금을 움직인다. HTX는 별도
//! 무기한선물 테스트넷을 운영하지 않으므로(2026-06 기준), 검증은 최소 수량·먼 지정가로
//! 한다. 본 어댑터의 서명 경로는 doc 미공개로 **openssl 교차검증 self-consistency**까지만
//! 검증됐고, **라이브 키 end-to-end 주문은 미검증**이다(키 미보유).

use serde::Deserialize;

use crate::global::htx::client::{ApiCall, HtxClient};
use crate::global::htx::error::Result;

/// 주문 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Buy,
    Sell,
}

impl Direction {
    fn as_str(self) -> &'static str {
        match self {
            Direction::Buy => "buy",
            Direction::Sell => "sell",
        }
    }
}

/// 포지션 개폐 방향. HTX는 direction(매수/매도)과 offset(개/청산)을 분리한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offset {
    /// 신규 진입.
    Open,
    /// 기존 포지션 청산.
    Close,
}

impl Offset {
    fn as_str(self) -> &'static str {
        match self {
            Offset::Open => "open",
            Offset::Close => "close",
        }
    }
}

/// 가격 유형 (`order_price_type`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderPriceType {
    /// 지정가. `price` 필수.
    Limit,
    /// 상대가(시장가 유사 — 상대 호가 1틱).
    Opponent,
    /// 메이커 전용(테이커면 취소).
    PostOnly,
    /// 즉시 체결·잔량 취소.
    Ioc,
    /// 전량 즉시 체결 아니면 취소.
    Fok,
}

impl OrderPriceType {
    fn as_str(self) -> &'static str {
        match self {
            OrderPriceType::Limit => "limit",
            OrderPriceType::Opponent => "opponent",
            OrderPriceType::PostOnly => "post_only",
            OrderPriceType::Ioc => "ioc",
            OrderPriceType::Fok => "fok",
        }
    }
}

/// 주문 요청. `volume`은 계약 수(정수). `price`는 정밀도 보존 위해 String.
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub contract_code: String,
    /// 주문 수량 (계약 수).
    pub volume: u64,
    pub direction: Direction,
    pub offset: Offset,
    /// 레버리지 배수.
    pub lever_rate: u32,
    pub order_price_type: OrderPriceType,
    /// 지정가. `Limit`/`PostOnly` 필수, 시장성 유형이면 None.
    pub price: Option<String>,
    /// 사용자 지정 주문 ID(멱등 추적). HTX는 정수형을 권장하나 String 보존.
    pub client_order_id: Option<String>,
}

impl OrderRequest {
    /// 지정가 진입 주문.
    pub fn limit(
        contract_code: impl Into<String>,
        direction: Direction,
        offset: Offset,
        volume: u64,
        lever_rate: u32,
        price: impl Into<String>,
    ) -> Self {
        Self {
            contract_code: contract_code.into(),
            volume,
            direction,
            offset,
            lever_rate,
            order_price_type: OrderPriceType::Limit,
            price: Some(price.into()),
            client_order_id: None,
        }
    }

    /// 상대가(시장가 유사) 진입 주문.
    pub fn opponent(
        contract_code: impl Into<String>,
        direction: Direction,
        offset: Offset,
        volume: u64,
        lever_rate: u32,
    ) -> Self {
        Self {
            contract_code: contract_code.into(),
            volume,
            direction,
            offset,
            lever_rate,
            order_price_type: OrderPriceType::Opponent,
            price: None,
            client_order_id: None,
        }
    }

    /// 사용자 주문 ID 부여 (builder).
    pub fn client_order_id(mut self, id: impl Into<String>) -> Self {
        self.client_order_id = Some(id.into());
        self
    }

    fn to_body(&self) -> serde_json::Value {
        let mut body = serde_json::json!({
            "contract_code": self.contract_code,
            "volume": self.volume,
            "direction": self.direction.as_str(),
            "offset": self.offset.as_str(),
            "lever_rate": self.lever_rate,
            "order_price_type": self.order_price_type.as_str(),
        });
        let obj = body.as_object_mut().expect("json object");
        if let Some(price) = &self.price {
            obj.insert("price".into(), serde_json::json!(price));
        }
        if let Some(id) = &self.client_order_id {
            obj.insert("client_order_id".into(), serde_json::json!(id));
        }
        body
    }
}

/// 주문 제출 응답 `data` (`POST /linear-swap-api/v1/swap_cross_order`).
#[derive(Debug, Clone, Deserialize)]
pub struct OrderResponse {
    /// 주문 ID(정수).
    pub order_id: i64,
    /// 주문 ID(문자열, 정밀도 보존).
    #[serde(default)]
    pub order_id_str: String,
    /// 사용자 지정 주문 ID(있을 때).
    #[serde(default)]
    pub client_order_id: Option<i64>,
}

/// 포지션 1건 (`POST /linear-swap-api/v1/swap_cross_position_info`).
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub contract_code: String,
    /// 보유 수량(계약).
    pub volume: f64,
    /// "buy"(롱) / "sell"(숏).
    pub direction: String,
    pub cost_open: f64,
    /// 미실현손익(USDT).
    pub profit_unreal: f64,
    pub lever_rate: f64,
    #[serde(default)]
    pub position_margin: f64,
}

/// 교차마진 계좌 1건 (`POST /linear-swap-api/v1/swap_cross_account_info`).
#[derive(Debug, Clone, Deserialize)]
pub struct AccountInfo {
    /// 마진 통화("USDT").
    pub margin_asset: String,
    /// 계좌 잔고.
    pub margin_balance: f64,
    /// 사용 가능 잔고.
    pub margin_available: f64,
    /// 미실현손익.
    #[serde(default)]
    pub profit_unreal: f64,
    /// 명목 위험률.
    #[serde(default)]
    pub risk_rate: Option<f64>,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a HtxClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a HtxClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (cross, `POST /linear-swap-api/v1/swap_cross_order`). **실체결.**
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed_post(
                "/linear-swap-api/v1/swap_cross_order",
                req.to_body(),
            ))
            .await?
            .parse()
    }

    /// 주문 취소 (`POST /linear-swap-api/v1/swap_cross_cancel`). `order_id`로 취소.
    pub async fn cancel(&self, contract_code: &str, order_id: i64) -> Result<serde_json::Value> {
        self.client
            .call(ApiCall::signed_post(
                "/linear-swap-api/v1/swap_cross_cancel",
                serde_json::json!({
                    "contract_code": contract_code,
                    "order_id": order_id.to_string(),
                }),
            ))
            .await?
            .parse()
    }

    /// 포지션 조회 (`POST /linear-swap-api/v1/swap_cross_position_info`).
    /// `contract_code=None`이면 전체.
    pub async fn positions(&self, contract_code: Option<&str>) -> Result<Vec<Position>> {
        let body = match contract_code {
            Some(c) => serde_json::json!({ "contract_code": c }),
            None => serde_json::json!({}),
        };
        self.client
            .call(ApiCall::signed_post(
                "/linear-swap-api/v1/swap_cross_position_info",
                body,
            ))
            .await?
            .parse()
    }

    /// 교차마진 계좌 정보 (`POST /linear-swap-api/v1/swap_cross_account_info`).
    /// `margin_account=None`이면 전체("USDT").
    pub async fn accounts(&self, margin_account: Option<&str>) -> Result<Vec<AccountInfo>> {
        let body = match margin_account {
            Some(a) => serde_json::json!({ "margin_account": a }),
            None => serde_json::json!({}),
        };
        self.client
            .call(ApiCall::signed_post(
                "/linear-swap-api/v1/swap_cross_account_info",
                body,
            ))
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_order_body() {
        let req = OrderRequest::limit(
            "SAMSUNG-USDT",
            Direction::Buy,
            Offset::Open,
            3,
            5,
            "230.00",
        )
        .client_order_id("kr-001");
        let b = req.to_body();
        assert_eq!(b["contract_code"], "SAMSUNG-USDT");
        assert_eq!(b["volume"], 3);
        assert_eq!(b["direction"], "buy");
        assert_eq!(b["offset"], "open");
        assert_eq!(b["lever_rate"], 5);
        assert_eq!(b["order_price_type"], "limit");
        assert_eq!(b["price"], "230.00");
        assert_eq!(b["client_order_id"], "kr-001");
    }

    #[test]
    fn opponent_order_omits_price() {
        let req = OrderRequest::opponent("SKHYNIX-USDT", Direction::Sell, Offset::Close, 2, 10);
        let b = req.to_body();
        assert_eq!(b["order_price_type"], "opponent");
        assert_eq!(b["direction"], "sell");
        assert_eq!(b["offset"], "close");
        assert!(b.get("price").is_none());
        assert!(b.get("client_order_id").is_none());
    }

    #[test]
    fn order_response_parses() {
        let v = serde_json::json!({
            "order_id": 979131854869454848i64,
            "order_id_str": "979131854869454848",
            "client_order_id": 1001
        });
        let o: OrderResponse = serde_json::from_value(v).unwrap();
        assert_eq!(o.order_id_str, "979131854869454848");
        assert_eq!(o.client_order_id, Some(1001));
    }

    #[test]
    fn position_parses() {
        let v = serde_json::json!({
            "contract_code": "HYUNDAI-USDT",
            "volume": 2.0,
            "direction": "sell",
            "cost_open": 190.5,
            "profit_unreal": 5.0,
            "lever_rate": 10.0,
            "position_margin": 38.1
        });
        let p: Position = serde_json::from_value(v).unwrap();
        assert_eq!(p.contract_code, "HYUNDAI-USDT");
        assert_eq!(p.direction, "sell");
        assert_eq!(p.profit_unreal, 5.0);
    }

    #[test]
    fn account_parses() {
        let v = serde_json::json!({
            "margin_asset": "USDT",
            "margin_balance": 1000.0,
            "margin_available": 850.0,
            "profit_unreal": 5.0,
            "risk_rate": 12.5
        });
        let a: AccountInfo = serde_json::from_value(v).unwrap();
        assert_eq!(a.margin_asset, "USDT");
        assert_eq!(a.margin_available, 850.0);
        assert_eq!(a.risk_rate, Some(12.5));
    }
}
