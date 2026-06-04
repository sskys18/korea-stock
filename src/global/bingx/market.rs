//! 시세 도메인 (키 불필요) — 마크가·펀딩·호가·티커·계약정보.
//!
//! BingX Perpetual Swap V2 공개 엔드포인트(`/openApi/swap/v2/quote/*`). 응답은
//! `{code,msg,data}` envelope의 `data`만 파싱한다(언래핑은 client). 가격·수량은
//! 정밀도 보존 위해 String.

use serde::Deserialize;

use crate::global::bingx::client::{ApiCall, BingxClient};
use crate::global::bingx::error::{BingxError, Result};

/// 마크가·펀딩 지표 (`GET /openApi/swap/v2/quote/premiumIndex`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PremiumIndex {
    pub symbol: String,
    /// 마크가 (청산·미실현손익 기준가).
    pub mark_price: String,
    /// 지수가 (현물 바스켓).
    pub index_price: String,
    /// 직전 펀딩비율 (예: "0.00080000" = 0.08%).
    pub last_funding_rate: String,
    /// 다음 펀딩 정산 시각 (epoch ms).
    pub next_funding_time: i64,
    /// 펀딩 주기(시간). BingX는 8h.
    #[serde(default)]
    pub funding_interval_hours: i64,
}

/// 24시간 티커 (`GET /openApi/swap/v2/quote/ticker`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    pub symbol: String,
    pub last_price: String,
    /// 직전 체결 수량.
    #[serde(default)]
    pub last_qty: String,
    pub price_change: String,
    pub price_change_percent: String,
    pub high_price: String,
    pub low_price: String,
    /// 거래량 (계약 수).
    pub volume: String,
    /// 거래대금 (USDT).
    pub quote_volume: String,
    pub open_price: String,
    pub open_time: i64,
    pub close_time: i64,
    /// 최우선 매도호가.
    #[serde(default)]
    pub ask_price: String,
    #[serde(default)]
    pub ask_qty: String,
    /// 최우선 매수호가.
    #[serde(default)]
    pub bid_price: String,
    #[serde(default)]
    pub bid_qty: String,
}

/// 호가창 (`GET /openApi/swap/v2/quote/depth`). 각 항목은 `[price, qty]` 문자열 쌍.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Depth {
    /// 시세 시각 (epoch ms).
    #[serde(default, rename = "T")]
    pub time: i64,
    /// 매수호가 `[가격, 잔량]` (높은 가격순).
    pub bids: Vec<[String; 2]>,
    /// 매도호가 `[가격, 잔량]` (낮은 가격순).
    pub asks: Vec<[String; 2]>,
}

