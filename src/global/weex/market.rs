//! 시세 도메인 (키 불필요) — 티커(last/mark/index)·호가·펀딩·계약 정보.
//!
//! WEEX Contract 공개 엔드포인트(`/capi/v2/market/*`). 가격·수량은 정밀도 보존
//! 위해 String.

use serde::Deserialize;

use crate::global::weex::client::{ApiCall, WeexClient};
use crate::global::weex::error::{Result, WeexError};

/// 티커 (`GET /capi/v2/market/ticker?symbol=cmt_...`).
///
/// `last`/`best_bid`/`best_ask`/`markPrice`/`indexPrice`를 한 번에 준다 —
/// 호가 1단(bid/ask)은 별도 depth 호출 없이 이 응답으로 충족된다.
#[derive(Debug, Clone, Deserialize)]
pub struct Ticker {
    pub symbol: String,
    /// 최종 체결가.
    pub last: String,
    /// 최우선 매수호가.
    pub best_bid: String,
    /// 최우선 매도호가.
    pub best_ask: String,
    /// 마크가 (청산·미실현손익 기준가).
    #[serde(rename = "markPrice")]
    pub mark_price: String,
    /// 지수가 (현물 바스켓).
    #[serde(rename = "indexPrice")]
    pub index_price: String,
    pub high_24h: String,
    pub low_24h: String,
    /// 24시간 거래량 (계약 수).
    pub volume_24h: String,
    /// 24시간 거래대금 (USDT).
    #[serde(default)]
    pub base_volume: String,
    /// 24시간 등락률.
    #[serde(default)]
    pub price_change_percent: String,
    /// 시세 시각 (epoch ms, 문자열).
    pub timestamp: String,
}

/// 호가창 (`GET /capi/v2/market/depth?symbol=...&limit=15`).
/// 각 항목은 `[price, qty]` 문자열 쌍.
#[derive(Debug, Clone, Deserialize)]
pub struct Depth {
    /// 매수호가 `[가격, 잔량]` (높은 가격순).
    pub bids: Vec<[String; 2]>,
    /// 매도호가 `[가격, 잔량]` (낮은 가격순).
    pub asks: Vec<[String; 2]>,
}

/// 펀딩비 1건 (`GET /capi/v2/market/funding_rate` 응답 배열 원소).
///
/// **주의:** 이 엔드포인트는 `symbol` 쿼리를 무시하고 **전체 심볼 배열**을
/// 돌려준다(`symbol` 필드는 항상 null). 종목 식별자는 `baseCurrency`
/// (예 `SAMSUNG_USDT`)에 담긴다. [`Market::funding_rate`]가 클라이언트측에서
/// 필터한다.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingRate {
    /// 종목 식별자 (예 `SAMSUNG_USDT`). `symbol` 필드가 아닌 여기에 담긴다.
    pub base_currency: String,
    /// 직전 정산 펀딩비율.
    pub funding_rate: String,
    /// 펀딩 정산 주기 (분).
    pub collect_cycle: i64,
    /// 정산 시각 (epoch ms).
    pub timestamp: i64,
}

