use chrono::{Local, Months, NaiveDate};
use serde::Deserialize;

use crate::client::{ApiCall, KisResponse};
use crate::domestic_stock::DomesticStock;
use crate::error::{KisError, Result};
use crate::trid::TrId;

const TR_PSBL_RVSECNCL: TrId = TrId::real_only("TTTC0084R");
const TR_BALANCE: TrId = TrId::both("TTTC8434R", "VTTC8434R");
const TR_PSBL_ORDER: TrId = TrId::both("TTTC8908R", "VTTC8908R");
const TR_DAILY_CCLD_RECENT: TrId = TrId::both("TTTC0081R", "VTTC0081R");
const TR_DAILY_CCLD_OLD: TrId = TrId::both("CTSC9215R", "VTSC9215R");

/// 정정취소가능주문 1건 (TR5 output 배열 요소). 필드 전체는 §5 응답표.
#[derive(Debug, Clone, Deserialize)]
pub struct RevisableOrder {
    pub ord_gno_brno: String,
    pub odno: String,
    pub orgn_odno: String,
    pub ord_dvsn_name: String,
    pub pdno: String,
    pub prdt_name: String,
    pub rvse_cncl_dvsn_name: String,
    pub ord_qty: String,
    pub ord_unpr: String,
    pub ord_tmd: String,
    pub tot_ccld_qty: String,
    pub tot_ccld_amt: String,
    pub psbl_qty: String,
    pub sll_buy_dvsn_cd: String,
    pub ord_dvsn_cd: String,
    pub mgco_aptm_odno: String,
    pub excg_dvsn_cd: String,
    pub excg_id_dvsn_cd: String,
    pub excg_id_dvsn_name: String,
    pub stpm_cndt_pric: String,
    pub stpm_efct_occr_yn: String,
}

/// 보유종목 1건 (TR6 output1 요소). 필드 전체는 §6 output1 표.
#[derive(Debug, Clone, Deserialize)]
pub struct BalanceItem {
    pub pdno: String,
    pub prdt_name: String,
    pub trad_dvsn_name: String,
    pub bfdy_buy_qty: String,
    pub bfdy_sll_qty: String,
    pub thdt_buyqty: String,
    pub thdt_sll_qty: String,
    pub hldg_qty: String,
    pub ord_psbl_qty: String,
    pub pchs_avg_pric: String,
    pub pchs_amt: String,
    pub prpr: String,
    pub evlu_amt: String,
    pub evlu_pfls_amt: String,
    pub evlu_pfls_rt: String,
    pub evlu_erng_rt: String,
    pub loan_dt: String,
    pub loan_amt: String,
    pub stln_slng_chgs: String,
    pub expd_dt: String,
    pub fltt_rt: String,
    pub bfdy_cprs_icdc: String,
    pub item_mgna_rt_name: String,
    pub grta_rt_name: String,
    pub sbst_pric: String,
    pub stck_loan_unpr: String,
}

/// 계좌 요약 (TR6 output2). 필드 전체는 §6 output2 표.
#[derive(Debug, Clone, Deserialize)]
pub struct BalanceSummary {
    pub dnca_tot_amt: String,
    pub nxdy_excc_amt: String,
    pub prvs_rcdl_excc_amt: String,
    pub cma_evlu_amt: String,
    pub bfdy_buy_amt: String,
    pub thdt_buy_amt: String,
    pub nxdy_auto_rdpt_amt: String,
    pub bfdy_sll_amt: String,
    pub thdt_sll_amt: String,
    pub d2_auto_rdpt_amt: String,
    pub bfdy_tlex_amt: String,
    pub thdt_tlex_amt: String,
    pub tot_loan_amt: String,
    pub scts_evlu_amt: String,
    pub tot_evlu_amt: String,
    pub nass_amt: String,
    pub fncg_gld_auto_rdpt_yn: String,
    pub pchs_amt_smtl_amt: String,
    pub evlu_amt_smtl_amt: String,
    pub evlu_pfls_smtl_amt: String,
    pub tot_stln_slng_chgs: String,
    pub bfdy_tot_asst_evlu_amt: String,
    pub asst_icdc_amt: String,
    pub asst_icdc_erng_rt: String,
}

