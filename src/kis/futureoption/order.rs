use serde::Deserialize;

use crate::kis::client::ApiCall;
use crate::kis::error::Result;
use crate::kis::futureoption::{FutureOption, Session};
use crate::kis::trid::TrId;

/// 주문 세션별 tr_id. futureoption.md §1 표 verbatim.
fn order_tr(session: Session) -> TrId {
    match session {
        Session::Day => TrId::both("TTTO1101U", "VTTO1101U"),
        Session::Night => TrId::real_only("STTN1101U"),
    }
}

/// 정정취소 세션별 tr_id. futureoption.md §2 표 verbatim.
fn rvsecncl_tr(session: Session) -> TrId {
    match session {
        Session::Day => TrId::both("TTTO1103U", "VTTO1103U"),
        Session::Night => TrId::real_only("TTTN1103U"),
    }
}

/// 선물옵션 주문 응답 (output). futureoption.md §1 응답표 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionOrderResult {
    /// 한국거래소전송주문조직번호.
    #[serde(alias = "KRX_FWDG_ORD_ORGNO", alias = "krx_fwdg_ord_orgno")]
    pub krx_fwdg_ord_orgno: String,
    /// 주문번호.
    #[serde(alias = "ODNO", alias = "odno")]
    pub odno: String,
    /// 주문시각.
    #[serde(alias = "ORD_TMD", alias = "ord_tmd")]
    pub ord_tmd: String,
}

/// 선물옵션 정정취소 응답 (output). futureoption.md §2 응답표 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionReviseCancelResult {
    #[serde(alias = "ACNT_NAME", alias = "acnt_name")]
    pub acnt_name: String, // 계좌명
    #[serde(alias = "TRAD_DVSN_NAME", alias = "trad_dvsn_name")]
    pub trad_dvsn_name: String, // 매매구분명
    #[serde(alias = "ITEM_NAME", alias = "item_name")]
    pub item_name: String, // 종목명
    #[serde(alias = "ORD_TMD", alias = "ord_tmd")]
    pub ord_tmd: String, // 주문시각
    #[serde(alias = "ORD_GNO_BRNO", alias = "ord_gno_brno")]
    pub ord_gno_brno: String, // 주문채번지점번호
    #[serde(alias = "ORGN_ODNO", alias = "orgn_odno")]
    pub orgn_odno: String, // 원주문번호
    #[serde(alias = "ODNO", alias = "odno")]
    pub odno: String, // 주문번호
}

/// 매도/매수 구분 (`SLL_BUY_DVSN_CD`).
#[derive(Debug, Clone, Copy)]
pub enum SellBuy {
    /// `01` 매도.
    Sell,
    /// `02` 매수.
    Buy,
}

impl SellBuy {
    pub(crate) fn code(self) -> &'static str {
        match self {
            SellBuy::Sell => "01",
            SellBuy::Buy => "02",
        }
    }
}

/// 호가유형 — `NMPR_TYPE_CD` + 대응 `ORD_DVSN_CD`. 자주 쓰는 2종 + 코드 탈출구.
#[derive(Debug, Clone)]
pub enum FoOrderType {
    /// 지정가 (NMPR_TYPE_CD=01, ORD_DVSN_CD=01).
    Limit,
    /// 시장가 (NMPR_TYPE_CD=02, ORD_DVSN_CD=02).
    Market,
    /// 임의 코드 (nmpr_type_cd, ord_dvsn_cd).
    Code {
        nmpr_type_cd: String,
        ord_dvsn_cd: String,
    },
}

impl FoOrderType {
    fn nmpr_type_cd(&self) -> &str {
        match self {
            FoOrderType::Limit => "01",
            FoOrderType::Market => "02",
            FoOrderType::Code { nmpr_type_cd, .. } => nmpr_type_cd,
        }
    }

    fn ord_dvsn_cd(&self) -> &str {
        match self {
            FoOrderType::Limit => "01",
            FoOrderType::Market => "02",
            FoOrderType::Code { ord_dvsn_cd, .. } => ord_dvsn_cd,
        }
    }
}

/// 선물옵션 주문 파라미터.
#[derive(Debug, Clone)]
pub struct FutureOptionOrderReq {
    pub session: Session,
    pub sell_buy: SellBuy,
    /// 단축상품번호 (선물 6자리 예 `101W09`, 옵션 9자리 예 `201S03370`).
    pub item_code: String,
    pub order_type: FoOrderType,
    pub quantity: u64,
    /// 주문가격. 시장가·최유리는 0.
    pub price: f64,
}

/// 선물옵션 정정/취소 파라미터.
#[derive(Debug, Clone)]
pub struct FutureOptionReviseCancelReq {
    pub session: Session,
    /// 원주문번호 ORGN_ODNO.
    pub orig_order_no: String,
    pub order_type: FoOrderType,
    /// 주문수량. `all=true`면 0(전량).
    pub quantity: u64,
    /// 주문가격. 취소·시장가는 0.
    pub price: f64,
    /// 잔량 전부 대상이면 true.
    pub all: bool,
}

impl FutureOption<'_> {
    /// 선물옵션 주문 (TR 1).
    pub async fn order(&self, req: FutureOptionOrderReq) -> Result<FutureOptionOrderResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "ORD_PRCS_DVSN_CD": "02",
            "SLL_BUY_DVSN_CD": req.sell_buy.code(),
            "SHTN_PDNO": req.item_code,
            "ORD_QTY": req.quantity.to_string(),
            "UNIT_PRICE": req.price.to_string(),
            "NMPR_TYPE_CD": req.order_type.nmpr_type_cd(),
            "KRX_NMPR_CNDT_CD": "0",
            "ORD_DVSN_CD": req.order_type.ord_dvsn_cd(),
            "CTAC_TLNO": "",
            "FUOP_ITEM_DVSN_CD": "",
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/domestic-futureoption/v1/trading/order".into(),
                tr_id: order_tr(req.session).resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }

    /// 선물옵션 주문 정정 (TR 2). `RVSE_CNCL_DVSN_CD=01`.
    pub async fn revise(
        &self,
        req: FutureOptionReviseCancelReq,
    ) -> Result<FutureOptionReviseCancelResult> {
        self.order_rvsecncl(req, "01").await
    }

    /// 선물옵션 주문 취소 (TR 2). `RVSE_CNCL_DVSN_CD=02`.
    pub async fn cancel(
        &self,
        req: FutureOptionReviseCancelReq,
    ) -> Result<FutureOptionReviseCancelResult> {
        self.order_rvsecncl(req, "02").await
    }

    async fn order_rvsecncl(
        &self,
        req: FutureOptionReviseCancelReq,
        dvsn: &str,
    ) -> Result<FutureOptionReviseCancelResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "ORD_PRCS_DVSN_CD": "02",
            "RVSE_CNCL_DVSN_CD": dvsn,
            "ORGN_ODNO": req.orig_order_no,
            "ORD_QTY": if req.all { "0".to_string() } else { req.quantity.to_string() },
            "UNIT_PRICE": req.price.to_string(),
            "NMPR_TYPE_CD": req.order_type.nmpr_type_cd(),
            "KRX_NMPR_CNDT_CD": "0",
            "RMN_QTY_YN": if req.all { "Y" } else { "N" },
            "ORD_DVSN_CD": req.order_type.ord_dvsn_cd(),
            "FUOP_ITEM_DVSN_CD": "",
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/domestic-futureoption/v1/trading/order-rvsecncl".into(),
                tr_id: rvsecncl_tr(req.session).resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }
}