/// 계약 정보 1건 (`GET /capi/v2/market/contracts` 원소).
#[derive(Debug, Clone, Deserialize)]
pub struct Contract {
    /// 거래 심볼 (예 `cmt_samsungusdt`).
    pub symbol: String,
    /// 기초자산 코드 (예 `SAMSUNG`).
    pub underlying_index: String,
    pub quote_currency: String,
    /// 1계약당 명목수량 (예 "0.01").
    pub contract_val: String,
    /// 가격 틱 정밀도(소수 자릿수).
    pub tick_size: String,
    /// 수량 증분 정밀도(소수 자릿수).
    pub size_increment: String,
    #[serde(rename = "minLeverage")]
    pub min_leverage: i64,
    #[serde(rename = "maxLeverage")]
    pub max_leverage: i64,
    #[serde(rename = "minOrderSize")]
    pub min_order_size: String,
    #[serde(rename = "maxOrderSize")]
    pub max_order_size: String,
    #[serde(rename = "makerFeeRate")]
    pub maker_fee_rate: String,
    #[serde(rename = "takerFeeRate")]
    pub taker_fee_rate: String,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a WeexClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a WeexClient) -> Self {
        Self { client }
    }

    /// 티커 조회 (last/mark/index/bid/ask). 시세의 1차 진입점.
    pub async fn ticker(&self, symbol: &str) -> Result<Ticker> {
        self.client
            .call(ApiCall::public_get(
                "/capi/v2/market/ticker",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()
    }

    /// 호가창 조회. `limit` 유효값 예 {15,…}; `None`이면 서버 기본.
    /// **`limit=5`는 거부**(`40020 参数limit错误`)되므로 15 이상을 권장.
    pub async fn depth(&self, symbol: &str, limit: Option<u32>) -> Result<Depth> {
        let mut params = vec![("symbol".to_string(), symbol.to_string())];
        if let Some(l) = limit {
            params.push(("limit".into(), l.to_string()));
        }
        self.client
            .call(ApiCall::public_get("/capi/v2/market/depth", params))
            .await?
            .parse()
    }

    /// 전체 펀딩비 목록 (모든 심볼). [`Market::funding_rate`]가 이걸 필터한다.
    pub async fn funding_rates(&self) -> Result<Vec<FundingRate>> {
        self.client
            .call(ApiCall::public_get("/capi/v2/market/funding_rate", vec![]))
            .await?
            .parse()
    }

    /// 단일 종목 펀딩비. market-data 심볼(`cmt_samsungusdt`)을 받아 내부적으로
    /// funding 식별자(`SAMSUNG_USDT`)로 변환해 클라이언트측 필터한다.
    pub async fn funding_rate(&self, symbol: &str) -> Result<FundingRate> {
        let key = funding_key(symbol);
        self.funding_rates()
            .await?
            .into_iter()
            .find(|f| f.base_currency.eq_ignore_ascii_case(&key))
            .ok_or_else(|| WeexError::Decode(format!("funding not found: {symbol} ({key})")))
    }

    /// 전체 계약 목록. KR 종목만 보려면 [`crate::global::weex::KR_SYMBOLS`]로 필터.
    pub async fn contracts(&self) -> Result<Vec<Contract>> {
        self.client
            .call(ApiCall::public_get("/capi/v2/market/contracts", vec![]))
            .await?
            .parse()
    }

    /// 단일 계약 정보 (없으면 [`WeexError::Decode`]).
    pub async fn contract(&self, symbol: &str) -> Result<Contract> {
        self.contracts()
            .await?
            .into_iter()
            .find(|c| c.symbol == symbol)
            .ok_or_else(|| WeexError::Decode(format!("contract not found: {symbol}")))
    }
}

/// market-data 심볼(`cmt_samsungusdt`) → funding `baseCurrency`(`SAMSUNG_USDT`).
/// `cmt_` 접두어 제거 → 대문자화 → 말미 `USDT` 앞에 `_` 삽입.
fn funding_key(symbol: &str) -> String {
    let s = symbol.strip_prefix("cmt_").unwrap_or(symbol).to_ascii_uppercase();
    match s.strip_suffix("USDT") {
        Some(base) if !base.is_empty() => format!("{base}_USDT"),
        _ => s,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn funding_key_maps_market_symbol_to_base_currency() {
        assert_eq!(funding_key("cmt_samsungusdt"), "SAMSUNG_USDT");
        assert_eq!(funding_key("cmt_skhynixusdt"), "SKHYNIX_USDT");
        assert_eq!(funding_key("cmt_hyundaiusdt"), "HYUNDAI_USDT");
    }

    #[test]
    fn ticker_parses_renamed_fields() {
        let v = serde_json::json!({
            "symbol": "cmt_samsungusdt",
            "last": "234.85",
            "best_ask": "235.00",
            "best_bid": "234.70",
            "high_24h": "259.73",
            "low_24h": "230.76",
            "volume_24h": "894026.1728",
            "base_volume": "210000000",
            "markPrice": "234.90",
            "indexPrice": "234.88",
            "priceChangePercent": "-0.05",
            "timestamp": "1780548561387"
        });
        let t: Ticker = serde_json::from_value(v).unwrap();
        assert_eq!(t.last, "234.85");
        assert_eq!(t.mark_price, "234.90");
        assert_eq!(t.index_price, "234.88");
        assert_eq!(t.best_bid, "234.70");
        assert_eq!(t.best_ask, "235.00");
    }

    #[test]
    fn depth_parses_price_qty_pairs() {
        let v = serde_json::json!({
            "asks": [["234.99", "0.48"], ["235.01", "0.41"]],
            "bids": [["234.69", "0.50"]]
        });
        let d: Depth = serde_json::from_value(v).unwrap();
        assert_eq!(d.asks[0][0], "234.99");
        assert_eq!(d.asks[0][1], "0.48");
        assert_eq!(d.bids[0][0], "234.69");
    }

    #[test]
    fn funding_rate_parses_base_currency() {
        let v = serde_json::json!({
            "baseCurrency": "SAMSUNG_USDT",
            "symbol": null,
            "fundingRate": "0.00705484359",
            "collectCycle": 480,
            "timestamp": 1780531200000i64
        });
        let f: FundingRate = serde_json::from_value(v).unwrap();
        assert_eq!(f.base_currency, "SAMSUNG_USDT");
        assert_eq!(f.funding_rate, "0.00705484359");
        assert_eq!(f.collect_cycle, 480);
    }

    #[test]
    fn contract_parses() {
        let v = serde_json::json!({
            "symbol": "cmt_samsungusdt",
            "underlying_index": "SAMSUNG",
            "quote_currency": "USDT",
            "coin": "USDT",
            "contract_val": "0.01",
            "tick_size": "2",
            "size_increment": "2",
            "minLeverage": 1,
            "maxLeverage": 20,
            "minOrderSize": "0.01",
            "maxOrderSize": "40",
            "makerFeeRate": "0",
            "takerFeeRate": "0"
        });
        let c: Contract = serde_json::from_value(v).unwrap();
        assert_eq!(c.symbol, "cmt_samsungusdt");
        assert_eq!(c.underlying_index, "SAMSUNG");
        assert_eq!(c.max_leverage, 20);
        assert_eq!(c.tick_size, "2");
    }
}
