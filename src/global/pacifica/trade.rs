//! 거래 도메인 (Ed25519 서명 + 게이트) — 주문 생성/취소.
//!
//! **실거래 경고 + 게이트:** 모든 호출은 실자금을 움직인다. 메시지 정규화·Ed25519
//! 코어는 공식 `python-sdk` 골든 벡터로 검증됐으나(서명 바이트 일치), 실키로 서버가
//! 주문을 수락하는 end-to-end 경로는 미검증이다. 따라서 [`Trade::place`]·[`Trade::cancel`]
//! 은 [`crate::global::pacifica::PacificaConfig::allow_unverified_signing`] 게이트(기본
//! false) 뒤에 있고, 닫혀 있으면 [`crate::global::pacifica::PacificaError::SignerUnavailable`]
//! 로 거부한다. 개발·검증은 테스트넷에서.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::global::pacifica::client::{ApiCall, PacificaClient};
use crate::global::pacifica::error::{PacificaError, Result};
use crate::global::pacifica::sign::{self, SignHeader};

/// 주문 방향. Pacifica는 `"bid"`(매수)/`"ask"`(매도)를 쓴다 — BUY/SELL 아님.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Bid,
    Ask,
}

impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Side::Bid => "bid",
            Side::Ask => "ask",
        }
    }
}

/// 체결 조건(Time-in-Force). 한정가 주문 필수.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// 취소 전까지 유효.
    Gtc,
    /// 즉시 체결·잔량 취소.
    Ioc,
    /// 전량 즉시 체결 아니면 취소.
    Fok,
    /// 메이커 전용(체결 시 취소).
    Alo,
}

impl TimeInForce {
    fn as_str(self) -> &'static str {
        match self {
            TimeInForce::Gtc => "GTC",
            TimeInForce::Ioc => "IOC",
            TimeInForce::Fok => "FOK",
            TimeInForce::Alo => "ALO",
        }
    }
}

/// 한정가 주문 요청. 수량·가격은 정밀도 보존 위해 String(심볼 tick/lot에 맞춰 포맷).
#[derive(Debug, Clone)]
pub struct LimitOrder {
    pub symbol: String,
    pub side: Side,
    /// 지정가.
    pub price: String,
    /// 수량(base).
    pub amount: String,
    pub tif: TimeInForce,
    /// 포지션 축소 전용.
    pub reduce_only: bool,
    /// 클라이언트 주문 ID(UUID 권장, 멱등 추적).
    pub client_order_id: Option<String>,
}

impl LimitOrder {
    /// 한정가 주문 생성(기본 GTC, reduce_only=false).
    pub fn new(
        symbol: impl Into<String>,
        side: Side,
        price: impl Into<String>,
        amount: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            price: price.into(),
            amount: amount.into(),
            tif: TimeInForce::Gtc,
            reduce_only: false,
            client_order_id: None,
        }
    }

    /// TIF 지정(builder).
    pub fn tif(mut self, tif: TimeInForce) -> Self {
        self.tif = tif;
        self
    }

    /// 포지션 축소 전용 표시(builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = v;
        self
    }

    /// 클라이언트 주문 ID 부여(builder).
    pub fn client_order_id(mut self, id: impl Into<String>) -> Self {
        self.client_order_id = Some(id.into());
        self
    }

    /// 서명 페이로드(SDK `signature_payload`, `create_order`). 키 정렬은 서명기가 한다.
    fn payload(&self) -> BTreeMap<String, Value> {
        let mut p: BTreeMap<String, Value> = BTreeMap::new();
        p.insert("symbol".into(), json!(self.symbol));
        p.insert("price".into(), json!(self.price));
        p.insert("amount".into(), json!(self.amount));
        p.insert("side".into(), json!(self.side.as_str()));
        p.insert("tif".into(), json!(self.tif.as_str()));
        p.insert("reduce_only".into(), json!(self.reduce_only));
        if let Some(id) = &self.client_order_id {
            p.insert("client_order_id".into(), json!(id));
        }
        p
    }
}

