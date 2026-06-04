//! 시세 도메인 (키 불필요) — 티커·펀딩·호가·캔들·계약정보.
//!
//! MEXC Contract(`/api/v1/contract/*`) 공개 엔드포인트. 가격·수량은 정밀도 보존 위해
//! String으로 보관한다. MEXC는 이 값들을 **JSON 숫자**(decimal/long)로 내려주므로
//! [`num_or_str`] 디시리얼라이저로 정수·문자열은 무손실 변환하고, f64만 재포맷한다
//! (그 경우의 정밀도 손실은 잔여 한계로 문서화).

use serde::de::{self, Deserializer};
use serde::Deserialize;
use serde_json::Value;

use crate::mexc::client::{ApiCall, MexcClient};
use crate::mexc::error::{MexcError, Result};

/// 숫자-or-문자열 → String (정밀도 보존). 정수/문자열은 무손실, f64는 표준 표기로 재포맷.
///
/// **잔여 한계:** 응답 필드가 JSON float로 오고 소수 자릿수가 f64 표현 한계를 넘으면
/// 마지막 자리에서 미세 오차가 날 수 있다. MEXC 가격 정밀도(소수 ~4자리)에서는 안전.
pub(crate) fn num_or_str<'de, D>(d: D) -> std::result::Result<String, D::Error>
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

/// 24시간 티커 (`GET /api/v1/contract/ticker?symbol=...` → data 단일 객체).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    pub symbol: String,
    #[serde(deserialize_with = "num_or_str")]
    pub last_price: String,
    /// 최우선 매수호가.
    #[serde(deserialize_with = "num_or_str")]
    pub bid1: String,
    /// 최우선 매도호가.
    #[serde(deserialize_with = "num_or_str")]
    pub ask1: String,
    /// 24h 거래량 (계약 수).
    #[serde(deserialize_with = "num_or_str")]
    pub volume24: String,
    /// 24h 거래대금.
    #[serde(deserialize_with = "num_or_str")]
    pub amount24: String,
    /// 미결제약정 (계약 수).
    #[serde(deserialize_with = "num_or_str")]
    pub hold_vol: String,
    /// 지수가 (현물 바스켓).
    #[serde(deserialize_with = "num_or_str")]
    pub index_price: String,
    /// 공정가/마크가 (청산·미실현손익 기준가).
    #[serde(deserialize_with = "num_or_str")]
    pub fair_price: String,
    /// 현재 펀딩비율.
    #[serde(deserialize_with = "num_or_str")]
    pub funding_rate: String,
    /// 시세 시각 (epoch ms).
    pub timestamp: i64,
}

/// 현재 펀딩비 (`GET /api/v1/contract/funding_rate/{symbol}` → data 단일 객체).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingRate {
    pub symbol: String,
    #[serde(deserialize_with = "num_or_str")]
    pub funding_rate: String,
    #[serde(deserialize_with = "num_or_str")]
    pub max_funding_rate: String,
    #[serde(deserialize_with = "num_or_str")]
    pub min_funding_rate: String,
    /// 정산 주기(시간).
    pub collect_cycle: i64,
    /// 다음 정산 시각 (epoch ms).
    pub next_settle_time: i64,
    pub timestamp: i64,
}

/// 호가 한 레벨 `[price, vol]` 또는 `[price, vol, orderCount]`. MEXC는 2원소/3원소를
/// 혼용하므로 가변 길이로 받는다(고정 배열로 받으면 역직렬화가 실패한다).
#[derive(Debug, Clone)]
pub struct DepthLevel {
    pub price: String,
    /// 잔량 (계약 수).
    pub vol: String,
    /// 해당 가격의 주문 건수 (없을 수 있음).
    pub order_count: Option<i64>,
}

