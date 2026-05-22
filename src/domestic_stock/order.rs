use serde::Deserialize;

use crate::client::ApiCall;
use crate::domestic_stock::DomesticStock;
use crate::error::Result;
use crate::trid::TrId;

const TR_BUY: TrId = TrId::both("TTTC0012U", "VTTC0012U");
const TR_SELL: TrId = TrId::both("TTTC0011U", "VTTC0011U");
const TR_RVSECNCL: TrId = TrId::both("TTTC0013U", "VTTC0013U");

/// 주문 응답 (output). 매수/매도/정정/취소 공통.
#[derive(Debug, Clone, Deserialize)]
pub struct OrderResult {
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

/// 주문 구분 — KIS `ORD_DVSN` 코드. 자주 쓰는 2종 + 임의 코드 탈출구.
#[derive(Debug, Clone)]
pub enum OrderType {
    /// "00" 지정가.
    Limit,
    /// "01" 시장가.
    Market,
    /// 그 외 ORD_DVSN 코드 (예: "02" 조건부지정가, "03" 최유리지정가 등).
    Code(String),
}

impl OrderType {
    /// query/body에 넣을 ORD_DVSN 코드 문자열.
    pub(crate) fn code(&self) -> &str {
        match self {
            OrderType::Limit => "00",
            OrderType::Market => "01",
            OrderType::Code(c) => c,
        }
    }
}

/// 매수/매도 주문 파라미터.
#[derive(Debug, Clone)]
pub struct OrderReq {
    /// 종목코드 6자리.
    pub stock_code: String,
    pub order_type: OrderType,
    /// 주문수량.
    pub quantity: u64,
    /// 주문단가. 시장가는 0.
    pub price: u64,
    /// 거래소ID구분코드. 기본 "KRX".
    pub exchange: String,
}

impl OrderReq {
    /// KRX 거래소 기본 주문 파라미터.
    pub fn new(
        stock_code: impl Into<String>,
        order_type: OrderType,
        quantity: u64,
        price: u64,
    ) -> Self {
        Self {
            stock_code: stock_code.into(),
            order_type,
            quantity,
            price,
            exchange: "KRX".into(),
        }
    }
}

/// 정정/취소 파라미터. 원주문의 OrderResult에서 식별자 획득.
#[derive(Debug, Clone)]
pub struct ReviseCancelReq {
    /// 원주문 KRX_FWDG_ORD_ORGNO.
    pub krx_fwdg_ord_orgno: String,
    /// 원주문번호 ODNO.
    pub orig_order_no: String,
    pub order_type: OrderType,
    /// 주문수량. `all=true`면 무시 가능하나 KIS는 값 요구 → 잔량 전달 권장.
    pub quantity: u64,
    /// 주문단가. 취소 시에도 전달.
    pub price: u64,
    /// 잔량 전부 대상이면 true.
    pub all: bool,
    pub exchange: String,
}

impl DomesticStock<'_> {
    /// 현금 매수 (TR 1).
    pub async fn buy(&self, req: OrderReq) -> Result<OrderResult> {
        self.order_cash(req, TR_BUY).await
    }

    /// 현금 매도 (TR 2).
    pub async fn sell(&self, req: OrderReq) -> Result<OrderResult> {
        self.order_cash(req, TR_SELL).await
    }

    async fn order_cash(&self, req: OrderReq, tr: TrId) -> Result<OrderResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "PDNO": req.stock_code,
            "ORD_DVSN": req.order_type.code(),
            "ORD_QTY": req.quantity.to_string(),
            "ORD_UNPR": req.price.to_string(),
            "EXCG_ID_DVSN_CD": req.exchange,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/domestic-stock/v1/trading/order-cash".into(),
                tr_id: tr.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }

    /// 주문 정정 (TR 3). `RVSE_CNCL_DVSN_CD=01`.
    pub async fn revise(&self, req: ReviseCancelReq) -> Result<OrderResult> {
        self.order_rvsecncl(req, "01").await
    }

    /// 주문 취소 (TR 4). `RVSE_CNCL_DVSN_CD=02`.
    pub async fn cancel(&self, req: ReviseCancelReq) -> Result<OrderResult> {
        self.order_rvsecncl(req, "02").await
    }

    async fn order_rvsecncl(&self, req: ReviseCancelReq, dvsn: &str) -> Result<OrderResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "KRX_FWDG_ORD_ORGNO": req.krx_fwdg_ord_orgno,
            "ORGN_ODNO": req.orig_order_no,
            "ORD_DVSN": req.order_type.code(),
            "RVSE_CNCL_DVSN_CD": dvsn,
            "ORD_QTY": req.quantity.to_string(),
            "ORD_UNPR": req.price.to_string(),
            "QTY_ALL_ORD_YN": if req.all { "Y" } else { "N" },
            "EXCG_ID_DVSN_CD": req.exchange,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/domestic-stock/v1/trading/order-rvsecncl".into(),
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
