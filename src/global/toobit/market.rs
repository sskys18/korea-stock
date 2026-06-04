//! 시세 도메인 (키 불필요) — 마크가·펀딩·호가·티커·거래소정보.
//!
//! 시세는 `/quote/v1/*`, 펀딩·메타는 `/api/v1/*`. 가격·수량은 정밀도 보존 위해 String.
//!
//! Toobit 응답 주의점(라이브 검증 2026-06-04):
//!  - 티커·펀딩은 단일 심볼도 **배열**로 반환한다(`Vec<_>` → `.first()`).
//!  - 마크가·호가는 단일 객체.
//!  - 티커는 1글자 키(`c/b/a/h/l/o/v/qv/t/s/...`)라 필드별 `rename` 필수.
//!  - `nextFundingTime`은 **문자열**, 마크가·티커·호가의 시각은 **정수(ms)**.

use serde::Deserialize;

use crate::global::toobit::client::{ApiCall, ToobitClient};
use crate::global::toobit::error::{Result, ToobitError};

/// 마크가 (`GET /quote/v1/markPrice`). 단일 객체.
#[derive(Debug, Clone, Deserialize)]
pub struct MarkPrice {
    /// 거래소 ID (Toobit 내부 식별자).
    #[serde(rename = "exchangeId")]
    pub exchange_id: i64,
    /// 심볼 (`SAMSUNG-SWAP-USDT`).
    #[serde(rename = "symbolId")]
    pub symbol: String,
    /// 마크가 (청산·미실현손익 기준가).
    pub price: String,
    /// 시세 시각 (epoch ms).
    pub time: i64,
}

/// 펀딩비 1건 (`GET /api/v1/futures/fundingRate`). 응답은 배열.
#[derive(Debug, Clone, Deserialize)]
pub struct FundingRate {
    pub symbol: String,
    /// 현 펀딩비율 (예: "0.00736558238292526").
    pub rate: String,
    /// 펀딩 주기 (예: "8H").
    pub period: String,
    /// 다음 펀딩 정산 시각 (epoch ms, **문자열**).
    #[serde(rename = "nextFundingTime")]
    pub next_funding_time: String,
    /// 누적 이자.
    #[serde(default)]
    pub interest: String,
    /// 펀딩비율 상한.
    #[serde(rename = "fundingRateCap", default)]
    pub funding_rate_cap: String,
    /// 펀딩비율 하한.
    #[serde(rename = "fundingRateFloor", default)]
    pub funding_rate_floor: String,
}

/// 24시간 티커 (`GET /quote/v1/ticker/24hr`). 응답은 배열, 1글자 키.
#[derive(Debug, Clone, Deserialize)]
pub struct Ticker24hr {
    /// 심볼 (`s`).
    #[serde(rename = "s")]
    pub symbol: String,
    /// 최근가/종가 (`c`).
    #[serde(rename = "c")]
    pub last_price: String,
    /// 최우선 매수호가 (`b`).
    #[serde(rename = "b")]
    pub bid_price: String,
    /// 최우선 매도호가 (`a`).
    #[serde(rename = "a")]
    pub ask_price: String,
    /// 24h 시가 (`o`).
    #[serde(rename = "o")]
    pub open_price: String,
    /// 24h 고가 (`h`).
    #[serde(rename = "h")]
    pub high_price: String,
    /// 24h 저가 (`l`).
    #[serde(rename = "l")]
    pub low_price: String,
    /// 거래량 (계약 수, `v`).
    #[serde(rename = "v")]
    pub volume: String,
    /// 거래대금 (USDT, `qv`).
    #[serde(rename = "qv")]
    pub quote_volume: String,
    /// 가격 변동폭 (`pc`).
    #[serde(rename = "pc", default)]
    pub price_change: String,
    /// 가격 변동률 (`pcp`).
    #[serde(rename = "pcp", default)]
    pub price_change_percent: String,
    /// 시세 시각 (epoch ms, `t`).
    #[serde(rename = "t")]
    pub time: i64,
}

/// 호가창 (`GET /quote/v1/depth`). 각 항목은 `[price, qty]` 문자열 쌍.
#[derive(Debug, Clone, Deserialize)]
pub struct Depth {
    /// 시세 시각 (epoch ms).
    #[serde(rename = "t")]
    pub time: i64,
    /// 매수호가 `[가격, 잔량]` (높은 가격순).
    #[serde(rename = "b")]
    pub bids: Vec<[String; 2]>,
    /// 매도호가 `[가격, 잔량]` (낮은 가격순).
    #[serde(rename = "a")]
    pub asks: Vec<[String; 2]>,
}

