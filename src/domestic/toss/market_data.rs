//! 시세 도메인 — 호가·현재가·체결·상하한가·캔들.

use serde::Deserialize;

use crate::domestic::toss::client::{ApiCall, TossClient};
use crate::domestic::toss::error::Result;

/// 호가 1건 (매수/매도 공통). 가격·잔량은 정밀도 보존 위해 문자열.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderbookEntry {
    /// 호가.
    pub price: String,
    /// 잔량.
    pub volume: String,
}

/// 호가 조회 응답.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderbookResponse {
    /// 데이터 시각 (ISO 8601). 데이터 미제공 시 null.
    #[serde(default)]
    pub timestamp: Option<String>,
    /// 통화 코드 (KRW/USD). unknown 허용 위해 String 보존.
    pub currency: String,
    /// 매도호가 목록 (낮은 가격순).
    pub asks: Vec<OrderbookEntry>,
    /// 매수호가 목록 (높은 가격순).
    pub bids: Vec<OrderbookEntry>,
}

/// 현재가 조회 응답 (`/prices`는 배열 반환).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceResponse {
    pub symbol: String,
    /// 데이터 시각. 체결 미발생 시 null.
    #[serde(default)]
    pub timestamp: Option<String>,
    /// 현재가.
    pub last_price: String,
    pub currency: String,
}

/// 최근 체결 1건.
#[derive(Debug, Clone, Deserialize)]
pub struct Trade {
    /// 체결가.
    pub price: String,
    /// 체결 수량.
    pub volume: String,
    /// 체결 시각.
    pub timestamp: String,
    pub currency: String,
}

/// 상/하한가 조회 응답.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PriceLimitResponse {
    pub timestamp: String,
    /// 상한가. 미국 주식 등 가격제한 없는 시장에서는 null.
    #[serde(default)]
    pub upper_limit_price: Option<String>,
    /// 하한가. 미국 주식 등 가격제한 없는 시장에서는 null.
    #[serde(default)]
    pub lower_limit_price: Option<String>,
    pub currency: String,
}

/// 캔들(봉) 1건.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candle {
    /// 봉 시작 시각.
    pub timestamp: String,
    pub open_price: String,
    pub high_price: String,
    pub low_price: String,
    pub close_price: String,
    pub volume: String,
    pub currency: String,
}

/// 캔들 페이지 응답. 페이징 커서를 응답에 그대로 보존.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CandlePage {
    pub candles: Vec<Candle>,
    /// 다음 페이지 조회 시 `before` 쿼리에 그대로 전달. 마지막 페이지면 null.
    #[serde(default)]
    pub next_before: Option<String>,
}

/// 캔들 봉 단위. 요청 파라미터 — 값을 우리가 통제하므로 타입 enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandleInterval {
    /// 1분봉.
    Minute1,
    /// 1일봉.
    Day1,
}

impl CandleInterval {
    fn code(self) -> &'static str {
        match self {
            CandleInterval::Minute1 => "1m",
            CandleInterval::Day1 => "1d",
        }
    }
}

/// 캔들 조회 파라미터.
#[derive(Debug, Clone)]
pub struct CandleReq {
    pub symbol: String,
    pub interval: CandleInterval,
    /// 조회 봉 수 (1~200). None이면 서버 기본 100.
    pub count: Option<u32>,
    /// 페이지네이션 상한 (exclusive, ISO 8601). 이전 응답의 `next_before` 전달.
    pub before: Option<String>,
    /// 수정주가 적용 여부. None이면 서버 기본 true.
    pub adjusted: Option<bool>,
}

impl CandleReq {
    /// 최소 파라미터 (symbol + interval) 생성.
    pub fn new(symbol: impl Into<String>, interval: CandleInterval) -> Self {
        Self {
            symbol: symbol.into(),
            interval,
            count: None,
            before: None,
            adjusted: None,
        }
    }
}

/// 시세 도메인 액세서. `client.market_data()`로 획득.
pub struct MarketData<'a> {
    client: &'a TossClient,
}

impl<'a> MarketData<'a> {
    pub(crate) fn new(client: &'a TossClient) -> Self {
        Self { client }
    }

    /// 호가 조회.
    pub async fn orderbook(&self, symbol: &str) -> Result<OrderbookResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/orderbook".into(),
                params: serde_json::json!({ "symbol": symbol }),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }

    /// 현재가 조회. 최대 200개 심볼. (콤마 결합은 내부 처리.)
    pub async fn prices(&self, symbols: &[&str]) -> Result<Vec<PriceResponse>> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/prices".into(),
                params: serde_json::json!({ "symbols": symbols.join(",") }),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }

    /// 최근 체결 내역 조회. `count` 최대 50 (None이면 서버 기본 50).
    pub async fn trades(&self, symbol: &str, count: Option<u32>) -> Result<Vec<Trade>> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/trades".into(),
                params: serde_json::json!({ "symbol": symbol, "count": count }),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }

    /// 상/하한가 조회.
    pub async fn price_limits(&self, symbol: &str) -> Result<PriceLimitResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/price-limits".into(),
                params: serde_json::json!({ "symbol": symbol }),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }

    /// 캔들 차트 조회.
    pub async fn candles(&self, req: CandleReq) -> Result<CandlePage> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/candles".into(),
                params: serde_json::json!({
                    "symbol": req.symbol,
                    "interval": req.interval.code(),
                    "count": req.count,
                    "before": req.before,
                    "adjusted": req.adjusted,
                }),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_codes() {
        assert_eq!(CandleInterval::Minute1.code(), "1m");
        assert_eq!(CandleInterval::Day1.code(), "1d");
    }

    #[test]
    fn orderbook_deserializes_with_null_timestamp() {
        let json = serde_json::json!({
            "timestamp": null,
            "currency": "KRW",
            "asks": [{"price": "72300", "volume": "1200"}],
            "bids": [{"price": "72000", "volume": "5200"}]
        });
        let ob: OrderbookResponse = serde_json::from_value(json).unwrap();
        assert_eq!(ob.currency, "KRW");
        assert!(ob.timestamp.is_none());
        assert_eq!(ob.asks[0].price, "72300");
    }
}
