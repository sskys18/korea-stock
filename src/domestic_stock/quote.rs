use serde::Deserialize;

use crate::client::ApiCall;
use crate::domestic_stock::{DomesticStock, Market};
use crate::error::Result;
use crate::trid::TrId;

const TR_PRICE: TrId = TrId::same("FHKST01010100");

/// 주식현재가 시세 응답. 필드 전체는 docs/kis-api/domestic-stock.md §9.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct CurrentPrice {
    pub iscd_stat_cls_code: String,
    pub marg_rate: String,
    pub rprs_mrkt_kor_name: String,
    pub new_hgpr_lwpr_cls_code: String,
    pub bstp_kor_isnm: String,
    pub temp_stop_yn: String,
    pub oprc_rang_cont_yn: String,
    pub clpr_rang_cont_yn: String,
    pub crdt_able_yn: String,
    pub grmn_rate_cls_code: String,
    pub elw_pblc_yn: String,
    pub stck_prpr: String,
    pub prdy_vrss: String,
    pub prdy_vrss_sign: String,
    pub prdy_ctrt: String,
    pub acml_tr_pbmn: String,
    pub acml_vol: String,
    pub prdy_vrss_vol_rate: String,
    pub stck_oprc: String,
    pub stck_hgpr: String,
    pub stck_lwpr: String,
    pub stck_mxpr: String,
    pub stck_llam: String,
    pub stck_sdpr: String,
    pub wghn_avrg_stck_prc: String,
    pub hts_frgn_ehrt: String,
    pub frgn_ntby_qty: String,
    pub pgtr_ntby_qty: String,
    pub pvt_scnd_dmrs_prc: String,
    pub pvt_frst_dmrs_prc: String,
    pub pvt_pont_val: String,
    pub pvt_frst_dmsp_prc: String,
    pub pvt_scnd_dmsp_prc: String,
    pub dmrs_val: String,
    pub dmsp_val: String,
    pub cpfn: String,
    pub rstc_wdth_prc: String,
    pub stck_fcam: String,
    pub stck_sspr: String,
    pub aspr_unit: String,
    pub hts_deal_qty_unit_val: String,
    pub lstn_stcn: String,
    pub hts_avls: String,
    pub per: String,
    pub pbr: String,
    pub stac_month: String,
    pub vol_tnrt: String,
    pub eps: String,
    pub bps: String,
    pub d250_hgpr: String,
    pub d250_hgpr_date: String,
    pub d250_hgpr_vrss_prpr_rate: String,
    pub d250_lwpr: String,
    pub d250_lwpr_date: String,
    pub d250_lwpr_vrss_prpr_rate: String,
    pub stck_dryy_hgpr: String,
    pub dryy_hgpr_vrss_prpr_rate: String,
    pub dryy_hgpr_date: String,
    pub stck_dryy_lwpr: String,
    pub dryy_lwpr_vrss_prpr_rate: String,
    pub dryy_lwpr_date: String,
    pub w52_hgpr: String,
    pub w52_hgpr_vrss_prpr_ctrt: String,
    pub w52_hgpr_date: String,
    pub w52_lwpr: String,
    pub w52_lwpr_vrss_prpr_ctrt: String,
    pub w52_lwpr_date: String,
    pub whol_loan_rmnd_rate: String,
    pub ssts_yn: String,
    pub stck_shrn_iscd: String,
    pub fcam_cnnm: String,
    pub cpfn_cnnm: String,
    pub apprch_rate: String,
    pub frgn_hldn_qty: String,
    pub vi_cls_code: String,
    pub ovtm_vi_cls_code: String,
    pub last_ssts_cntg_qty: String,
    pub invt_caful_yn: String,
    pub mrkt_warn_cls_code: String,
    pub short_over_yn: String,
    pub sltr_yn: String,
    pub mang_issu_cls_code: String,
}

impl CurrentPrice {
    /// 현재가를 f64로 파싱.
    pub fn price(&self) -> Option<f64> {
        self.stck_prpr.trim().parse().ok()
    }
}

const TR_ASKING: TrId = TrId::same("FHKST01010200");