/// 가격/수량 필터 1건 (`exchangeInfo` contracts[].filters[]).
/// 필터 종류별로 채워지는 필드가 달라 모두 Option.
#[derive(Debug, Clone, Deserialize)]
pub struct ContractFilter {
    /// "PRICE_FILTER" / "LOT_SIZE" / "MIN_NOTIONAL" 등.
    #[serde(rename = "filterType")]
    pub filter_type: String,
    #[serde(rename = "minPrice", default)]
    pub min_price: Option<String>,
    #[serde(rename = "maxPrice", default)]
    pub max_price: Option<String>,
    #[serde(rename = "tickSize", default)]
    pub tick_size: Option<String>,
    #[serde(rename = "minQty", default)]
    pub min_qty: Option<String>,
    #[serde(rename = "maxQty", default)]
    pub max_qty: Option<String>,
    #[serde(rename = "stepSize", default)]
    pub step_size: Option<String>,
    #[serde(rename = "minNotional", default)]
    pub min_notional: Option<String>,
}

/// 무기한 스왑 계약 정보 1건 (`GET /api/v1/exchangeInfo` → contracts[]).
///
/// Toobit precision 필드는 정수가 아니라 **틱 크기 문자열**(예 "0.01")이다.
#[derive(Debug, Clone, Deserialize)]
pub struct ContractInfo {
    pub symbol: String,
    /// "TRADING" / "PENDING" 등.
    pub status: String,
    #[serde(rename = "baseAsset")]
    pub base_asset: String,
    #[serde(rename = "quoteAsset")]
    pub quote_asset: String,
    /// 기초자산 코드 (예 "SAMSUNG").
    #[serde(default)]
    pub underlying: String,
    /// 계약 승수 (계약 1개당 기초자산 수량).
    #[serde(rename = "contractMultiplier", default)]
    pub contract_multiplier: String,
    /// 마진 토큰 (보통 "USDT").
    #[serde(rename = "marginToken", default)]
    pub margin_token: String,
    /// 가격 틱(문자열, 예 "0.01").
    #[serde(rename = "quoteAssetPrecision", default)]
    pub quote_asset_precision: String,
    /// 호가/수량 필터.
    #[serde(default)]
    pub filters: Vec<ContractFilter>,
}