/// raw 호가 레벨: 길이 가변 JSON 배열. 수동 변환.
fn parse_levels(raw: &Value) -> Result<Vec<DepthLevel>> {
    let arr = raw
        .as_array()
        .ok_or_else(|| MexcError::Decode("depth side must be an array".into()))?;
    let mut out = Vec::with_capacity(arr.len());
    for lvl in arr {
        let cells = lvl
            .as_array()
            .ok_or_else(|| MexcError::Decode("depth level must be an array".into()))?;
        let price = cells
            .first()
            .map(scalar_to_string)
            .ok_or_else(|| MexcError::Decode("depth level missing price".into()))?;
        let vol = cells
            .get(1)
            .map(scalar_to_string)
            .ok_or_else(|| MexcError::Decode("depth level missing vol".into()))?;
        let order_count = cells.get(2).and_then(Value::as_i64);
        out.push(DepthLevel {
            price,
            vol,
            order_count,
        });
    }
    Ok(out)
}

fn scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        other => other.to_string(),
    }
}

/// 호가창 (`GET /api/v1/contract/depth/{symbol}`).
#[derive(Debug, Clone)]
pub struct Depth {
    /// 매도호가 `[가격, 잔량, (건수)]` (낮은 가격순).
    pub asks: Vec<DepthLevel>,
    /// 매수호가 `[가격, 잔량, (건수)]` (높은 가격순).
    pub bids: Vec<DepthLevel>,
    pub version: i64,
    pub timestamp: i64,
}

/// depth 원본 envelope data: asks/bids가 가변 길이 배열.
#[derive(Deserialize)]
struct DepthRaw {
    asks: Value,
    bids: Value,
    #[serde(default)]
    version: i64,
    #[serde(default)]
    timestamp: i64,
}

impl DepthRaw {
    fn into_depth(self) -> Result<Depth> {
        Ok(Depth {
            asks: parse_levels(&self.asks)?,
            bids: parse_levels(&self.bids)?,
            version: self.version,
            timestamp: self.timestamp,
        })
    }
}

/// 캔들(봉) 1건. MEXC kline은 **평행 배열**(`{time:[], open:[], ...}`)로 오므로
/// 내부 [`KlineRaw`]로 받아 zip해 행 단위로 변환한다.
#[derive(Debug, Clone)]
pub struct Kline {
    /// 봉 시작 시각 (epoch **seconds** — MEXC kline은 초 단위).
    pub time: i64,
    pub open: String,
    pub close: String,
    pub high: String,
    pub low: String,
    /// 거래량 (계약 수).
    pub vol: String,
    /// 거래대금.
    pub amount: String,
}

/// kline 원본: 평행 배열. 각 Vec의 길이는 같다고 가정하고 짧은 쪽에 맞춰 zip.
#[derive(Deserialize)]
struct KlineRaw {
    #[serde(default)]
    time: Vec<i64>,
    #[serde(default)]
    open: Vec<Value>,
    #[serde(default)]
    close: Vec<Value>,
    #[serde(default)]
    high: Vec<Value>,
    #[serde(default)]
    low: Vec<Value>,
    #[serde(default)]
    vol: Vec<Value>,
    #[serde(default)]
    amount: Vec<Value>,
}

impl KlineRaw {
    fn into_klines(self) -> Vec<Kline> {
        let n = self.time.len();
        (0..n)
            .map(|i| Kline {
                time: self.time[i],
                open: self.open.get(i).map(scalar_to_string).unwrap_or_default(),
                close: self.close.get(i).map(scalar_to_string).unwrap_or_default(),
                high: self.high.get(i).map(scalar_to_string).unwrap_or_default(),
                low: self.low.get(i).map(scalar_to_string).unwrap_or_default(),
                vol: self.vol.get(i).map(scalar_to_string).unwrap_or_default(),
                amount: self.amount.get(i).map(scalar_to_string).unwrap_or_default(),
            })
            .collect()
    }
}