/// 호가 정보 (output1). 필드 전체는 §10 output1 표.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AskingPrice {
    pub aspr_acpt_hour: String,
    pub askp1: String,
    pub askp2: String,
    pub askp3: String,
    pub askp4: String,
    pub askp5: String,
    pub askp6: String,
    pub askp7: String,
    pub askp8: String,
    pub askp9: String,
    pub askp10: String,
    pub bidp1: String,
    pub bidp2: String,
    pub bidp3: String,
    pub bidp4: String,
    pub bidp5: String,
    pub bidp6: String,
    pub bidp7: String,
    pub bidp8: String,
    pub bidp9: String,
    pub bidp10: String,
    pub askp_rsqn1: String,
    pub askp_rsqn2: String,
    pub askp_rsqn3: String,
    pub askp_rsqn4: String,
    pub askp_rsqn5: String,
    pub askp_rsqn6: String,
    pub askp_rsqn7: String,
    pub askp_rsqn8: String,
    pub askp_rsqn9: String,
    pub askp_rsqn10: String,
    pub bidp_rsqn1: String,
    pub bidp_rsqn2: String,
    pub bidp_rsqn3: String,
    pub bidp_rsqn4: String,
    pub bidp_rsqn5: String,
    pub bidp_rsqn6: String,
    pub bidp_rsqn7: String,
    pub bidp_rsqn8: String,
    pub bidp_rsqn9: String,
    pub bidp_rsqn10: String,
    pub askp_rsqn_icdc1: String,
    pub askp_rsqn_icdc2: String,
    pub askp_rsqn_icdc3: String,
    pub askp_rsqn_icdc4: String,
    pub askp_rsqn_icdc5: String,
    pub askp_rsqn_icdc6: String,
    pub askp_rsqn_icdc7: String,
    pub askp_rsqn_icdc8: String,
    pub askp_rsqn_icdc9: String,
    pub askp_rsqn_icdc10: String,
    pub bidp_rsqn_icdc1: String,
    pub bidp_rsqn_icdc2: String,
    pub bidp_rsqn_icdc3: String,
    pub bidp_rsqn_icdc4: String,
    pub bidp_rsqn_icdc5: String,
    pub bidp_rsqn_icdc6: String,
    pub bidp_rsqn_icdc7: String,
    pub bidp_rsqn_icdc8: String,
    pub bidp_rsqn_icdc9: String,
    pub bidp_rsqn_icdc10: String,
    pub total_askp_rsqn: String,
    pub total_bidp_rsqn: String,
    pub total_askp_rsqn_icdc: String,
    pub total_bidp_rsqn_icdc: String,
    pub ovtm_total_askp_icdc: String,
    pub ovtm_total_bidp_icdc: String,
    pub ovtm_total_askp_rsqn: String,
    pub ovtm_total_bidp_rsqn: String,
    pub ntby_aspr_rsqn: String,
    pub new_mkop_cls_code: String,
    pub antc_mkop_cls_code: String,
}

/// 예상체결 정보 (output2). 필드 전체는 §10 output2 표.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ExpectedConclusion {
    pub antc_cnpr: String,
    pub antc_cntg_vrss_sign: String,
    pub antc_cntg_vrss: String,
    pub antc_cntg_prdy_ctrt: String,
    pub antc_vol: String,
    pub stck_prpr: String,
    pub stck_oprc: String,
    pub stck_hgpr: String,
    pub stck_lwpr: String,
    pub stck_sdpr: String,
    pub stck_shrn_iscd: String,
    pub vi_cls_code: String,
}

const TR_PERIOD: TrId = TrId::same("FHKST03010100");

/// 기간별시세 종목 요약 (output1). 필드 전체는 §11 output1 표.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PeriodSummary {
    pub prdy_vrss: String,
    pub prdy_vrss_sign: String,
    pub prdy_ctrt: String,
    pub stck_prdy_clpr: String,
    pub acml_vol: String,
    pub acml_tr_pbmn: String,
    pub hts_kor_isnm: String,
    pub stck_prpr: String,
    pub stck_shrn_iscd: String,
    pub prdy_vol: String,
    pub stck_mxpr: String,
    pub stck_llam: String,
    pub stck_oprc: String,
    pub stck_hgpr: String,
    pub stck_lwpr: String,
    pub stck_prdy_oprc: String,
    pub stck_prdy_hgpr: String,
    pub stck_prdy_lwpr: String,
    pub askp: String,
    pub bidp: String,
    pub prdy_vrss_vol: String,
    pub vol_tnrt: String,
    pub stck_fcam: String,
    pub lstn_stcn: String,
    pub cpfn: String,
    pub hts_avls: String,
    pub per: String,
    pub eps: String,
    pub pbr: String,
    pub itewhol_loan_rmnd_ratem: String,
}

/// 기간별 봉 1건 (output2 배열 요소). 필드 전체는 §11 output2 표.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct PeriodCandle {
    pub stck_bsop_date: String,
    pub stck_clpr: String,
    pub stck_oprc: String,
    pub stck_hgpr: String,
    pub stck_lwpr: String,
    pub acml_vol: String,
    pub acml_tr_pbmn: String,
    pub flng_cls_code: String,
    pub prtt_rate: String,
    pub mod_yn: String,
    pub prdy_vrss_sign: String,
    pub prdy_vrss: String,
    pub revl_issu_reas: String,
}

/// 기간 분류. D=일/W=주/M=월/Y=년.
#[derive(Debug, Clone, Copy)]
pub enum Period {
    Daily,
    Weekly,
    Monthly,
    Yearly,
}

