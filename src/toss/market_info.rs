//! 시장정보 도메인 — 환율·국내/미국 장 운영 정보.

use serde::Deserialize;

use crate::toss::client::{ApiCall, TossClient};
use crate::toss::error::Result;

/// 환율 조회 응답. 통화·등락구분은 unknown 허용 위해 String.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExchangeRateResponse {
    /// 기준 통화.
    pub base_currency: String,
    /// 표시 통화 (quote currency).
    pub quote_currency: String,
    /// 매수 환율 (1 base = ? quote).
    pub rate: String,
    /// 매매기준율 (은행간 mid rate).
    pub mid_rate: String,
    /// midRate 대비 basis points.
    pub basis_point: String,
    /// 등락 구분 (UP/EQUAL/DOWN).
    pub rate_change_type: String,
    pub valid_from: String,
    pub valid_until: String,
}

/// 프리/정규/애프터 세션 공통 시각 정보 (국내 preMarket/afterMarket는 단일가 구간 시각 포함).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub start_time: String,
    pub end_time: String,
    /// 단일가 구간 시작 (프리/정규). 결손 시 null.
    #[serde(default)]
    pub single_price_auction_start_time: Option<String>,
    /// 단일가 구간 종료 (국내 애프터마켓). 결손 시 null.
    #[serde(default)]
    pub single_price_auction_end_time: Option<String>,
}

/// 거래 가능 시간 (통합 모드 KRX+NXT). 각 세션 nullable.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IntegratedHour {
    #[serde(default)]
    pub pre_market: Option<Session>,
    #[serde(default)]
    pub regular_market: Option<Session>,
    #[serde(default)]
    pub after_market: Option<Session>,
}

/// 국내 영업일 정보.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KrMarketDay {
    /// 영업일 (KST).
    pub date: String,
    /// 거래 가능 시간. 휴장이면 null.
    #[serde(default)]
    pub integrated: Option<IntegratedHour>,
}

/// 국내 장 운영 정보 조회 응답.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KrMarketCalendarResponse {
    pub today: KrMarketDay,
    pub previous_business_day: KrMarketDay,
    pub next_business_day: KrMarketDay,
}

/// 미국 영업일 정보. 4 세션 각각 nullable.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsMarketDay {
    /// 영업일 (미국 현지 기준).
    pub date: String,
    #[serde(default)]
    pub day_market: Option<Session>,
    #[serde(default)]
    pub pre_market: Option<Session>,
    #[serde(default)]
    pub regular_market: Option<Session>,
    #[serde(default)]
    pub after_market: Option<Session>,
}

/// 미국 장 운영 정보 조회 응답.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsMarketCalendarResponse {
    pub today: UsMarketDay,
    pub previous_business_day: UsMarketDay,
    pub next_business_day: UsMarketDay,
}

/// 시장정보 도메인 액세서. `client.market_info()`로 획득.
pub struct MarketInfoApi<'a> {
    client: &'a TossClient,
}

impl<'a> MarketInfoApi<'a> {
    pub(crate) fn new(client: &'a TossClient) -> Self {
        Self { client }
    }

    /// 환율 조회. `date_time`(ISO 8601) 지정 시 특정 시점 환율.
    pub async fn exchange_rate(
        &self,
        base_currency: &str,
        quote_currency: &str,
        date_time: Option<&str>,
    ) -> Result<ExchangeRateResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/exchange-rate".into(),
                params: serde_json::json!({
                    "baseCurrency": base_currency,
                    "quoteCurrency": quote_currency,
                    "dateTime": date_time,
                }),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }

    /// 국내 장 운영 정보 조회. `date`(YYYY-MM-DD) 미지정 시 오늘 기준.
    pub async fn calendar_kr(&self, date: Option<&str>) -> Result<KrMarketCalendarResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/market-calendar/KR".into(),
                params: serde_json::json!({ "date": date }),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }

    /// 미국 장 운영 정보 조회. `date` 미지정 시 오늘 기준.
    pub async fn calendar_us(&self, date: Option<&str>) -> Result<UsMarketCalendarResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/market-calendar/US".into(),
                params: serde_json::json!({ "date": date }),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }
}
