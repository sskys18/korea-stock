//! 시세 도메인 (키 불필요) — contract·티커·호가·펀딩비.
//!
//! Gate APIv4 USDT Futures(`/api/v4/futures/usdt/*`) 공개 엔드포인트.
//! 필드명은 Gate-native snake_case라 `rename_all` 불필요. 가격·수량은 정밀도
//! 보존 위해 String.

use serde::Deserialize;

use crate::global::gateio::client::{ApiCall, GateioClient};
use crate::global::gateio::error::{GateioError, Result};

const FUTURES_BASE: &str = "/api/v4/futures/usdt";

/// 선물 contract 명세 (`GET /api/v4/futures/usdt/contracts/{contract}`).
///
/// Gate 선물 수량은 *계약 수*다. base 자산 수량 = `size × quanto_multiplier`.
/// 미사용 필드는 생략 — 필요 시 `raw_call`로 전체 JSON을 받는다.
#[derive(Debug, Clone, Deserialize)]
pub struct Contract {
    /// contract 이름(심볼). 예: `SAMSUNG_USDT`.
    pub name: String,
    /// "trading" / "delisting" 등.
    #[serde(default)]
    pub status: String,
    /// contract 유형. KR 종목은 "stocks".
    #[serde(default)]
    pub contract_type: String,
    /// 마크가 (청산·미실현손익 기준가).
    pub mark_price: String,
    /// 지수가 (현물 바스켓).
    pub index_price: String,
    /// 최근 체결가.
    pub last_price: String,
    /// 현재 펀딩비율 (예: "0.000652").
    pub funding_rate: String,
    /// 펀딩 주기(초). KR 종목은 28800 (8h).
    pub funding_interval: i64,
    /// 다음 펀딩 정산 시각 (epoch sec).
    pub funding_next_apply: f64,
    /// 1 계약당 base 자산 수량 (수량 환산 계수). KR 종목은 "0.01".
    pub quanto_multiplier: String,
    /// 최소 주문 수량(계약).
    pub order_size_min: i64,
    /// 최대 주문 수량(계약).
    pub order_size_max: i64,
    /// 최대 레버리지.
    pub leverage_max: String,
    /// 최소 레버리지.
    pub leverage_min: String,
}

/// 티커 1건 (`GET /api/v4/futures/usdt/tickers?contract=...`).
///
/// 마크가·펀딩·최우선호가를 한 호출에 담는다 — KR probe의 mark/funding/bid/ask를
/// 이 엔드포인트 하나로 충족한다.
#[derive(Debug, Clone, Deserialize)]
pub struct Ticker {
    /// contract 이름(심볼).
    pub contract: String,
    /// 최근 체결가.
    pub last: String,
    /// 마크가.
    pub mark_price: String,
    /// 지수가.
    pub index_price: String,
    /// 현재 펀딩비율.
    pub funding_rate: String,
    /// 최우선 매수호가.
    pub highest_bid: String,
    /// 최우선 매수호가 잔량(계약).
    #[serde(default)]
    pub highest_size: String,
    /// 최우선 매도호가.
    pub lowest_ask: String,
    /// 최우선 매도호가 잔량(계약).
    #[serde(default)]
    pub lowest_size: String,
    /// 24h 거래량(계약).
    #[serde(default)]
    pub volume_24h: String,
    /// 24h 등락률(%).
    #[serde(default)]
    pub change_percentage: String,
    /// 1 계약당 base 자산 수량.
    #[serde(default)]
    pub quanto_multiplier: String,
}

/// 호가창 한 단계 (`{"p": 가격, "s": 잔량}`). Gate는 객체 배열이다(Binance의
/// `[price, qty]` 배열과 다름). `s`는 계약 수(정수).
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBookLevel {
    /// 가격(정밀도 보존 String).
    pub p: String,
    /// 잔량 (계약 수).
    pub s: i64,
}

/// 호가창 (`GET /api/v4/futures/usdt/order_book?contract=...&limit=...`).
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBook {
    /// 응답 생성 시각 (epoch sec, 소수).
    #[serde(default)]
    pub current: f64,
    /// 마지막 갱신 시각 (epoch sec, 소수).
    #[serde(default)]
    pub update: f64,
    /// 매수호가 (높은 가격순).
    pub bids: Vec<OrderBookLevel>,
    /// 매도호가 (낮은 가격순).
    pub asks: Vec<OrderBookLevel>,
}

