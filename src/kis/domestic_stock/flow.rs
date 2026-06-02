//! 국내주식 투자자 플로우 TR — 외국인/기관 순매수, 프로그램매매, 추정 집계.
//!
//! 응답 필드는 `koreainvestment/open-trading-api` `examples_llm` 샘플의
//! COLUMN_MAPPING에서 추출(추측 없음). 알파 관련 핵심 필드만 타입화하고
//! `#[serde(default)]`로 누락·미수록 필드 내성 확보. 전체 필드는
//! `docs/kis-api/domestic-stock.md` §13~16 참조.

use serde::Deserialize;

use crate::kis::client::{ApiCall, KisResponse};
use crate::kis::domestic_stock::{DomesticStock, Market};
use crate::kis::error::Result;
use crate::kis::trid::TrId;

const TR_INVESTOR_DAILY: TrId = TrId::same("FHPTJ04160001");
const TR_PROGRAM_TODAY: TrId = TrId::same("FHPPG04600101");
const TR_PROGRAM_DAILY: TrId = TrId::same("FHPPG04600001");
const TR_INVESTOR_ESTIMATE: TrId = TrId::same("HHPTJ04160200");
const TR_SHORT_SALE_DAILY: TrId = TrId::same("FHPST04830000");

/// 시장 구분. 프로그램매매 TR의 `FID_MRKT_CLS_CODE`.
#[derive(Debug, Clone, Copy)]
pub enum MarketClass {
    /// K — 코스피.
    Kospi,
    /// Q — 코스닥.
    Kosdaq,
}

impl MarketClass {
    fn code(self) -> &'static str {
        match self {
            MarketClass::Kospi => "K",
            MarketClass::Kosdaq => "Q",
        }
    }
}

/// 종목별 투자자매매동향(일별) 요약 — output1(현재가 스냅샷 헤더).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InvestorTrendSummary {
    pub stck_prpr: String,
    pub prdy_vrss: String,
    pub prdy_vrss_sign: String,
    pub prdy_ctrt: String,
    pub acml_vol: String,
    pub prdy_vol: String,
    pub rprs_mrkt_kor_name: String,
}

/// 종목별 투자자매매동향(일별) 1일 — output2 배열 요소.
///
/// 순매수 수량(`*_ntby_qty`)·대금(`*_ntby_tr_pbmn`)이 핵심 알파.
/// 투자자 세분류(증권/투신/사모/은행/보험/기금/기타법인) 일부 노출.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InvestorTrendDay {
    pub stck_bsop_date: String,
    pub stck_clpr: String,
    pub prdy_vrss: String,
    pub prdy_vrss_sign: String,
    pub prdy_ctrt: String,
    pub acml_vol: String,
    pub acml_tr_pbmn: String,
    /// 외국인 순매수 수량.
    pub frgn_ntby_qty: String,
    /// 개인 순매수 수량.
    pub prsn_ntby_qty: String,
    /// 기관계 순매수 수량.
    pub orgn_ntby_qty: String,
    /// 외국인 순매수 거래대금.
    pub frgn_ntby_tr_pbmn: String,
    /// 개인 순매수 거래대금.
    pub prsn_ntby_tr_pbmn: String,
    /// 기관계 순매수 거래대금.
    pub orgn_ntby_tr_pbmn: String,
    pub frgn_reg_ntby_qty: String,
    pub frgn_nreg_ntby_qty: String,
    pub scrt_ntby_qty: String,
    pub ivtr_ntby_qty: String,
    pub pe_fund_ntby_vol: String,
    pub bank_ntby_qty: String,
    pub insu_ntby_qty: String,
    pub fund_ntby_qty: String,
    pub etc_corp_ntby_vol: String,
}

/// 프로그램매매 종합현황(시간) 1행 — output 배열 요소.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProgramTradeToday {
    pub stck_bsop_date: String,
    pub stck_clpr: String,
    pub prdy_vrss: String,
    pub prdy_vrss_sign: String,
    pub prdy_ctrt: String,
    pub acml_vol: String,
    pub acml_tr_pbmn: String,
    pub whol_smtn_seln_vol: String,
    pub whol_smtn_shnu_vol: String,
    /// 전체 합계 프로그램 순매수 수량.
    pub whol_smtn_ntby_qty: String,
    pub whol_smtn_seln_tr_pbmn: String,
    pub whol_smtn_shnu_tr_pbmn: String,
    /// 전체 합계 프로그램 순매수 거래대금.
    pub whol_smtn_ntby_tr_pbmn: String,
    pub whol_ntby_vol_icdc: String,
    pub whol_ntby_tr_pbmn_icdc2: String,
}

