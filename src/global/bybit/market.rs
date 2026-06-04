//! 시세 도메인 (키 불필요) — 티커(마크/지수/펀딩/최우선호가)·호가창·펀딩이력·계약정보.
//!
//! Bybit v5(`/v5/market/*`) 공개 엔드포인트, `category=linear`(USDT 무기한). 가격·수량은
//! Bybit가 모두 **문자열**로 내려주므로 그대로 String으로 보관해 정밀도를 보존한다.
//! 응답은 `result.list[]` 배열에 담겨 오므로 내부 래퍼로 벗겨 단건/리스트로 반환한다.

use serde::Deserialize;

use crate::global::bybit::client::{ApiCall, BybitClient};
use crate::global::bybit::error::{BybitError, Result};

/// `result.list[]` 래퍼. Bybit v5 시세 응답은 단건이라도 list에 담겨 온다.
#[derive(Debug, Clone, Deserialize)]
struct ListWrap<T> {
    #[serde(default = "Vec::new")]
    list: Vec<T>,
}

/// 선형(linear) 티커 (`GET /v5/market/tickers?category=linear&symbol=...`).
///
/// 마크가·지수가·펀딩비·최우선 매수/매도 호가를 한 번에 담는다 — KR perp 시세 프로브는
/// 이 한 호출로 충분하다(별도 orderbook 호출 불필요).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    pub symbol: String,
    /// 최종 체결가.
    pub last_price: String,
    /// 지수가 (현물 바스켓).
    pub index_price: String,
    /// 마크가 (청산·미실현손익 기준가).
    pub mark_price: String,
    /// 현재 펀딩비율 (예: "0.0001" = 0.01%).
    pub funding_rate: String,
    /// 다음 펀딩 정산 시각 (epoch ms, 문자열).
    pub next_funding_time: String,
    /// 24h 거래량 (계약 수).
    #[serde(default)]
    pub volume24h: String,
    /// 24h 거래대금 (USDT).
    #[serde(default)]
    pub turnover24h: String,
    /// 미결제약정 (계약 수).
    #[serde(default)]
    pub open_interest: String,
    /// 최우선 매수호가.
    #[serde(default)]
    pub bid1_price: String,
    /// 최우선 매수 잔량.
    #[serde(default)]
    pub bid1_size: String,
    /// 최우선 매도호가.
    #[serde(default)]
    pub ask1_price: String,
    /// 최우선 매도 잔량.
    #[serde(default)]
    pub ask1_size: String,
}

/// 호가창 (`GET /v5/market/orderbook?category=linear&symbol=...&limit=...`).
///
/// Bybit는 필드명을 축약한다: `s`=심볼, `b`=매수, `a`=매도, `ts`=시각, `u`=업데이트ID.
/// 각 레벨은 `[price, size]` 문자열 2원소 배열.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBook {
    /// 심볼.
    #[serde(rename = "s")]
    pub symbol: String,
    /// 매수호가 `[가격, 잔량]` (높은 가격순).
    #[serde(rename = "b", default)]
    pub bids: Vec<[String; 2]>,
    /// 매도호가 `[가격, 잔량]` (낮은 가격순).
    #[serde(rename = "a", default)]
    pub asks: Vec<[String; 2]>,
    /// 생성 시각 (epoch ms).
    #[serde(rename = "ts", default)]
    pub ts: i64,
    /// 업데이트 ID.
    #[serde(rename = "u", default)]
    pub update_id: i64,
}

/// 펀딩비 이력 1건 (`GET /v5/market/funding/history` → result.list[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingHistory {
    pub symbol: String,
    /// 해당 회차 펀딩비율.
    pub funding_rate: String,
    /// 정산 시각 (epoch ms, 문자열).
    pub funding_rate_timestamp: String,
}

/// 가격 필터 (`priceFilter`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceFilter {
    #[serde(default)]
    pub min_price: String,
    #[serde(default)]
    pub max_price: String,
    /// 호가 단위.
    pub tick_size: String,
}

/// 수량 필터 (`lotSizeFilter`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LotSizeFilter {
    pub max_order_qty: String,
    pub min_order_qty: String,
    /// 수량 단위.
    pub qty_step: String,
    #[serde(default)]
    pub min_notional_value: String,
}

