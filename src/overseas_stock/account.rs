use serde::Deserialize;

use crate::client::{ApiCall, KisResponse};
use crate::error::Result;
use crate::overseas_stock::{OverseasExchange, OverseasStock};
use crate::trid::TrId;

const TR_BALANCE: TrId = TrId::both("TTTS3012R", "VTTS3012R");
const TR_NCCS: TrId = TrId::real_only("TTTS3018R");
const TR_CCNL: TrId = TrId::both("TTTS3035R", "VTTS3035R");

/// 보유종목 1건 (TR4 output1 추정). overseas-stock.md §4 output1 표 verbatim.
/// output1/2 귀속이 명세상 불명확 — 전 필드 `#[serde(default)]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OverseasBalanceItem {
    pub cano: String,               // 종합계좌번호
    pub acnt_prdt_cd: String,       // 계좌상품코드
    pub prdt_type_cd: String,       // 상품유형코드
    pub ovrs_pdno: String,          // 해외상품번호(종목코드)
    pub ovrs_item_name: String,     // 해외종목명 [미확인 — 샘플 매핑 누락]
    pub frcr_evlu_pfls_amt: String, // 외화평가손익금액
    pub evlu_pfls_rt: String,       // 평가손익율
    pub pchs_avg_pric: String,      // 매입평균가격
    pub ovrs_cblc_qty: String,      // 해외잔고수량
    pub ord_psbl_qty: String,       // 주문가능수량
    pub frcr_pchs_amt1: String,     // 외화매입금액1
    pub ovrs_stck_evlu_amt: String, // 해외주식평가금액
    pub now_pric2: String,          // 현재가격2
    pub tr_crcy_cd: String,         // 거래통화코드
    pub ovrs_excg_cd: String,       // 해외거래소코드
    pub loan_type_cd: String,       // 대출유형코드
    pub loan_dt: String,            // 대출일자
    pub expd_dt: String,            // 만기일자
}

/// 잔고 요약 (TR4 output2 추정). §4 output2 표 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OverseasBalanceSummary {
    pub frcr_buy_amt_smtl1: String,  // 외화매수금액합계1
    pub frcr_buy_amt_smtl2: String,  // 외화매수금액합계2
    pub ovrs_rlzt_pfls_amt: String,  // 해외실현손익금액
    pub ovrs_rlzt_pfls_amt2: String, // 해외실현손익금액2
    pub ovrs_tot_pfls: String,       // 해외총손익
    pub rlzt_erng_rt: String,        // 실현수익율
    pub tot_evlu_pfls_amt: String,   // 총평가손익금액
    pub tot_pftrt: String,           // 총수익률
}

/// 미체결 주문 1건 (TR5 output 배열 요소). §5 응답표 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OverseasUnfilledOrder {
    pub ord_dt: String,               // 주문일자
    pub ord_gno_brno: String,         // 주문채번지점번호
    pub odno: String,                 // 주문번호
    pub orgn_odno: String,            // 원주문번호
    pub pdno: String,                 // 상품번호
    pub sll_buy_dvsn_cd: String,      // 매도매수구분코드
    pub rvse_cncl_dvsn_cd: String,    // 정정취소구분코드
    pub rjct_rson: String,            // 거부사유
    pub ord_tmd: String,              // 주문시각
    pub tr_crcy_cd: String,           // 거래통화코드
    pub natn_cd: String,              // 국가코드
    pub ft_ord_qty: String,           // FT주문수량
    pub ft_ccld_qty: String,          // FT체결수량
    pub nccs_qty: String,             // 미체결수량
    pub ft_ord_unpr3: String,         // FT주문단가3
    pub ft_ccld_unpr3: String,        // FT체결단가3
    pub ft_ccld_amt3: String,         // FT체결금액3
    pub ovrs_excg_cd: String,         // 해외거래소코드
    pub loan_type_cd: String,         // 대출유형코드
    pub loan_dt: String,              // 대출일자
    pub usa_amk_exts_rqst_yn: String, // 미국애프터마켓연장신청여부
}

/// 주문체결 1건 (TR6 output 배열 요소). §6 응답표 32필드 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OverseasConclusion {
    pub ord_dt: String,               // 주문일자
    pub ord_gno_brno: String,         // 주문채번지점번호
    pub odno: String,                 // 주문번호
    pub orgn_odno: String,            // 원주문번호
    pub sll_buy_dvsn_cd: String,      // 매도매수구분코드
    pub sll_buy_dvsn_cd_name: String, // 매도매수구분코드명
    pub rvse_cncl_dvsn: String,       // 정정취소구분
    pub rvse_cncl_dvsn_name: String,  // 정정취소구분명
    pub pdno: String,                 // 상품번호
    pub prdt_name: String,            // 상품명
    pub ft_ord_qty: String,           // FT주문수량
    pub ft_ord_unpr3: String,         // FT주문단가3
    pub ft_ccld_qty: String,          // FT체결수량
    pub ft_ccld_unpr3: String,        // FT체결단가3
    pub ft_ccld_amt3: String,         // FT체결금액3
    pub nccs_qty: String,             // 미체결수량
    pub prcs_stat_name: String,       // 처리상태명
    pub rjct_rson: String,            // 거부사유
    pub rjct_rson_name: String,       // 거부사유명
    pub ord_tmd: String,              // 주문시각
    pub tr_mket_name: String,         // 거래시장명
    pub tr_crcy_cd: String,           // 거래통화코드
    pub tr_natn: String,              // 거래국가
    pub tr_natn_name: String,         // 거래국가명
    pub ovrs_excg_cd: String,         // 해외거래소코드
    pub dmst_ord_dt: String,          // 국내주문일자
    pub thco_ord_tmd: String,         // 당사주문시각
    pub loan_type_cd: String,         // 대출유형코드
    pub loan_dt: String,              // 대출일자
    pub mdia_dvsn_name: String,       // 매체구분명
    pub usa_amk_exts_rqst_yn: String, // 미국애프터마켓연장신청여부
    pub splt_buy_attr_name: String,   // 분할매수/매도속성명
}

