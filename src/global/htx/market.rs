//! 시세 도메인 (키 불필요) — 계약정보·시세상세(merged)·호가·펀딩비.
//!
//! USDT-M Linear Swap 공개 엔드포인트. 시세 채널(`/linear-swap-ex/market/*`)은
//! `tick` 페이로드를, REST API(`/linear-swap-api/v1/*`)는 `data` 페이로드를 쓴다.
//! 가격·수량 정밀도 보존을 위해 가능한 한 String/f64 원본을 보존한다.

use serde::Deserialize;

use crate::global::htx::client::{ApiCall, HtxClient};
use crate::global::htx::error::{HtxError, Result};

/// 계약 정보 1건 (`GET /linear-swap-api/v1/swap_contract_info`).
#[derive(Debug, Clone, Deserialize)]
pub struct ContractInfo {
    /// 기초 심볼 (예: "SAMSUNG").
    pub symbol: String,
    /// 계약 코드 (예: "SAMSUNG-USDT"). API 파라미터로 쓰는 식별자.
    pub contract_code: String,
    /// 1계약 크기 (기초자산 단위).
    pub contract_size: f64,
    /// 호가 단위.
    pub price_tick: f64,
    /// 계약 상태. 1=상장(거래중). (그 외: 0 미상장, 5 정산중 등.)
    pub contract_status: i64,
    /// 마진 모드 지원 ("all"/"cross"/"isolated").
    #[serde(default)]
    pub support_margin_mode: String,
    /// 정산 통화 파티션 ("USDT").
    #[serde(default)]
    pub trade_partition: String,
    /// 거래쌍 (예: "SAMSUNG-USDT").
    #[serde(default)]
    pub pair: String,
    /// 비즈니스 타입 ("swap").
    #[serde(default)]
    pub contract_type: String,
}

/// 시세 상세(merged) `tick` (`GET /linear-swap-ex/market/detail/merged`).
///
/// `ask`/`bid`는 `[price, size]` 2원소 배열. 가격은 f64, 그 외 OHLC는 String 원본.
#[derive(Debug, Clone, Deserialize)]
pub struct MarketDetail {
    /// 최우선 매도호가 `[가격, 잔량]`.
    pub ask: [f64; 2],
    /// 최우선 매수호가 `[가격, 잔량]`.
    pub bid: [f64; 2],
    /// 최근 체결가(종가).
    pub close: String,
    pub open: String,
    pub high: String,
    pub low: String,
    /// 24h 거래량(계약 수).
    pub vol: String,
    /// 24h 거래대금(USDT).
    pub trade_turnover: String,
    /// 체결 건수.
    #[serde(default)]
    pub count: i64,
    /// 시세 시각(epoch ms).
    pub ts: i64,
}

impl MarketDetail {
    /// 최우선 매도호가 가격.
    pub fn ask_price(&self) -> f64 {
        self.ask[0]
    }
    /// 최우선 매수호가 가격.
    pub fn bid_price(&self) -> f64 {
        self.bid[0]
    }
}

/// 호가창 `tick` (`GET /linear-swap-ex/market/depth`). 각 항목은 `[price, size]`.
#[derive(Debug, Clone, Deserialize)]
pub struct Depth {
    /// 매수호가 `[가격, 잔량]` (높은 가격순).
    pub bids: Vec<[f64; 2]>,
    /// 매도호가 `[가격, 잔량]` (낮은 가격순).
    pub asks: Vec<[f64; 2]>,
    /// 호가 버전 id.
    #[serde(default)]
    pub version: i64,
    /// 시세 시각(epoch ms).
    #[serde(default)]
    pub ts: i64,
}

