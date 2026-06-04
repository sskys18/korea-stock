//! 거래·계좌 도메인 (HMAC + passphrase 서명 필수) — 주문/취소/조회·포지션·잔고.
//!
//! Bitget v2 mix `productType=USDT-FUTURES`(USDT 무기한). 모든 변경 호출은 **POST + JSON
//! 본문**, 조회는 GET이다. 서명은 [`crate::global::bitget::client`]가 `ACCESS-*` 헤더로
//! 부착한다. 주문은 **단방향(one-way) 모드** 기준으로 모델링한다(`tradeSide` 미사용 —
//! hedge 모드는 미지원). 포지션 축소는 `reduceOnly="YES"`로 표현한다.
//!
//! **실거래 경고:** 이 모듈의 모든 변경 호출은 실제 자금을 움직인다. 본 어댑터의 거래
//! 코드는 키가 없어 **실주문으로 검증되지 않았다** — 운영 전 소액으로 직접 검증하라.

use serde::Deserialize;

use crate::global::bitget::client::{ApiCall, BitgetClient};
use crate::global::bitget::error::{BitgetError, Result};

/// 주문 방향. Bitget v2 mix는 소문자 `buy`/`sell`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    fn as_str(self) -> &'static str {
        match self {
            Side::Buy => "buy",
            Side::Sell => "sell",
        }
    }
}

/// 주문 유형. (지정가·시장가만 우선 지원.) Bitget은 소문자 `limit`/`market`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

impl OrderType {
    fn as_str(self) -> &'static str {
        match self {
            OrderType::Limit => "limit",
            OrderType::Market => "market",
        }
    }
}

/// 체결 조건(`force`). Bitget은 소문자 `gtc`/`ioc`/`fok`/`post_only`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Force {
    /// 취소 전까지 유효.
    Gtc,
    /// 즉시 체결·잔량 취소.
    Ioc,
    /// 전량 즉시 체결 아니면 취소.
    Fok,
    /// 메이커 전용(테이커면 거부).
    PostOnly,
}

impl Force {
    fn as_str(self) -> &'static str {
        match self {
            Force::Gtc => "gtc",
            Force::Ioc => "ioc",
            Force::Fok => "fok",
            Force::PostOnly => "post_only",
        }
    }
}

/// 마진 모드. Bitget은 `isolated`(격리)/`crossed`(교차).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarginMode {
    Isolated,
    Crossed,
}

impl MarginMode {
    fn as_str(self) -> &'static str {
        match self {
            MarginMode::Isolated => "isolated",
            MarginMode::Crossed => "crossed",
        }
    }
}

/// 주문 요청. 수량·가격은 정밀도 보존 위해 String (심볼 `pricePlace`/`volumePlace`에
/// 맞춰 호출부가 포맷).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    /// 마진 모드. 기본 격리(isolated).
    pub margin_mode: MarginMode,
    /// 주문 수량 (base coin 수량).
    pub size: String,
    /// 지정가. Limit 필수, Market이면 None.
    pub price: Option<String>,
    /// 체결 조건. Limit 기본 GTC, Market이면 보통 None(테이커).
    pub force: Option<Force>,
    /// 포지션 축소 전용 여부(단방향 모드). true면 `reduceOnly="YES"`.
    pub reduce_only: Option<bool>,
    /// 사용자 지정 주문 ID (멱등 추적).
    pub client_oid: Option<String>,
}