/// 매수가능 정보 (TR7 output). 필드 전체는 §7 응답표.
#[derive(Debug, Clone, Deserialize)]
pub struct BuyableInfo {
    pub ord_psbl_cash: String,
    pub ord_psbl_sbst: String,
    pub ruse_psbl_amt: String,
    pub fund_rpch_chgs: String,
    pub psbl_qty_calc_unpr: String,
    pub nrcvb_buy_amt: String,
    pub nrcvb_buy_qty: String,
    pub max_buy_amt: String,
    pub max_buy_qty: String,
    pub cma_evlu_amt: String,
    pub ovrs_re_use_amt_wcrc: String,
    pub ord_psbl_frcr_amt_wcrc: String,
}

/// 주문체결 1건 (TR8 output1 요소). 필드 전체는 §8 output1 표.
#[derive(Debug, Clone, Deserialize)]
pub struct DailyConclusion {
    pub ord_dt: String,
    pub ord_gno_brno: String,
    pub odno: String,
    pub orgn_odno: String,
    pub ord_dvsn_name: String,
    pub sll_buy_dvsn_cd: String,
    pub sll_buy_dvsn_cd_name: String,
    pub pdno: String,
    pub prdt_name: String,
    pub ord_qty: String,
    pub ord_unpr: String,
    pub ord_tmd: String,
    pub tot_ccld_qty: String,
    pub avg_prvs: String,
    pub cncl_yn: String,
    pub tot_ccld_amt: String,
    pub loan_dt: String,
    pub ordr_empno: String,
    pub ord_dvsn_cd: String,
    pub cnc_cfrm_qty: String,
    pub rmn_qty: String,
    pub rjct_qty: String,
    pub ccld_cndt_name: String,
    pub inqr_ip_addr: String,
    pub cpbc_ordp_ord_rcit_dvsn_cd: String,
    pub cpbc_ordp_infm_mthd_dvsn_cd: String,
    pub infm_tmd: String,
    pub ctac_tlno: String,
    pub prdt_type_cd: String,
    pub excg_dvsn_cd: String,
    pub cpbc_ordp_mtrl_dvsn_cd: String,
    pub ord_orgno: String,
    pub rsvn_ord_end_dt: String,
    pub excg_id_dvsn_cd: String,
    pub stpm_cndt_pric: String,
    pub stpm_efct_occr_dtmd: String,
}

/// 주문체결 합계 (TR8 output2). 필드 전체는 §8 output2 표.
#[derive(Debug, Clone, Deserialize)]
pub struct DailyConclusionSummary {
    pub tot_ord_qty: String,
    pub tot_ccld_qty: String,
    pub tot_ccld_amt: String,
    pub pchs_avg_pric: String,
    pub prsm_tlex_smtl: String,
}

/// 매도/매수 구분. 잔고·체결 조회 필터.
#[derive(Debug, Clone, Copy)]
pub enum SellBuy {
    All,
    Sell,
    Buy,
}

impl SellBuy {
    fn code(self) -> &'static str {
        match self {
            SellBuy::All => "00",
            SellBuy::Sell => "01",
            SellBuy::Buy => "02",
        }
    }

    fn code_one_digit(self) -> &'static str {
        match self {
            SellBuy::All => "0",
            SellBuy::Sell => "1",
            SellBuy::Buy => "2",
        }
    }
}

