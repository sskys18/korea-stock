//! 거래·계좌 도메인 (HMAC 서명 필수) — 주문/취소·포지션·잔고.
//!
//! Phemex Perpetual v2 hedged 엔드포인트(`/g-orders`, `/g-accounts/*`). 가격·수량은
//! 모두 real value 문자열(`Rp`/`Rq` suffix). **실거래 경고:** 모든 호출이 실제 자금을
//! 움직인다. 개발·검증은 반드시 [`crate::global::phemex::PhemexConfig::testnet`]에서 한다.

use serde::Deserialize;
use serde_json::{json, Value};

use crate::global::phemex::client::{ApiCall, PhemexClient};
use crate::global::phemex::error::Result;

/// 주문 방향.
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

/// 포지션 방향 (hedged 모드 필수). one-way 계정은 `Long`/`Short`를 side에 맞춰 보낸다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PosSide {
    Long,
    Short,
}

impl PosSide {
    fn as_str(self) -> &'static str {
        match self {
            PosSide::Long => "Long",
            PosSide::Short => "Short",
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
            OrderType::Limit => "Limit",
            OrderType::Market => "Market",
        }
    }
}

/// 체결 조건. Phemex 표기.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// 취소 전까지 유효.
    GoodTillCancel,
    /// 즉시 체결·잔량 취소.
    ImmediateOrCancel,
    /// 전량 즉시 체결 아니면 취소.
    FillOrKill,
    /// 포스트온리(메이커 전용).
    PostOnly,
}

impl TimeInForce {
    fn as_str(self) -> &'static str {
        match self {
            TimeInForce::GoodTillCancel => "GoodTillCancel",
            TimeInForce::ImmediateOrCancel => "ImmediateOrCancel",
            TimeInForce::FillOrKill => "FillOrKill",
            TimeInForce::PostOnly => "PostOnly",
        }
    }
}

/// 주문 요청. 수량·가격은 정밀도 보존 위해 String (심볼 `tickSize`/`qtyStepSize`에
/// 맞춰 호출부가 포맷). real value(`Rp`/`Rq`)로 그대로 전송된다.
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub pos_side: PosSide,
    pub order_type: OrderType,
    /// 주문 수량 (real, `orderQtyRq`).
    pub order_qty_rq: String,
    /// 지정가 (real, `priceRp`). LIMIT 필수, MARKET이면 None.
    pub price_rp: Option<String>,
    /// 체결 조건. LIMIT 필수.
    pub time_in_force: Option<TimeInForce>,
    /// 포지션 축소 전용 여부.
    pub reduce_only: Option<bool>,
    /// 사용자 지정 주문 ID (멱등 추적, `clOrdID`).
    pub cl_ord_id: Option<String>,
}

impl OrderRequest {
    /// 시장가 주문.
    pub fn market(
        symbol: impl Into<String>,
        side: Side,
        pos_side: PosSide,
        order_qty_rq: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            pos_side,
            order_type: OrderType::Market,
            order_qty_rq: order_qty_rq.into(),
            price_rp: None,
            time_in_force: None,
            reduce_only: None,
            cl_ord_id: None,
        }
    }

    /// 지정가 주문 (기본 GoodTillCancel).
    pub fn limit(
        symbol: impl Into<String>,
        side: Side,
        pos_side: PosSide,
        order_qty_rq: impl Into<String>,
        price_rp: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            pos_side,
            order_type: OrderType::Limit,
            order_qty_rq: order_qty_rq.into(),
            price_rp: Some(price_rp.into()),
            time_in_force: Some(TimeInForce::GoodTillCancel),
            reduce_only: None,
            cl_ord_id: None,
        }
    }

    /// 포지션 축소 전용으로 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = Some(v);
        self
    }

    /// 사용자 주문 ID 부여 (builder).
    pub fn cl_ord_id(mut self, id: impl Into<String>) -> Self {
        self.cl_ord_id = Some(id.into());
        self
    }

    /// `POST /g-orders` JSON 본문으로 직렬화. 키 순서는 서명·전송 모두 이 값을 보므로
    /// serde_json object(BTreeMap 정렬) 직렬화로 결정적이다.
    pub(crate) fn to_body(&self) -> Value {
        let mut body = json!({
            "symbol": self.symbol,
            "side": self.side.as_str(),
            "posSide": self.pos_side.as_str(),
            "ordType": self.order_type.as_str(),
            "orderQtyRq": self.order_qty_rq,
        });
        let obj = body.as_object_mut().unwrap();
        if let Some(p) = &self.price_rp {
            obj.insert("priceRp".into(), json!(p));
        }
        if let Some(tif) = self.time_in_force {
            obj.insert("timeInForce".into(), json!(tif.as_str()));
        }
        if let Some(ro) = self.reduce_only {
            obj.insert("reduceOnly".into(), json!(ro));
        }
        if let Some(id) = &self.cl_ord_id {
            obj.insert("clOrdID".into(), json!(id));
        }
        body
    }
}