impl OrderRequest {
    /// 시장가 주문 (격리 마진 기본).
    pub fn market(symbol: impl Into<String>, side: Side, size: impl Into<String>) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            order_type: OrderType::Market,
            margin_mode: MarginMode::Isolated,
            size: size.into(),
            price: None,
            force: None,
            reduce_only: None,
            client_oid: None,
        }
    }

    /// 지정가 주문 (격리 마진·기본 GTC).
    pub fn limit(
        symbol: impl Into<String>,
        side: Side,
        size: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            order_type: OrderType::Limit,
            margin_mode: MarginMode::Isolated,
            size: size.into(),
            price: Some(price.into()),
            force: Some(Force::Gtc),
            reduce_only: None,
            client_oid: None,
        }
    }

    /// 마진 모드 지정 (builder).
    pub fn margin_mode(mut self, m: MarginMode) -> Self {
        self.margin_mode = m;
        self
    }

    /// 체결 조건 지정 (builder).
    pub fn force(mut self, f: Force) -> Self {
        self.force = Some(f);
        self
    }

    /// 포지션 축소 전용으로 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = Some(v);
        self
    }

    /// 사용자 주문 ID 부여 (builder).
    pub fn client_oid(mut self, id: impl Into<String>) -> Self {
        self.client_oid = Some(id.into());
        self
    }

    fn to_params(&self) -> Vec<(String, String)> {
        let mut p = vec![
            ("symbol".to_string(), self.symbol.clone()),
            ("productType".to_string(), "USDT-FUTURES".to_string()),
            ("marginMode".to_string(), self.margin_mode.as_str().to_string()),
            ("marginCoin".to_string(), "USDT".to_string()),
            ("size".to_string(), self.size.clone()),
            ("side".to_string(), self.side.as_str().to_string()),
            ("orderType".to_string(), self.order_type.as_str().to_string()),
        ];
        if let Some(price) = &self.price {
            p.push(("price".into(), price.clone()));
        }
        if let Some(force) = self.force {
            p.push(("force".into(), force.as_str().to_string()));
        }
        if let Some(ro) = self.reduce_only {
            // Bitget 단방향 모드: reduceOnly는 "YES"/"NO" 문자열.
            p.push(("reduceOnly".into(), if ro { "YES" } else { "NO" }.to_string()));
        }
        if let Some(id) = &self.client_oid {
            p.push(("clientOid".into(), id.clone()));
        }
        p
    }
}

/// 주문 생성/취소 ACK (`data`: `{orderId, clientOid}`).
#[derive(Debug, Clone, Deserialize)]
pub struct OrderAck {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "clientOid", default)]
    pub client_oid: String,
}

/// 미체결 주문 1건 (`GET /api/v2/mix/order/orders-pending` → data.entrustedList[]).
#[derive(Debug, Clone, Deserialize)]
pub struct OrderDetail {
    #[serde(rename = "orderId")]
    pub order_id: String,
    #[serde(rename = "clientOid", default)]
    pub client_oid: String,
    pub symbol: String,
    #[serde(default)]
    pub side: String,
    #[serde(rename = "orderType", default)]
    pub order_type: String,
    /// live / partially_filled / filled / cancelled 등.
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub price: String,
    #[serde(default)]
    pub size: String,
    /// 체결 수량.
    #[serde(rename = "baseVolume", default)]
    pub base_volume: String,
    #[serde(default)]
    pub force: String,
    #[serde(rename = "reduceOnly", default)]
    pub reduce_only: String,
}

/// 미체결 주문 목록 응답 래퍼 (`data.entrustedList`).
#[derive(Debug, Clone, Deserialize)]
struct PendingWrap {
    #[serde(rename = "entrustedList", default)]
    entrusted_list: Option<Vec<OrderDetail>>,
}

/// 포지션 1건 (`GET /api/v2/mix/position/all-position` → data[]).
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    pub symbol: String,
    /// "long"/"short"; 무포지션 행은 보통 반환되지 않는다.
    #[serde(rename = "holdSide", default)]
    pub hold_side: String,
    /// 보유 수량 (base coin).
    #[serde(default)]
    pub total: String,
    /// 사용 가능(미잠금) 수량.
    #[serde(default)]
    pub available: String,
    #[serde(rename = "openPriceAvg", default)]
    pub open_price_avg: String,
    #[serde(rename = "marginCoin", default)]
    pub margin_coin: String,
    #[serde(rename = "markPrice", default)]
    pub mark_price: String,
    #[serde(rename = "liquidationPrice", default)]
    pub liquidation_price: String,
    #[serde(default)]
    pub leverage: String,
    /// 미실현손익 (USDT).
    #[serde(rename = "unrealizedPL", default)]
    pub unrealized_pl: String,
}

