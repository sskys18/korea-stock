//! 시세 도메인 (키 불필요) — 티커(마크가·펀딩·호가1)·호가창·상품정보.
//!
//! Phemex Perpetual v2 공개 엔드포인트. `Rp`/`Rq`/`Rr` suffix 필드는 모두 **real value**
//! 평문 문자열(스케일 정수 Ep/Ev 아님). 가격·수량은 정밀도 보존 위해 String.

use serde::Deserialize;

use crate::global::phemex::client::{ApiCall, PhemexClient};
use crate::global::phemex::error::{PhemexError, Result};

/// 24시간 티커 (`GET /md/v3/ticker/24hr?symbol=...`). v2 perp 전용 엔드포인트.
///
/// 마크가·지수가·펀딩비·1호가(bid/ask)·최근가를 한 번에 담는다. KR 종목 시세 probe의
/// 핵심 소스다(별도 호가창 호출 없이 last/mark/funding/bid/ask 전부 획득).
#[derive(Debug, Clone, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    /// 최근 체결가 (real). 일부 응답은 `closeRp`만 줄 수 있어 fallback 처리.
    #[serde(rename = "lastRp", default)]
    pub last_rp: String,
    /// 종가(=최근가) (real). v2 ticker 변종 호환용.
    #[serde(rename = "closeRp", default)]
    pub close_rp: String,
    /// 마크가 (real).
    #[serde(rename = "markRp", default)]
    pub mark_rp: String,
    /// 지수가 (real).
    #[serde(rename = "indexRp", default)]
    pub index_rp: String,
    /// 1호가 매수 (real).
    #[serde(rename = "bidRp", default)]
    pub bid_rp: String,
    /// 1호가 매도 (real).
    #[serde(rename = "askRp", default)]
    pub ask_rp: String,
    /// 직전 펀딩비율 (real ratio, 예 "0.00005" = 0.005%).
    #[serde(rename = "fundingRateRr", default)]
    pub funding_rate_rr: String,
    /// 예상 다음 펀딩비율 (real ratio).
    #[serde(rename = "predFundingRateRr", default)]
    pub pred_funding_rate_rr: String,
    /// 24h 고가 (real).
    #[serde(rename = "highRp", default)]
    pub high_rp: String,
    /// 24h 저가 (real).
    #[serde(rename = "lowRp", default)]
    pub low_rp: String,
    /// 24h 거래량 (real, 계약수).
    #[serde(rename = "volumeRq", default)]
    pub volume_rq: String,
    /// 미결제약정 (real value).
    #[serde(rename = "openInterestRv", default)]
    pub open_interest_rv: String,
    /// 시세 시각 (epoch ns).
    #[serde(default)]
    pub timestamp: i64,
}

impl Ticker {
    /// 사람이 읽는 최근가. `lastRp`가 비어 있으면 `closeRp`로 폴백.
    pub fn last(&self) -> &str {
        if !self.last_rp.is_empty() {
            &self.last_rp
        } else {
            &self.close_rp
        }
    }
}

/// 호가창 한쪽(매수/매도) 1개 레벨: `[price, qty]` real 문자열 쌍.
pub type Level = [String; 2];

/// 호가창 내부 본문 (`result.orderbook_p`). `_p` = plain(real) 가격.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBookInner {
    /// 매수호가 `[가격, 잔량]`.
    #[serde(default)]
    pub bids: Vec<Level>,
    /// 매도호가 `[가격, 잔량]`.
    #[serde(default)]
    pub asks: Vec<Level>,
}

/// 호가창 (`GET /md/v2/orderbook?symbol=...`).
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBook {
    pub symbol: String,
    /// 호가 깊이.
    #[serde(default)]
    pub depth: i64,
    /// real 가격 호가창.
    #[serde(rename = "orderbook_p")]
    pub orderbook_p: OrderBookInner,
    /// 시세 시각 (epoch ns).
    #[serde(default)]
    pub timestamp: i64,
}

impl OrderBook {
    /// 최우선 매수호가 가격.
    pub fn best_bid(&self) -> Option<&str> {
        self.orderbook_p.bids.first().map(|l| l[0].as_str())
    }
    /// 최우선 매도호가 가격.
    pub fn best_ask(&self) -> Option<&str> {
        self.orderbook_p.asks.first().map(|l| l[0].as_str())
    }
}