/// 시장가 주문 요청.
#[derive(Debug, Clone)]
pub struct MarketOrder {
    pub symbol: String,
    pub side: Side,
    /// 수량(base).
    pub amount: String,
    /// 최대 슬리피지(%) 문자열(예 "0.5").
    pub slippage_percent: String,
    pub reduce_only: bool,
    pub client_order_id: Option<String>,
}

impl MarketOrder {
    /// 시장가 주문 생성(reduce_only=false).
    pub fn new(
        symbol: impl Into<String>,
        side: Side,
        amount: impl Into<String>,
        slippage_percent: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            amount: amount.into(),
            slippage_percent: slippage_percent.into(),
            reduce_only: false,
            client_order_id: None,
        }
    }

    /// 포지션 축소 전용 표시(builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = v;
        self
    }

    /// 클라이언트 주문 ID 부여(builder).
    pub fn client_order_id(mut self, id: impl Into<String>) -> Self {
        self.client_order_id = Some(id.into());
        self
    }

    /// 서명 페이로드(SDK `signature_payload`, `create_market_order`).
    fn payload(&self) -> BTreeMap<String, Value> {
        let mut p: BTreeMap<String, Value> = BTreeMap::new();
        p.insert("symbol".into(), json!(self.symbol));
        p.insert("amount".into(), json!(self.amount));
        p.insert("side".into(), json!(self.side.as_str()));
        p.insert("slippage_percent".into(), json!(self.slippage_percent));
        p.insert("reduce_only".into(), json!(self.reduce_only));
        if let Some(id) = &self.client_order_id {
            p.insert("client_order_id".into(), json!(id));
        }
        p
    }
}