/// 주문 응답 / 조회 결과 (`data`). real value 문자열 필드.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderResponse {
    #[serde(rename = "orderID", default)]
    pub order_id: String,
    #[serde(rename = "clOrdID", default)]
    pub cl_ord_id: String,
    #[serde(default)]
    pub symbol: String,
    /// Init / New / PartiallyFilled / Filled / Canceled / Rejected.
    #[serde(rename = "ordStatus", default)]
    pub ord_status: String,
    #[serde(rename = "orderType", default)]
    pub order_type: String,
    #[serde(default)]
    pub side: String,
    /// 주문 가격 (real).
    #[serde(rename = "priceRp", default)]
    pub price_rp: String,
    /// 주문 수량 (real).
    #[serde(rename = "orderQtyRq", default)]
    pub order_qty_rq: String,
    /// 누적 체결 수량 (real).
    #[serde(rename = "cumQtyRq", default)]
    pub cum_qty_rq: String,
    /// 잔여 수량 (real).
    #[serde(rename = "leavesQtyRq", default)]
    pub leaves_qty_rq: String,
    /// 비즈니스 에러 코드(0이면 정상).
    #[serde(rename = "bizError", default)]
    pub biz_error: i64,
    #[serde(rename = "actionTimeNs", default)]
    pub action_time_ns: i64,
}

/// 포지션 1건 (`GET /g-accounts/accountPositions` → data.positions[]).
#[derive(Debug, Clone, Deserialize)]
pub struct Position {
    #[serde(default)]
    pub symbol: String,
    /// 포지션 방향 ("Long"/"Short"; hedged 모드).
    #[serde(rename = "posSide", default)]
    pub pos_side: String,
    /// 포지션 수량 (real). "0"이면 무포지션.
    #[serde(default)]
    pub size: String,
    /// 평균 진입가 (real).
    #[serde(rename = "avgEntryPriceRp", default)]
    pub avg_entry_price_rp: String,
    /// 마크가 (real).
    #[serde(rename = "markPriceRp", default)]
    pub mark_price_rp: String,
    /// 청산가 (real).
    #[serde(rename = "liquidationPriceRp", default)]
    pub liquidation_price_rp: String,
    /// 레버리지 (real ratio; 0이면 cross).
    #[serde(rename = "leverageRr", default)]
    pub leverage_rr: String,
    /// 포지션 마진 (real value).
    #[serde(rename = "positionMarginRv", default)]
    pub position_margin_rv: String,
}

/// 계좌 잔고 (`GET /g-accounts/accountPositions` → data.account).
#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    #[serde(default)]
    pub currency: String,
    /// 계좌 잔고 (real value).
    #[serde(rename = "accountBalanceRv", default)]
    pub account_balance_rv: String,
    /// 사용중 잔고 (real value).
    #[serde(rename = "totalUsedBalanceRv", default)]
    pub total_used_balance_rv: String,
    #[serde(rename = "accountId", default)]
    pub account_id: i64,
}

