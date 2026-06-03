//! 거래·계좌 도메인 (HMAC 서명 필수) — 주문/취소·포지션·자산.
//!
//! **실거래 경고:** 이 모듈의 변경 호출은 실제 자금을 움직인다. MEXC는 별도 테스트넷이
//! 없으므로 최소 수량으로 신중히 검증한다.
//!
//! **MEXC 거래 제약(중요):** MEXC는 2022-07-25부로 대부분 계정에서 Contract **신규 주문
//! 제출(`/private/order/submit`)·취소(`/private/order/cancel`)를 점검(중단)** 상태로 두었다.
//! 본 모듈은 서명을 정상 구현하고 [`Trade::place`]/[`Trade::cancel`]도 **실제 서명 호출**로
//! 구현한다. 서버가 막혀 있으면 MEXC의 점검 에러([`crate::mexc::MexcError::Api`])가 그대로
//! 반환된다(허구 스텁이 아님 — Rule 3 준수). 조회(포지션·자산·주문)는 정상 동작한다.

use serde::Deserialize;
use serde_json::json;

use crate::mexc::client::{ApiCall, MexcClient};
use crate::mexc::error::Result;
use crate::mexc::market::num_or_str;

/// 주문 방향(매매). MEXC Contract는 `side` 정수로 인코딩한다:
/// 1=open long, 2=close short, 3=open short, 4=close long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    OpenLong,
    CloseShort,
    OpenShort,
    CloseLong,
}

impl Side {
    fn code(self) -> i64 {
        match self {
            Side::OpenLong => 1,
            Side::CloseShort => 2,
            Side::OpenShort => 3,
            Side::CloseLong => 4,
        }
    }
}

/// 주문 유형. MEXC Contract `type`: 1=limit, 5=market(시장가), 등.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

impl OrderType {
    fn code(self) -> i64 {
        match self {
            OrderType::Limit => 1,
            OrderType::Market => 5,
        }
    }
}

/// 마진 방식. `openType`: 1=isolated(격리), 2=cross(교차).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenType {
    Isolated,
    Cross,
}

impl OpenType {
    fn code(self) -> i64 {
        match self {
            OpenType::Isolated => 1,
            OpenType::Cross => 2,
        }
    }
}

/// 주문 요청 (`POST /api/v1/private/order/submit`).
///
/// 수량(`vol`)은 **계약 수**, 가격은 정밀도 보존 위해 String. MEXC는 body의 `vol`/`price`를
/// 숫자로 받지만, 서명 대상 JSON 본문과 전송 본문이 동일해야 하므로 호출부는 String으로
/// 포맷해 넘기고 본 모듈이 JSON 숫자로 직렬화한다(아래 [`OrderRequest::to_body`]).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    pub symbol: String,
    pub side: Side,
    pub order_type: OrderType,
    pub open_type: OpenType,
    /// 주문 수량 (계약 수).
    pub vol: String,
    /// 지정가. LIMIT 필수, MARKET이면 None.
    pub price: Option<String>,
    /// 레버리지. 격리마진(isolated) 신규 진입 시 필수.
    pub leverage: Option<u32>,
    /// 사용자 지정 주문 ID (멱등 추적). `externalOid`로 전송.
    pub external_oid: Option<String>,
    /// 포지션 축소 전용 여부.
    pub reduce_only: Option<bool>,
}

impl OrderRequest {
    /// 시장가 주문.
    pub fn market(
        symbol: impl Into<String>,
        side: Side,
        open_type: OpenType,
        vol: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            order_type: OrderType::Market,
            open_type,
            vol: vol.into(),
            price: None,
            leverage: None,
            external_oid: None,
            reduce_only: None,
        }
    }

    /// 지정가 주문.
    pub fn limit(
        symbol: impl Into<String>,
        side: Side,
        open_type: OpenType,
        vol: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            side,
            order_type: OrderType::Limit,
            open_type,
            vol: vol.into(),
            price: Some(price.into()),
            leverage: None,
            external_oid: None,
            reduce_only: None,
        }
    }

    /// 레버리지 부여 (builder). 격리마진 신규 진입 시 필요.
    pub fn leverage(mut self, lev: u32) -> Self {
        self.leverage = Some(lev);
        self
    }

    /// 사용자 주문 ID 부여 (builder).
    pub fn external_oid(mut self, id: impl Into<String>) -> Self {
        self.external_oid = Some(id.into());
        self
    }

    /// 포지션 축소 전용 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = Some(v);
        self
    }

    /// 서명·전송용 JSON 본문 생성. `vol`/`price` 문자열은 JSON 숫자로 파싱하되,
    /// 파싱 실패 시(빈 문자열 등) 문자열 그대로 둔다(서버가 거부하면 점검/검증 에러로 노출).
    pub(crate) fn to_body(&self) -> serde_json::Value {
        let mut m = serde_json::Map::new();
        m.insert("symbol".into(), json!(self.symbol));
        m.insert("side".into(), json!(self.side.code()));
        m.insert("type".into(), json!(self.order_type.code()));
        m.insert("openType".into(), json!(self.open_type.code()));
        m.insert("vol".into(), str_to_json_number(&self.vol));
        if let Some(p) = &self.price {
            m.insert("price".into(), str_to_json_number(p));
        }
        if let Some(l) = self.leverage {
            m.insert("leverage".into(), json!(l));
        }
        if let Some(oid) = &self.external_oid {
            m.insert("externalOid".into(), json!(oid));
        }
        if let Some(ro) = self.reduce_only {
            m.insert("reduceOnly".into(), json!(ro));
        }
        serde_json::Value::Object(m)
    }
}

