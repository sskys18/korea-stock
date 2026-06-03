//! 거래·계좌 도메인 — 주문/취소·포지션·잔고.
//!
//! **상태: data-only.** Lighter 주문 서명은 zk 친화 해시(poseidon)+schnorr 류 커스텀
//! 스킴이며, 공식 `lighter-python`·비공식 `lighter-rust` 모두 네이티브 Go
//! 바이너리(`lighter-go`)를 FFI로 호출해 서명한다. crates.io에 순수 Rust 구현이 없고
//! 이 세션에서 알고리즘을 공식 문서로 재현 검증할 수 없어, **서명을 날조하지 않는다**
//! (HARD RULE 3).
//!
//! 따라서 이 모듈은:
//! - 타입드 주문/취소 요청·응답 구조체와 호출 경로를 **완비**한다.
//! - 서명이 필요 없는 [`Trade::next_nonce`](`GET /api/v1/nextNonce`)는 **동작**한다.
//! - 실제 서명 제출([`Trade::place`]/[`Trade::cancel`])은
//!   [`LighterError::SignerUnavailable`]로 명확히 거부한다.
//!
//! 서명 구현 시 추가해야 할 것: `lighter-go`의 poseidon 해시 + schnorr 서명을 순수
//! Rust로 포팅하거나 검증된 FFI로 래핑 → `tx_info`(서명된 트랜잭션 JSON) 생성 →
//! `POST /api/v1/sendTx`(form: `tx_type`, `tx_info`, `price_protection`) 또는
//! `POST /api/v1/sendTxBatch`(form: `tx_types`, `tx_infos`).

use serde::Deserialize;

use crate::lighter::client::{ApiCall, LighterClient};
use crate::lighter::error::{LighterError, Result};

/// 주문 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// 매수 (is_ask=false).
    Buy,
    /// 매도 (is_ask=true).
    Sell,
}

impl Side {
    /// Lighter 트랜잭션 필드 `is_ask`로 변환 (매도=true).
    pub fn is_ask(self) -> bool {
        matches!(self, Side::Sell)
    }
}

/// 주문 유형. Lighter 트랜잭션 `order_type` 코드에 대응 (서명 구현 시 사용).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

/// 체결 조건. Lighter 트랜잭션 `time_in_force` 코드에 대응.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// 취소 전까지 유효.
    GoodTillTime,
    /// 즉시 체결·잔량 취소.
    ImmediateOrCancel,
    /// 전량 즉시 체결 아니면 취소.
    FillOrKill,
}

/// 주문 요청. 수량·가격은 정밀도 보존 위해 String. 서명 단계에서 Lighter는 이를
/// `supported_*_decimals` 기준 정수 스케일로 바꾸므로, 호출부는 사람이 읽는 십진
/// 문자열을 그대로 넣는다(스케일 변환은 서명 구현이 담당).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    /// 마켓 식별자. 심볼→id는 [`crate::lighter::market::Market::market_id`].
    pub market_id: u32,
    pub side: Side,
    pub order_type: OrderType,
    /// 주문 수량 (base 토큰, 십진 문자열).
    pub base_amount: String,
    /// 지정가 (십진 문자열). LIMIT 필수, MARKET이면 None.
    pub price: Option<String>,
    /// 체결 조건. LIMIT 필수.
    pub time_in_force: Option<TimeInForce>,
    /// 포지션 축소 전용 여부.
    pub reduce_only: bool,
    /// 클라이언트 주문 인덱스 (멱등 추적; Lighter `client_order_index`).
    pub client_order_index: Option<u64>,
}

impl OrderRequest {
    /// 시장가 주문.
    pub fn market(market_id: u32, side: Side, base_amount: impl Into<String>) -> Self {
        Self {
            market_id,
            side,
            order_type: OrderType::Market,
            base_amount: base_amount.into(),
            price: None,
            time_in_force: Some(TimeInForce::ImmediateOrCancel),
            reduce_only: false,
            client_order_index: None,
        }
    }

    /// 지정가 주문 (기본 GoodTillTime).
    pub fn limit(
        market_id: u32,
        side: Side,
        base_amount: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            market_id,
            side,
            order_type: OrderType::Limit,
            base_amount: base_amount.into(),
            price: Some(price.into()),
            time_in_force: Some(TimeInForce::GoodTillTime),
            reduce_only: false,
            client_order_index: None,
        }
    }

    /// 포지션 축소 전용으로 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = v;
        self
    }

    /// 클라이언트 주문 인덱스 부여 (builder).
    pub fn client_order_index(mut self, i: u64) -> Self {
        self.client_order_index = Some(i);
        self
    }
}

/// nextNonce 응답 (`GET /api/v1/nextNonce`). 서명 불필요 — **동작**한다.
#[derive(Debug, Clone, Deserialize)]
pub struct NextNonce {
    /// 다음 트랜잭션에 쓸 nonce.
    pub nonce: u64,
}

