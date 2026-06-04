//! 시세 도메인 (키 불필요) — 티커(마크/지수/펀딩/최우선호가)·호가창·계약정보.
//!
//! Bitget v2(`/api/v2/mix/market/*`) 공개 엔드포인트, `productType=USDT-FUTURES`.
//! 가격·수량은 정밀도 보존 위해 모두 **String**으로 보관한다 — 단, **호가창
//! (merge-depth)만은 Bitget이 가격/수량을 JSON 숫자로 내려주므로**(티커는 문자열)
//! 커스텀 디시리얼라이저로 숫자/문자열을 모두 받아 String으로 정규화한다.

use serde::de::{self, Deserializer, SeqAccess, Visitor};
use serde::Deserialize;
use std::fmt;

use crate::global::bitget::client::{ApiCall, BitgetClient};
use crate::global::bitget::error::{BitgetError, Result};

/// 선형 티커 (`GET /api/v2/mix/market/ticker?symbol=...&productType=USDT-FUTURES`).
///
/// 마크가·지수가·펀딩비·최우선 매수/매도 호가를 한 번에 담는다 — KR perp 시세 프로브는
/// 이 한 호출로 충분하다(별도 호가창 호출 불필요). 모든 값은 Bitget이 문자열로 내려준다.
#[derive(Debug, Clone, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    /// 최종 체결가.
    #[serde(rename = "lastPr")]
    pub last_price: String,
    /// 최우선 매도호가.
    #[serde(rename = "askPr", default)]
    pub ask_price: String,
    /// 최우선 매수호가.
    #[serde(rename = "bidPr", default)]
    pub bid_price: String,
    /// 최우선 매수 잔량.
    #[serde(rename = "bidSz", default)]
    pub bid_size: String,
    /// 최우선 매도 잔량.
    #[serde(rename = "askSz", default)]
    pub ask_size: String,
    /// 지수가 (현물 바스켓).
    #[serde(rename = "indexPrice", default)]
    pub index_price: String,
    /// 마크가 (청산·미실현손익 기준가).
    #[serde(rename = "markPrice", default)]
    pub mark_price: String,
    /// 현재 펀딩비율 (예: "0.001" = 0.1%).
    #[serde(rename = "fundingRate", default)]
    pub funding_rate: String,
    /// 24h 거래량 (base coin 수량).
    #[serde(rename = "baseVolume", default)]
    pub base_volume: String,
    /// 24h 거래대금 (USDT).
    #[serde(rename = "quoteVolume", default)]
    pub quote_volume: String,
    /// 미결제약정 (base coin 수량).
    #[serde(rename = "holdingAmount", default)]
    pub holding_amount: String,
    /// 응답 생성 시각 (epoch ms, 문자열).
    #[serde(default)]
    pub ts: String,
}

/// 호가 레벨 `[가격, 수량]`. **Bitget merge-depth는 숫자로 내려주므로** 커스텀
/// 디시리얼라이저로 숫자/문자열 모두 받아 정밀도 보존을 위해 String으로 보관한다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Level {
    pub price: String,
    pub size: String,
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct LevelVisitor;

        impl<'de> Visitor<'de> for LevelVisitor {
            type Value = Level;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a [price, size] array of numbers or strings")
            }

            fn visit_seq<A>(self, mut seq: A) -> std::result::Result<Level, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let price: NumOrStr = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let size: NumOrStr = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                Ok(Level {
                    price: price.0,
                    size: size.0,
                })
            }
        }

        deserializer.deserialize_seq(LevelVisitor)
    }
}

/// JSON 숫자 또는 문자열을 정밀도 보존 String으로 받는다(`serde_json`의 숫자 텍스트
/// 표현을 그대로 사용 — `arbitrary_precision` 비활성 시 f64 왕복이지만 호가 정밀도엔 충분).
struct NumOrStr(String);