/// 프로그램매매 종합현황(일별) 1일 — output 배열 요소.
///
/// 차익(`arbt_*`)·비차익(`nabt_*`) 합계 순매수가 바스켓/지수 차익 압력 신호.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ProgramTradeDaily {
    pub stck_bsop_date: String,
    /// 차익 합계 순매수 수량.
    pub arbt_smtm_ntby_qty: String,
    /// 차익 합계 순매수 거래대금.
    pub arbt_smtn_ntby_tr_pbmn: String,
    /// 비차익 합계 순매수 수량.
    pub nabt_smtn_ntby_qty: String,
    /// 비차익 합계 순매수 거래대금.
    pub nabt_smtn_ntby_tr_pbmn: String,
    /// 전체 위탁 순매수 수량.
    pub whol_entm_ntby_qty: String,
    pub arbt_entm_ntby_qty: String,
    pub nabt_entm_ntby_tr_pbmn: String,
}

/// 종목별 외국인·기관 추정 가집계(실시간 추정) — output2 배열 요소.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct InvestorTrendEstimate {
    /// 입력 구분(시간대).
    pub bsop_hour_gb: String,
    /// 외국인 수량(가집계).
    pub frgn_fake_ntby_qty: String,
    /// 기관 수량(가집계).
    pub orgn_fake_ntby_qty: String,
    /// 합산 수량(가집계).
    pub sum_fake_ntby_qty: String,
}

/// 일별 공매도 요약 — output1(현재가 스냅샷).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShortSaleSummary {
    pub stck_prpr: String,
    pub prdy_vrss: String,
    pub prdy_vrss_sign: String,
    pub prdy_ctrt: String,
    pub acml_vol: String,
    pub prdy_vol: String,
}

/// 일별 공매도 1일 — output2 배열 요소.
///
/// 공매도 *거래*(체결) 신호. 잔고(outstanding)는 KIS 미제공 — KRX 외부 소스.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShortSaleDay {
    pub stck_bsop_date: String,
    pub stck_clpr: String,
    pub acml_vol: String,
    /// 공매도 체결 수량.
    pub ssts_cntg_qty: String,
    /// 공매도 거래량 비중(%).
    pub ssts_vol_rlim: String,
    /// 누적 공매도 체결 수량.
    pub acml_ssts_cntg_qty: String,
    /// 누적 공매도 체결 수량 비중(%).
    pub acml_ssts_cntg_qty_rlim: String,
    /// 공매도 거래 대금.
    pub ssts_tr_pbmn: String,
    /// 공매도 거래대금 비중(%).
    pub ssts_tr_pbmn_rlim: String,
    /// 공매도 평균가격.
    pub avrg_prc: String,
}