/// 계좌+포지션 묶음 (`data`).
#[derive(Debug, Clone, Deserialize)]
pub struct AccountPositions {
    pub account: Account,
    #[serde(default)]
    pub positions: Vec<Position>,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a PhemexClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a PhemexClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /g-orders`). **실체결.**
    pub async fn place(&self, req: &OrderRequest) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed_post("/g-orders", req.to_body()))
            .await?
            .parse()
    }

    /// 주문 취소 (`DELETE /g-orders/cancel`). hedged 모드는 `posSide` 필요.
    pub async fn cancel(
        &self,
        symbol: &str,
        order_id: &str,
        pos_side: PosSide,
    ) -> Result<OrderResponse> {
        self.client
            .call(ApiCall::signed_delete(
                "/g-orders/cancel",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("orderID".into(), order_id.into()),
                    ("posSide".into(), pos_side.as_str().into()),
                ],
            ))
            .await?
            .parse()
    }

    /// 심볼 전체 미체결 주문 취소 (`DELETE /g-orders/all`).
    pub async fn cancel_all(&self, symbol: &str) -> Result<Value> {
        self.client
            .call(ApiCall::signed_delete(
                "/g-orders/all",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 미체결 주문 목록 (`GET /g-orders/activeList`).
    pub async fn open_orders(&self, symbol: &str) -> Result<Value> {
        self.client
            .call(ApiCall::signed_get(
                "/g-orders/activeList",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 계좌·포지션 조회 (`GET /g-accounts/accountPositions`). `currency`는 보통 "USDT".
    /// `symbol=None`이면 통화 전체.
    pub async fn account_positions(
        &self,
        currency: &str,
        symbol: Option<&str>,
    ) -> Result<AccountPositions> {
        let mut q = vec![("currency".to_string(), currency.to_string())];
        if let Some(s) = symbol {
            q.push(("symbol".into(), s.into()));
        }
        self.client
            .call(ApiCall::signed_get("/g-accounts/accountPositions", q))
            .await?
            .parse()
    }

    /// 포지션만 추려 반환(편의). 내부적으로 [`Self::account_positions`] 호출.
    pub async fn positions(&self, currency: &str, symbol: Option<&str>) -> Result<Vec<Position>> {
        Ok(self.account_positions(currency, symbol).await?.positions)
    }

    /// 잔고(account)만 추려 반환(편의).
    pub async fn balance(&self, currency: &str) -> Result<Account> {
        Ok(self.account_positions(currency, None).await?.account)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_order_body() {
        let req = OrderRequest::market("SAMSUNGUSDT", Side::Buy, PosSide::Long, "1");
        let b = req.to_body();
        assert_eq!(b["ordType"], "Market");
        assert_eq!(b["side"], "Buy");
        assert_eq!(b["posSide"], "Long");
        assert_eq!(b["orderQtyRq"], "1");
        // MARKET은 priceRp/timeInForce 없음.
        assert!(b.get("priceRp").is_none());
        assert!(b.get("timeInForce").is_none());
    }

    #[test]
    fn limit_order_body_default_gtc() {
        let req = OrderRequest::limit("SKHYNIXUSDT", Side::Sell, PosSide::Short, "0.5", "1514.5")
            .reduce_only(true)
            .cl_ord_id("kr-001");
        let b = req.to_body();
        assert_eq!(b["ordType"], "Limit");
        assert_eq!(b["priceRp"], "1514.5");
        assert_eq!(b["timeInForce"], "GoodTillCancel");
        assert_eq!(b["reduceOnly"], true);
        assert_eq!(b["clOrdID"], "kr-001");
        assert_eq!(b["posSide"], "Short");
    }

    #[test]
    fn order_body_serializes_to_deterministic_json() {
        // serde_json object는 키 정렬 직렬화 → 서명 대상/전송 바이트가 결정적.
        let req = OrderRequest::limit("SAMSUNGUSDT", Side::Buy, PosSide::Long, "1", "235.0");
        let s = serde_json::to_string(&req.to_body()).unwrap();
        assert_eq!(
            s,
            r#"{"ordType":"Limit","orderQtyRq":"1","posSide":"Long","priceRp":"235.0","side":"Buy","symbol":"SAMSUNGUSDT","timeInForce":"GoodTillCancel"}"#
        );
    }

    #[test]
    fn order_response_parses() {
        let v = serde_json::json!({
            "orderID": "ab90a08c-b728-4b6b-97c4-36fa497335bf",
            "clOrdID": "kr-001",
            "symbol": "SAMSUNGUSDT",
            "ordStatus": "New",
            "orderType": "Limit",
            "side": "Buy",
            "priceRp": "235.0",
            "orderQtyRq": "1",
            "cumQtyRq": "0",
            "leavesQtyRq": "1",
            "bizError": 0,
            "actionTimeNs": 1580547265848034600i64
        });
        let o: OrderResponse = serde_json::from_value(v).unwrap();
        assert_eq!(o.order_id, "ab90a08c-b728-4b6b-97c4-36fa497335bf");
        assert_eq!(o.ord_status, "New");
        assert_eq!(o.order_type, "Limit");
        assert_eq!(o.biz_error, 0);
    }

    #[test]
    fn account_positions_parses() {
        let v = serde_json::json!({
            "account": {
                "currency": "USDT",
                "accountBalanceRv": "1000.5",
                "totalUsedBalanceRv": "120.0",
                "accountId": 123450001i64
            },
            "positions": [{
                "symbol": "HYUNDAIUSDT",
                "posSide": "Long",
                "size": "2",
                "avgEntryPriceRp": "190.5",
                "markPriceRp": "188.0",
                "liquidationPriceRp": "150.0",
                "leverageRr": "10",
                "positionMarginRv": "38.0"
            }]
        });
        let ap: AccountPositions = serde_json::from_value(v).unwrap();
        assert_eq!(ap.account.currency, "USDT");
        assert_eq!(ap.account.account_balance_rv, "1000.5");
        assert_eq!(ap.positions[0].symbol, "HYUNDAIUSDT");
        assert_eq!(ap.positions[0].pos_side, "Long");
        assert_eq!(ap.positions[0].size, "2");
    }
}
