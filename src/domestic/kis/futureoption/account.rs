use serde::Deserialize;

use crate::domestic::kis::client::{ApiCall, KisResponse};
use crate::domestic::kis::error::Result;
use crate::domestic::kis::futureoption::{FutureOption, SellBuy};
use crate::domestic::kis::trid::TrId;

const TR_BALANCE: TrId = TrId::both("CTFO6118R", "VTFO6118R");
const TR_CCNL: TrId = TrId::both("TTTO5201R", "VTTO5201R");
const TR_PSBL_ORDER: TrId = TrId::both("TTTO5105R", "VTTO5105R");

/// 잔고 종목 1건 (TR3 output1 추정). futureoption.md §3 통합표 종목 단위 필드 verbatim.
/// output1/2 귀속이 chk 코드상 불명확 — 전 필드 `#[serde(default)]`. 모의 실호출로 확정.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionBalanceItem {
    pub cano: String,              // 종합계좌번호
    pub acnt_prdt_cd: String,      // 계좌상품코드
    pub pdno: String,              // 상품번호
    pub prdt_type_cd: String,      // 상품유형코드
    pub shtn_pdno: String,         // 단축상품번호
    pub prdt_name: String,         // 상품명
    pub sll_buy_dvsn_name: String, // 매도매수구분명
    pub cblc_qty: String,          // 잔고수량
    pub excc_unpr: String,         // 정산단가
    pub ccld_avg_unpr1: String,    // 체결평균단가1
    pub idx_clpr: String,          // 지수종가
    pub pchs_amt: String,          // 매입금액
    pub evlu_amt: String,          // 평가금액
    pub evlu_pfls_amt: String,     // 평가손익금액
    pub trad_pfls_amt: String,     // 매매손익금액
    pub lqd_psbl_qty: String,      // 청산가능수량
}

/// 잔고 계좌 요약 (TR3 output2 추정). §3 통합표 계좌/예수금/증거금 필드 verbatim.
/// output1/2 귀속이 chk 코드상 불명확 — 전 필드 `#[serde(default)]`. 모의 실호출로 확정.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionBalanceSummary {
    pub dnca_cash: String,          // 예수금현금
    pub frcr_dncl_amt: String,      // 외화예수금액
    pub dnca_sbst: String,          // 예수금대용
    pub tot_dncl_amt: String,       // 총예수금액
    pub tot_ccld_amt: String,       // 총체결금액
    pub cash_mgna: String,          // 현금증거금
    pub sbst_mgna: String,          // 대용증거금
    pub mgna_tota: String,          // 증거금총액
    pub opt_dfpa: String,           // 옵션차금
    pub thdt_dfpa: String,          // 당일차금
    pub rnwl_dfpa: String,          // 갱신차금
    pub fee: String,                // 수수료
    pub nxdy_dnca: String,          // 익일예수금
    pub nxdy_dncl_amt: String,      // 익일예수금액
    pub prsm_dpast: String,         // 추정예탁자산
    pub prsm_dpast_amt: String,     // 추정예탁자산금액
    pub pprt_ord_psbl_cash: String, // 적정주문가능현금
    pub add_mgna_cash: String,      // 추가증거금현금
    pub add_mgna_tota: String,      // 추가증거금총액
    pub futr_trad_pfls_amt: String, // 선물매매손익금액
    pub opt_trad_pfls_amt: String,  // 옵션매매손익금액
    pub futr_evlu_pfls_amt: String, // 선물평가손익금액
    pub opt_evlu_pfls_amt: String,  // 옵션평가손익금액
    pub trad_pfls_amt_smtl: String, // 매매손익금액합계
    pub evlu_pfls_amt_smtl: String, // 평가손익금액합계
    pub wdrw_psbl_tot_amt: String,  // 인출가능총금액
    pub ord_psbl_cash: String,      // 주문가능현금
    pub ord_psbl_sbst: String,      // 주문가능대용
    pub ord_psbl_tota: String,      // 주문가능총액
    pub pchs_amt_smtl: String,      // 매입금액합계
    pub evlu_amt_smtl: String,      // 평가금액합계
}