/// 거래 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a PacificaClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a PacificaClient) -> Self {
        Self { client }
    }

    /// 게이트 + 키 점검. 통과 시 [`ed25519_dalek::SigningKey`] 반환.
    fn guard(&self) -> Result<ed25519_dalek::SigningKey> {
        let cfg = self.client.config();
        if !cfg.allow_unverified_signing {
            return Err(PacificaError::SignerUnavailable(
                "set allow_unverified_signing=true after verifying e2e on testnet".into(),
            ));
        }
        if cfg.solana_secret_key.is_empty() {
            return Err(PacificaError::Auth("missing solana_secret_key".into()));
        }
        sign::parse_secret_key(&cfg.solana_secret_key).map_err(PacificaError::Auth)
    }

    /// 서명 헤더+페이로드로 (정규 메시지, base58 서명, account)를 만든다.
    fn sign_op(
        &self,
        sk: &ed25519_dalek::SigningKey,
        kind: &'static str,
        payload: &BTreeMap<String, Value>,
    ) -> (u64, String) {
        let cfg = self.client.config();
        let timestamp = PacificaClient::timestamp_ms();
        let header = SignHeader {
            kind,
            timestamp,
            expiry_window: cfg.expiry_window_ms,
        };
        let msg = sign::build_message(&header, payload);
        let sig = sign::sign_message(sk, &msg);
        (timestamp, sig)
    }

    /// 서명 본문을 조립한다 — `{account, signature, timestamp, expiry_window, ...payload}`.
    /// 서명된 구조(`data` 래퍼)와 달리 HTTP 본문은 payload를 최상위로 평탄화한다.
    fn build_body(
        &self,
        sk: &ed25519_dalek::SigningKey,
        timestamp: u64,
        signature: String,
        payload: BTreeMap<String, Value>,
    ) -> Value {
        let cfg = self.client.config();
        let mut body = serde_json::Map::new();
        body.insert("account".into(), json!(sign::account_address(sk)));
        body.insert("signature".into(), json!(signature));
        body.insert("timestamp".into(), json!(timestamp));
        body.insert("expiry_window".into(), json!(cfg.expiry_window_ms));
        for (k, v) in payload {
            body.insert(k, v);
        }
        Value::Object(body)
    }

    /// 한정가 주문 제출 (`POST /api/v1/orders/create`). **게이트.**
    pub async fn place_limit(&self, order: &LimitOrder) -> Result<Value> {
        let sk = self.guard()?;
        let payload = order.payload();
        let (ts, sig) = self.sign_op(&sk, "create_order", &payload);
        let body = self.build_body(&sk, ts, sig, payload);
        Ok(self
            .client
            .call(ApiCall::post("/api/v1/orders/create", body))
            .await?
            .data)
    }

    /// 시장가 주문 제출 (`POST /api/v1/orders/create_market`). **게이트.**
    pub async fn place_market(&self, order: &MarketOrder) -> Result<Value> {
        let sk = self.guard()?;
        let payload = order.payload();
        let (ts, sig) = self.sign_op(&sk, "create_market_order", &payload);
        let body = self.build_body(&sk, ts, sig, payload);
        Ok(self
            .client
            .call(ApiCall::post("/api/v1/orders/create_market", body))
            .await?
            .data)
    }

    /// 주문 취소 (`POST /api/v1/orders/cancel`). `order_id`는 서버 주문 ID(JSON number). **게이트.**
    pub async fn cancel(&self, symbol: &str, order_id: u64) -> Result<Value> {
        let sk = self.guard()?;
        let mut payload: BTreeMap<String, Value> = BTreeMap::new();
        payload.insert("symbol".into(), json!(symbol));
        payload.insert("order_id".into(), json!(order_id));
        let (ts, sig) = self.sign_op(&sk, "cancel_order", &payload);
        let body = self.build_body(&sk, ts, sig, payload);
        Ok(self
            .client
            .call(ApiCall::post("/api/v1/orders/cancel", body))
            .await?
            .data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_strings_are_bid_ask() {
        assert_eq!(Side::Bid.as_str(), "bid");
        assert_eq!(Side::Ask.as_str(), "ask");
    }

    #[test]
    fn limit_payload_has_expected_typed_fields() {
        let o = LimitOrder::new("SAMSUNG", Side::Bid, "234.20", "0.1")
            .reduce_only(false)
            .client_order_id("cid-1");
        let p = o.payload();
        assert_eq!(p["symbol"], json!("SAMSUNG"));
        assert_eq!(p["price"], json!("234.20")); // string
        assert_eq!(p["side"], json!("bid"));
        assert_eq!(p["tif"], json!("GTC"));
        assert_eq!(p["reduce_only"], json!(false)); // bool
        assert_eq!(p["client_order_id"], json!("cid-1"));
    }

    #[test]
    fn market_payload_has_slippage_no_price() {
        let o = MarketOrder::new("SKHYNIX", Side::Ask, "1", "0.5");
        let p = o.payload();
        assert_eq!(p["slippage_percent"], json!("0.5"));
        assert!(!p.contains_key("price"));
        assert!(!p.contains_key("tif"));
        assert_eq!(p["side"], json!("ask"));
    }

    #[test]
    fn gate_closed_rejects_signing() {
        use crate::global::pacifica::{PacificaClient, PacificaConfig};
        // 키가 있어도 게이트가 닫혀 있으면 SignerUnavailable.
        let cfg = PacificaConfig::new(bs58::encode([7u8; 32]).into_string());
        let client = PacificaClient::new(cfg).unwrap();
        let err = client.trade().guard().unwrap_err();
        assert!(matches!(err, PacificaError::SignerUnavailable(_)));
    }

    #[test]
    fn gate_open_with_key_parses() {
        use crate::global::pacifica::{PacificaClient, PacificaConfig};
        let cfg = PacificaConfig::new(bs58::encode([7u8; 32]).into_string())
            .allow_unverified_signing(true);
        let client = PacificaClient::new(cfg).unwrap();
        assert!(client.trade().guard().is_ok());
    }

    #[test]
    fn gate_open_but_no_key_rejects() {
        use crate::global::pacifica::{PacificaClient, PacificaConfig};
        let cfg = PacificaConfig::public().allow_unverified_signing(true);
        let client = PacificaClient::new(cfg).unwrap();
        assert!(matches!(
            client.trade().guard().unwrap_err(),
            PacificaError::Auth(_)
        ));
    }
}