/// 수량/가격 문자열을 JSON 숫자로. 정수면 i64, 아니면 f64, 실패 시 문자열 유지.
fn str_to_json_number(s: &str) -> serde_json::Value {
    if let Ok(i) = s.parse::<i64>() {
        return json!(i);
    }
    if let Ok(f) = s.parse::<f64>() {
        if let Some(n) = serde_json::Number::from_f64(f) {
            return serde_json::Value::Number(n);
        }
    }
    json!(s)
}

/// 포지션 1건 (`GET /api/v1/private/position/open_positions` → data[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub position_id: i64,
    pub symbol: String,
    /// 보유 수량 (계약 수).
    #[serde(deserialize_with = "num_or_str")]
    pub hold_vol: String,
    /// 1=long, 2=short.
    pub position_type: i64,
    /// 1=isolated, 2=cross.
    pub open_type: i64,
    /// 평균 보유 단가.
    #[serde(deserialize_with = "num_or_str")]
    pub hold_avg_price: String,
    /// 평균 진입 단가.
    #[serde(default, deserialize_with = "num_or_str")]
    pub open_avg_price: String,
    /// 청산가.
    #[serde(default, deserialize_with = "num_or_str")]
    pub liquidate_price: String,
    /// 실현손익.
    #[serde(default, deserialize_with = "num_or_str")]
    pub realised: String,
    pub leverage: i64,
}

/// 자산(통화별 잔고) 1건 (`GET /api/v1/private/account/assets` → data[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub currency: String,
    /// 포지션 마진.
    #[serde(deserialize_with = "num_or_str")]
    pub position_margin: String,
    /// 주문 가능 잔고.
    #[serde(deserialize_with = "num_or_str")]
    pub available_balance: String,
    /// 현금 잔고.
    #[serde(deserialize_with = "num_or_str")]
    pub cash_balance: String,
    /// 동결 잔고.
    #[serde(default, deserialize_with = "num_or_str")]
    pub frozen_balance: String,
    /// 자산 평가액(equity).
    #[serde(default, deserialize_with = "num_or_str")]
    pub equity: String,
    /// 미실현손익.
    #[serde(default, deserialize_with = "num_or_str")]
    pub unrealized: String,
}

