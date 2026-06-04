//! 거래·계좌 도메인 (HMAC-SHA512 서명 필수) — 주문/취소/조회·포지션·잔고.
//!
//! **실거래 경고:** 이 모듈의 모든 호출은 실제 자금을 움직인다. Gate 선물은
//! 테스트넷이 별도 base URL이며, 운영 전 소액·`reduce_only`로 검증하라.
//!
//! Gate 주문 본문은 **JSON**이다(폼 파라미터 아님). 수량 `size`는 부호 있는
//! 정수(계약 수) — 양수=롱, 음수=숏. base 자산 수량 = `size × quanto_multiplier`.
//! 시장가는 `price="0"` + `tif="ioc"`(Gate는 시장가에 GTC를 거부).

use serde::{Deserialize, Serialize};

use crate::global::gateio::client::{ApiCall, GateioClient};
use crate::global::gateio::error::Result;

const ORDERS_PATH: &str = "/api/v4/futures/usdt/orders";
const POSITIONS_PATH: &str = "/api/v4/futures/usdt/positions";
const ACCOUNTS_PATH: &str = "/api/v4/futures/usdt/accounts";

/// 주문 방향. Gate는 `side` 필드가 없고 `size` 부호로 방향을 표현한다 —
/// 이 enum은 호출부 가독성을 위해 두고, `size` 부호로 접는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// 롱 (size 양수).
    Buy,
    /// 숏 (size 음수).
    Sell,
}

impl Side {
    /// 절대 계약 수에 방향 부호를 적용한다.
    fn signed(self, size: i64) -> i64 {
        let mag = size.abs();
        match self {
            Side::Buy => mag,
            Side::Sell => -mag,
        }
    }
}

/// 체결 조건 (Gate `tif`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// 취소 전까지 유효 (지정가 기본).
    Gtc,
    /// 즉시 체결·잔량 취소 (시장가 필수).
    Ioc,
    /// 전량 즉시 체결 아니면 취소.
    Fok,
    /// Post-only (메이커 전용).
    Poc,
}

impl TimeInForce {
    fn as_str(self) -> &'static str {
        match self {
            TimeInForce::Gtc => "gtc",
            TimeInForce::Ioc => "ioc",
            TimeInForce::Fok => "fok",
            TimeInForce::Poc => "poc",
        }
    }
}

/// 주문 본문 (Gate `POST /futures/usdt/orders`). JSON 직렬화된다.
///
/// `size`: 부호 있는 계약 수(양수=롱/음수=숏). 0은 불가.
/// `price`: 지정가 문자열; 시장가는 `"0"`.
/// `tif`: 체결 조건. 시장가는 `"ioc"`(또는 fok).
#[derive(Debug, Clone, Serialize)]
pub struct OrderRequest {
    /// contract 심볼. 예: `SAMSUNG_USDT`.
    pub contract: String,
    /// 부호 있는 주문 수량 (계약 수). base 수량 = `size × quanto_multiplier`.
    pub size: i64,
    /// 지정가 (정밀도 보존 String). 시장가는 `"0"`.
    pub price: String,
    /// 체결 조건.
    pub tif: String,
    /// 포지션 축소 전용 여부. 생략 시 false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduce_only: Option<bool>,
    /// 사용자 지정 주문 ID (멱등 추적). Gate는 `t-` 프리픽스를 요구.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

impl OrderRequest {
    /// 시장가 주문. Gate 규약상 `price="0"` + `tif="ioc"` (GTC 거부).
    /// `size`는 절대 계약 수(부호는 `side`가 결정).
    pub fn market(contract: impl Into<String>, side: Side, size: i64) -> Self {
        Self {
            contract: contract.into(),
            size: side.signed(size),
            price: "0".to_string(),
            tif: TimeInForce::Ioc.as_str().to_string(),
            reduce_only: None,
            text: None,
        }
    }

    /// 지정가 주문 (기본 GTC). `size`는 절대 계약 수.
    pub fn limit(
        contract: impl Into<String>,
        side: Side,
        size: i64,
        price: impl Into<String>,
    ) -> Self {
        Self {
            contract: contract.into(),
            size: side.signed(size),
            price: price.into(),
            tif: TimeInForce::Gtc.as_str().to_string(),
            reduce_only: None,
            text: None,
        }
    }

