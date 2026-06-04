//! 시세 도메인 (키 불필요) — 마켓 메타·호가·캔들·펀딩·거래소 통계.
//!
//! Lighter REST 공개 엔드포인트(`/api/v1/*`). 가격·수량은 String으로 보존한다. string으로
//! 오는 필드는 원문 그대로, JSON **number**로 오는 필드는 [`str_or_num`]가 f64 최단 왕복
//! 표현으로 문자열화한다 — Lighter가 이 값들을 float로 송신하므로 f64가 와이어 원본값이다.
//!
//! 라이브 응답 형태는 2026-06 기준 `mainnet.zklighter.elliot.ai`에서 확인했다.

use serde::de::{self, Deserializer};
use serde::Deserialize;
use serde_json::Value;

use crate::lighter::client::{ApiCall, LighterClient};
use crate::lighter::error::{LighterError, Result};

/// JSON number 또는 string을 String으로. number는 `Value::to_string`(f64 최단
/// 왕복 표현)을 쓴다 — Lighter는 이 수치들을 JSON float로 송신하므로 f64가 곧 와이어
/// 원본값과 동일하다(라이브에서 관측된 `5222.6927940000005`처럼 서버측 f64 산물 포함).
/// string으로 오는 필드(price·base_amount 등)는 원문을 그대로 보존한다.
fn str_or_num<'de, D>(d: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(d)? {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        Value::Null => Ok(String::new()),
        other => Err(de::Error::custom(format!("expected number or string, got {other}"))),
    }
}

/// 마켓 메타 1건 (`GET /api/v1/orderBookDetails` → order_book_details[]).
///
/// 심볼·market_id 매핑과 가격/수량 소수 자리수, 활성 상태를 담는다. KR 종목 거래 전
/// `status=="active"` 확인용.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBookDetail {
    pub symbol: String,
    pub market_id: u32,
    /// "perp" / "spot".
    pub market_type: String,
    /// "active" / "inactive".
    pub status: String,
    /// 호가 가격 소수 자리수.
    pub supported_price_decimals: u32,
    /// 주문 수량 소수 자리수.
    pub supported_size_decimals: u32,
    /// 최근 체결가 (number → String).
    #[serde(default, deserialize_with = "str_or_num")]
    pub last_trade_price: String,
    /// 미결제약정 (number → String).
    #[serde(default, deserialize_with = "str_or_num")]
    pub open_interest: String,
    /// 최소 주문 수량 (string).
    #[serde(default)]
    pub min_base_amount: String,
    /// 최소 주문 금액 (string).
    #[serde(default)]
    pub min_quote_amount: String,
}

#[derive(Deserialize)]
struct OrderBookDetailsResp {
    order_book_details: Vec<OrderBookDetail>,
}

/// 호가 1건 (`GET /api/v1/orderBookOrders` → bids[]/asks[]).
#[derive(Debug, Clone, Deserialize)]
pub struct BookOrder {
    pub order_id: String,
    /// 호가 (string).
    pub price: String,
    /// 최초 주문 수량 (string).
    pub initial_base_amount: String,
    /// 미체결 잔량 (string).
    pub remaining_base_amount: String,
    /// 주문 소유 계정 인덱스.
    pub owner_account_index: u64,
    /// 주문 만료 (epoch ms).
    #[serde(default)]
    pub order_expiry: i64,
}

/// 호가창 (`GET /api/v1/orderBookOrders`). bids/asks 각각 [`BookOrder`] 목록.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBook {
    pub total_asks: i64,
    pub total_bids: i64,
    /// 매도호가 (낮은 가격순).
    pub asks: Vec<BookOrder>,
    /// 매수호가 (높은 가격순).
    pub bids: Vec<BookOrder>,
}

/// 캔들(봉) 1건 (`GET /api/v1/candles` → c[]).
///
/// 라이브 응답은 단문자 키(t/o/h/l/c/v/V/i)에 가격을 number로 싣는다. serde rename으로
/// 매핑하고 [`str_or_num`]으로 무손실 문자열화한다.
#[derive(Debug, Clone, Deserialize)]
pub struct Candle {
    /// 봉 시작 시각 (epoch ms).
    #[serde(rename = "t")]
    pub open_time: i64,
    #[serde(rename = "o", deserialize_with = "str_or_num")]
    pub open: String,
    #[serde(rename = "h", deserialize_with = "str_or_num")]
    pub high: String,
    #[serde(rename = "l", deserialize_with = "str_or_num")]
    pub low: String,
    #[serde(rename = "c", deserialize_with = "str_or_num")]
    pub close: String,
    /// 거래량(base 토큰).
    #[serde(rename = "v", deserialize_with = "str_or_num")]
    pub base_volume: String,
    /// 거래대금(quote, USD).
    #[serde(rename = "V", deserialize_with = "str_or_num")]
    pub quote_volume: String,
}

