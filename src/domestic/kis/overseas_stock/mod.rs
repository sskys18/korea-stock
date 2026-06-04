//! 해외주식 도메인 — 주문·계좌·시세 TR.

mod account;
mod order;
mod quote;

pub use account::*;
pub use order::*;
pub use quote::*;

use crate::domestic::kis::client::KisClient;

/// 해외 거래소. 주문/계좌 TR은 `ovrs_excg_cd()`, 시세 TR은 `excd()` 코드 사용.
/// 미국 3거래소(NASD/NYSE/AMEX)는 주문 tr_id가 동일하므로 한 그룹으로 분기된다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverseasExchange {
    /// 미국 나스닥.
    Nasd,
    /// 미국 뉴욕.
    Nyse,
    /// 미국 아멕스.
    Amex,
    /// 홍콩.
    Sehk,
    /// 중국 상해.
    Shaa,
    /// 중국 심천.
    Szaa,
    /// 일본 도쿄.
    Tkse,
    /// 베트남 하노이.
    Hase,
    /// 베트남 호치민.
    Vnse,
}

impl OverseasExchange {
    /// 주문/계좌 TR용 해외거래소코드 (`OVRS_EXCG_CD`).
    pub fn ovrs_excg_cd(self) -> &'static str {
        match self {
            OverseasExchange::Nasd => "NASD",
            OverseasExchange::Nyse => "NYSE",
            OverseasExchange::Amex => "AMEX",
            OverseasExchange::Sehk => "SEHK",
            OverseasExchange::Shaa => "SHAA",
            OverseasExchange::Szaa => "SZAA",
            OverseasExchange::Tkse => "TKSE",
            OverseasExchange::Hase => "HASE",
            OverseasExchange::Vnse => "VNSE",
        }
    }

    /// 시세 TR용 거래소코드 (`EXCD`).
    pub fn excd(self) -> &'static str {
        match self {
            OverseasExchange::Nasd => "NAS",
            OverseasExchange::Nyse => "NYS",
            OverseasExchange::Amex => "AMS",
            OverseasExchange::Sehk => "HKS",
            OverseasExchange::Shaa => "SHS",
            OverseasExchange::Szaa => "SZS",
            OverseasExchange::Tkse => "TSE",
            OverseasExchange::Hase => "HNX",
            OverseasExchange::Vnse => "HSX",
        }
    }

    /// 기본 거래통화코드 (`TR_CRCY_CD`). 잔고 TR 등에서 사용.
    pub fn currency(self) -> &'static str {
        match self {
            OverseasExchange::Nasd | OverseasExchange::Nyse | OverseasExchange::Amex => "USD",
            OverseasExchange::Sehk => "HKD",
            OverseasExchange::Shaa | OverseasExchange::Szaa => "CNY",
            OverseasExchange::Tkse => "JPY",
            OverseasExchange::Hase | OverseasExchange::Vnse => "VND",
        }
    }
}

/// 해외주식 도메인 액세서. `client.overseas_stock()`으로 획득.
pub struct OverseasStock<'a> {
    pub(crate) client: &'a KisClient,
}

impl<'a> OverseasStock<'a> {
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
    fn exchange_dual_code_system() {
        assert_eq!(OverseasExchange::Nasd.ovrs_excg_cd(), "NASD");
        assert_eq!(OverseasExchange::Nasd.excd(), "NAS");
        assert_eq!(OverseasExchange::Hase.ovrs_excg_cd(), "HASE");
        assert_eq!(OverseasExchange::Hase.excd(), "HNX");
        assert_eq!(OverseasExchange::Vnse.ovrs_excg_cd(), "VNSE");
        assert_eq!(OverseasExchange::Vnse.excd(), "HSX");
        assert_eq!(OverseasExchange::Sehk.currency(), "HKD");
    }
}