    /// 체결 조건 교체 (builder).
    pub fn tif(mut self, tif: TimeInForce) -> Self {
        self.tif = tif.as_str().to_string();
        self
    }

    /// 포지션 축소 전용으로 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = Some(v);
        self
    }

    /// 사용자 주문 ID 부여 (builder). Gate는 `t-` 프리픽스 필요.
    pub fn text(mut self, id: impl Into<String>) -> Self {
        self.text = Some(id.into());
        self
    }
}

/// 주문 응답 / 조회 결과 (`/futures/usdt/orders`).
#[derive(Debug, Clone, Deserialize)]
pub struct OrderResponse {
    /// Gate 주문 ID.
    pub id: i64,
    pub contract: String,
    /// "open" / "finished".
    #[serde(default)]
    pub status: String,
    /// 원 주문 수량(부호 있는 계약 수).
    pub size: i64,
    /// 미체결 잔량(부호 있는 계약 수). 0이면 전량 체결/취소.
    #[serde(default)]
    pub left: i64,
    /// 주문가 ("0"=시장가).
    #[serde(default)]
    pub price: String,
    /// 평균 체결가.
    #[serde(default)]
    pub fill_price: String,
    #[serde(default)]
    pub tif: String,
    #[serde(default)]
    pub reduce_only: bool,
    /// 사용자 주문 ID.
    #[serde(default)]
    pub text: String,
    /// 종료 사유 (취소·체결 등). 예: "filled", "cancelled".
    #[serde(default)]
    pub finish_as: String,
    /// 생성 시각 (epoch sec, 소수 가능).
    #[serde(default)]
    pub create_time: f64,
}

/// 포지션 1건 (`GET /futures/usdt/positions/{contract}`).
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub contract: String,
    /// 포지션 수량 (부호 = 방향; 음수 short). 0이면 무포지션.
    pub size: i64,
    /// 진입가.
    #[serde(default)]
    pub entry_price: String,
    /// 마크가.
    #[serde(default)]
    pub mark_price: String,
    /// 미실현손익 (USDT).
    #[serde(default)]
    pub unrealised_pnl: String,
    /// 청산가.
    #[serde(default)]
    pub liq_price: String,
    /// 레버리지 ("0"=cross).
    #[serde(default)]
    pub leverage: String,
}

