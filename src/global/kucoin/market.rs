//! 시세 도메인 (키 불필요) — 계약정보(마크/지수/펀딩/틱)·티커(최우선호가)·호가창.
//!
//! KuCoin Futures(`/api/v1/*`) 공개 엔드포인트. KuCoin은 마크가·지수가·펀딩비·계약
//! 메타를 **계약 상세**(`/contracts/{symbol}`)에, 최우선 호가를 **티커**(`/ticker`)에
//! 담는다(Bybit처럼 한 호출에 모이지 않는다). 가격·수량은 정밀도 보존을 위해 모두
//! String으로 보관하되, KuCoin이 일부를 JSON **숫자**로 내려주므로
//! [`num_or_str`]/[`level`] 디시리얼라이저로 받아 String화한다(부동소수 손실 방지).

use serde::de::{self, Deserializer};
use serde::Deserialize;
use serde_json::Value;

use crate::global::kucoin::client::{ApiCall, KucoinClient};
use crate::global::kucoin::error::Result;

/// JSON 숫자 **또는** 문자열을 정밀도 보존 String으로 받는다. KuCoin은 markPrice·
/// fundingFeeRate 등을 number로, bestBidPrice 등을 string으로 내려준다(혼재).
fn num_or_str<'de, D>(de: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(de)? {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        Value::Null => Ok(String::new()),
        other => Err(de::Error::custom(format!("expected number or string, got {other}"))),
    }
}

/// 호가창 레벨 `[가격, 잔량]`. KuCoin depth는 둘 다 number로 내려주므로 String 2원소로
/// 변환해 정밀도를 보존한다.
fn levels<'de, D>(de: D) -> std::result::Result<Vec<[String; 2]>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: Vec<[Value; 2]> = Vec::deserialize(de)?;
    let mut out = Vec::with_capacity(raw.len());
    for [p, q] in raw {
        let to_s = |v: Value| match v {
            Value::String(s) => s,
            Value::Number(n) => n.to_string(),
            _ => String::new(),
        };
        out.push([to_s(p), to_s(q)]);
    }
    Ok(out)
}

/// 계약(심볼) 정보 (`GET /api/v1/contracts/{symbol}` 또는 `/contracts/active` 항목 1건).
///
/// 마크가·지수가·펀딩비·계약 메타(승수·틱·로트)를 담는다 — KR perp 시세 프로브의
/// 마크/펀딩 출처. KuCoin은 이 값들을 대부분 JSON **숫자**로 내려준다.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Contract {
    pub symbol: String,
    /// "Open"(거래중) / "Pause" / "Close" 등.
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub base_currency: String,
    #[serde(default)]
    pub quote_currency: String,
    #[serde(default)]
    pub settle_currency: String,
    /// 마크가 (청산·미실현손익 기준가).
    #[serde(deserialize_with = "num_or_str", default)]
    pub mark_price: String,
    /// 지수가 (현물 바스켓).
    #[serde(deserialize_with = "num_or_str", default)]
    pub index_price: String,
    /// 현재 펀딩비율 (예: 0.004285).
    #[serde(deserialize_with = "num_or_str", default)]
    pub funding_fee_rate: String,
    /// 계약 승수 (1 lot = multiplier 단위 자산).
    #[serde(deserialize_with = "num_or_str", default)]
    pub multiplier: String,
    /// 호가 단위.
    #[serde(deserialize_with = "num_or_str", default)]
    pub tick_size: String,
    /// 수량 단위 (lots).
    #[serde(deserialize_with = "num_or_str", default)]
    pub lot_size: String,
    /// 최대 주문 수량 (lots).
    #[serde(deserialize_with = "num_or_str", default)]
    pub max_order_qty: String,
    /// 다음 펀딩 정산까지 남은 시간(ms) 또는 시각 — KuCoin 버전에 따라 의미 상이.
    #[serde(deserialize_with = "num_or_str", default)]
    pub next_funding_rate_time: String,
}

/// 티커 (`GET /api/v1/ticker?symbol=...`). 최우선 매수/매도 호가·최종 체결가.
///
/// 마크가·펀딩비는 여기에 없다 — [`Contract`](계약 상세)에서 가져온다.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    pub symbol: String,
    /// 최종 체결가 (문자열).
    #[serde(deserialize_with = "num_or_str", default)]
    pub price: String,
    /// 최우선 매수호가 (문자열).
    #[serde(deserialize_with = "num_or_str", default)]
    pub best_bid_price: String,
    /// 최우선 매수 잔량 (lots, 숫자→문자열).
    #[serde(deserialize_with = "num_or_str", default)]
    pub best_bid_size: String,
    /// 최우선 매도호가 (문자열).
    #[serde(deserialize_with = "num_or_str", default)]
    pub best_ask_price: String,
    /// 최우선 매도 잔량 (lots, 숫자→문자열).
    #[serde(deserialize_with = "num_or_str", default)]
    pub best_ask_size: String,
    /// 체결 시각 (epoch ns).
    #[serde(default)]
    pub ts: i64,
}