impl<'de> Deserialize<'de> for NumOrStr {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct V;
        impl Visitor<'_> for V {
            type Value = NumOrStr;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("number or string")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> std::result::Result<NumOrStr, E> {
                Ok(NumOrStr(v.to_string()))
            }
            fn visit_f64<E: de::Error>(self, v: f64) -> std::result::Result<NumOrStr, E> {
                Ok(NumOrStr(v.to_string()))
            }
            fn visit_u64<E: de::Error>(self, v: u64) -> std::result::Result<NumOrStr, E> {
                Ok(NumOrStr(v.to_string()))
            }
            fn visit_i64<E: de::Error>(self, v: i64) -> std::result::Result<NumOrStr, E> {
                Ok(NumOrStr(v.to_string()))
            }
        }
        deserializer.deserialize_any(V)
    }
}

/// 호가창 (`GET /api/v2/mix/market/merge-depth?symbol=...&productType=...&limit=...`).
#[derive(Debug, Clone, Deserialize)]
pub struct OrderBook {
    /// 매수호가 `[가격, 잔량]` (높은 가격순).
    #[serde(default)]
    pub bids: Vec<Level>,
    /// 매도호가 `[가격, 잔량]` (낮은 가격순).
    #[serde(default)]
    pub asks: Vec<Level>,
    /// 생성 시각 (epoch ms, 문자열).
    #[serde(default)]
    pub ts: String,
    /// 병합 정밀도 스케일(예: "0.01").
    #[serde(default)]
    pub scale: String,
}