/// 계약(심볼) 정보 1건 (`GET /openApi/swap/v2/quote/contracts`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub symbol: String,
    /// 1=거래 가능. (BingX `status` 정수.)
    pub status: i64,
    /// 기초자산명 (예: "NCSKSAMSUNG2USD").
    #[serde(default)]
    pub asset: String,
    /// 결제통화 (예: "USDT").
    #[serde(default)]
    pub currency: String,
    /// 표시명 (예: "SAMSUNG-USDT", 지수는 "KR200-USDT").
    #[serde(default)]
    pub display_name: String,
    pub price_precision: u32,
    pub quantity_precision: u32,
    /// 최소 주문 수량 (USDT 환산).
    #[serde(default)]
    pub trade_min_usdt: f64,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a BingxClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a BingxClient) -> Self {
        Self { client }
    }

    /// 마크가·펀딩 지표 조회.
    pub async fn premium_index(&self, symbol: &str) -> Result<PremiumIndex> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/openApi/swap/v2/quote/premiumIndex",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 24시간 티커 조회 (last·고저·bid/ask 포함).
    pub async fn ticker(&self, symbol: &str) -> Result<Ticker> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/openApi/swap/v2/quote/ticker",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 호가창 조회. `limit` ∈ {5,10,20,50,100,...} (None이면 서버 기본).
    pub async fn depth(&self, symbol: &str, limit: Option<u32>) -> Result<Depth> {
        let mut params = vec![("symbol".to_string(), symbol.to_string())];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/openApi/swap/v2/quote/depth",
                params,
            ))
            .await?
            .parse()
    }

    /// 전체 계약 목록. KR 종목만 보려면 [`crate::global::bingx::KR_SYMBOLS`]로 필터.
    pub async fn contracts(&self) -> Result<Vec<Contract>> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/openApi/swap/v2/quote/contracts",
                vec![],
            ))
            .await?
            .parse()
    }

    /// 단일 계약 정보 조회 (없으면 [`BingxError::Decode`]).
    pub async fn contract(&self, symbol: &str) -> Result<Contract> {
        self.contracts()
            .await?
            .into_iter()
            .find(|c| c.symbol == symbol)
            .ok_or_else(|| BingxError::Decode(format!("symbol not found: {symbol}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn premium_index_parses() {
        // 라이브 응답 형태(envelope의 data).
        let v = serde_json::json!({
            "symbol": "NCSKSAMSUNG2USD-USDT",
            "markPrice": "234.11",
            "indexPrice": "234.11",
            "lastFundingRate": "0.00080000",
            "nextFundingTime": 1780560000000i64,
            "fundingIntervalHours": 8,
            "minFundingRate": "-0.02",
            "maxFundingRate": "0.02",
            "updateTime": 1780531200000i64
        });
        let p: PremiumIndex = serde_json::from_value(v).unwrap();
        assert_eq!(p.symbol, "NCSKSAMSUNG2USD-USDT");
        assert_eq!(p.mark_price, "234.11");
        assert_eq!(p.last_funding_rate, "0.00080000");
        assert_eq!(p.next_funding_time, 1780560000000);
        assert_eq!(p.funding_interval_hours, 8);
    }

    #[test]
    fn ticker_parses_bid_ask() {
        let v = serde_json::json!({
            "symbol": "NCSKSAMSUNG2USD-USDT",
            "priceChange": "-22.27",
            "priceChangePercent": "-8.69",
            "lastPrice": "234.12",
            "lastQty": "0.03563",
            "highPrice": "259.97",
            "lowPrice": "231.27",
            "volume": "11276.17000",
            "quoteVolume": "2796102.86",
            "openPrice": "256.39",
            "openTime": 1780549599845i64,
            "closeTime": 1780550195045i64,
            "askPrice": "234.13",
            "askQty": "0.39952",
            "bidPrice": "234.09",
            "bidQty": "3114.11466"
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.last_price, "234.12");
        assert_eq!(t.bid_price, "234.09");
        assert_eq!(t.ask_price, "234.13");
    }

    #[test]
    fn depth_parses_price_qty_pairs() {
        let v = serde_json::json!({
            "T": 1780550195362i64,
            "bids": [["234.09", "3114.11466"], ["234.06", "0.07899"]],
            "asks": [["234.14", "0.33322"]]
        });
        let d: Depth = serde_json::from_value(v).unwrap();
        assert_eq!(d.bids[0][0], "234.09");
        assert_eq!(d.bids[0][1], "3114.11466");
        assert_eq!(d.asks[0][0], "234.14");
        assert_eq!(d.time, 1780550195362);
    }

    #[test]
    fn contract_parses_index_display_name() {
        // KOSPI 지수 계약: displayName "KR200-USDT", NCSI 접두.
        let v = serde_json::json!({
            "contractId": "102659",
            "symbol": "NCSIKOSPI2USD-USDT",
            "size": "0.000001",
            "quantityPrecision": 6,
            "pricePrecision": 2,
            "currency": "USDT",
            "asset": "NCSIKOSPI2USD",
            "status": 1,
            "tradeMinUSDT": 2,
            "displayName": "KR200-USDT"
        });
        let c: Contract = serde_json::from_value(v).unwrap();
        assert_eq!(c.symbol, "NCSIKOSPI2USD-USDT");
        assert_eq!(c.status, 1);
        assert_eq!(c.display_name, "KR200-USDT");
        assert_eq!(c.price_precision, 2);
    }
}