/// 호가창 (`GET /api/v1/level2/depth20?symbol=...`).
///
/// **주의:** KuCoin Futures는 `depth5`를 미지원(404)하고 `depth20`만 응답한다.
/// 레벨은 `[price, size]`를 JSON **숫자** 2원소로 내려주므로 String으로 변환해 보관한다.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderBook {
    pub symbol: String,
    /// 매수호가 `[가격, 잔량]` (높은 가격순).
    #[serde(deserialize_with = "levels", default)]
    pub bids: Vec<[String; 2]>,
    /// 매도호가 `[가격, 잔량]` (낮은 가격순).
    #[serde(deserialize_with = "levels", default)]
    pub asks: Vec<[String; 2]>,
    /// 시퀀스 번호.
    #[serde(default)]
    pub sequence: i64,
    /// 생성 시각 (epoch ms).
    #[serde(default)]
    pub ts: i64,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a KucoinClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a KucoinClient) -> Self {
        Self { client }
    }

    /// 단일 심볼 계약 상세 (`GET /api/v1/contracts/{symbol}`) — 마크가·지수가·펀딩비·메타.
    pub async fn contract(&self, symbol: &str) -> Result<Contract> {
        self.client
            .call(ApiCall::public_get(
                format!("/api/v1/contracts/{symbol}"),
                vec![],
            ))
            .await?
            .parse()
    }

    /// 전체 활성 계약 목록 (`GET /api/v1/contracts/active`). KR 종목만 보려면
    /// [`crate::global::kucoin::KR_SYMBOLS`]로 필터.
    pub async fn contracts(&self) -> Result<Vec<Contract>> {
        self.client
            .call(ApiCall::public_get("/api/v1/contracts/active", vec![]))
            .await?
            .parse()
    }

    /// 티커 조회 (`GET /api/v1/ticker?symbol=...`) — 최우선 매수/매도 호가.
    pub async fn ticker(&self, symbol: &str) -> Result<Ticker> {
        self.client
            .call(ApiCall::public_get(
                "/api/v1/ticker",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 호가창 조회 (`GET /api/v1/level2/depth20?symbol=...`). KuCoin Futures는 depth5
    /// 미지원이라 20레벨 스냅샷을 쓴다.
    pub async fn orderbook(&self, symbol: &str) -> Result<OrderBook> {
        self.client
            .call(ApiCall::public_get(
                "/api/v1/level2/depth20",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_parses_numeric_mark_funding() {
        // 라이브 응답(2026-06-04) 형태: markPrice 등이 JSON 숫자.
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDTM",
            "status": "Open",
            "baseCurrency": "SAMSUNG",
            "quoteCurrency": "USDT",
            "settleCurrency": "USDT",
            "markPrice": 233.64,
            "indexPrice": 233.2,
            "fundingFeeRate": 0.004285,
            "multiplier": 0.01,
            "tickSize": 0.01,
            "lotSize": 1,
            "maxOrderQty": 100000,
            "nextFundingRateTime": 9833377
        });
        let c: Contract = serde_json::from_value(v).unwrap();
        assert_eq!(c.symbol, "SAMSUNGUSDTM");
        assert_eq!(c.status, "Open");
        // 숫자가 String으로 정밀 보존.
        assert_eq!(c.mark_price, "233.64");
        assert_eq!(c.index_price, "233.2");
        assert_eq!(c.funding_fee_rate, "0.004285");
        assert_eq!(c.tick_size, "0.01");
        assert_eq!(c.multiplier, "0.01");
    }

    #[test]
    fn ticker_parses_string_bid_numeric_size() {
        // 라이브: bestBidPrice는 문자열, bestBidSize는 숫자.
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDTM",
            "price": "233.26",
            "bestBidPrice": "233.25",
            "bestBidSize": 31,
            "bestAskPrice": "234.18",
            "bestAskSize": 44,
            "ts": 1780550154482000000i64
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.best_bid_price, "233.25");
        assert_eq!(t.best_bid_size, "31");
        assert_eq!(t.best_ask_price, "234.18");
        assert_eq!(t.best_ask_size, "44");
        assert_eq!(t.price, "233.26");
    }

    #[test]
    fn orderbook_parses_numeric_levels_as_strings() {
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDTM",
            "sequence": 1780540620220i64,
            "bids": [[233.25, 31], [233.24, 50]],
            "asks": [[234.18, 44]],
            "ts": 1780550154482i64
        });
        let ob: OrderBook = serde_json::from_value(v).unwrap();
        assert_eq!(ob.symbol, "SAMSUNGUSDTM");
        assert_eq!(ob.bids[0], ["233.25".to_string(), "31".to_string()]);
        assert_eq!(ob.asks[0], ["234.18".to_string(), "44".to_string()]);
    }
}
