//! 시세 도메인 (키 불필요) — 거래쌍·티커·호가·펀딩.
//!
//! Bitunix Futures(`/api/v1/futures/market/*`) 공개 엔드포인트. 가격·수량은 정밀도
//! 보존 위해 String.
//!
//! **PREVIEW 심볼 주의:** KR 심볼은 현재 `symbolStatus=="PREVIEW"`라 `/tickers`에
//! 나타나지 않고 `/depth`는 빈 호가창을 반환한다. 마크가·펀딩은 [`Market::funding_rate`]
//! 만 응답한다(`markPrice` 포함).

use serde::Deserialize;

use crate::global::bitunix::client::{ApiCall, BitunixClient};
use crate::global::bitunix::error::{BitunixError, Result};

/// 거래쌍 정보 1건 (`GET /api/v1/futures/market/trading_pairs`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TradingPair {
    pub symbol: String,
    pub base: String,
    pub quote: String,
    /// 최소 주문 수량.
    pub min_trade_volume: String,
    /// 가격 소수 자릿수.
    pub quote_precision: u32,
    /// 수량 소수 자릿수.
    pub base_precision: u32,
    pub max_leverage: u32,
    pub min_leverage: u32,
    /// 심볼 상태: "OPEN"(거래중) / "PREVIEW"(상장예고) 등.
    pub symbol_status: String,
}

/// 24시간 티커 1건 (`GET /api/v1/futures/market/tickers`).
///
/// **PREVIEW 심볼은 이 목록에 포함되지 않는다.** 마크가만 필요하면 [`FundingRate`].
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    pub symbol: String,
    pub mark_price: String,
    pub last_price: String,
    pub open: String,
    pub last: String,
    /// 거래대금(quote).
    pub quote_vol: String,
    /// 거래량(base).
    pub base_vol: String,
    pub high: String,
    pub low: String,
}

/// 호가창 (`GET /api/v1/futures/market/depth`). 각 항목 `[price, qty]` 문자열 쌍.
///
/// Binance와 달리 `lastUpdateId` 필드가 없다. PREVIEW 심볼은 빈 배열을 반환한다.
#[derive(Debug, Clone, Deserialize)]
pub struct Depth {
    /// 매도호가 `[가격, 잔량]` (낮은 가격순).
    pub asks: Vec<[String; 2]>,
    /// 매수호가 `[가격, 잔량]` (높은 가격순).
    pub bids: Vec<[String; 2]>,
}

