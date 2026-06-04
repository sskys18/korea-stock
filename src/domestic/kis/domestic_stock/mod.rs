//! 국내주식 도메인 — 주문·계좌·시세 TR.

mod account;
mod flow;
mod order;
mod quote;
mod ranking;

pub use account::*;
pub use flow::*;
pub use order::*;
pub use quote::*;
pub use ranking::*;

use crate::domestic::kis::client::KisClient;

/// 거래소(시장) 구분 — 시세 조회 REST(`FID_COND_MRKT_DIV_CODE`) + 실시간 WS tr_id 공용.
///
/// NXT(넥스트레이드) 대체거래소 도입(2025-03). `Unified`는 KRX+NXT 통합시세.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Market {
    /// 한국거래소(KRX). REST `J`, WS 인픽스 `ST`.
    #[default]
    Krx,
    /// 넥스트레이드(NXT). REST `NX`, WS 인픽스 `NX`.
    Nxt,
    /// KRX+NXT 통합. REST `UN`, WS 인픽스 `UN`.
    Unified,
}

impl Market {
    /// 시세 조회 REST `FID_COND_MRKT_DIV_CODE` 코드.
    pub fn fid_code(self) -> &'static str {
        match self {
            Market::Krx => "J",
            Market::Nxt => "NX",
            Market::Unified => "UN",
        }
    }

    /// 실시간 WS tr_id 인픽스 (`H0{infix}{suffix}0`).
    pub(crate) fn ws_infix(self) -> &'static str {
        match self {
            Market::Krx => "ST",
            Market::Nxt => "NX",
            Market::Unified => "UN",
        }
    }
}

/// 국내주식 도메인 액세서. `client.domestic_stock()`으로 획득.
pub struct DomesticStock<'a> {
    pub(crate) client: &'a KisClient,
}

impl<'a> DomesticStock<'a> {
    pub(crate) fn new(client: &'a KisClient) -> Self {
        Self { client }
    }

    /// 요청 object에 CANO/ACNT_PRDT_CD 주입. 주문·계좌 TR 공용.
    pub(crate) fn with_account(&self, mut params: serde_json::Value) -> serde_json::Value {
        let cfg = self.client.config();
        if let Some(obj) = params.as_object_mut() {
            obj.insert("CANO".into(), cfg.account_no.clone().into());
            obj.insert("ACNT_PRDT_CD".into(), cfg.account_product.clone().into());
        }
        params
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_codes() {
        assert_eq!(Market::Krx.fid_code(), "J");
        assert_eq!(Market::Nxt.fid_code(), "NX");
        assert_eq!(Market::Unified.fid_code(), "UN");
        assert_eq!(Market::Krx.ws_infix(), "ST");
        assert_eq!(Market::Nxt.ws_infix(), "NX");
        assert_eq!(Market::Unified.ws_infix(), "UN");
        assert_eq!(Market::default(), Market::Krx);
    }
}