/// 계정 단건 잔고 (`GET /api/v2/mix/account/account` → data, 단일 객체).
#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    #[serde(rename = "marginCoin")]
    pub margin_coin: String,
    /// 사용 가능 잔고.
    #[serde(default)]
    pub available: String,
    /// 계정 자산(equity).
    #[serde(rename = "accountEquity", default)]
    pub account_equity: String,
    /// USDT 환산 자산.
    #[serde(rename = "usdtEquity", default)]
    pub usdt_equity: String,
    /// 미실현손익.
    #[serde(rename = "unrealizedPL", default)]
    pub unrealized_pl: String,
    /// 잠긴(주문/포지션 마진) 금액.
    #[serde(default)]
    pub locked: String,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a BitgetClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a BitgetClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /api/v2/mix/order/place-order`). **실체결.**
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderAck> {
        self.client
            .call(ApiCall::signed_post(
                "/api/v2/mix/order/place-order",
                req.to_params(),
            ))
            .await?
            .parse()
    }

    /// 주문 취소 (`POST /api/v2/mix/order/cancel-order`). `order_id` 또는 `client_oid` 중 하나.
    pub async fn cancel(&self, symbol: &str, order_id: &str) -> Result<OrderAck> {
        self.client
            .call(ApiCall::signed_post(
                "/api/v2/mix/order/cancel-order",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("productType".into(), "USDT-FUTURES".into()),
                    ("orderId".into(), order_id.into()),
                ],
            ))
            .await?
            .parse()
    }

    /// 미체결 주문 목록 (`GET /api/v2/mix/order/orders-pending`).
    /// `symbol=None`이면 productType 전체.
    pub async fn open_orders(&self, symbol: Option<&str>) -> Result<Vec<OrderDetail>> {
        let mut params = vec![("productType".to_string(), "USDT-FUTURES".to_string())];
        if let Some(s) = symbol {
            params.push(("symbol".into(), s.to_string()));
        }
        let wrap: PendingWrap = self
            .client
            .call(ApiCall::signed_get("/api/v2/mix/order/orders-pending", params))
            .await?
            .parse()?;
        Ok(wrap.entrusted_list.unwrap_or_default())
    }

    /// 전체 포지션 조회 (`GET /api/v2/mix/position/all-position`).
    pub async fn positions(&self) -> Result<Vec<Position>> {
        self.client
            .call(ApiCall::signed_get(
                "/api/v2/mix/position/all-position",
                vec![
                    ("productType".into(), "USDT-FUTURES".into()),
                    ("marginCoin".into(), "USDT".into()),
                ],
            ))
            .await?
            .parse()
    }

    /// 단일 심볼 계정 잔고 (`GET /api/v2/mix/account/account`).
    pub async fn account(&self, symbol: &str) -> Result<Account> {
        self.client
            .call(ApiCall::signed_get(
                "/api/v2/mix/account/account",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("productType".into(), "USDT-FUTURES".into()),
                    ("marginCoin".into(), "USDT".into()),
                ],
            ))
            .await?
            .parse()
            .map_err(|_| BitgetError::Decode("account: empty/unexpected data".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_order_params() {
        let req = OrderRequest::market("SAMSUNGUSDT", Side::Buy, "0.1");
        let p = req.to_params();
        assert!(p.contains(&("productType".to_string(), "USDT-FUTURES".to_string())));
        assert!(p.contains(&("orderType".to_string(), "market".to_string())));
        assert!(p.contains(&("side".to_string(), "buy".to_string())));
        assert!(p.contains(&("size".to_string(), "0.1".to_string())));
        assert!(p.contains(&("marginMode".to_string(), "isolated".to_string())));
        assert!(p.contains(&("marginCoin".to_string(), "USDT".to_string())));
        // Market은 price/force 없음.
        assert!(!p.iter().any(|(k, _)| k == "price"));
        assert!(!p.iter().any(|(k, _)| k == "force"));
    }

    #[test]
    fn limit_order_params_default_gtc() {
        let req = OrderRequest::limit("SKHYNIXUSDT", Side::Sell, "0.2", "120.50")
            .reduce_only(true)
            .client_oid("kr-001");
        let p = req.to_params();
        assert!(p.contains(&("orderType".to_string(), "limit".to_string())));
        assert!(p.contains(&("price".to_string(), "120.50".to_string())));
        assert!(p.contains(&("force".to_string(), "gtc".to_string())));
        assert!(p.contains(&("side".to_string(), "sell".to_string())));
        // reduceOnly는 YES/NO 문자열.
        assert!(p.contains(&("reduceOnly".to_string(), "YES".to_string())));
        assert!(p.contains(&("clientOid".to_string(), "kr-001".to_string())));
    }

    #[test]
    fn reduce_only_false_serializes_no() {
        let req = OrderRequest::market("HYUNDAIUSDT", Side::Buy, "0.1").reduce_only(false);
        let p = req.to_params();
        assert!(p.contains(&("reduceOnly".to_string(), "NO".to_string())));
    }

    #[test]
    fn crossed_margin_and_post_only_builder() {
        let req = OrderRequest::limit("SAMSUNGUSDT", Side::Buy, "0.1", "234.0")
            .margin_mode(MarginMode::Crossed)
            .force(Force::PostOnly);
        let p = req.to_params();
        assert!(p.contains(&("marginMode".to_string(), "crossed".to_string())));
        assert!(p.contains(&("force".to_string(), "post_only".to_string())));
    }

    #[test]
    fn order_ack_parses() {
        let v = serde_json::json!({ "orderId": "1736012345678", "clientOid": "kr-001" });
        let a: OrderAck = serde_json::from_value(v).unwrap();
        assert_eq!(a.order_id, "1736012345678");
        assert_eq!(a.client_oid, "kr-001");
    }

    #[test]
    fn pending_wrap_parses_entrusted_list() {
        let v = serde_json::json!({
            "entrustedList": [{
                "orderId": "1", "clientOid": "c1", "symbol": "SAMSUNGUSDT",
                "side": "buy", "orderType": "limit", "status": "live",
                "price": "234.00", "size": "0.1", "baseVolume": "0",
                "force": "gtc", "reduceOnly": "NO"
            }]
        });
        let w: PendingWrap = serde_json::from_value(v).unwrap();
        let list = w.entrusted_list.unwrap();
        assert_eq!(list[0].order_id, "1");
        assert_eq!(list[0].status, "live");
        assert_eq!(list[0].reduce_only, "NO");
    }

    #[test]
    fn pending_wrap_null_list_is_empty() {
        let v = serde_json::json!({ "entrustedList": null });
        let w: PendingWrap = serde_json::from_value(v).unwrap();
        assert!(w.entrusted_list.unwrap_or_default().is_empty());
    }

    #[test]
    fn position_parses_unrealized_field() {
        let v = serde_json::json!({
            "symbol": "HYUNDAIUSDT",
            "holdSide": "short",
            "total": "0.2",
            "available": "0.2",
            "openPriceAvg": "190.50",
            "marginCoin": "USDT",
            "markPrice": "188.00",
            "liquidationPrice": "250.00",
            "leverage": "10",
            "unrealizedPL": "5.00"
        });
        let pos: Position = serde_json::from_value(v).unwrap();
        assert_eq!(pos.hold_side, "short");
        assert_eq!(pos.total, "0.2");
        assert_eq!(pos.unrealized_pl, "5.00");
        assert_eq!(pos.leverage, "10");
    }

    #[test]
    fn account_parses() {
        let v = serde_json::json!({
            "marginCoin": "USDT",
            "available": "850.00",
            "accountEquity": "1000.00",
            "usdtEquity": "1000.00",
            "unrealizedPL": "5.00",
            "locked": "150.00"
        });
        let a: Account = serde_json::from_value(v).unwrap();
        assert_eq!(a.margin_coin, "USDT");
        assert_eq!(a.available, "850.00");
        assert_eq!(a.usdt_equity, "1000.00");
    }
}
