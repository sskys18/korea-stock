//! 시세 도메인 (키 불필요) — 마크가·펀딩·호가·티커·캔들·거래소정보.
//!
//! Aster fapi(`/fapi/v1/*`) 공개 엔드포인트(Binance 호환). 가격·수량은 정밀도
//! 보존 위해 String.

use serde::Deserialize;

use crate::global::aster::client::{ApiCall, AsterClient};
use crate::global::aster::error::{AsterError, Result};

/// 마크가·펀딩 지표 (`GET /fapi/v1/premiumIndex`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PremiumIndex {
    pub symbol: String,
    /// 마크가 (청산·미실현손익 기준가).
    pub mark_price: String,
    /// 지수가 (현물 바스켓).
    pub index_price: String,
    /// 직전 펀딩비율 (예: "0.00010000" = 0.01%).
    pub last_funding_rate: String,
    /// 다음 펀딩 정산 시각 (epoch ms).
    pub next_funding_time: i64,
    /// 시세 시각 (epoch ms).
    pub time: i64,
}

/// 펀딩비 이력 1건 (`GET /fapi/v1/fundingRate`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingRate {
    pub symbol: String,
    /// 해당 회차 펀딩비율.
    pub funding_rate: String,
    /// 정산 시각 (epoch ms).
    pub funding_time: i64,
}

/// 24시간 티커 (`GET /fapi/v1/ticker/24hr`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticker24hr {
    pub symbol: String,
    pub last_price: String,
    pub price_change: String,
    pub price_change_percent: String,
    pub high_price: String,
    pub low_price: String,
    /// 거래량 (계약 수).
    pub volume: String,
    /// 거래대금 (USDT).
    pub quote_volume: String,
    pub open_time: i64,
    pub close_time: i64,
}

/// 호가창 (`GET /fapi/v1/depth`). 각 항목은 `[price, qty]` 문자열 쌍.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Depth {
    pub last_update_id: i64,
    /// 매수호가 `[가격, 잔량]` (높은 가격순).
    pub bids: Vec<[String; 2]>,
    /// 매도호가 `[가격, 잔량]` (낮은 가격순).
    pub asks: Vec<[String; 2]>,
}

/// 캔들(봉) 1건. Aster klines는 Binance와 동일한 이종 배열이라 내부 [`KlineRaw`]로
/// 받아 변환한다.
#[derive(Debug, Clone)]
pub struct Kline {
    /// 봉 시작 시각 (epoch ms).
    pub open_time: i64,
    pub open: String,
    pub high: String,
    pub low: String,
    pub close: String,
    /// 거래량 (계약 수).
    pub volume: String,
    /// 봉 종료 시각 (epoch ms).
    pub close_time: i64,
    /// 거래대금 (USDT).
    pub quote_volume: String,
    pub trades: i64,
}

/// klines 원본: 12원소 이종 배열. serde 튜플로 받는다(arity가 정확히 맞아야
/// 역직렬화되므로 미사용 필드 9~11도 보존). [`Kline`]으로 변환 시 버린다.
#[derive(Deserialize)]
#[allow(dead_code)]
struct KlineRaw(
    i64,    // 0 openTime
    String, // 1 open
    String, // 2 high
    String, // 3 low
    String, // 4 close
    String, // 5 volume
    i64,    // 6 closeTime
    String, // 7 quoteVolume
    i64,    // 8 trades
    String, // 9 takerBuyBase
    String, // 10 takerBuyQuote
    String, // 11 ignore
);

impl From<KlineRaw> for Kline {
    fn from(r: KlineRaw) -> Self {
        Self {
            open_time: r.0,
            open: r.1,
            high: r.2,
            low: r.3,
            close: r.4,
            volume: r.5,
            close_time: r.6,
            quote_volume: r.7,
            trades: r.8,
        }
    }
}

/// 거래소 심볼 정보 1건 (`GET /fapi/v1/exchangeInfo` → symbols[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    pub symbol: String,
    /// "TRADING" / "PENDING_TRADING" 등.
    pub status: String,
    pub base_asset: String,
    pub quote_asset: String,
    /// "PERPETUAL" 등.
    #[serde(default)]
    pub contract_type: String,
    pub price_precision: u32,
    pub quantity_precision: u32,
}

#[derive(Deserialize)]
struct ExchangeInfo {
    symbols: Vec<SymbolInfo>,
}

/// 캔들 봉 단위. 요청 파라미터 — 값을 우리가 통제하므로 enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KlineInterval {
    Min1,
    Min5,
    Min15,
    Hour1,
    Hour4,
    Day1,
}

