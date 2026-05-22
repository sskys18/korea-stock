//! 국내선물옵션 도메인 — 주문·계좌·시세 TR.

mod account;
mod order;
mod quote;

pub use account::*;
pub use order::*;
pub use quote::*;

use crate::client::KisClient;

/// 선물옵션 거래 세션. 주문·정정취소 tr_id 선택용.
/// 야간 세션은 모의투자 미지원(KIS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    /// 주간 거래.
    Day,
    /// 야간 거래 (모의 미지원).
    Night,
}

/// 선물옵션 도메인 액세서. `client.futureoption()`으로 획득.
pub struct FutureOption<'a> {
    pub(crate) client: &'a KisClient,
}

impl<'a> FutureOption<'a> {
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
