use serde::Deserialize;

use crate::domestic::kis::client::ApiCall;
use crate::domestic::kis::error::Result;
use crate::domestic::kis::overseas_stock::{OverseasExchange, OverseasStock};
use crate::domestic::kis::trid::TrId;

const TR_RVSECNCL: TrId = TrId::both("TTTT1004U", "VTTT1004U");

/// 매수 거래소별 tr_id. overseas-stock.md §1 표 verbatim.
fn buy_tr(exchange: OverseasExchange) -> TrId {
    use OverseasExchange::*;
    match exchange {
        Nasd | Nyse | Amex => TrId::both("TTTT1002U", "VTTT1002U"),
        Sehk => TrId::both("TTTS1002U", "VTTS1002U"),
        Shaa => TrId::both("TTTS0202U", "VTTS0202U"),
        Szaa => TrId::both("TTTS0305U", "VTTS0305U"),
        Tkse => TrId::both("TTTS0308U", "VTTS0308U"),
        Hase | Vnse => TrId::both("TTTS0311U", "VTTS0311U"),
    }
}

/// 매도 거래소별 tr_id. overseas-stock.md §2 표 verbatim.
fn sell_tr(exchange: OverseasExchange) -> TrId {
    use OverseasExchange::*;
    match exchange {
        Nasd | Nyse | Amex => TrId::both("TTTT1006U", "VTTT1006U"),
        Sehk => TrId::both("TTTS1001U", "VTTS1001U"),
        Shaa => TrId::both("TTTS1005U", "VTTS1005U"),
        Szaa => TrId::both("TTTS0304U", "VTTS0304U"),
        Tkse => TrId::both("TTTS0307U", "VTTS0307U"),
        Hase | Vnse => TrId::both("TTTS0310U", "VTTS0310U"),
    }
}

/// 해외주문 응답 (output). 매수/매도/정정취소 공통. §1·§3 응답표 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OverseasOrderResult {
    /// 한국거래소전송주문조직번호 — 정정/취소 시 사용.
    #[serde(alias = "KRX_FWDG_ORD_ORGNO", alias = "krx_fwdg_ord_orgno")]
    pub krx_fwdg_ord_orgno: String,
    /// 주문번호 — 정정/취소 시 사용.
    #[serde(alias = "ODNO", alias = "odno")]
    pub odno: String,
    /// 주문시각.
    #[serde(alias = "ORD_TMD", alias = "ord_tmd")]
    pub ord_tmd: String,
}

/// 해외 주문구분 — KIS `ORD_DVSN` 코드. 자주 쓰는 1종 + 임의 코드 탈출구.
#[derive(Debug, Clone)]
pub enum OverseasOrderType {
    /// "00" 지정가.
    Limit,
    /// 그 외 ORD_DVSN 코드 (31:MOO, 32:LOO, 33:MOC, 34:LOC 등).
    Code(String),
}

impl OverseasOrderType {
    pub(crate) fn code(&self) -> &str {
        match self {
            OverseasOrderType::Limit => "00",
            OverseasOrderType::Code(c) => c,
        }
    }
}

/// 해외 매수/매도 주문 파라미터.
#[derive(Debug, Clone)]
pub struct OverseasOrderReq {
    pub exchange: OverseasExchange,
    /// 종목코드 (예 `AAPL`).
    pub symbol: String,
    pub order_type: OverseasOrderType,
    /// 주문수량.
    pub quantity: u64,
    /// 해외주문단가. 시장가류는 0.
    pub price: f64,
}

/// 해외 정정/취소 파라미터. 원주문의 OverseasOrderResult에서 식별자 획득.
#[derive(Debug, Clone)]
pub struct OverseasReviseCancelReq {
    pub exchange: OverseasExchange,
    pub symbol: String,
    /// 원주문번호 ORGN_ODNO.
    pub orig_order_no: String,
    pub order_type: OverseasOrderType,
    pub quantity: u64,
    /// 주문단가. 취소 시 0.
    pub price: f64,
}

impl OverseasStock<'_> {
    /// 해외주식 매수 (TR 1).
    pub async fn buy(&self, req: OverseasOrderReq) -> Result<OverseasOrderResult> {
        let tr = buy_tr(req.exchange);
        self.order(req, tr).await
    }

    /// 해외주식 매도 (TR 2).
    pub async fn sell(&self, req: OverseasOrderReq) -> Result<OverseasOrderResult> {
        let tr = sell_tr(req.exchange);
        self.order(req, tr).await
    }

    async fn order(&self, req: OverseasOrderReq, tr: TrId) -> Result<OverseasOrderResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "OVRS_EXCG_CD": req.exchange.ovrs_excg_cd(),
            "PDNO": req.symbol,
            "ORD_QTY": req.quantity.to_string(),
            "OVRS_ORD_UNPR": req.price.to_string(),
            "ORD_DVSN": req.order_type.code(),
            "CTAC_TLNO": "",
            "MGCO_APTM_ODNO": "",
            "SLL_TYPE": "",
            "ORD_SVR_DVSN_CD": "0",
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/overseas-stock/v1/trading/order".into(),
                tr_id: tr.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }

    /// 해외주식 주문 정정 (TR 3). `RVSE_CNCL_DVSN_CD=01`.
    pub async fn revise(&self, req: OverseasReviseCancelReq) -> Result<OverseasOrderResult> {
        self.order_rvsecncl(req, "01").await
    }

    /// 해외주식 주문 취소 (TR 3). `RVSE_CNCL_DVSN_CD=02`.
    pub async fn cancel(&self, req: OverseasReviseCancelReq) -> Result<OverseasOrderResult> {
        self.order_rvsecncl(req, "02").await
    }

    async fn order_rvsecncl(
        &self,
        req: OverseasReviseCancelReq,
        dvsn: &str,
    ) -> Result<OverseasOrderResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "OVRS_EXCG_CD": req.exchange.ovrs_excg_cd(),
            "PDNO": req.symbol,
            "ORGN_ODNO": req.orig_order_no,
            "RVSE_CNCL_DVSN_CD": dvsn,
            "ORD_QTY": req.quantity.to_string(),
            "OVRS_ORD_UNPR": req.price.to_string(),
            "MGCO_APTM_ODNO": "",
            "ORD_SVR_DVSN_CD": "0",
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/overseas-stock/v1/trading/order-rvsecncl".into(),
                tr_id: TR_RVSECNCL.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }
}