/// 매도/매수 구분. 체결내역 조회 필터 (`SLL_BUY_DVSN`).
#[derive(Debug, Clone, Copy)]
pub enum OverseasSellBuy {
    All,
    Sell,
    Buy,
}

impl OverseasSellBuy {
    fn code(self) -> &'static str {
        match self {
            OverseasSellBuy::All => "00",
            OverseasSellBuy::Sell => "01",
            OverseasSellBuy::Buy => "02",
        }
    }
}

/// 체결/미체결 구분. 체결내역 조회 필터 (`CCLD_NCCS_DVSN`).
#[derive(Debug, Clone, Copy)]
pub enum FilledFilter {
    All,
    Filled,
    Unfilled,
}

impl FilledFilter {
    fn code(self) -> &'static str {
        match self {
            FilledFilter::All => "00",
            FilledFilter::Filled => "01",
            FilledFilter::Unfilled => "02",
        }
    }
}

impl OverseasStock<'_> {
    /// 해외주식 잔고 (TR 4). 한 페이지. envelope의 `data`는 (보유종목, 요약).
    pub async fn balance(
        &self,
        exchange: OverseasExchange,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<(Vec<OverseasBalanceItem>, Vec<OverseasBalanceSummary>)>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "OVRS_EXCG_CD": exchange.ovrs_excg_cd(),
            "TR_CRCY_CD": exchange.currency(),
            "CTX_AREA_FK200": fk,
            "CTX_AREA_NK200": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/overseas-stock/v1/trading/inquire-balance".into(),
                tr_id: TR_BALANCE.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<OverseasBalanceItem> = resp.field("output1")?;
        let summary: Vec<OverseasBalanceSummary> = resp.field("output2")?;
        Ok(resp.envelope((items, summary)))
    }

    /// 해외주식 잔고 전체 페이지 수집.
    pub async fn balance_all(
        &self,
        exchange: OverseasExchange,
    ) -> Result<(Vec<OverseasBalanceItem>, Vec<OverseasBalanceSummary>)> {
        let mut items = Vec::new();
        let mut summary = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let page = self
                .balance(
                    exchange,
                    cursor.as_ref().map(|(f, n)| (f.as_str(), n.as_str())),
                )
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

    /// 해외주식 미체결내역 (TR 5). 모의투자 미지원 → 모의 환경에서 `UnsupportedInMock`.
    pub async fn unfilled_orders(
        &self,
        exchange: OverseasExchange,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<Vec<OverseasUnfilledOrder>>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "OVRS_EXCG_CD": exchange.ovrs_excg_cd(),
            "SORT_SQN": "",
            "CTX_AREA_FK200": fk,
            "CTX_AREA_NK200": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/overseas-stock/v1/trading/inquire-nccs".into(),
                tr_id: TR_NCCS.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<OverseasUnfilledOrder> = resp.field("output")?;
        Ok(resp.envelope(items))
    }

    /// 해외주식 주문체결내역 (TR 6). 기간 조회. 한 페이지.
    #[allow(clippy::too_many_arguments)]
    pub async fn conclusions(
        &self,
        start: &str,
        end: &str,
        sell_buy: OverseasSellBuy,
        filled: FilledFilter,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<Vec<OverseasConclusion>>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "PDNO": "%",
            "ORD_STRT_DT": start,
            "ORD_END_DT": end,
            "SLL_BUY_DVSN": sell_buy.code(),
            "CCLD_NCCS_DVSN": filled.code(),
            "OVRS_EXCG_CD": "%",
            "SORT_SQN": "DS",
            "ORD_DT": "",
            "ORD_GNO_BRNO": "",
            "ODNO": "",
            "CTX_AREA_FK200": fk,
            "CTX_AREA_NK200": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/overseas-stock/v1/trading/inquire-ccnl".into(),
                tr_id: TR_CCNL.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<OverseasConclusion> = resp.field("output")?;
        Ok(resp.envelope(items))
    }

    /// 해외주식 주문체결내역 전체 페이지 수집.
    pub async fn conclusions_all(
        &self,
        start: &str,
        end: &str,
        sell_buy: OverseasSellBuy,
        filled: FilledFilter,
    ) -> Result<Vec<OverseasConclusion>> {
        let mut all = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let page = self
                .conclusions(
                    start,
                    end,
                    sell_buy,
                    filled,
                    cursor.as_ref().map(|(f, n)| (f.as_str(), n.as_str())),
                )
                .await?;
            let has_next = page.has_next();
            let next_cursor = match (page.ctx_area_fk.clone(), page.ctx_area_nk.clone()) {
                (Some(f), Some(n)) => Some((f, n)),
                _ => None,
            };
            all.extend(page.data);
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
}