#[derive(Deserialize)]
struct ExchangeInfo {
    #[serde(default)]
    contracts: Vec<ContractInfo>,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a ToobitClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a ToobitClient) -> Self {
        Self { client }
    }

    /// 마크가 조회 (`GET /quote/v1/markPrice`).
    pub async fn mark_price(&self, symbol: &str) -> Result<MarkPrice> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/quote/v1/markPrice",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 펀딩비 조회 (`GET /api/v1/futures/fundingRate`). 배열에서 첫 건 반환.
    pub async fn funding_rate(&self, symbol: &str) -> Result<FundingRate> {
        let rows: Vec<FundingRate> = self
            .client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/api/v1/futures/fundingRate",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()?;
        rows.into_iter()
            .next()
            .ok_or_else(|| ToobitError::Decode(format!("no funding rate for {symbol}")))
    }

    /// 24시간 티커 조회 (`GET /quote/v1/ticker/24hr`). 배열에서 첫 건 반환.
    pub async fn ticker_24hr(&self, symbol: &str) -> Result<Ticker24hr> {
        let rows: Vec<Ticker24hr> = self
            .client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/quote/v1/ticker/24hr",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()?;
        rows.into_iter()
            .next()
            .ok_or_else(|| ToobitError::Decode(format!("no ticker for {symbol}")))
    }

    /// 호가창 조회 (`GET /quote/v1/depth`). `limit`은 서버 허용값(예 5/10/20...).
    pub async fn depth(&self, symbol: &str, limit: Option<u32>) -> Result<Depth> {
        let mut params = vec![("symbol".to_string(), symbol.to_string())];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/quote/v1/depth",
                params,
            ))
            .await?
            .parse()
    }

    /// 전체 스왑 계약 목록. KR 종목만 보려면 [`crate::global::toobit::KR_SYMBOLS`]로 필터.
    pub async fn markets(&self) -> Result<Vec<ContractInfo>> {
        let info: ExchangeInfo = self
            .client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/api/v1/exchangeInfo",
                vec![],
            ))
            .await?
            .parse()?;
        Ok(info.contracts)
    }

    /// 단일 심볼 계약 정보 조회 (없으면 [`ToobitError::Decode`]).
    pub async fn market_info(&self, symbol: &str) -> Result<ContractInfo> {
        self.markets()
            .await?
            .into_iter()
            .find(|c| c.symbol == symbol)
            .ok_or_else(|| ToobitError::Decode(format!("symbol not found: {symbol}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_price_parses() {
        let v = serde_json::json!({
            "exchangeId": 301,
            "symbolId": "SAMSUNG-SWAP-USDT",
            "price": "235.41",
            "time": 1780548073000i64
        });
        let m: MarkPrice = serde_json::from_value(v).unwrap();
        assert_eq!(m.symbol, "SAMSUNG-SWAP-USDT");
        assert_eq!(m.price, "235.41");
        assert_eq!(m.time, 1780548073000);
    }

    #[test]
    fn funding_rate_parses_string_next_time() {
        let v = serde_json::json!([{
            "symbol": "SAMSUNG-SWAP-USDT",
            "rate": "0.00736558238292526",
            "period": "8H",
            "nextFundingTime": "1780560000000",
            "interest": "0.0",
            "fundingRateCap": "0.02",
            "fundingRateFloor": "-0.02"
        }]);
        let rows: Vec<FundingRate> = serde_json::from_value(v).unwrap();
        let f = &rows[0];
        assert_eq!(f.rate, "0.00736558238292526");
        assert_eq!(f.period, "8H");
        // 문자열 시각 — 정수로 받으면 디코드 실패한다.
        assert_eq!(f.next_funding_time, "1780560000000");
    }

    #[test]
    fn ticker_parses_terse_keys() {
        let v = serde_json::json!([{
            "t": 1780548069421i64,
            "s": "SAMSUNG-SWAP-USDT",
            "c": "235.41",
            "h": "259.71",
            "l": "230.82",
            "o": "252.87",
            "b": "235.51",
            "a": "235.53",
            "v": "700139",
            "qv": "1718794.8648",
            "op": "6730",
            "pc": "-17.46",
            "pcp": "-0.069"
        }]);
        let rows: Vec<Ticker24hr> = serde_json::from_value(v).unwrap();
        let t = &rows[0];
        assert_eq!(t.symbol, "SAMSUNG-SWAP-USDT");
        assert_eq!(t.last_price, "235.41");
        assert_eq!(t.bid_price, "235.51");
        assert_eq!(t.ask_price, "235.53");
        assert_eq!(t.time, 1780548069421);
    }

    #[test]
    fn depth_parses_b_a_pairs() {
        let v = serde_json::json!({
            "t": 1780547791899i64,
            "b": [["234.98", "388"], ["234.97", "706"]],
            "a": [["235", "199"], ["235.01", "166"]]
        });
        let d: Depth = serde_json::from_value(v).unwrap();
        assert_eq!(d.bids[0][0], "234.98");
        assert_eq!(d.bids[0][1], "388");
        assert_eq!(d.asks[0][0], "235");
    }

    #[test]
    fn contract_info_parses_string_precision_and_filters() {
        let v = serde_json::json!({
            "symbol": "SAMSUNG-SWAP-USDT",
            "status": "TRADING",
            "baseAsset": "SAMSUNG-SWAP-USDT",
            "quoteAsset": "USDT",
            "underlying": "SAMSUNG",
            "contractMultiplier": "0.01",
            "marginToken": "USDT",
            "quoteAssetPrecision": "0.01",
            "filters": [
                {"minPrice": "0.01", "maxPrice": "10000000.00000000", "tickSize": "0.01", "filterType": "PRICE_FILTER"},
                {"minQty": "0.02", "maxQty": "20000", "stepSize": "0.01", "filterType": "LOT_SIZE"}
            ]
        });
        let c: ContractInfo = serde_json::from_value(v).unwrap();
        assert_eq!(c.symbol, "SAMSUNG-SWAP-USDT");
        assert_eq!(c.status, "TRADING");
        assert_eq!(c.underlying, "SAMSUNG");
        // precision은 정수가 아니라 틱 문자열.
        assert_eq!(c.quote_asset_precision, "0.01");
        assert_eq!(c.filters[0].filter_type, "PRICE_FILTER");
        assert_eq!(c.filters[0].tick_size.as_deref(), Some("0.01"));
        assert_eq!(c.filters[1].step_size.as_deref(), Some("0.01"));
    }
}