/// 펀딩비 이력 1건 (`GET /api/v4/futures/usdt/funding_rate?contract=...`).
#[derive(Debug, Clone, Deserialize)]
pub struct FundingRate {
    /// 정산 시각 (epoch sec).
    pub t: i64,
    /// 해당 회차 펀딩비율.
    pub r: String,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a GateioClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a GateioClient) -> Self {
        Self { client }
    }

    /// 전체 contract 목록. KR 종목만 보려면 [`crate::global::gateio::KR_SYMBOLS`]로 필터.
    pub async fn contracts(&self) -> Result<Vec<Contract>> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                format!("{FUTURES_BASE}/contracts"),
                vec![],
            ))
            .await?
            .parse()
    }

    /// 단일 contract 명세 조회.
    pub async fn contract(&self, contract: &str) -> Result<Contract> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                format!("{FUTURES_BASE}/contracts/{contract}"),
                vec![],
            ))
            .await?
            .parse()
    }

    /// 티커 조회 (마크가·펀딩·최우선호가). Gate tickers는 배열을 반환 —
    /// 단일 contract 필터 시에도 1원소 배열이라 첫 원소를 꺼낸다.
    pub async fn ticker(&self, contract: &str) -> Result<Ticker> {
        let v: Vec<Ticker> = self
            .client
            .call(ApiCall::public(
                reqwest::Method::GET,
                format!("{FUTURES_BASE}/tickers"),
                vec![("contract".into(), contract.into())],
            ))
            .await?
            .parse()?;
        v.into_iter()
            .next()
            .ok_or_else(|| GateioError::Decode(format!("no ticker for {contract}")))
    }

    /// 호가창 조회. `limit`(None이면 서버 기본).
    pub async fn order_book(&self, contract: &str, limit: Option<u32>) -> Result<OrderBook> {
        let mut params = vec![("contract".to_string(), contract.to_string())];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                format!("{FUTURES_BASE}/order_book"),
                params,
            ))
            .await?
            .parse()
    }

    /// 펀딩비 이력 조회. `limit`(None이면 서버 기본).
    pub async fn funding_rate_history(
        &self,
        contract: &str,
        limit: Option<u32>,
    ) -> Result<Vec<FundingRate>> {
        let mut params = vec![("contract".to_string(), contract.to_string())];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                format!("{FUTURES_BASE}/funding_rate"),
                params,
            ))
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_parses() {
        let v = serde_json::json!({
            "name": "SAMSUNG_USDT",
            "status": "trading",
            "contract_type": "stocks",
            "mark_price": "235.93",
            "index_price": "234.345",
            "last_price": "235.94",
            "funding_rate": "0.000652",
            "funding_interval": 28800,
            "funding_next_apply": 1780560000.0,
            "quanto_multiplier": "0.01",
            "order_size_min": 1,
            "order_size_max": 1000000,
            "leverage_max": "20",
            "leverage_min": "1"
        });
        let c: Contract = serde_json::from_value(v).unwrap();
        assert_eq!(c.name, "SAMSUNG_USDT");
        assert_eq!(c.quanto_multiplier, "0.01");
        assert_eq!(c.funding_interval, 28800);
        assert_eq!(c.order_size_min, 1);
    }

    #[test]
    fn ticker_parses() {
        let v = serde_json::json!({
            "contract": "SAMSUNG_USDT",
            "last": "235.94",
            "mark_price": "235.93",
            "index_price": "234.345",
            "funding_rate": "0.000652",
            "highest_bid": "235.79",
            "highest_size": "276",
            "lowest_ask": "236.02",
            "lowest_size": "6",
            "volume_24h": "310028",
            "change_percentage": "-8.73",
            "quanto_multiplier": "0.01"
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.contract, "SAMSUNG_USDT");
        assert_eq!(t.mark_price, "235.93");
        assert_eq!(t.funding_rate, "0.000652");
        assert_eq!(t.highest_bid, "235.79");
        assert_eq!(t.lowest_ask, "236.02");
    }

    #[test]
    fn order_book_parses_object_levels() {
        let v = serde_json::json!({
            "current": 1780550183.094,
            "update": 1780550182.849,
            "asks": [{"s": 66, "p": "236.72"}, {"s": 13, "p": "236.73"}],
            "bids": [{"s": 276, "p": "235.79"}, {"s": 50, "p": "235.51"}]
        });
        let ob: OrderBook = serde_json::from_value(v).unwrap();
        assert_eq!(ob.asks[0].p, "236.72");
        assert_eq!(ob.asks[0].s, 66);
        assert_eq!(ob.bids[0].p, "235.79");
        assert_eq!(ob.bids[0].s, 276);
    }

    #[test]
    fn funding_rate_history_parses() {
        let v = serde_json::json!([{"t": 1780531200, "r": "0.000652"}]);
        let f: Vec<FundingRate> = serde_json::from_value(v).unwrap();
        assert_eq!(f[0].t, 1780531200);
        assert_eq!(f[0].r, "0.000652");
    }
}