impl DomesticStock<'_> {
    /// 주식잔고조회 (TR 6). 한 페이지. envelope의 `data`는 (보유종목, 요약).
    pub async fn balance(
        &self,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<(Vec<BalanceItem>, Vec<BalanceSummary>)>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "AFHR_FLPR_YN": "N",
            "OFL_YN": "",
            "INQR_DVSN": "02",
            "UNPR_DVSN": "01",
            "FUND_STTL_ICLD_YN": "N",
            "FNCG_AMT_AUTO_RDPT_YN": "N",
            "PRCS_DVSN": "00",
            "CTX_AREA_FK100": fk,
            "CTX_AREA_NK100": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/trading/inquire-balance".into(),
                tr_id: TR_BALANCE.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<BalanceItem> = resp.field("output1")?;
        let summary: Vec<BalanceSummary> = resp.field("output2")?;
        Ok(resp.envelope((items, summary)))
    }

    /// 잔고 전체 페이지 수집.
    pub async fn balance_all(&self) -> Result<(Vec<BalanceItem>, Vec<BalanceSummary>)> {
        let mut items = Vec::new();
        let mut summary = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let page = self
                .balance(cursor.as_ref().map(|(f, n)| (f.as_str(), n.as_str())))
                .await?;
            let has_next = page.has_next();
            let next_cursor = match (page.ctx_area_fk.clone(), page.ctx_area_nk.clone()) {
                (Some(f), Some(n)) => Some((f, n)),
                _ => None,
            };
            let (mut it, mut su) = page.data;
            items.append(&mut it);
            summary.append(&mut su);
            if has_next {
                match next_cursor {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            } else {
                break;
            }
        }
        Ok((items, summary))
    }

    /// 매수가능조회 (TR 7). `order_type`은 시장가 권장(증거금율 반영).
    pub async fn buyable(
        &self,
        stock_code: &str,
        price: u64,
        order_type: super::OrderType,
    ) -> Result<BuyableInfo> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "PDNO": stock_code,
            "ORD_UNPR": price.to_string(),
            "ORD_DVSN": order_type.code(),
            "CMA_EVLU_AMT_ICLD_YN": "N",
            "OVRS_ICLD_YN": "N",
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/trading/inquire-psbl-order".into(),
                tr_id: TR_PSBL_ORDER.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }

    /// 주식일별주문체결조회 (TR 8). 조회시작일 기준 3개월 이내/이전 tr_id 자동 선택.
    /// envelope의 `data`는 (체결내역, 합계).
    pub async fn daily_conclusions(
        &self,
        start: &str,
        end: &str,
        sell_buy: SellBuy,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<(Vec<DailyConclusion>, DailyConclusionSummary)>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let start_date = NaiveDate::parse_from_str(start, "%Y%m%d")
            .map_err(|e| KisError::Decode(format!("invalid start date {start}: {e}")))?;
        let cutoff = Local::now()
            .date_naive()
            .checked_sub_months(Months::new(3))
            .ok_or_else(|| KisError::Decode("invalid 3-month cutoff date".into()))?;
        let tr = if start_date >= cutoff {
            TR_DAILY_CCLD_RECENT
        } else {
            TR_DAILY_CCLD_OLD
        };
        let params = self.with_account(serde_json::json!({
            "INQR_STRT_DT": start,
            "INQR_END_DT": end,
            "SLL_BUY_DVSN_CD": sell_buy.code(),
            "CCLD_DVSN": "00",
            "INQR_DVSN": "00",
            "INQR_DVSN_3": "00",
            "PDNO": "",
            "ORD_GNO_BRNO": "",
            "ODNO": "",
            "INQR_DVSN_1": "",
            "EXCG_ID_DVSN_CD": "KRX",
            "CTX_AREA_FK100": fk,
            "CTX_AREA_NK100": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/trading/inquire-daily-ccld".into(),
                tr_id: tr.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<DailyConclusion> = resp.field("output1")?;
        let summary: DailyConclusionSummary = resp.field("output2")?;
        Ok(resp.envelope((items, summary)))
    }

    /// 정정취소가능주문조회 (TR 5). `inqr_dvsn_2`: All/Sell/Buy 필터.
    pub async fn revisable_orders(
        &self,
        sell_buy: SellBuy,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<Vec<RevisableOrder>>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "INQR_DVSN_1": "0",
            "INQR_DVSN_2": sell_buy.code_one_digit(),
            "CTX_AREA_FK100": fk,
            "CTX_AREA_NK100": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl".into(),
                tr_id: TR_PSBL_RVSECNCL.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<RevisableOrder> = resp.field("output")?;
        Ok(resp.envelope(items))
    }
}
