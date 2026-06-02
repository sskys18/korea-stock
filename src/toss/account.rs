//! 계좌·자산 도메인.
//!
//! `Accounts`는 계좌 헤더가 필요 없는 계좌 목록 조회, `Asset`은
//! `X-Tossinvest-Account` 헤더가 필요한 보유주식 조회를 담당한다.
//! `Asset`은 생성 시점에 `account_seq`를 보유해 헤더 누락을 구조적으로 차단한다.

use serde::Deserialize;

use crate::toss::client::{ApiCall, TossClient};
use crate::toss::error::Result;

/// 계좌 1건. accountType은 unknown 허용 위해 String.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// 계좌번호.
    pub account_no: String,
    /// 계좌 식별 키. 이후 모든 계좌 스코프 API의 `X-Tossinvest-Account` 값.
    pub account_seq: i64,
    /// 계좌 유형 (BROKERAGE/OVERSEAS_DERIVATIVES/PENSION_SAVINGS/RESHORING_INVESTMENT).
    pub account_type: String,
}

/// 통화별 합산 금액 (환율 환산 미포함).
#[derive(Debug, Clone, Deserialize)]
pub struct Price {
    /// KRW 거래 국내 종목 합산. 없으면 0.
    pub krw: String,
    /// USD 거래 해외 종목 합산. 없으면 null.
    #[serde(default)]
    pub usd: Option<String>,
}

/// 전체 자산 시장 평가 (통화별 합산).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewMarketValue {
    pub amount: Price,
    /// 세금/수수료 공제 후 평가금액.
    pub amount_after_cost: Price,
}

/// 전체 자산 손익 (통화별 합산).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverviewProfitLoss {
    pub amount: Price,
    pub amount_after_cost: Price,
    /// 손익률 (소수비율, 0.1516 = 15.16%). 전체 원화 환산 기준.
    pub rate: String,
    pub rate_after_cost: String,
}

/// 전체 자산 일간 손익.
#[derive(Debug, Clone, Deserialize)]
pub struct OverviewDailyProfitLoss {
    pub amount: Price,
    /// 일간 손익률 (소수비율).
    pub rate: String,
}

/// 종목별 시장 평가 (거래 통화 기준).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketValue {
    pub purchase_amount: String,
    pub amount: String,
    pub amount_after_cost: String,
}

/// 종목별 손익 (거래 통화 기준).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfitLoss {
    pub amount: String,
    pub amount_after_cost: String,
    /// 손익률 (소수비율).
    pub rate: String,
    pub rate_after_cost: String,
}

/// 종목별 일간 손익 (거래 통화 기준).
#[derive(Debug, Clone, Deserialize)]
pub struct DailyProfitLoss {
    pub amount: String,
    pub rate: String,
}

/// 종목별 비용 (거래 통화 기준).
#[derive(Debug, Clone, Deserialize)]
pub struct Cost {
    pub commission: String,
    /// 세금. 없으면 null.
    #[serde(default)]
    pub tax: Option<String>,
}

/// 보유 종목 1건. marketCountry/currency는 unknown 허용 위해 String.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldingsItem {
    pub symbol: String,
    pub name: String,
    /// 시장 국가 (KR/US).
    pub market_country: String,
    /// 통화 (KRW/USD).
    pub currency: String,
    /// 보유 수량.
    pub quantity: String,
    /// 현재가 (거래 통화 기준).
    pub last_price: String,
    /// 매수 평균가 (거래 통화 기준).
    pub average_purchase_price: String,
    pub market_value: MarketValue,
    pub profit_loss: ProfitLoss,
    pub daily_profit_loss: DailyProfitLoss,
    pub cost: Cost,
}

/// 보유 주식 조회 응답 (요약 + 종목 목록).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HoldingsOverview {
    /// 투자원금 (통화별 합산).
    pub total_purchase_amount: Price,
    pub market_value: OverviewMarketValue,
    pub profit_loss: OverviewProfitLoss,
    pub daily_profit_loss: OverviewDailyProfitLoss,
    /// 보유 종목 목록. 없으면 빈 배열.
    pub items: Vec<HoldingsItem>,
}

/// 계좌 목록 액세서 (계좌 헤더 불필요). `client.accounts()`로 획득.
pub struct Accounts<'a> {
    client: &'a TossClient,
}

impl<'a> Accounts<'a> {
    pub(crate) fn new(client: &'a TossClient) -> Self {
        Self { client }
    }

    /// 계좌 목록 조회. 응답의 `account_seq`가 계좌 스코프 호출의 식별자.
    pub async fn list(&self) -> Result<Vec<Account>> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/accounts".into(),
                params: serde_json::json!({}),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }
}

/// 자산(보유주식) 액세서. `client.asset(account_seq)`로 획득.
/// `account_seq`를 구조적으로 보유해 `X-Tossinvest-Account` 누락이 불가능하다.
pub struct Asset<'a> {
    client: &'a TossClient,
    account_seq: i64,
}

impl<'a> Asset<'a> {
    pub(crate) fn new(client: &'a TossClient, account_seq: i64) -> Self {
        Self {
            client,
            account_seq,
        }
    }

    /// 보유 주식 조회. `symbol` 지정 시 해당 종목만 필터(요약도 그 종목 기준 재계산).
    pub async fn holdings(&self, symbol: Option<&str>) -> Result<HoldingsOverview> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/holdings".into(),
                params: serde_json::json!({ "symbol": symbol }),
                is_post: false,
                account_seq: Some(self.account_seq),
            })
            .await?
            .parse()
    }
}