/// 활성 주문 1건 (`GET /api/v1/accountActiveOrders` → orders[]).
///
/// **주의:** 이 엔드포인트는 `auth` 토큰(서명 파생)을 요구하므로 본 어댑터에서는
/// 호출할 수 없다. 구조체는 서명 구현 후를 위해 정의해 둔다.
#[derive(Debug, Clone, Deserialize)]
pub struct ActiveOrder {
    pub order_id: String,
    pub market_id: u32,
    pub is_ask: bool,
    pub price: String,
    pub remaining_base_amount: String,
    pub initial_base_amount: String,
    #[serde(default)]
    pub status: String,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a LighterClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a LighterClient) -> Self {
        Self { client }
    }

    /// 다음 nonce 조회 (`GET /api/v1/nextNonce`). 서명 불필요 — **동작**한다.
    /// 서명 구현 시 트랜잭션 빌드 전에 호출한다.
    pub async fn next_nonce(&self) -> Result<NextNonce> {
        let cfg = self.client.config();
        let account_index = cfg.account_index.ok_or_else(|| {
            LighterError::Auth("nextNonce requires account_index (config.account_index)".into())
        })?;
        self.client
            .call(ApiCall::get(
                "/api/v1/nextNonce",
                vec![
                    ("account_index".into(), account_index.to_string()),
                    ("api_key_index".into(), cfg.api_key_index.to_string()),
                ],
            ))
            .await?
            .parse()
    }

    /// 주문 제출. **차단됨** — 서명 미검증(data-only).
    ///
    /// 서명이 구현되면: nextNonce → 트랜잭션 빌드 → poseidon 해시 → schnorr 서명 →
    /// `tx_info` 직렬화 → `POST /api/v1/sendTx`. 현재는 서명을 날조하지 않고
    /// [`LighterError::SignerUnavailable`]을 반환한다.
    pub async fn place(&self, _req: &OrderRequest) -> Result<()> {
        self.ensure_signer()?;
        Err(LighterError::SignerUnavailable(
            "create_order 서명 미구현 — lighter-go poseidon/schnorr 스킴을 순수 Rust로 \
             검증·포팅하기 전까지 제출 차단(data-only)"
                .into(),
        ))
    }

    /// 주문 취소. **차단됨** — 서명 미검증(data-only).
    pub async fn cancel(&self, _market_id: u32, _order_id: &str) -> Result<()> {
        self.ensure_signer()?;
        Err(LighterError::SignerUnavailable(
            "cancel_order 서명 미구현 — 제출 차단(data-only)".into(),
        ))
    }

    /// 활성 주문 조회. **차단됨** — `auth`(서명 파생) 토큰 미구현.
    pub async fn active_orders(&self) -> Result<Vec<ActiveOrder>> {
        self.ensure_signer()?;
        Err(LighterError::SignerUnavailable(
            "accountActiveOrders는 서명 파생 auth 토큰을 요구 — 미구현(data-only)".into(),
        ))
    }

    /// 자격증명(account_index + 개인키) 존재 확인. 없으면 [`LighterError::Auth`].
    /// 자격증명이 있어도 서명기 자체가 없으므로 제출 메서드는 SignerUnavailable로 끝난다.
    fn ensure_signer(&self) -> Result<()> {
        let cfg = self.client.config();
        if cfg.account_index.is_none() || cfg.api_key_private_key.is_empty() {
            return Err(LighterError::Auth(
                "trading requires account_index + api_key_private_key".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_is_ask_mapping() {
        assert!(!Side::Buy.is_ask());
        assert!(Side::Sell.is_ask());
    }

    #[test]
    fn market_order_defaults_ioc() {
        let req = OrderRequest::market(162, Side::Buy, "3");
        assert_eq!(req.market_id, 162);
        assert_eq!(req.order_type, OrderType::Market);
        assert_eq!(req.time_in_force, Some(TimeInForce::ImmediateOrCancel));
        assert!(req.price.is_none());
    }

    #[test]
    fn limit_order_defaults_gtt_and_builders() {
        let req = OrderRequest::limit(161, Side::Sell, "2", "120.50")
            .reduce_only(true)
            .client_order_index(7);
        assert_eq!(req.order_type, OrderType::Limit);
        assert_eq!(req.price.as_deref(), Some("120.50"));
        assert_eq!(req.time_in_force, Some(TimeInForce::GoodTillTime));
        assert!(req.reduce_only);
        assert_eq!(req.client_order_index, Some(7));
    }

    #[test]
    fn next_nonce_parses() {
        // 라이브: {"code":200,"nonce":1}.
        let v = serde_json::json!({ "code": 200, "nonce": 1 });
        let n: NextNonce = serde_json::from_value(v).unwrap();
        assert_eq!(n.nonce, 1);
    }

    #[test]
    fn active_order_parses() {
        let v = serde_json::json!({
            "order_id": "45880421206297236",
            "market_id": 162,
            "is_ask": true,
            "price": "244.045",
            "remaining_base_amount": "22.531",
            "initial_base_amount": "22.531",
            "status": "open"
        });
        let o: ActiveOrder = serde_json::from_value(v).unwrap();
        assert_eq!(o.market_id, 162);
        assert!(o.is_ask);
        assert_eq!(o.price, "244.045");
    }
}