/// 선물 계좌 잔고 (`GET /futures/usdt/accounts`).
#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    /// 정산 통화. 예: "USDT".
    #[serde(default)]
    pub currency: String,
    /// 총 잔고.
    #[serde(default)]
    pub total: String,
    /// 사용 가능 잔고.
    #[serde(default)]
    pub available: String,
    /// 미실현손익.
    #[serde(default)]
    pub unrealised_pnl: String,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a GateioClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a GateioClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /futures/usdt/orders`). **실체결.**
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderResponse> {
        let body = serde_json::to_string(req)?;
        self.client
            .call(ApiCall::signed(
                reqwest::Method::POST,
                ORDERS_PATH,
                vec![],
                Some(body),
            ))
            .await?
            .parse()
    }

    /// 단일 주문 취소 (`DELETE /futures/usdt/orders/{id}`).
    pub async fn cancel(&self, order_id: i64) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::DELETE,
                format!("{ORDERS_PATH}/{order_id}"),
                vec![],
                None,
            ))
            .await?
            .parse()
    }

    /// contract 전체 미체결 주문 취소 (`DELETE /futures/usdt/orders?contract=...`).
    pub async fn cancel_all(&self, contract: &str) -> Result<Vec<OrderResponse>> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::DELETE,
                ORDERS_PATH,
                vec![("contract".into(), contract.into())],
                None,
            ))
            .await?
            .parse()
    }

    /// 단일 주문 조회 (`GET /futures/usdt/orders/{id}`).
    pub async fn get_order(&self, order_id: i64) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                format!("{ORDERS_PATH}/{order_id}"),
                vec![],
                None,
            ))
            .await?
            .parse()
    }

    /// 미체결 주문 목록 (`GET /futures/usdt/orders?status=open`).
    /// `contract=None`이면 전체.
    pub async fn open_orders(&self, contract: Option<&str>) -> Result<Vec<OrderResponse>> {
        let mut params = vec![("status".to_string(), "open".to_string())];
        if let Some(c) = contract {
            params.push(("contract".into(), c.to_string()));
        }
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                ORDERS_PATH,
                params,
                None,
            ))
            .await?
            .parse()
    }

    /// 단일 contract 포지션 조회 (`GET /futures/usdt/positions/{contract}`).
    pub async fn position(&self, contract: &str) -> Result<Position> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                format!("{POSITIONS_PATH}/{contract}"),
                vec![],
                None,
            ))
            .await?
            .parse()
    }

    /// 전체 포지션 조회 (`GET /futures/usdt/positions`).
    pub async fn positions(&self) -> Result<Vec<Position>> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                POSITIONS_PATH,
                vec![],
                None,
            ))
            .await?
            .parse()
    }

    /// 선물 계좌 잔고 (`GET /futures/usdt/accounts`).
    pub async fn account(&self) -> Result<Account> {
        self.client
            .call(ApiCall::signed(
                reqwest::Method::GET,
                ACCOUNTS_PATH,
                vec![],
                None,
            ))
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_order_is_price_zero_ioc_signed() {
        // 시장가 매도 → price="0", tif="ioc", size 음수.
        let req = OrderRequest::market("SAMSUNG_USDT", Side::Sell, 3);
        assert_eq!(req.price, "0");
        assert_eq!(req.tif, "ioc");
        assert_eq!(req.size, -3);
        let body = serde_json::to_string(&req).unwrap();
        // reduce_only/text는 None이라 직렬화 제외.
        assert_eq!(
            body,
            r#"{"contract":"SAMSUNG_USDT","size":-3,"price":"0","tif":"ioc"}"#
        );
    }

    #[test]
    fn limit_order_default_gtc_long_positive_size() {
        let req = OrderRequest::limit("SKHYNIX_USDT", Side::Buy, 2, "120.50");
        assert_eq!(req.size, 2);
        assert_eq!(req.price, "120.50");
        assert_eq!(req.tif, "gtc");
    }

    #[test]
    fn builders_set_reduce_only_and_text() {
        let req = OrderRequest::limit("HYUNDAI_USDT", Side::Sell, 5, "230.00")
            .reduce_only(true)
            .text("t-kr-001");
        assert_eq!(req.size, -5);
        assert_eq!(req.reduce_only, Some(true));
        assert_eq!(req.text.as_deref(), Some("t-kr-001"));
        let body = serde_json::to_string(&req).unwrap();
        assert!(body.contains(r#""reduce_only":true"#));
        assert!(body.contains(r#""text":"t-kr-001""#));
    }

    #[test]
    fn order_response_parses() {
        let v = serde_json::json!({
            "id": 123456789i64,
            "contract": "SAMSUNG_USDT",
            "status": "open",
            "size": 3,
            "left": 3,
            "price": "230.00",
            "fill_price": "0",
            "tif": "gtc",
            "reduce_only": false,
            "text": "t-kr-001",
            "finish_as": "",
            "create_time": 1780550000.0
        });
        let o: OrderResponse = serde_json::from_value(v).unwrap();
        assert_eq!(o.id, 123456789);
        assert_eq!(o.size, 3);
        assert_eq!(o.left, 3);
        assert_eq!(o.status, "open");
    }

    #[test]
    fn position_parses_signed_size() {
        let v = serde_json::json!({
            "contract": "HYUNDAI_USDT",
            "size": -2,
            "entry_price": "190.50",
            "mark_price": "188.00",
            "unrealised_pnl": "5.0",
            "liq_price": "250.00",
            "leverage": "10"
        });
        let p: Position = serde_json::from_value(v).unwrap();
        assert_eq!(p.size, -2);
        assert_eq!(p.unrealised_pnl, "5.0");
        assert_eq!(p.leverage, "10");
    }
}