impl Period {
    fn code(self) -> &'static str {
        match self {
            Period::Daily => "D",
            Period::Weekly => "W",
            Period::Monthly => "M",
            Period::Yearly => "Y",
        }
    }
}

const TR_MINUTE: TrId = TrId::same("FHKST03010200");
const TR_MINUTE_DAILY: TrId = TrId::same("FHKST03010230");

/// 분봉 종목 요약 (output1). 필드 전체는 §12 output1 표.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MinuteSummary {
    pub prdy_vrss: String,
    pub prdy_vrss_sign: String,
    pub prdy_ctrt: String,
    pub stck_prdy_clpr: String,
    pub acml_vol: String,
    pub acml_tr_pbmn: String,
    pub hts_kor_isnm: String,
    pub stck_prpr: String,
}

/// 분봉 1건 (output2 배열 요소). 필드 전체는 §12 output2 표.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct MinuteCandle {
    pub stck_bsop_date: String,
    pub stck_cntg_hour: String,
    pub stck_prpr: String,
    pub stck_oprc: String,
    pub stck_hgpr: String,
    pub stck_lwpr: String,
    pub cntg_vol: String,
    pub acml_tr_pbmn: String,
}

impl DomesticStock<'_> {
    /// 주식현재가 시세 (TR 9). `market` KRX/NXT/통합 선택.
    pub async fn current_price(&self, stock_code: &str, market: Market) -> Result<CurrentPrice> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/inquire-price".into(),
                tr_id: TR_PRICE.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.fid_code(),
                    "FID_INPUT_ISCD": stock_code,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }

    /// 주식현재가 호가/예상체결 (TR 10). (호가, 예상체결) 튜플 반환.
    pub async fn asking_price(
        &self,
        stock_code: &str,
        market: Market,
    ) -> Result<(AskingPrice, ExpectedConclusion)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/inquire-asking-price-exp-ccn".into(),
                tr_id: TR_ASKING.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.fid_code(),
                    "FID_INPUT_ISCD": stock_code,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        Ok((resp.field("output1")?, resp.field("output2")?))
    }

    /// 국내주식기간별시세 (TR 11). 최대 100건. (요약, 봉배열) 반환.
    pub async fn period_price(
        &self,
        stock_code: &str,
        start: &str,
        end: &str,
        period: Period,
        adjusted: bool,
        market: Market,
    ) -> Result<(PeriodSummary, Vec<PeriodCandle>)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/inquire-daily-itemchartprice".into(),
                tr_id: TR_PERIOD.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.fid_code(),
                    "FID_INPUT_ISCD": stock_code,
                    "FID_INPUT_DATE_1": start,
                    "FID_INPUT_DATE_2": end,
                    "FID_PERIOD_DIV_CODE": period.code(),
                    "FID_ORG_ADJ_PRC": if adjusted { "0" } else { "1" },
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        Ok((resp.field("output1")?, resp.field("output2")?))
    }

    /// 주식당일분봉조회 (TR 12). `time` HHMMSS, 최대 30건.
    pub async fn minute_chart(
        &self,
        stock_code: &str,
        time: &str,
        include_past: bool,
        market: Market,
    ) -> Result<(MinuteSummary, Vec<MinuteCandle>)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/inquire-time-itemchartprice".into(),
                tr_id: TR_MINUTE.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.fid_code(),
                    "FID_INPUT_ISCD": stock_code,
                    "FID_INPUT_HOUR_1": time,
                    "FID_PW_DATA_INCU_YN": if include_past { "Y" } else { "N" },
                    "FID_ETC_CLS_CODE": "",
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        Ok((resp.field("output1")?, resp.field("output2")?))
    }

    /// 주식일별분봉조회 (특정일 1분봉) — TR `FHKST03010230`.
    ///
    /// `date` YYYYMMDD(과거일 지정 가능), `time` HHMMSS(이 시각 이전 봉부터).
    /// `include_past=true`면 과거 봉 포함. 한 호출 최대 ~120건 — 전 세션은 호출자가
    /// `time`을 내려가며 페이징. 당일분봉([`minute_chart`])과 달리 **임의 날짜** 조회.
    pub async fn minute_chart_by_date(
        &self,
        stock_code: &str,
        date: &str,
        time: &str,
        include_past: bool,
        market: Market,
    ) -> Result<(MinuteSummary, Vec<MinuteCandle>)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/inquire-time-dailychartprice".into(),
                tr_id: TR_MINUTE_DAILY.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.fid_code(),
                    "FID_INPUT_ISCD": stock_code,
                    "FID_INPUT_HOUR_1": time,
                    "FID_INPUT_DATE_1": date,
                    "FID_PW_DATA_INCU_YN": if include_past { "Y" } else { "N" },
                    "FID_FAKE_TICK_INCU_YN": "",
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        Ok((resp.field("output1")?, resp.field("output2")?))
    }
}