/// 계약(심볼) 정보 1건 (`GET /api/v1/contract/detail` → data[]).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContractDetail {
    pub symbol: String,
    /// 0=거래중(enabled). 그 외는 휴장/점검 등.
    #[serde(default)]
    pub state: i64,
    #[serde(default)]
    pub base_coin: String,
    #[serde(default)]
    pub quote_coin: String,
    /// 가격 단위(틱). String 보존.
    #[serde(default, deserialize_with = "num_or_str")]
    pub price_unit: String,
    /// 1계약당 기초자산 수량(계약 승수).
    #[serde(default, deserialize_with = "num_or_str")]
    pub contract_size: String,
    /// 최대 레버리지.
    #[serde(default)]
    pub max_leverage: i64,
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
    /// MEXC Contract kline 봉 코드(`Min1`/`Min5`/`Hour1`/`Day1` 등 PascalCase).
    fn code(self) -> &'static str {
        match self {
            KlineInterval::Min1 => "Min1",
            KlineInterval::Min5 => "Min5",
            KlineInterval::Min15 => "Min15",
            KlineInterval::Hour1 => "Hour1",
            KlineInterval::Hour4 => "Hour4",
            KlineInterval::Day1 => "Day1",
        }
    }
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a MexcClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a MexcClient) -> Self {
        Self { client }
    }

    /// 단일 심볼 티커 조회 (`GET /api/v1/contract/ticker?symbol=...`).
    pub async fn ticker(&self, symbol: &str) -> Result<Ticker> {
        self.client
            .call(ApiCall::public_get(
                "/api/v1/contract/ticker",
                serde_json::json!({ "symbol": symbol }),
            ))
            .await?
            .parse()
    }

    /// 현재 펀딩비 조회 (`GET /api/v1/contract/funding_rate/{symbol}`).
    pub async fn funding_rate(&self, symbol: &str) -> Result<FundingRate> {
        self.client
            .call(ApiCall::public_get(
                format!("/api/v1/contract/funding_rate/{symbol}"),
                Value::Object(Default::default()),
            ))
            .await?
            .parse()
    }

    /// 호가창 조회 (`GET /api/v1/contract/depth/{symbol}`). `limit`=레벨 수(None이면 서버 기본).
    pub async fn depth(&self, symbol: &str, limit: Option<u32>) -> Result<Depth> {
        let params = match limit {
            Some(l) => serde_json::json!({ "limit": l }),
            None => Value::Object(Default::default()),
        };
        let raw: DepthRaw = self
            .client
            .call(ApiCall::public_get(
                format!("/api/v1/contract/depth/{symbol}"),
                params,
            ))
            .await?
            .parse()?;
        raw.into_depth()
    }

    /// 캔들 조회 (`GET /api/v1/contract/kline/{symbol}`). `start`/`end`는 epoch **초**(선택).
    pub async fn klines(
        &self,
        symbol: &str,
        interval: KlineInterval,
        start: Option<i64>,
        end: Option<i64>,
    ) -> Result<Vec<Kline>> {
        let mut params = serde_json::Map::new();
        params.insert("interval".into(), Value::String(interval.code().into()));
        if let Some(s) = start {
            params.insert("start".into(), Value::Number(s.into()));
        }
        if let Some(e) = end {
            params.insert("end".into(), Value::Number(e.into()));
        }
        let raw: KlineRaw = self
            .client
            .call(ApiCall::public_get(
                format!("/api/v1/contract/kline/{symbol}"),
                Value::Object(params),
            ))
            .await?
            .parse()?;
        Ok(raw.into_klines())
    }

    /// 전체 계약 목록 (`GET /api/v1/contract/detail`). KR 종목 검증용
    /// — [`crate::mexc::KR_SYMBOLS`] 문자열과 `state==0`을 대조한다.
    pub async fn contracts(&self) -> Result<Vec<ContractDetail>> {
        self.client
            .call(ApiCall::public_get(
                "/api/v1/contract/detail",
                Value::Object(Default::default()),
            ))
            .await?
            .parse()
    }

    /// 단일 계약 정보 (없으면 [`MexcError::Decode`]).
    pub async fn contract(&self, symbol: &str) -> Result<ContractDetail> {
        self.contracts()
            .await?
            .into_iter()
            .find(|c| c.symbol == symbol)
            .ok_or_else(|| MexcError::Decode(format!("symbol not found: {symbol}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn interval_codes() {
        assert_eq!(KlineInterval::Min1.code(), "Min1");
        assert_eq!(KlineInterval::Hour4.code(), "Hour4");
        assert_eq!(KlineInterval::Day1.code(), "Day1");
    }

    #[test]
    fn ticker_parses_numeric_fields_to_string() {
        // MEXC는 가격을 JSON 숫자로 내려준다 — num_or_str가 String 보존해야 한다.
        let v = json!({
            "symbol": "SAMSUNG_USDT",
            "lastPrice": 65.12,
            "bid1": 65.10,
            "ask1": 65.14,
            "volume24": 35210.0,
            "amount24": 2290000.0,
            "holdVol": 120000.0,
            "indexPrice": 65.105,
            "fairPrice": 65.11,
            "fundingRate": 0.0001,
            "timestamp": 1717398000000i64
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.symbol, "SAMSUNG_USDT");
        assert_eq!(t.last_price, "65.12");
        assert_eq!(t.fair_price, "65.11");
        assert_eq!(t.funding_rate, "0.0001");
        assert_eq!(t.timestamp, 1717398000000);
    }

    #[test]
    fn ticker_accepts_string_numbers_too() {
        // 일부 게이트웨이는 문자열로 줄 수도 — 무손실 통과.
        let v = json!({
            "symbol": "SKHYNIX_USDT",
            "lastPrice": "120.50",
            "bid1": "120.49",
            "ask1": "120.51",
            "volume24": "10",
            "amount24": "1205",
            "holdVol": "500",
            "indexPrice": "120.40",
            "fairPrice": "120.45",
            "fundingRate": "-0.00005",
            "timestamp": 1717398000000i64
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.last_price, "120.50");
        assert_eq!(t.funding_rate, "-0.00005");
    }

    #[test]
    fn funding_rate_parses() {
        let v = json!({
            "symbol": "SAMSUNG_USDT",
            "fundingRate": 0.0001,
            "maxFundingRate": 0.003,
            "minFundingRate": -0.003,
            "collectCycle": 8,
            "nextSettleTime": 1717400000000i64,
            "timestamp": 1717398000000i64
        });
        let f: FundingRate = serde_json::from_value(v).unwrap();
        assert_eq!(f.funding_rate, "0.0001");
        assert_eq!(f.collect_cycle, 8);
        assert_eq!(f.next_settle_time, 1717400000000);
    }

    #[test]
    fn depth_parses_variable_length_levels() {
        // 2원소/3원소 혼용 — 고정 배열이면 실패할 케이스.
        let v = json!({
            "asks": [[65.14, 121], [65.16, 160, 4]],
            "bids": [[65.10, 179, 4], [65.08, 914]],
            "version": 123456i64,
            "timestamp": 1717398000000i64
        });
        let raw: DepthRaw = serde_json::from_value(v).unwrap();
        let d = raw.into_depth().unwrap();
        assert_eq!(d.asks[0].price, "65.14");
        assert_eq!(d.asks[0].vol, "121");
        assert_eq!(d.asks[0].order_count, None);
        assert_eq!(d.asks[1].order_count, Some(4));
        assert_eq!(d.bids[1].price, "65.08");
        assert_eq!(d.bids[1].order_count, None);
    }

    #[test]
    fn kline_parallel_arrays_zip_to_rows() {
        let v = json!({
            "time": [1609740600i64, 1609740660i64],
            "open": [33016.5, 33040.5],
            "close": [33040.5, 33055.0],
            "high": [33094.0, 33060.0],
            "low": [32995.0, 33030.0],
            "vol": [67332.0, 12000.0],
            "amount": [222515.85925, 39600.0]
        });
        let raw: KlineRaw = serde_json::from_value(v).unwrap();
        let k = raw.into_klines();
        assert_eq!(k.len(), 2);
        assert_eq!(k[0].time, 1609740600);
        assert_eq!(k[0].open, "33016.5");
        assert_eq!(k[1].close, "33055.0");
    }

    #[test]
    fn contract_detail_parses() {
        let v = json!({
            "symbol": "SAMSUNG_USDT",
            "state": 0,
            "baseCoin": "SAMSUNG",
            "quoteCoin": "USDT",
            "priceUnit": 0.01,
            "contractSize": 1.0,
            "maxLeverage": 20
        });
        let c: ContractDetail = serde_json::from_value(v).unwrap();
        assert_eq!(c.symbol, "SAMSUNG_USDT");
        assert_eq!(c.state, 0);
        assert_eq!(c.price_unit, "0.01");
        assert_eq!(c.max_leverage, 20);
    }
}