/// 펀딩비·마크가 (`GET /api/v1/futures/market/funding_rate`).
///
/// **PREVIEW 심볼도 응답하는 유일한 시세 엔드포인트** — `markPrice`로 KR 마크가를
/// 얻는다. `nextFundingTime`은 따옴표 묶인 **문자열**(epoch ms), `fundingInterval`은
/// 맨 정수다(라이브 응답 형태 그대로 모델링).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingRate {
    pub symbol: String,
    /// 마크가 (청산·미실현손익 기준가).
    pub mark_price: String,
    /// 최근 체결가. PREVIEW로 미체결이면 "0".
    pub last_price: String,
    /// 현재 펀딩비율. PREVIEW면 "0".
    pub funding_rate: String,
    /// 펀딩 주기 (시간 단위, 예: 8).
    pub funding_interval: i64,
    /// 다음 펀딩 정산 시각 (epoch ms, **문자열**).
    pub next_funding_time: String,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a BitunixClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a BitunixClient) -> Self {
        Self { client }
    }

    /// 전체 거래쌍 목록. KR 종목만 보려면 [`crate::global::bitunix::KR_SYMBOLS`]로 필터.
    pub async fn trading_pairs(&self) -> Result<Vec<TradingPair>> {
        self.client
            .call(ApiCall::public_get(
                "/api/v1/futures/market/trading_pairs",
                serde_json::json!({}),
            ))
            .await?
            .parse()
    }

    /// 단일 거래쌍 정보 (없으면 [`BitunixError::Decode`]).
    pub async fn trading_pair(&self, symbol: &str) -> Result<TradingPair> {
        self.trading_pairs()
            .await?
            .into_iter()
            .find(|p| p.symbol == symbol)
            .ok_or_else(|| BitunixError::Decode(format!("symbol not found: {symbol}")))
    }

    /// 전체 티커 목록. **PREVIEW 심볼은 미포함.**
    pub async fn tickers(&self) -> Result<Vec<Ticker>> {
        self.client
            .call(ApiCall::public_get(
                "/api/v1/futures/market/tickers",
                serde_json::json!({}),
            ))
            .await?
            .parse()
    }

    /// 단일 심볼 티커. PREVIEW 심볼은 응답에 없어 [`BitunixError::Decode`].
    pub async fn ticker(&self, symbol: &str) -> Result<Ticker> {
        let list: Vec<Ticker> = self
            .client
            .call(ApiCall::public_get(
                "/api/v1/futures/market/tickers",
                serde_json::json!({ "symbols": symbol }),
            ))
            .await?
            .parse()?;
        list.into_iter()
            .find(|t| t.symbol == symbol)
            .ok_or_else(|| BitunixError::Decode(format!("ticker not found (PREVIEW?): {symbol}")))
    }

    /// 호가창 조회. `limit` ∈ {1,5,15,50,...} (None이면 서버 기본). PREVIEW는 빈 배열.
    pub async fn depth(&self, symbol: &str, limit: Option<u32>) -> Result<Depth> {
        let mut params = serde_json::json!({ "symbol": symbol });
        if let Some(l) = limit {
            params["limit"] = serde_json::json!(l.to_string());
        }
        self.client
            .call(ApiCall::public_get(
                "/api/v1/futures/market/depth",
                params,
            ))
            .await?
            .parse()
    }

    /// 펀딩비·마크가 조회. **PREVIEW 심볼도 응답** — KR 마크가는 여기서 얻는다.
    pub async fn funding_rate(&self, symbol: &str) -> Result<FundingRate> {
        self.client
            .call(ApiCall::public_get(
                "/api/v1/futures/market/funding_rate",
                serde_json::json!({ "symbol": symbol }),
            ))
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trading_pair_parses_preview_status() {
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDT",
            "base": "SAMSUNG",
            "quote": "USDT",
            "minTradeVolume": "0.04",
            "minBuyPriceOffset": null,
            "maxSellPriceOffset": null,
            "basePrecision": 2,
            "quotePrecision": 2,
            "maxLeverage": 50,
            "minLeverage": 1,
            "defaultLeverage": 20,
            "symbolStatus": "PREVIEW"
        });
        let p: TradingPair = serde_json::from_value(v).unwrap();
        assert_eq!(p.symbol, "SAMSUNGUSDT");
        assert_eq!(p.base, "SAMSUNG");
        assert_eq!(p.symbol_status, "PREVIEW");
        assert_eq!(p.max_leverage, 50);
    }

    #[test]
    fn funding_rate_parses_string_next_funding_time() {
        // 라이브 응답 형태: nextFundingTime은 문자열, fundingInterval은 정수.
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDT",
            "markPrice": "235.1",
            "lastPrice": "0",
            "fundingRate": "0",
            "fundingInterval": 8,
            "nextFundingTime": "1780547730814"
        });
        let f: FundingRate = serde_json::from_value(v).unwrap();
        assert_eq!(f.symbol, "SAMSUNGUSDT");
        assert_eq!(f.mark_price, "235.1");
        assert_eq!(f.funding_interval, 8);
        assert_eq!(f.next_funding_time, "1780547730814");
    }

    #[test]
    fn depth_parses_without_last_update_id() {
        let v = serde_json::json!({
            "asks": [["64290.4", "3.0951"], ["64290.5", "2.1061"]],
            "bids": [["64290.3", "1.9784"]]
        });
        let d: Depth = serde_json::from_value(v).unwrap();
        assert_eq!(d.asks[0][0], "64290.4");
        assert_eq!(d.bids[0][1], "1.9784");
    }

    #[test]
    fn depth_parses_empty_preview_book() {
        let v = serde_json::json!({ "asks": [], "bids": [] });
        let d: Depth = serde_json::from_value(v).unwrap();
        assert!(d.asks.is_empty());
        assert!(d.bids.is_empty());
    }

    #[test]
    fn ticker_parses() {
        let v = serde_json::json!({
            "symbol": "BTCUSDT",
            "markPrice": "64274.5",
            "lastPrice": "64274.3",
            "open": "66436.2",
            "last": "64287.3",
            "quoteVol": "4537646470.8006",
            "baseVol": "69774.5165",
            "high": "67484.6",
            "low": "61351.7"
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.symbol, "BTCUSDT");
        assert_eq!(t.mark_price, "64274.5");
        assert_eq!(t.base_vol, "69774.5165");
    }
}