/// 상품(계약) 정보 1건 (`GET /public/products` → `perpProductsV2[]`).
#[derive(Debug, Clone, Deserialize)]
pub struct Product {
    pub symbol: String,
    /// "PerpetualV2" 등.
    #[serde(rename = "type", default)]
    pub product_type: String,
    /// "Listed" / "Delisted" 등.
    #[serde(default)]
    pub status: String,
    #[serde(rename = "settleCurrency", default)]
    pub settle_currency: String,
    #[serde(rename = "quoteCurrency", default)]
    pub quote_currency: String,
    /// 가격 스케일. v2 perp은 0(평문 가격).
    #[serde(rename = "priceScale", default)]
    pub price_scale: i64,
    /// 최소 호가 단위 (real).
    #[serde(rename = "tickSize", default)]
    pub tick_size: String,
    /// 최소 수량 단위 (real).
    #[serde(rename = "qtyStepSize", default)]
    pub qty_step_size: String,
}

#[derive(Deserialize)]
struct ProductsEnvelope {
    #[serde(rename = "perpProductsV2", default)]
    perp_products_v2: Vec<Product>,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a PhemexClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a PhemexClient) -> Self {
        Self { client }
    }

    /// 24h 티커 조회 (마크가·펀딩·1호가·최근가 포함).
    pub async fn ticker(&self, symbol: &str) -> Result<Ticker> {
        self.client
            .call(ApiCall::public_get(
                "/md/v3/ticker/24hr",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 호가창 조회 (`/md/v2/orderbook`).
    pub async fn orderbook(&self, symbol: &str) -> Result<OrderBook> {
        self.client
            .call(ApiCall::public_get(
                "/md/v2/orderbook",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 전체 v2 perp 상품 목록. KR 종목만 보려면 [`crate::global::phemex::KR_SYMBOLS`]로 필터.
    pub async fn contracts(&self) -> Result<Vec<Product>> {
        let env: ProductsEnvelope = self
            .client
            .call(ApiCall::public_get("/public/products", vec![]))
            .await?
            .parse()?;
        Ok(env.perp_products_v2)
    }

    /// 단일 심볼 상품 정보 (없으면 [`PhemexError::Decode`]).
    pub async fn contract(&self, symbol: &str) -> Result<Product> {
        self.contracts()
            .await?
            .into_iter()
            .find(|p| p.symbol == symbol)
            .ok_or_else(|| PhemexError::Decode(format!("symbol not found: {symbol}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_parses_real_value_fields() {
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDT",
            "lastRp": "235.19",
            "markRp": "235.43",
            "indexRp": "235.43",
            "bidRp": "235.3",
            "askRp": "235.48",
            "fundingRateRr": "0",
            "predFundingRateRr": "0",
            "highRp": "260",
            "lowRp": "231.23",
            "volumeRq": "2689.54",
            "openInterestRv": "1905.53",
            "timestamp": 1780547766439157443i64
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.symbol, "SAMSUNGUSDT");
        assert_eq!(t.last(), "235.19");
        assert_eq!(t.mark_rp, "235.43");
        assert_eq!(t.bid_rp, "235.3");
        assert_eq!(t.ask_rp, "235.48");
        assert_eq!(t.funding_rate_rr, "0");
    }

    #[test]
    fn ticker_last_falls_back_to_close() {
        // v2 변종은 lastRp 대신 closeRp를 줄 수 있다.
        let v = serde_json::json!({
            "symbol": "SKHYNIXUSDT",
            "closeRp": "1514.5",
            "markRp": "1514.28",
            "fundingRateRr": "0"
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.last(), "1514.5");
    }

    #[test]
    fn orderbook_plain_prices_best_bid_ask() {
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDT",
            "depth": 30,
            "orderbook_p": {
                "asks": [["235.23", "0.23"], ["235.24", "0.28"]],
                "bids": [["235.14", "0.66"], ["235.10", "1.2"]]
            },
            "timestamp": 1780547751411856408i64
        });
        let ob: OrderBook = serde_json::from_value(v).unwrap();
        assert_eq!(ob.best_ask(), Some("235.23"));
        assert_eq!(ob.best_bid(), Some("235.14"));
    }

    #[test]
    fn product_parses_v2_scale_zero() {
        let v = serde_json::json!({
            "symbol": "HYUNDAIUSDT",
            "type": "PerpetualV2",
            "status": "Listed",
            "settleCurrency": "USDT",
            "quoteCurrency": "USDT",
            "priceScale": 0,
            "tickSize": "0.01",
            "qtyStepSize": "0.01"
        });
        let p: Product = serde_json::from_value(v).unwrap();
        assert_eq!(p.symbol, "HYUNDAIUSDT");
        assert_eq!(p.product_type, "PerpetualV2");
        assert_eq!(p.status, "Listed");
        assert_eq!(p.price_scale, 0);
        assert_eq!(p.tick_size, "0.01");
    }
}