/// 주문체결 1건 (TR4 output1 추정). §4 통합표 주문/체결 단위 필드 verbatim.
/// output1/2 귀속이 chk 코드상 불명확 — 전 필드 `#[serde(default)]`. 모의 실호출로 확정.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionConclusion {
    pub ord_gno_brno: String,             // 주문채번지점번호
    pub cano: String,                     // 종합계좌번호
    pub csac_name: String,                // 종합계좌명
    pub acnt_prdt_cd: String,             // 계좌상품코드
    pub ord_dt: String,                   // 주문일자
    pub odno: String,                     // 주문번호
    pub orgn_odno: String,                // 원주문번호
    pub sll_buy_dvsn_cd: String,          // 매도매수구분코드
    pub trad_dvsn_name: String,           // 매매구분명
    pub nmpr_type_cd: String,             // 호가유형코드
    pub nmpr_type_name: String,           // 호가유형명
    pub pdno: String,                     // 상품번호
    pub prdt_name: String,                // 상품명
    pub prdt_type_cd: String,             // 상품유형코드
    pub ord_qty: String,                  // 주문수량
    pub ord_idx: String,                  // 주문지수
    pub qty: String,                      // 잔량
    pub ord_tmd: String,                  // 주문시각
    pub tot_ccld_qty: String,             // 총체결수량
    pub avg_idx: String,                  // 평균지수
    pub tot_ccld_amt: String,             // 총체결금액
    pub rjct_qty: String,                 // 거부수량
    pub ingr_trad_rjct_rson_cd: String,   // 장내매매거부사유코드
    pub ingr_trad_rjct_rson_name: String, // 장내매매거부사유명
    pub ord_stfno: String,                // 주문직원번호
    pub sprd_item_yn: String,             // 스프레드종목여부
    pub ord_ip_addr: String,              // 주문IP주소
}

/// 주문체결 요약 (TR4 output2 추정). §4 통합표 합계 필드 verbatim.
/// output1/2 귀속이 chk 코드상 불명확 — 전 필드 `#[serde(default)]`. 모의 실호출로 확정.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionConclusionSummary {
    pub tot_ord_qty: String,       // 총주문수량
    pub tot_ccld_amt_smtl: String, // 총체결금액합계
    pub tot_ccld_qty_smtl: String, // 총체결수량합계
    pub fee_smtl: String,          // 수수료합계
    pub ctac_tlno: String,         // 연락전화번호
}

/// 매수가능 정보 (TR5 output). futureoption.md §5 응답표 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionBuyable {
    pub tot_psbl_qty: String,  // 총가능수량
    pub lqd_psbl_qty1: String, // 청산가능수량1
    pub ord_psbl_qty: String,  // 주문가능수량
    pub bass_idx: String,      // 기준지수
}

/// 매도/매수 구분. 체결내역 필터 (`SLL_BUY_DVSN_CD`).
#[derive(Debug, Clone, Copy)]
pub enum CcnlSellBuy {
    All,
    Sell,
    Buy,
}

impl CcnlSellBuy {
    fn code(self) -> &'static str {
        match self {
            CcnlSellBuy::All => "00",
            CcnlSellBuy::Sell => "01",
            CcnlSellBuy::Buy => "02",
        }
    }
}

/// 체결/미체결 구분. 체결내역 필터 (`CCLD_NCCS_DVSN`).
#[derive(Debug, Clone, Copy)]
pub enum CcnlFilter {
    All,
    Filled,
    Unfilled,
}

impl CcnlFilter {
    fn code(self) -> &'static str {
        match self {
            CcnlFilter::All => "00",
            CcnlFilter::Filled => "01",
            CcnlFilter::Unfilled => "02",
        }
    }
}