impl KlineInterval {
    fn code(self) -> &'static str {
        match self {
            KlineInterval::Min1 => "1m",
            KlineInterval::Min5 => "5m",
            KlineInterval::Min15 => "15m",
            KlineInterval::Hour1 => "1h",
            KlineInterval::Hour4 => "4h",
            KlineInterval::Day1 => "1d",
        }
    }
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a AsterClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a AsterClient) -> Self {
        Self { client }
    }

    /// 마크가·펀딩 지표 조회.
    pub async fn premium_index(&self, symbol: &str) -> Result<PremiumIndex> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/fapi/v1/premiumIndex",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 펀딩비 이력 조회. `limit` 최대 1000 (None이면 서버 기본).
    pub async fn funding_rate_history(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<FundingRate>> {
        let mut params = vec![("symbol".to_string(), symbol.to_string())];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/fapi/v1/fundingRate",
                params,
            ))
            .await?
            .parse()
    }

    /// 24시간 티커 조회.
    pub async fn ticker_24hr(&self, symbol: &str) -> Result<Ticker24hr> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/fapi/v1/ticker/24hr",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 호가창 조회. `limit` ∈ {5,10,20,50,100,500,1000} (None이면 서버 기본 500).
    pub async fn depth(&self, symbol: &str, limit: Option<u32>) -> Result<Depth> {
        let mut params = vec![("symbol".to_string(), symbol.to_string())];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        self.client
            .call(ApiCall::public(reqwest::Method::GET, "/fapi/v1/depth", params))
            .await?
            .parse()
    }

    /// 캔들 조회. `limit` 최대 1500 (None이면 서버 기본 500).
    pub async fn klines(
        &self,
        symbol: &str,
        interval: KlineInterval,
        limit: Option<u32>,
    ) -> Result<Vec<Kline>> {
        let mut params = vec![
            ("symbol".to_string(), symbol.to_string()),
            ("interval".to_string(), interval.code().to_string()),
        ];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        let raw: Vec<KlineRaw> = self
            .client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/fapi/v1/klines",
                params,
            ))
            .await?
            .parse()?;
        Ok(raw.into_iter().map(Kline::from).collect())
    }

    /// 전체 심볼 목록. KR 종목만 보려면 [`crate::global::aster::KR_SYMBOLS`]로 필터.
    pub async fn markets(&self) -> Result<Vec<SymbolInfo>> {
        let info: ExchangeInfo = self
            .client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/fapi/v1/exchangeInfo",
                vec![],
            ))
            .await?
            .parse()?;
        Ok(info.symbols)
    }

    /// 단일 심볼 정보 조회 (없으면 [`AsterError::Decode`]).
    pub async fn market_info(&self, symbol: &str) -> Result<SymbolInfo> {
        self.markets()
            .await?
            .into_iter()
            .find(|s| s.symbol == symbol)
            .ok_or_else(|| AsterError::Decode(format!("symbol not found: {symbol}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_codes() {
        assert_eq!(KlineInterval::Min1.code(), "1m");
        assert_eq!(KlineInterval::Hour4.code(), "4h");
        assert_eq!(KlineInterval::Day1.code(), "1d");
    }

    #[test]
    fn premium_index_parses() {
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDT",
            "markPrice": "65.12000000",
            "indexPrice": "65.10500000",
            "estimatedSettlePrice": "65.11000000",
            "lastFundingRate": "0.00010000",
            "interestRate": "0.00010000",
            "nextFundingTime": 1717400000000i64,
            "time": 1717398000000i64
        });
        let p: PremiumIndex = serde_json::from_value(v).unwrap();
        assert_eq!(p.symbol, "SAMSUNGUSDT");
        assert_eq!(p.last_funding_rate, "0.00010000");
        assert_eq!(p.next_funding_time, 1717400000000);
    }

    #[test]
    fn depth_parses_price_qty_pairs() {
        let v = serde_json::json!({
            "lastUpdateId": 123,
            "E": 1717398000000i64,
            "T": 1717398000000i64,
            "bids": [["65.10", "1200"], ["65.09", "800"]],
            "asks": [["65.12", "1000"]]
        });
        let d: Depth = serde_json::from_value(v).unwrap();
        assert_eq!(d.bids[0][0], "65.10");
        assert_eq!(d.bids[0][1], "1200");
        assert_eq!(d.asks[0][0], "65.12");
    }

    #[test]
    fn klines_heterogeneous_array_maps_to_named() {
        let v = serde_json::json!([
            [
                1717398000000i64, "65.00", "65.30", "64.90", "65.12", "35210",
                1717398059999i64, "2290000", 412, "18000", "1170000", "0"
            ]
        ]);
        let raw: Vec<KlineRaw> = serde_json::from_value(v).unwrap();
        let k: Vec<Kline> = raw.into_iter().map(Kline::from).collect();
        assert_eq!(k[0].open, "65.00");
        assert_eq!(k[0].close, "65.12");
        assert_eq!(k[0].close_time, 1717398059999);
        assert_eq!(k[0].trades, 412);
    }

    #[test]
    fn symbol_info_parses() {
        let v = serde_json::json!({
            "symbol": "SKHYNIXUSDT",
            "status": "TRADING",
            "baseAsset": "SKHYNIX",
            "quoteAsset": "USDT",
            "contractType": "PERPETUAL",
            "pricePrecision": 2,
            "quantityPrecision": 0
        });
        let s: SymbolInfo = serde_json::from_value(v).unwrap();
        assert_eq!(s.symbol, "SKHYNIXUSDT");
        assert_eq!(s.status, "TRADING");
        assert_eq!(s.price_precision, 2);
    }
}