#[derive(Deserialize)]
struct CandlesResp {
    #[serde(rename = "c")]
    candles: Vec<Candle>,
}

/// 펀딩비 현황 1건 (`GET /api/v1/funding-rates` → funding_rates[]).
///
/// Lighter는 외부 거래소(binance 등) 기준 펀딩율을 마켓별로 노출한다. `rate`는 number.
#[derive(Debug, Clone, Deserialize)]
pub struct FundingRate {
    pub market_id: u32,
    pub symbol: String,
    /// 기준 거래소 (예: "binance").
    #[serde(default)]
    pub exchange: String,
    /// 펀딩율 (number → String, 정밀도 보존).
    #[serde(deserialize_with = "str_or_num")]
    pub rate: String,
}

#[derive(Deserialize)]
struct FundingRatesResp {
    funding_rates: Vec<FundingRate>,
}

/// 펀딩 정산 이력 1건 (`GET /api/v1/fundings` → fundings[]).
#[derive(Debug, Clone, Deserialize)]
pub struct FundingHistory {
    /// 정산 시각 (epoch sec).
    pub timestamp: i64,
    /// 펀딩 금액 (string).
    #[serde(default)]
    pub value: String,
    /// 펀딩율 (string).
    #[serde(default)]
    pub rate: String,
    /// "long" / "short".
    #[serde(default)]
    pub direction: String,
}

#[derive(Deserialize)]
struct FundingsResp {
    fundings: Vec<FundingHistory>,
}

/// 거래소 통계 1건 (`GET /api/v1/exchangeStats` → order_book_stats[]).
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBookStat {
    pub symbol: String,
    #[serde(default, deserialize_with = "str_or_num")]
    pub last_trade_price: String,
    pub daily_trades_count: i64,
    #[serde(default, deserialize_with = "str_or_num")]
    pub daily_base_token_volume: String,
    #[serde(default, deserialize_with = "str_or_num")]
    pub daily_quote_token_volume: String,
    #[serde(default, deserialize_with = "str_or_num")]
    pub daily_price_change: String,
}

