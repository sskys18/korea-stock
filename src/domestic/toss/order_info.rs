//! 주문정보 도메인 — 매수가능금액·판매가능수량·매매수수료. 모두 `X-Tossinvest-Account` 필수.

use serde::Deserialize;

use crate::domestic::toss::client::{ApiCall, TossClient};
use crate::domestic::toss::error::Result;

/// 매수 가능 금액 조회 응답.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BuyingPowerResponse {
    /// 통화 (KRW/USD).
    pub currency: String,
    /// 현금 기반 매수 가능 금액 (미수 미발생 기준).
    pub cash_buying_power: String,
}

/// 판매 가능 수량 조회 응답.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SellableQuantityResponse {
    /// 판매 가능 수량. KR 정수, US 소수 가능.
    pub sellable_quantity: String,
}

/// 매매 수수료 1건. marketCountry는 unknown 허용 위해 String.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Commission {
    /// 시장 국가 (KR/US).
    pub market_country: String,
    /// 수수료율 (%). 0.015 = 0.015%.
    pub commission_rate: String,
    /// 적용 시작일 (YYYY-MM-DD, KST). 해외주식은 null.
    #[serde(default)]
    pub start_date: Option<String>,
    /// 적용 종료일. 무기한 적용 시 null.
    #[serde(default)]
    pub end_date: Option<String>,
}

/// 주문정보 도메인 액세서. `client.order_info(account_seq)`로 획득.
/// `account_seq`를 구조적으로 보유해 `X-Tossinvest-Account` 누락이 불가능하다.
pub struct OrderInfo<'a> {
    client: &'a TossClient,
    account_seq: i64,
}

impl<'a> OrderInfo<'a> {
    pub(crate) fn new(client: &'a TossClient, account_seq: i64) -> Self {
        Self {
            client,
            account_seq,
        }
    }

    /// 매수 가능 금액 조회. `currency`(KRW/USD) 지정.
    pub async fn buying_power(&self, currency: &str) -> Result<BuyingPowerResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/buying-power".into(),
                params: serde_json::json!({ "currency": currency }),
                is_post: false,
                account_seq: Some(self.account_seq),
            })
            .await?
            .parse()
    }

    /// 판매 가능 수량 조회.
    pub async fn sellable_quantity(&self, symbol: &str) -> Result<SellableQuantityResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/sellable-quantity".into(),
                params: serde_json::json!({ "symbol": symbol }),
                is_post: false,
                account_seq: Some(self.account_seq),
            })
            .await?
            .parse()
    }

    /// 매매 수수료 조회.
    pub async fn commissions(&self) -> Result<Vec<Commission>> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/commissions".into(),
                params: serde_json::json!({}),
                is_post: false,
                account_seq: Some(self.account_seq),
            })
            .await?
            .parse()
    }
}
