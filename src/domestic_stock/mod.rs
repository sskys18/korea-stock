//! 국내주식 도메인 — 주문·계좌·시세 TR.

mod account;
mod flow;
mod order;
mod quote;

pub use account::*;
pub use flow::*;
pub use order::*;
pub use quote::*;

use crate::client::KisClient;

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
