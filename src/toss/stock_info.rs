//! 종목정보 도메인 — 종목 기본정보·매수 유의사항.

use serde::Deserialize;

use crate::toss::client::{ApiCall, TossClient};
use crate::toss::error::Result;

/// 국내 시장 상세 정보. 국내 종목에만 제공.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KrMarketDetail {
    /// 정리매매 여부 (상장폐지 절차 진행 중).
    pub liquidation_trading: bool,
    /// NXT 대체거래소 지원 여부.
    pub nxt_supported: bool,
    /// KRX 거래정지 여부.
    pub krx_trading_suspended: bool,
    /// NXT 거래정지 여부. NXT 미지원 종목은 null.
    #[serde(default)]
    pub nxt_trading_suspended: Option<bool>,
}

/// 종목 기본 정보. enum성 필드(market/securityType/status/currency)는 unknown 허용 위해 String.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StockInfo {
    pub symbol: String,
    /// 종목명 (한글).
    pub name: String,
    /// 영문 종목명.
    pub english_name: String,
    /// 국제증권식별번호 (ISO 6166).
    pub isin_code: String,
    /// 상장 시장 (KOSPI/KOSDAQ/NYSE/NASDAQ/AMEX/KR_ETC/US_ETC).
    pub market: String,
    /// 종목 유형 (STOCK/ETF/REIT/...).
    pub security_type: String,
    /// 보통주 여부. 우선주면 false.
    pub is_common_share: bool,
    /// 상장 상태 (SCHEDULED/ACTIVE/DELISTED).
    pub status: String,
    /// 통화 코드 (KRW/USD).
    pub currency: String,
    /// 상장일 (YYYY-MM-DD, KST). 미제공 시 null.
    #[serde(default)]
    pub list_date: Option<String>,
    /// 상장폐지일. 활성 종목은 null.
    #[serde(default)]
    pub delist_date: Option<String>,
    /// 발행주식수.
    pub shares_outstanding: String,
    /// 레버리지 배수. ETF/ETN에만 적용, 일반 종목은 null.
    #[serde(default)]
    pub leverage_factor: Option<String>,
    /// 국내 시장 상세. 해외 종목은 null.
    #[serde(default)]
    pub korean_market_detail: Option<KrMarketDetail>,
}

/// 매수 유의사항 1건. warningType은 unknown 허용 위해 String.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StockWarning {
    /// 유의사항 유형 (LIQUIDATION_TRADING/OVERHEATED/INVESTMENT_WARNING/INVESTMENT_RISK/VI_*/STOCK_WARRANTS).
    pub warning_type: String,
    /// 거래소 코드 (KRX/NXT 등 물리적 거래소 단위).
    pub exchange: String,
    /// 적용 시작일 (inclusive, YYYY-MM-DD, KST). 미정 시 null.
    #[serde(default)]
    pub start_date: Option<String>,
    /// 적용 종료일 (inclusive). 진행 중/미정 시 null.
    #[serde(default)]
    pub end_date: Option<String>,
}

/// 종목정보 도메인 액세서. `client.stock_info()`로 획득.
pub struct StockInfoApi<'a> {
    client: &'a TossClient,
}

impl<'a> StockInfoApi<'a> {
    pub(crate) fn new(client: &'a TossClient) -> Self {
        Self { client }
    }

    /// 종목 기본 정보 조회. 최대 200개 심볼. (콤마 결합은 내부 처리.)
    pub async fn stocks(&self, symbols: &[&str]) -> Result<Vec<StockInfo>> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/stocks".into(),
                params: serde_json::json!({ "symbols": symbols.join(",") }),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }

    /// 매수 유의사항 조회. 해당 종목에 유의사항이 없으면 빈 배열.
    pub async fn warnings(&self, symbol: &str) -> Result<Vec<StockWarning>> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: format!("/api/v1/stocks/{symbol}/warnings"),
                params: serde_json::json!({}),
                is_post: false,
                account_seq: None,
            })
            .await?
            .parse()
    }
}