impl DomesticStock<'_> {
    /// 일별 공매도 — TR 17 `FHPST04830000`.
    ///
    /// `start`/`end` YYYYMMDD. (요약 output1, 일별 output2) 반환.
    /// 종목별 일별 공매도 체결량·비중. 잔고는 미포함(KRX 외부).
    pub async fn short_sale_daily(
        &self,
        stock_code: &str,
        start: &str,
        end: &str,
    ) -> Result<(ShortSaleSummary, Vec<ShortSaleDay>)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/daily-short-sale".into(),
                tr_id: TR_SHORT_SALE_DAILY.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": "J",
                    "FID_INPUT_ISCD": stock_code,
                    "FID_INPUT_DATE_1": start,
                    "FID_INPUT_DATE_2": end,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        Ok((resp.field("output1")?, resp.field("output2")?))
    }

    /// 종목별 투자자매매동향(일별) 한 페이지 — TR 13 `FHPTJ04160001`.
    ///
    /// `date`: 기준일(YYYYMMDD). `cont=true`면 직전 호출에 이은 연속조회(헤더 `tr_cont=N`).
    /// envelope의 `data`는 (요약 output1, 일별배열 output2). `has_next()`로 다음 페이지 판단.
    /// 전체 history는 [`investor_trend_daily_all`]을 사용.
    ///
    /// 외국인/기관/개인 일별 순매수 누적 — 클래식 KR 알파.
    pub async fn investor_trend_daily(
        &self,
        stock_code: &str,
        date: &str,
        cont: bool,
        market: Market,
    ) -> Result<KisResponse<(InvestorTrendSummary, Vec<InvestorTrendDay>)>> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/investor-trade-by-stock-daily".into(),
                tr_id: TR_INVESTOR_DAILY.resolve(env)?.into(),
                // 연속조회는 헤더 tr_cont="N"만 사용(ctx_area 없음). 샘플 기준.
                tr_cont: cont.then(|| "N".to_string()),
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.fid_code(),
                    "FID_INPUT_ISCD": stock_code,
                    "FID_INPUT_DATE_1": date,
                    "FID_ORG_ADJ_PRC": "",
                    "FID_ETC_CLS_CODE": "",
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let summary: InvestorTrendSummary = resp.field("output1")?;
        let days: Vec<InvestorTrendDay> = resp.field("output2")?;
        Ok(resp.envelope((summary, days)))
    }

    /// 종목별 투자자매매동향(일별) 전체 페이지 수집.
    ///
    /// `tr_cont` 헤더 연속조회를 `has_next()`가 false일 때까지 반복.
    /// 헤더 전용 연속(ctx_area 없음)이라 진전 보장이 없어 `MAX_PAGES`(100)로 상한.
    pub async fn investor_trend_daily_all(
        &self,
        stock_code: &str,
        date: &str,
        market: Market,
    ) -> Result<(InvestorTrendSummary, Vec<InvestorTrendDay>)> {
        const MAX_PAGES: usize = 100;
        let mut summary = InvestorTrendSummary::default();
        let mut days = Vec::new();
        let mut cont = false;
        for _ in 0..MAX_PAGES {
            let page = self
                .investor_trend_daily(stock_code, date, cont, market)
                .await?;
            let has_next = page.has_next();
            let (s, mut d) = page.data;
            if !cont {
                summary = s; // 첫 페이지 요약만 보존.
            }
            days.append(&mut d);
            if has_next {
                cont = true;
            } else {
                break;
            }
        }
        Ok((summary, days))
    }

    /// 종목별 외국인·기관 추정 가집계 — TR 14 `HHPTJ04160200`. output2 배열.
    ///
    /// 장중 외국인/기관 매매 추정치(가집계). 확정 전 실시간 추정 플로우.
    pub async fn investor_trend_estimate(
        &self,
        stock_code: &str,
    ) -> Result<Vec<InvestorTrendEstimate>> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/investor-trend-estimate".into(),
                tr_id: TR_INVESTOR_ESTIMATE.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "MKSC_SHRN_ISCD": stock_code,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output2")
    }

    /// 프로그램매매 종합현황(시간) — TR 15 `FHPPG04600101`.
    ///
    /// 시장(K=코스피/Q=코스닥) 단위. `stock_code` 지정 시 종목 한정.
    /// 최근 30분 데이터, 다음조회 불가.
    pub async fn program_trade_today(
        &self,
        board: MarketClass,
        stock_code: Option<&str>,
        market: Market,
    ) -> Result<Vec<ProgramTradeToday>> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/comp-program-trade-today".into(),
                tr_id: TR_PROGRAM_TODAY.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.fid_code(),
                    "FID_MRKT_CLS_CODE": board.code(),
                    "FID_SCTN_CLS_CODE": "",
                    "FID_INPUT_ISCD": stock_code.unwrap_or(""),
                    "FID_COND_MRKT_DIV_CODE1": "",
                    "FID_INPUT_HOUR_1": "",
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }

    /// 프로그램매매 종합현황(일별) — TR 16 `FHPPG04600001`.
    ///
    /// 시장 단위. `start`/`end` YYYYMMDD. 차익/비차익 순매수 누적.
    pub async fn program_trade_daily(
        &self,
        board: MarketClass,
        start: &str,
        end: &str,
        market: Market,
    ) -> Result<Vec<ProgramTradeDaily>> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/comp-program-trade-daily".into(),
                tr_id: TR_PROGRAM_DAILY.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.fid_code(),
                    "FID_MRKT_CLS_CODE": board.code(),
                    "FID_INPUT_DATE_1": start,
                    "FID_INPUT_DATE_2": end,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_class_codes() {
        assert_eq!(MarketClass::Kospi.code(), "K");
        assert_eq!(MarketClass::Kosdaq.code(), "Q");
    }

    // 누락 필드·미수록 필드 내성: 부분 JSON도 default로 안전 역직렬화.
    #[test]
    fn investor_trend_day_serde_default() {
        let partial = serde_json::json!({
            "stck_bsop_date": "20250812",
            "frgn_ntby_qty": "12345",
            "unknown_extra_field": "ignored",
        });
        let day: InvestorTrendDay = serde_json::from_value(partial).unwrap();
        assert_eq!(day.stck_bsop_date, "20250812");
        assert_eq!(day.frgn_ntby_qty, "12345");
        assert_eq!(day.orgn_ntby_qty, ""); // 누락 → default
    }

    #[test]
    fn investor_trend_estimate_serde_default() {
        let v: InvestorTrendEstimate = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(v.sum_fake_ntby_qty, "");
    }

    #[test]
    fn short_sale_serde_default() {
        let day: ShortSaleDay = serde_json::from_value(serde_json::json!({
            "stck_bsop_date": "20240301",
            "ssts_cntg_qty": "999",
        }))
        .unwrap();
        assert_eq!(day.ssts_cntg_qty, "999");
        assert_eq!(day.avrg_prc, "");
    }

    #[test]
    fn program_trade_serde_default() {
        let today: ProgramTradeToday =
            serde_json::from_value(serde_json::json!({"whol_smtn_ntby_qty": "100"})).unwrap();
        assert_eq!(today.whol_smtn_ntby_qty, "100");
        let daily: ProgramTradeDaily =
            serde_json::from_value(serde_json::json!({"arbt_smtm_ntby_qty": "-50"})).unwrap();
        assert_eq!(daily.arbt_smtm_ntby_qty, "-50");
    }
}