/// 펀딩비 `data` (`GET /linear-swap-api/v1/swap_funding_rate`).
#[derive(Debug, Clone, Deserialize)]
pub struct FundingRate {
    pub symbol: String,
    pub contract_code: String,
    /// 현재 회차 펀딩비율 (예: "0.000100000000000000").
    pub funding_rate: String,
    /// 다음 회차 추정 펀딩비율 (없으면 None).
    #[serde(default)]
    pub estimated_rate: Option<String>,
    /// 펀딩 정산 시각(epoch ms, 문자열).
    #[serde(default)]
    pub funding_time: Option<String>,
    /// 다음 펀딩 시각(epoch ms, 문자열; null일 수 있음).
    #[serde(default)]
    pub next_funding_time: Option<String>,
    /// 수수료 통화("USDT").
    #[serde(default)]
    pub fee_asset: String,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a HtxClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a HtxClient) -> Self {
        Self { client }
    }

    /// 단일 계약 정보 조회.
    pub async fn contract_info(&self, contract_code: &str) -> Result<ContractInfo> {
        let infos: Vec<ContractInfo> = self
            .client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/linear-swap-api/v1/swap_contract_info",
                vec![("contract_code".into(), contract_code.into())],
            ))
            .await?
            .parse()?;
        infos
            .into_iter()
            .find(|c| c.contract_code == contract_code)
            .ok_or_else(|| HtxError::Decode(format!("contract not found: {contract_code}")))
    }

    /// 전체 계약 목록. KR 종목만 보려면 [`crate::global::htx::KR_SYMBOLS`]로 필터.
    pub async fn contracts(&self) -> Result<Vec<ContractInfo>> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/linear-swap-api/v1/swap_contract_info",
                vec![],
            ))
            .await?
            .parse()
    }

    /// 시세 상세(merged) 조회 — last/bid/ask/OHLC.
    pub async fn detail_merged(&self, contract_code: &str) -> Result<MarketDetail> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/linear-swap-ex/market/detail/merged",
                vec![("contract_code".into(), contract_code.into())],
            ))
            .await?
            .parse()
    }

    /// 호가창 조회. `type`은 집계 단계(기본 "step0" = 미집계 전체 호가).
    pub async fn depth(&self, contract_code: &str, step: Option<&str>) -> Result<Depth> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/linear-swap-ex/market/depth",
                vec![
                    ("contract_code".into(), contract_code.into()),
                    ("type".into(), step.unwrap_or("step0").into()),
                ],
            ))
            .await?
            .parse()
    }

    /// 펀딩비 조회.
    pub async fn funding_rate(&self, contract_code: &str) -> Result<FundingRate> {
        self.client
            .call(ApiCall::public(
                reqwest::Method::GET,
                "/linear-swap-api/v1/swap_funding_rate",
                vec![("contract_code".into(), contract_code.into())],
            ))
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_info_parses() {
        let v = serde_json::json!({
            "symbol": "SAMSUNG",
            "contract_code": "SAMSUNG-USDT",
            "contract_size": 0.01,
            "price_tick": 0.01,
            "contract_status": 1,
            "support_margin_mode": "all",
            "trade_partition": "USDT",
            "pair": "SAMSUNG-USDT",
            "contract_type": "swap"
        });
        let c: ContractInfo = serde_json::from_value(v).unwrap();
        assert_eq!(c.contract_code, "SAMSUNG-USDT");
        assert_eq!(c.contract_status, 1);
        assert_eq!(c.contract_size, 0.01);
    }

    #[test]
    fn market_detail_parses_bid_ask_arrays() {
        let v = serde_json::json!({
            "amount": "161.02", "ask": [236.06, 3], "bid": [234.42, 21],
            "close": "235.38", "count": 750, "high": "249.71", "id": 1780547886,
            "low": "232.15", "open": "249.71", "trade_turnover": "39803.603",
            "ts": 1780547886881i64, "vol": "16102"
        });
        let d: MarketDetail = serde_json::from_value(v).unwrap();
        assert_eq!(d.close, "235.38");
        assert_eq!(d.ask_price(), 236.06);
        assert_eq!(d.bid_price(), 234.42);
        assert_eq!(d.ts, 1780547886881);
    }

    #[test]
    fn depth_parses_price_size_pairs() {
        let v = serde_json::json!({
            "mrid": 100000002693916i64, "id": 1780547891,
            "bids": [[234.42, 21], [234.41, 74]],
            "asks": [[236.06, 3], [236.07, 71]],
            "version": 1780547891, "ts": 1780547891000i64
        });
        let d: Depth = serde_json::from_value(v).unwrap();
        assert_eq!(d.bids[0][0], 234.42);
        assert_eq!(d.bids[0][1], 21.0);
        assert_eq!(d.asks[0][0], 236.06);
    }

    #[test]
    fn funding_rate_parses_null_fields() {
        let v = serde_json::json!({
            "estimated_rate": null,
            "funding_rate": "0.000100000000000000",
            "contract_code": "SAMSUNG-USDT",
            "symbol": "SAMSUNG",
            "fee_asset": "USDT",
            "funding_time": "1780560000000",
            "next_funding_time": null,
            "trade_partition": "USDT"
        });
        let f: FundingRate = serde_json::from_value(v).unwrap();
        assert_eq!(f.funding_rate, "0.000100000000000000");
        assert_eq!(f.contract_code, "SAMSUNG-USDT");
        assert!(f.estimated_rate.is_none());
        assert!(f.next_funding_time.is_none());
        assert_eq!(f.funding_time.as_deref(), Some("1780560000000"));
    }
}