/// 계약(심볼) 정보 1건 (`GET /v5/market/instruments-info?category=linear` → result.list[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstrumentInfo {
    pub symbol: String,
    /// "LinearPerpetual" 등.
    #[serde(default)]
    pub contract_type: String,
    /// "Trading" / "PreLaunch" 등.
    pub status: String,
    pub base_coin: String,
    pub quote_coin: String,
    #[serde(default)]
    pub settle_coin: String,
    pub price_filter: PriceFilter,
    pub lot_size_filter: LotSizeFilter,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a BybitClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a BybitClient) -> Self {
        Self { client }
    }

    /// 선형 티커 조회 — 마크가·지수가·펀딩비·최우선호가를 한 번에.
    pub async fn ticker(&self, symbol: &str) -> Result<Ticker> {
        let wrap: ListWrap<Ticker> = self
            .client
            .call(ApiCall::public_get(
                "/v5/market/tickers",
                vec![
                    ("category".into(), "linear".into()),
                    ("symbol".into(), symbol.into()),
                ],
            ))
            .await?
            .parse()?;
        wrap.list
            .into_iter()
            .next()
            .ok_or_else(|| BybitError::Decode(format!("ticker not found: {symbol}")))
    }

    /// 호가창 조회. `limit` ∈ {1,25,50,100,200,500} (None이면 서버 기본 25 — linear).
    pub async fn orderbook(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        let mut params = vec![
            ("category".to_string(), "linear".to_string()),
            ("symbol".to_string(), symbol.to_string()),
        ];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        self.client
            .call(ApiCall::public_get("/v5/market/orderbook", params))
            .await?
            .parse()
    }

    /// 펀딩비 이력 조회. `limit` ∈ [1,200] (None이면 서버 기본 200).
    pub async fn funding_history(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<FundingHistory>> {
        let mut params = vec![
            ("category".to_string(), "linear".to_string()),
            ("symbol".to_string(), symbol.to_string()),
        ];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        let wrap: ListWrap<FundingHistory> = self
            .client
            .call(ApiCall::public_get("/v5/market/funding/history", params))
            .await?
            .parse()?;
        Ok(wrap.list)
    }

    /// 전체 linear 계약 목록. KR 종목만 보려면 [`crate::global::bybit::KR_SYMBOLS`]로 필터.
    pub async fn instruments(&self) -> Result<Vec<InstrumentInfo>> {
        let wrap: ListWrap<InstrumentInfo> = self
            .client
            .call(ApiCall::public_get(
                "/v5/market/instruments-info",
                vec![("category".into(), "linear".into())],
            ))
            .await?
            .parse()?;
        Ok(wrap.list)
    }

    /// 단일 심볼 계약정보 조회 (없으면 [`BybitError::Decode`]).
    pub async fn instrument(&self, symbol: &str) -> Result<InstrumentInfo> {
        let wrap: ListWrap<InstrumentInfo> = self
            .client
            .call(ApiCall::public_get(
                "/v5/market/instruments-info",
                vec![
                    ("category".into(), "linear".into()),
                    ("symbol".into(), symbol.into()),
                ],
            ))
            .await?
            .parse()?;
        wrap.list
            .into_iter()
            .next()
            .ok_or_else(|| BybitError::Decode(format!("symbol not found: {symbol}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_parses_mark_funding_bidask() {
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDT",
            "lastPrice": "65.12",
            "indexPrice": "65.105",
            "markPrice": "65.11",
            "fundingRate": "0.0001",
            "nextFundingTime": "1717400000000",
            "volume24h": "35210",
            "turnover24h": "2290000",
            "openInterest": "12000",
            "bid1Price": "65.10",
            "bid1Size": "1200",
            "ask1Price": "65.12",
            "ask1Size": "1000"
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.symbol, "SAMSUNGUSDT");
        assert_eq!(t.mark_price, "65.11");
        assert_eq!(t.funding_rate, "0.0001");
        assert_eq!(t.bid1_price, "65.10");
        assert_eq!(t.ask1_price, "65.12");
        assert_eq!(t.next_funding_time, "1717400000000");
    }

    #[test]
    fn orderbook_parses_abbreviated_fields() {
        let v = serde_json::json!({
            "s": "SKHYNIXUSDT",
            "b": [["120.40", "300"], ["120.39", "150"]],
            "a": [["120.42", "200"]],
            "ts": 1717398000000i64,
            "u": 99,
            "seq": 7,
            "cts": 1717398000001i64
        });
        let ob: OrderBook = serde_json::from_value(v).unwrap();
        assert_eq!(ob.symbol, "SKHYNIXUSDT");
        assert_eq!(ob.bids[0][0], "120.40");
        assert_eq!(ob.bids[0][1], "300");
        assert_eq!(ob.asks[0][0], "120.42");
        assert_eq!(ob.update_id, 99);
    }

    #[test]
    fn funding_history_parses() {
        let v = serde_json::json!({
            "symbol": "HYUNDAIUSDT",
            "fundingRate": "0.00012",
            "fundingRateTimestamp": "1717387200000"
        });
        let f: FundingHistory = serde_json::from_value(v).unwrap();
        assert_eq!(f.symbol, "HYUNDAIUSDT");
        assert_eq!(f.funding_rate, "0.00012");
        assert_eq!(f.funding_rate_timestamp, "1717387200000");
    }

    #[test]
    fn instrument_info_parses_nested_filters() {
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDT",
            "contractType": "LinearPerpetual",
            "status": "Trading",
            "baseCoin": "SAMSUNG",
            "quoteCoin": "USDT",
            "settleCoin": "USDT",
            "priceScale": "2",
            "priceFilter": { "minPrice": "0.01", "maxPrice": "1000", "tickSize": "0.01" },
            "lotSizeFilter": {
                "maxOrderQty": "10000", "minOrderQty": "1", "qtyStep": "1",
                "minNotionalValue": "5"
            }
        });
        let i: InstrumentInfo = serde_json::from_value(v).unwrap();
        assert_eq!(i.symbol, "SAMSUNGUSDT");
        assert_eq!(i.status, "Trading");
        assert_eq!(i.price_filter.tick_size, "0.01");
        assert_eq!(i.lot_size_filter.qty_step, "1");
    }
}