/// 계약(심볼) 정보 1건 (`GET /api/v2/mix/market/contracts?productType=USDT-FUTURES` → data[]).
#[derive(Debug, Clone, Deserialize)]
pub struct Contract {
    pub symbol: String,
    #[serde(rename = "baseCoin")]
    pub base_coin: String,
    #[serde(rename = "quoteCoin")]
    pub quote_coin: String,
    /// "perpetual" 등.
    #[serde(rename = "symbolType", default)]
    pub symbol_type: String,
    /// "normal" / "maintain" 등.
    #[serde(rename = "symbolStatus", default)]
    pub symbol_status: String,
    /// 최소 주문 수량.
    #[serde(rename = "minTradeNum", default)]
    pub min_trade_num: String,
    /// 가격 소수 자릿수.
    #[serde(rename = "pricePlace", default)]
    pub price_place: String,
    /// 수량 소수 자릿수.
    #[serde(rename = "volumePlace", default)]
    pub volume_place: String,
    /// 최소 주문 명목가(USDT).
    #[serde(rename = "minTradeUSDT", default)]
    pub min_trade_usdt: String,
    /// 펀딩 정산 주기(시간).
    #[serde(rename = "fundInterval", default)]
    pub fund_interval: String,
    /// 최소 레버리지.
    #[serde(rename = "minLever", default)]
    pub min_lever: String,
    /// 최대 레버리지.
    #[serde(rename = "maxLever", default)]
    pub max_lever: String,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a BitgetClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a BitgetClient) -> Self {
        Self { client }
    }

    /// 티커 조회 — 마크가·지수가·펀딩비·최우선호가를 한 번에. data는 단건 배열.
    pub async fn ticker(&self, symbol: &str) -> Result<Ticker> {
        let list: Vec<Ticker> = self
            .client
            .call(ApiCall::public_get(
                "/api/v2/mix/market/ticker",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("productType".into(), "USDT-FUTURES".into()),
                ],
            ))
            .await?
            .parse()?;
        list.into_iter()
            .next()
            .ok_or_else(|| BitgetError::Decode(format!("ticker not found: {symbol}")))
    }

    /// 호가창 조회. `limit` ∈ {1,5,15,50,max} (None이면 서버 기본).
    pub async fn orderbook(&self, symbol: &str, limit: Option<u32>) -> Result<OrderBook> {
        let mut params = vec![
            ("symbol".to_string(), symbol.to_string()),
            ("productType".to_string(), "USDT-FUTURES".to_string()),
        ];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        self.client
            .call(ApiCall::public_get("/api/v2/mix/market/merge-depth", params))
            .await?
            .parse()
    }

    /// 전체 USDT-FUTURES 계약 목록. KR 종목만 보려면
    /// [`crate::global::bitget::KR_SYMBOLS`]로 필터.
    pub async fn contracts(&self) -> Result<Vec<Contract>> {
        self.client
            .call(ApiCall::public_get(
                "/api/v2/mix/market/contracts",
                vec![("productType".into(), "USDT-FUTURES".into())],
            ))
            .await?
            .parse()
    }

    /// 단일 심볼 계약정보 조회 (없으면 [`BitgetError::Decode`]).
    pub async fn contract(&self, symbol: &str) -> Result<Contract> {
        let list: Vec<Contract> = self
            .client
            .call(ApiCall::public_get(
                "/api/v2/mix/market/contracts",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("productType".into(), "USDT-FUTURES".into()),
                ],
            ))
            .await?
            .parse()?;
        list.into_iter()
            .find(|c| c.symbol == symbol)
            .ok_or_else(|| BitgetError::Decode(format!("symbol not found: {symbol}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ticker_parses_mark_funding_bidask() {
        let v = serde_json::json!({
            "symbol": "SAMSUNGUSDT",
            "lastPr": "234.17",
            "askPr": "234.17",
            "bidPr": "234.12",
            "bidSz": "1.24",
            "askSz": "2.92",
            "high24h": "259.7",
            "low24h": "229.56",
            "ts": "1780550145615",
            "indexPrice": "233.2388144154439996",
            "fundingRate": "0.001",
            "holdingAmount": "6056.89",
            "baseVolume": "13401.86",
            "quoteVolume": "3304497.7941",
            "markPrice": "234.06"
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.symbol, "SAMSUNGUSDT");
        assert_eq!(t.mark_price, "234.06");
        assert_eq!(t.funding_rate, "0.001");
        assert_eq!(t.bid_price, "234.12");
        assert_eq!(t.ask_price, "234.17");
        assert_eq!(t.last_price, "234.17");
        assert_eq!(t.index_price, "233.2388144154439996");
    }

    #[test]
    fn orderbook_parses_numeric_levels_to_string() {
        // Bitget merge-depth는 가격/수량을 JSON **숫자**로 내려준다 — String으로 정규화.
        let v = serde_json::json!({
            "asks": [[234.17, 2.92], [234.21, 0.5]],
            "bids": [[234.12, 1.24], [234.0, 4.03]],
            "ts": "1780550146787",
            "scale": "0.01",
            "precision": "scale0",
            "isMaxPrecision": "NO"
        });
        let ob: OrderBook = serde_json::from_value(v).unwrap();
        assert_eq!(ob.asks[0], Level { price: "234.17".into(), size: "2.92".into() });
        assert_eq!(ob.bids[0].price, "234.12");
        assert_eq!(ob.bids[0].size, "1.24");
        // 234.0 → "234" (f64 텍스트 표현). 정수 가격 레벨도 파싱됨을 확인.
        assert_eq!(ob.bids[1].price, "234");
        assert_eq!(ob.scale, "0.01");
    }

    #[test]
    fn orderbook_also_accepts_string_levels() {
        // 문자열 레벨도 동일하게 받아야 한다(엔드포인트 변형 대비).
        let v = serde_json::json!({
            "asks": [["234.17", "2.92"]],
            "bids": [["234.12", "1.24"]],
            "ts": "1",
            "scale": "0.01"
        });
        let ob: OrderBook = serde_json::from_value(v).unwrap();
        assert_eq!(ob.asks[0].price, "234.17");
        assert_eq!(ob.bids[0].size, "1.24");
    }

    #[test]
    fn contract_parses_kr_fields() {
        let v = serde_json::json!({
            "symbol": "SKHYNIXUSDT",
            "baseCoin": "SKHYNIX",
            "quoteCoin": "USDT",
            "minTradeNum": "0.01",
            "pricePlace": "2",
            "volumePlace": "2",
            "symbolType": "perpetual",
            "minTradeUSDT": "5",
            "maxLever": "20",
            "minLever": "1",
            "fundInterval": "8",
            "symbolStatus": "normal"
        });
        let c: Contract = serde_json::from_value(v).unwrap();
        assert_eq!(c.symbol, "SKHYNIXUSDT");
        assert_eq!(c.base_coin, "SKHYNIX");
        assert_eq!(c.max_lever, "20");
        assert_eq!(c.symbol_status, "normal");
        assert_eq!(c.fund_interval, "8");
    }
}