impl FutureOption<'_> {
    /// 선물옵션 잔고현황 (TR 3). 한 페이지. envelope `data`는 (잔고종목, 요약).
    /// `MGNA_DVSN`=01(게시)/02(유지), `EXCC_STAT_CD`=1(정산)/2(본정산).
    pub async fn balance(
        &self,
        margin_div: &str,
        excc_stat: &str,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<(Vec<FutureOptionBalanceItem>, FutureOptionBalanceSummary)>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "MGNA_DVSN": margin_div,
            "EXCC_STAT_CD": excc_stat,
            "CTX_AREA_FK200": fk,
            "CTX_AREA_NK200": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-futureoption/v1/trading/inquire-balance".into(),
                tr_id: TR_BALANCE.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<FutureOptionBalanceItem> = resp.field("output1")?;
        let summary: FutureOptionBalanceSummary = resp.field("output2")?;
        Ok(resp.envelope((items, summary)))
    }

    /// 선물옵션 잔고현황 전체 페이지 수집.
    pub async fn balance_all(
        &self,
        margin_div: &str,
        excc_stat: &str,
    ) -> Result<(
        Vec<FutureOptionBalanceItem>,
        Vec<FutureOptionBalanceSummary>,
    )> {
        let mut items = Vec::new();
        let mut summaries = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let page = self
                .balance(
                    margin_div,
                    excc_stat,
                    cursor.as_ref().map(|(f, n)| (f.as_str(), n.as_str())),
                )
                .await?;
            let has_next = page.has_next();
            let next_cursor = match (page.ctx_area_fk.clone(), page.ctx_area_nk.clone()) {
                (Some(f), Some(n)) => Some((f, n)),
                _ => None,
            };
            let (mut it, su) = page.data;
            items.append(&mut it);
            summaries.push(su);
            if has_next {
                match next_cursor {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            } else {
                break;
            }
        }
        Ok((items, summaries))
    }

    /// 선물옵션 주문체결내역조회 (TR 4). 한 페이지. envelope `data`는 (체결내역, 요약).
    #[allow(clippy::too_many_arguments)]
    pub async fn conclusions(
        &self,
        start: &str,
        end: &str,
        sell_buy: CcnlSellBuy,
        filter: CcnlFilter,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<(Vec<FutureOptionConclusion>, FutureOptionConclusionSummary)>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "STRT_ORD_DT": start,
            "END_ORD_DT": end,
            "SLL_BUY_DVSN_CD": sell_buy.code(),
            "CCLD_NCCS_DVSN": filter.code(),
            "SORT_SQN": "DS",
            "PDNO": "",
            "STRT_ODNO": "",
            "MKET_ID_CD": "",
            "CTX_AREA_FK200": fk,
            "CTX_AREA_NK200": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-futureoption/v1/trading/inquire-ccnl".into(),
                tr_id: TR_CCNL.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<FutureOptionConclusion> = resp.field("output1")?;
        let summary: FutureOptionConclusionSummary = resp.field("output2")?;
        Ok(resp.envelope((items, summary)))
    }

    /// 선물옵션 주문체결내역 전체 페이지 수집.
    pub async fn conclusions_all(
        &self,
        start: &str,
        end: &str,
        sell_buy: CcnlSellBuy,
        filter: CcnlFilter,
    ) -> Result<Vec<FutureOptionConclusion>> {
        let mut all = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let page = self
                .conclusions(
                    start,
                    end,
                    sell_buy,
                    filter,
                    cursor.as_ref().map(|(f, n)| (f.as_str(), n.as_str())),
                )
                .await?;
            let has_next = page.has_next();
            let next_cursor = match (page.ctx_area_fk.clone(), page.ctx_area_nk.clone()) {
                (Some(f), Some(n)) => Some((f, n)),
                _ => None,
            };
            let (mut it, _su) = page.data;
            all.append(&mut it);
            if has_next {
                match next_cursor {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            } else {
                break;
            }
        }
        Ok(all)
    }

    /// 선물옵션 매수가능조회 (TR 5). 연속조회 미지원 — 단건.
    pub async fn buyable(
        &self,
        item_code: &str,
        sell_buy: SellBuy,
        price: f64,
        ord_dvsn_cd: &str,
    ) -> Result<FutureOptionBuyable> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "PDNO": item_code,
            "SLL_BUY_DVSN_CD": sell_buy.code(),
            "UNIT_PRICE": price.to_string(),
            "ORD_DVSN_CD": ord_dvsn_cd,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-futureoption/v1/trading/inquire-psbl-order".into(),
                tr_id: TR_PSBL_ORDER.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }
}