#[derive(Deserialize)]
struct ExchangeStatsResp {
    order_book_stats: Vec<OrderBookStat>,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a LighterClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a LighterClient) -> Self {
        Self { client }
    }

    /// 전체 마켓 메타 (`GET /api/v1/orderBookDetails`, market_id 생략 시 전 마켓).
    /// KR 종목만 보려면 [`crate::lighter::KR_SYMBOLS`]로 필터.
    pub async fn order_book_details_all(&self) -> Result<Vec<OrderBookDetail>> {
        let r: OrderBookDetailsResp = self
            .client
            .call(ApiCall::get("/api/v1/orderBookDetails", vec![]))
            .await?
            .parse()?;
        Ok(r.order_book_details)
    }

    /// 단일 마켓 메타 (`market_id` 지정).
    pub async fn order_book_details(&self, market_id: u32) -> Result<OrderBookDetail> {
        let r: OrderBookDetailsResp = self
            .client
            .call(ApiCall::get(
                "/api/v1/orderBookDetails",
                vec![("market_id".into(), market_id.to_string())],
            ))
            .await?
            .parse()?;
        r.order_book_details
            .into_iter()
            .next()
            .ok_or_else(|| LighterError::Decode(format!("no market detail for id {market_id}")))
    }

    /// 심볼 → `market_id` 해석. 활성·비활성 모두 조회된다(거래 전 status 확인 권장).
    pub async fn market_id(&self, symbol: &str) -> Result<u32> {
        self.order_book_details_all()
            .await?
            .into_iter()
            .find(|d| d.symbol == symbol)
            .map(|d| d.market_id)
            .ok_or_else(|| LighterError::Decode(format!("symbol not found: {symbol}")))
    }

    /// 호가창 (`GET /api/v1/orderBookOrders`). `limit` ∈ [1,250].
    pub async fn order_book_orders(&self, market_id: u32, limit: u32) -> Result<OrderBook> {
        self.client
            .call(ApiCall::get(
                "/api/v1/orderBookOrders",
                vec![
                    ("market_id".into(), market_id.to_string()),
                    ("limit".into(), limit.to_string()),
                ],
            ))
            .await?
            .parse()
    }

    /// 캔들 조회 (`GET /api/v1/candles`). `resolution` ∈ {1m,5m,15m,1h,4h,1d 등},
    /// `start_timestamp`/`end_timestamp`는 epoch sec, `count_back` 최대 500.
    pub async fn candles(
        &self,
        market_id: u32,
        resolution: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        count_back: u32,
    ) -> Result<Vec<Candle>> {
        let r: CandlesResp = self
            .client
            .call(ApiCall::get(
                "/api/v1/candles",
                vec![
                    ("market_id".into(), market_id.to_string()),
                    ("resolution".into(), resolution.to_string()),
                    ("start_timestamp".into(), start_timestamp.to_string()),
                    ("end_timestamp".into(), end_timestamp.to_string()),
                    ("count_back".into(), count_back.to_string()),
                ],
            ))
            .await?
            .parse()?;
        Ok(r.candles)
    }

    /// 전 마켓 펀딩율 현황 (`GET /api/v1/funding-rates`).
    pub async fn funding_rates(&self) -> Result<Vec<FundingRate>> {
        let r: FundingRatesResp = self
            .client
            .call(ApiCall::get("/api/v1/funding-rates", vec![]))
            .await?
            .parse()?;
        Ok(r.funding_rates)
    }

    /// 펀딩 정산 이력 (`GET /api/v1/fundings`). `resolution` ∈ {1h,1d}, ts는 epoch sec.
    pub async fn fundings(
        &self,
        market_id: u32,
        resolution: &str,
        start_timestamp: i64,
        end_timestamp: i64,
        count_back: u32,
    ) -> Result<Vec<FundingHistory>> {
        let r: FundingsResp = self
            .client
            .call(ApiCall::get(
                "/api/v1/fundings",
                vec![
                    ("market_id".into(), market_id.to_string()),
                    ("resolution".into(), resolution.to_string()),
                    ("start_timestamp".into(), start_timestamp.to_string()),
                    ("end_timestamp".into(), end_timestamp.to_string()),
                    ("count_back".into(), count_back.to_string()),
                ],
            ))
            .await?
            .parse()?;
        Ok(r.fundings)
    }

    /// 거래소 일간 통계 (`GET /api/v1/exchangeStats`).
    pub async fn exchange_stats(&self) -> Result<Vec<OrderBookStat>> {
        let r: ExchangeStatsResp = self
            .client
            .call(ApiCall::get("/api/v1/exchangeStats", vec![]))
            .await?
            .parse()?;
        Ok(r.order_book_stats)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_book_details_parses_and_coerces_numbers() {
        // 라이브 orderBookDetails 1건 발췌 (SAMSUNGUSD market_id=162 형태).
        let v = serde_json::json!({
            "code": 200,
            "order_book_details": [{
                "symbol": "SAMSUNGUSD",
                "market_id": 162,
                "market_type": "perp",
                "status": "active",
                "supported_price_decimals": 3,
                "supported_size_decimals": 3,
                "min_base_amount": "0.001",
                "min_quote_amount": "10.000000",
                "last_trade_price": 244.045,
                "open_interest": 1127.2006
            }]
        });
        let r: OrderBookDetailsResp = serde_json::from_value(v).unwrap();
        let d = &r.order_book_details[0];
        assert_eq!(d.symbol, "SAMSUNGUSD");
        assert_eq!(d.market_id, 162);
        assert_eq!(d.status, "active");
        // number가 무손실 String으로.
        assert_eq!(d.last_trade_price, "244.045");
        assert_eq!(d.open_interest, "1127.2006");
    }

    #[test]
    fn order_book_orders_parses() {
        // 라이브 orderBookOrders 발췌.
        let v = serde_json::json!({
            "code": 200,
            "total_asks": 1,
            "asks": [{
                "order_index": 45880421206297236i64,
                "order_id": "45880421206297236",
                "owner_account_index": 281474976564115i64,
                "initial_base_amount": "22.531",
                "remaining_base_amount": "22.531",
                "price": "244.045",
                "order_expiry": 1782828566525i64,
                "transaction_time": 0
            }],
            "total_bids": 1,
            "bids": [{
                "order_index": 46161896177437644i64,
                "order_id": "46161896177437644",
                "owner_account_index": 59294,
                "initial_base_amount": "1.000",
                "remaining_base_amount": "1.000",
                "price": "242.657",
                "order_expiry": 1782828575181i64,
                "transaction_time": 0
            }]
        });
        let ob: OrderBook = serde_json::from_value(v).unwrap();
        assert_eq!(ob.total_asks, 1);
        assert_eq!(ob.asks[0].price, "244.045");
        assert_eq!(ob.asks[0].remaining_base_amount, "22.531");
        assert_eq!(ob.bids[0].price, "242.657");
        assert_eq!(ob.bids[0].owner_account_index, 59294);
    }

    #[test]
    fn candles_short_keys_map_and_coerce() {
        // 라이브 candles 발췌: {code, r, c:[{t,o,h,l,c,v,V,i}]}, 가격은 number.
        let v = serde_json::json!({
            "code": 200,
            "r": "1h",
            "c": [{
                "t": 1780300800000i64,
                "o": 231.652, "h": 233.901, "l": 231.652, "c": 232.412,
                "v": 39.612, "V": 9248.119653, "i": 21235579103i64
            }]
        });
        let r: CandlesResp = serde_json::from_value(v).unwrap();
        let k = &r.candles[0];
        assert_eq!(k.open_time, 1780300800000);
        assert_eq!(k.open, "231.652");
        assert_eq!(k.close, "232.412");
        assert_eq!(k.base_volume, "39.612");
        assert_eq!(k.quote_volume, "9248.119653");
    }

    #[test]
    fn funding_rates_coerce_number_rate() {
        // 라이브 funding-rates 발췌.
        let v = serde_json::json!({
            "code": 200,
            "funding_rates": [
                { "market_id": 162, "exchange": "binance", "symbol": "SAMSUNGUSD", "rate": 0.00019977 },
                { "market_id": 22, "exchange": "binance", "symbol": "AI16Z", "rate": 0 }
            ]
        });
        let r: FundingRatesResp = serde_json::from_value(v).unwrap();
        assert_eq!(r.funding_rates[0].symbol, "SAMSUNGUSD");
        assert_eq!(r.funding_rates[0].rate, "0.00019977");
        // 정수 0도 무손실.
        assert_eq!(r.funding_rates[1].rate, "0");
    }

    #[test]
    fn fundings_history_parses() {
        // 라이브 fundings 발췌.
        let v = serde_json::json!({
            "code": 200,
            "resolution": "1h",
            "fundings": [
                { "timestamp": 1780326000, "value": "0.00092448", "rate": "0.0004", "direction": "long" }
            ]
        });
        let r: FundingsResp = serde_json::from_value(v).unwrap();
        assert_eq!(r.fundings[0].timestamp, 1780326000);
        assert_eq!(r.fundings[0].rate, "0.0004");
        assert_eq!(r.fundings[0].direction, "long");
    }

    #[test]
    fn exchange_stats_parses() {
        // 라이브 exchangeStats 발췌.
        let v = serde_json::json!({
            "code": 200,
            "total": 2,
            "order_book_stats": [{
                "symbol": "SAMSUNGUSD",
                "last_trade_price": 244.045,
                "daily_trades_count": 2739,
                "daily_base_token_volume": 3012.2736,
                "daily_quote_token_volume": 5243361.815596,
                "daily_price_change": 0.8283665593386769
            }]
        });
        let r: ExchangeStatsResp = serde_json::from_value(v).unwrap();
        let s = &r.order_book_stats[0];
        assert_eq!(s.symbol, "SAMSUNGUSD");
        assert_eq!(s.last_trade_price, "244.045");
        assert_eq!(s.daily_trades_count, 2739);
        assert_eq!(s.daily_quote_token_volume, "5243361.815596");
    }

    #[test]
    fn str_or_num_handles_string_passthrough() {
        // string은 그대로 보존.
        let v = serde_json::json!({ "price": "1.23456789012345678" });
        #[derive(Deserialize)]
        struct P {
            #[serde(deserialize_with = "str_or_num")]
            price: String,
        }
        let p: P = serde_json::from_value(v).unwrap();
        assert_eq!(p.price, "1.23456789012345678");
    }
}