/// 미체결 주문 1건 (`GET /api/v1/private/order/list/open_orders/{symbol}` → data[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenOrder {
    /// 주문 ID. MEXC는 큰 정수 또는 문자열 — 안전하게 String 보존.
    #[serde(deserialize_with = "num_or_str")]
    pub order_id: String,
    pub symbol: String,
    #[serde(default, deserialize_with = "num_or_str")]
    pub price: String,
    #[serde(default, deserialize_with = "num_or_str")]
    pub vol: String,
    /// 주문 상태 코드.
    #[serde(default)]
    pub state: i64,
    /// 방향 코드(1=open long 등).
    #[serde(default)]
    pub side: i64,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a MexcClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a MexcClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /api/v1/private/order/submit`). **실체결.**
    ///
    /// **MEXC 점검 주의:** 대부분 계정에서 이 엔드포인트는 2022-07-25부로 막혀 있어
    /// MEXC 점검 에러([`crate::mexc::MexcError::Api`])가 반환될 수 있다. 성공 시 data는
    /// 주문 ID(정수/문자열)이므로 raw `Value`로 반환한다.
    pub async fn place(&self, req: &OrderRequest) -> Result<serde_json::Value> {
        Ok(self
            .client
            .call(ApiCall::signed_post(
                "/api/v1/private/order/submit",
                req.to_body(),
            ))
            .await?
            .data)
    }

    /// 주문 취소 (`POST /api/v1/private/order/cancel`). body는 취소할 orderId 배열.
    ///
    /// **MEXC 점검 주의:** [`Trade::place`]와 동일.
    pub async fn cancel(&self, order_ids: &[&str]) -> Result<serde_json::Value> {
        Ok(self
            .client
            .call(ApiCall::signed_post(
                "/api/v1/private/order/cancel",
                json!(order_ids),
            ))
            .await?
            .data)
    }

    /// 현재 포지션 조회 (`GET /api/v1/private/position/open_positions`).
    /// `symbol=None`이면 전체.
    pub async fn positions(&self, symbol: Option<&str>) -> Result<Vec<Position>> {
        let params = match symbol {
            Some(s) => json!({ "symbol": s }),
            None => serde_json::Value::Object(Default::default()),
        };
        self.client
            .call(ApiCall::signed_get(
                "/api/v1/private/position/open_positions",
                params,
            ))
            .await?
            .parse()
    }

    /// 전체 자산 조회 (`GET /api/v1/private/account/assets`).
    pub async fn assets(&self) -> Result<Vec<Asset>> {
        self.client
            .call(ApiCall::signed_get(
                "/api/v1/private/account/assets",
                serde_json::Value::Object(Default::default()),
            ))
            .await?
            .parse()
    }

    /// 단일 통화 자산 조회 (`GET /api/v1/private/account/asset/{currency}`).
    pub async fn asset(&self, currency: &str) -> Result<Asset> {
        self.client
            .call(ApiCall::signed_get(
                format!("/api/v1/private/account/asset/{currency}"),
                serde_json::Value::Object(Default::default()),
            ))
            .await?
            .parse()
    }

    /// 심볼별 미체결 주문 조회 (`GET /api/v1/private/order/list/open_orders/{symbol}`).
    pub async fn open_orders(&self, symbol: &str) -> Result<Vec<OpenOrder>> {
        self.client
            .call(ApiCall::signed_get(
                format!("/api/v1/private/order/list/open_orders/{symbol}"),
                serde_json::Value::Object(Default::default()),
            ))
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_and_type_codes() {
        assert_eq!(Side::OpenLong.code(), 1);
        assert_eq!(Side::OpenShort.code(), 3);
        assert_eq!(OrderType::Limit.code(), 1);
        assert_eq!(OrderType::Market.code(), 5);
        assert_eq!(OpenType::Isolated.code(), 1);
        assert_eq!(OpenType::Cross.code(), 2);
    }

    #[test]
    fn limit_order_body_shape() {
        let req = OrderRequest::limit(
            "SAMSUNG_USDT",
            Side::OpenLong,
            OpenType::Isolated,
            "3",
            "65.00",
        )
        .leverage(20)
        .external_oid("kr-001");
        let b = req.to_body();
        assert_eq!(b["symbol"], json!("SAMSUNG_USDT"));
        assert_eq!(b["side"], json!(1));
        assert_eq!(b["type"], json!(1));
        assert_eq!(b["openType"], json!(1));
        assert_eq!(b["vol"], json!(3));
        // 65.00은 f64로 파싱되어 65.0 숫자.
        assert_eq!(b["price"], json!(65.0));
        assert_eq!(b["leverage"], json!(20));
        assert_eq!(b["externalOid"], json!("kr-001"));
    }

    #[test]
    fn market_order_omits_price() {
        let req = OrderRequest::market("SKHYNIX_USDT", Side::OpenShort, OpenType::Cross, "2");
        let b = req.to_body();
        assert_eq!(b["type"], json!(5));
        assert_eq!(b["side"], json!(3));
        assert!(b.get("price").is_none());
        assert!(b.get("leverage").is_none());
    }

    #[test]
    fn str_to_json_number_handles_int_float_garbage() {
        assert_eq!(str_to_json_number("3"), json!(3));
        assert_eq!(str_to_json_number("65.5"), json!(65.5));
        assert_eq!(str_to_json_number(""), json!(""));
    }

    #[test]
    fn position_parses_numeric_to_string() {
        let v = json!({
            "positionId": 9001i64,
            "symbol": "SAMSUNG_USDT",
            "holdVol": 3,
            "positionType": 1,
            "openType": 1,
            "holdAvgPrice": 65.10,
            "openAvgPrice": 65.10,
            "liquidatePrice": 40.00,
            "realised": 1.25,
            "leverage": 20
        });
        let p: Position = serde_json::from_value(v).unwrap();
        assert_eq!(p.position_id, 9001);
        assert_eq!(p.hold_vol, "3");
        assert_eq!(p.hold_avg_price, "65.1");
        assert_eq!(p.position_type, 1);
        assert_eq!(p.leverage, 20);
    }

    #[test]
    fn asset_parses() {
        let v = json!({
            "currency": "USDT",
            "positionMargin": 100.0,
            "availableBalance": 850.5,
            "cashBalance": 950.5,
            "frozenBalance": 0.0,
            "equity": 955.5,
            "unrealized": 5.0
        });
        let a: Asset = serde_json::from_value(v).unwrap();
        assert_eq!(a.currency, "USDT");
        assert_eq!(a.available_balance, "850.5");
        assert_eq!(a.cash_balance, "950.5");
        assert_eq!(a.unrealized, "5.0");
    }

    #[test]
    fn open_order_parses_id_as_string() {
        // orderId가 큰 정수로 와도 String 보존(정밀도 안전).
        let v = json!({
            "orderId": 102030405060708090i64,
            "symbol": "SAMSUNG_USDT",
            "price": 65.00,
            "vol": 3.0,
            "state": 2,
            "side": 1
        });
        let o: OpenOrder = serde_json::from_value(v).unwrap();
        assert_eq!(o.order_id, "102030405060708090");
        assert_eq!(o.symbol, "SAMSUNG_USDT");
        assert_eq!(o.side, 1);
    }
}
