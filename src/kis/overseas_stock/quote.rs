use serde::Deserialize;

use crate::kis::client::{ApiCall, KisResponse};
use crate::kis::error::Result;
use crate::kis::overseas_stock::{OverseasExchange, OverseasStock};
use crate::kis::trid::TrId;

const TR_PRICE: TrId = TrId::same("HHDFS00000300");

/// 해외주식 현재가 응답 (output). overseas-stock.md §7 응답표 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OverseasCurrentPrice {
    pub rsym: String, // 실시간조회종목코드
    pub zdiv: String, // 소수점자리수
    pub base: String, // 전일종가
    pub pvol: String, // 전일거래량
    pub last: String, // 현재가
    pub sign: String, // 대비기호
    pub diff: String, // 대비
    pub rate: String, // 등락율
    pub tvol: String, // 거래량
    pub tamt: String, // 거래대금
    pub ordy: String, // 매수가능여부
}

impl OverseasCurrentPrice {
    /// 현재가를 f64로 파싱.
    pub fn price(&self) -> Option<f64> {
        self.last.trim().parse().ok()
    }
}

const TR_PERIOD: TrId = TrId::same("HHDFS76240000");

/// 기간별시세 종목 요약 (output1). §8 output1 표 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OverseasPeriodSummary {
    pub rsym: String, // 실시간조회종목코드
    pub zdiv: String, // 소수점자리수
    pub nrec: String, // 전일종가
}

/// 기간별 봉 1건 (output2 배열 요소). §8 output2 표 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OverseasPeriodCandle {
    pub xymd: String, // 일자 (YYYYMMDD)
    pub clos: String, // 종가
    pub sign: String, // 대비기호
    pub diff: String, // 대비
    pub rate: String, // 등락율
    pub open: String, // 시가
    pub high: String, // 고가
    pub low: String,  // 저가
    pub tvol: String, // 거래량
    pub tamt: String, // 거래대금
    pub pbid: String, // 매수호가
    pub vbid: String, // 매수호가잔량
    pub pask: String, // 매도호가
    pub vask: String, // 매도호가잔량
}

/// 기간 분류. Daily=0 / Weekly=1 / Monthly=2 (overseas `GUBN`).
#[derive(Debug, Clone, Copy)]
pub enum OverseasPeriod {
    Daily,
    Weekly,
    Monthly,
}

impl OverseasPeriod {
    fn code(self) -> &'static str {
        match self {
            OverseasPeriod::Daily => "0",
            OverseasPeriod::Weekly => "1",
            OverseasPeriod::Monthly => "2",
        }
    }
}

impl OverseasStock<'_> {
    /// 해외주식 현재가 (TR 7).
    pub async fn current_price(
        &self,
        exchange: OverseasExchange,
        symbol: &str,
    ) -> Result<OverseasCurrentPrice> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/overseas-price/v1/quotations/price".into(),
                tr_id: TR_PRICE.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "AUTH": "",
                    "EXCD": exchange.excd(),
                    "SYMB": symbol,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }

    /// 해외주식 기간별시세 (TR 8). `base_date` YYYYMMDD(공란이면 최근일).
    /// envelope의 `data`는 (요약, 봉배열). 첫 페이지는 `cont=false`,
    /// 응답이 `has_next()`이면 `cont=true`로 재호출해 다음 페이지 수집.
    pub async fn period_price(
        &self,
        exchange: OverseasExchange,
        symbol: &str,
        period: OverseasPeriod,
        base_date: &str,
        adjusted: bool,
        cont: bool,
    ) -> Result<KisResponse<(OverseasPeriodSummary, Vec<OverseasPeriodCandle>)>> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/overseas-price/v1/quotations/dailyprice".into(),
                tr_id: TR_PERIOD.resolve(env)?.into(),
                tr_cont: if cont { Some("N".to_string()) } else { None },
                params: serde_json::json!({
                    "AUTH": "",
                    "EXCD": exchange.excd(),
                    "SYMB": symbol,
                    "GUBN": period.code(),
                    "BYMD": base_date,
                    "MODP": if adjusted { "1" } else { "0" },
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let summary: OverseasPeriodSummary = resp.field("output1")?;
        let candles: Vec<OverseasPeriodCandle> = resp.field("output2")?;
        Ok(resp.envelope((summary, candles)))
    }
}
