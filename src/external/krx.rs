//! KRX MDC(data.krx.co.kr) 클라이언트 — KIS 미제공 데이터.
//!
//! 공매도 *잔고*(outstanding)·외국인 보유량 추이는 KIS REST에 없어 KRX MDC에서 조회.
//! `getJsonData.cmd`에 `bld` + 쿼리를 POST(form). OTP 불필요(OTP는 CSV 다운로드 전용).
//!
//! bld 코드·응답 컬럼은 pykrx 소스/cassette에서 추출(추측 없음):
//! - 개별종목 공매도 잔고: `dbms/MDC/STAT/srt/MDCSTAT30502`, 응답 키 `OutBlock_1`.
//! - 외국인 보유량 개별추이: `dbms/MDC/STAT/standard/MDCSTAT03702`, 응답 키 `output`.
//!
//! **와이어 미검증** — 컴파일·`#[serde(default)]` 내성까지만 보장. KRX 엔드포인트는
//! 비공식이라 변경 시 깨질 수 있음. 실사용 시 런타임 검증 필요.

use serde::Deserialize;

use crate::error::{KisError, Result};

const MDC_URL: &str = "https://data.krx.co.kr/comm/bldAttendant/getJsonData.cmd";
const BLD_SHORT_BALANCE: &str = "dbms/MDC/STAT/srt/MDCSTAT30502";
const BLD_FOREIGN_HOLDING: &str = "dbms/MDC/STAT/standard/MDCSTAT03702";

/// KRX MDC 클라이언트. KIS 인증과 무관 — 독립 HTTP.
pub struct KrxClient {
    http: reqwest::Client,
}

impl Default for KrxClient {
    fn default() -> Self {
        Self::new()
    }
}

impl KrxClient {
    pub fn new() -> Self {
        Self {
            http: reqwest::Client::new(),
        }
    }

    /// MDC `getJsonData.cmd` POST. `bld` + 쿼리 form. 지정 응답 키를 T로 역직렬화.
    async fn fetch<T: serde::de::DeserializeOwned>(
        &self,
        bld: &str,
        extra: &[(&str, &str)],
        result_key: &str,
    ) -> Result<T> {
        let mut form: Vec<(&str, &str)> = vec![("bld", bld)];
        form.extend_from_slice(extra);
        let resp = self
            .http
            .post(MDC_URL)
            .header("Referer", "https://data.krx.co.kr/")
            .header(
                "User-Agent",
                "Mozilla/5.0 (compatible; kis-adapter-external)",
            )
            .form(&form)
            .send()
            .await?
            .error_for_status()?;
        let mut body: serde_json::Value = resp.json().await?;
        let v = body
            .get_mut(result_key)
            .map(serde_json::Value::take)
            .ok_or_else(|| KisError::External(format!("missing {result_key} in KRX response")))?;
        Ok(serde_json::from_value(v)?)
    }

    /// 개별종목 공매도 잔고 — bld `MDCSTAT30502`.
    ///
    /// `start`/`end` YYYYMMDD, `isin` 종목 ISIN(예: `KR7005930003`).
    /// KRX는 2년 초과 구간을 서버에서 잘라내므로, 장기간은 호출자가 분할 권장.
    pub async fn short_balance(
        &self,
        start: &str,
        end: &str,
        isin: &str,
    ) -> Result<Vec<ShortBalanceRow>> {
        self.fetch(
            BLD_SHORT_BALANCE,
            &[("strtDd", start), ("endDd", end), ("isuCd", isin)],
            "OutBlock_1",
        )
        .await
    }

    /// 외국인 보유량 개별추이 — bld `MDCSTAT03702`.
    ///
    /// `start`/`end` YYYYMMDD, `isin` 종목 ISIN. 외국인 보유율·소진율 추이.
    pub async fn foreign_holding(
        &self,
        start: &str,
        end: &str,
        isin: &str,
    ) -> Result<Vec<ForeignHoldingRow>> {
        self.fetch(
            BLD_FOREIGN_HOLDING,
            &[
                ("searchType", "2"),
                ("strtDd", start),
                ("endDd", end),
                ("isuCd", isin),
            ],
            "output",
        )
        .await
    }
}

/// 공매도 잔고 1행 — KRX `OutBlock_1`. 컬럼명은 KRX 원본(대문자, 콤마 포함 숫자 문자열).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ShortBalanceRow {
    /// 잔고 기준일(보고의무 발생일).
    #[serde(rename = "RPT_DUTY_OCCR_DD")]
    pub date: String,
    /// 공매도 잔고 수량.
    #[serde(rename = "BAL_QTY")]
    pub balance_qty: String,
    /// 상장 주식수.
    #[serde(rename = "LIST_SHRS")]
    pub list_shares: String,
    /// 공매도 잔고 금액.
    #[serde(rename = "BAL_AMT")]
    pub balance_amt: String,
    /// 시가총액.
    #[serde(rename = "MKTCAP")]
    pub market_cap: String,
    /// 공매도 잔고 비중(%).
    #[serde(rename = "BAL_RTO")]
    pub balance_ratio: String,
}

/// 외국인 보유량 1행 — KRX `output`(MDCSTAT03702).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct ForeignHoldingRow {
    /// 거래일.
    #[serde(rename = "TRD_DD")]
    pub date: String,
    /// 종가.
    #[serde(rename = "TDD_CLSPRC")]
    pub close: String,
    /// 상장 주식수.
    #[serde(rename = "LIST_SHRS")]
    pub list_shares: String,
    /// 외국인 보유 수량.
    #[serde(rename = "FORN_HD_QTY")]
    pub foreign_qty: String,
    /// 외국인 보유 비율(%).
    #[serde(rename = "FORN_SHR_RT")]
    pub foreign_ratio: String,
    /// 외국인 한도 소진율(%).
    #[serde(rename = "FORN_LMT_EXHST_RT")]
    pub foreign_limit_exhaust_ratio: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_balance_serde_default_and_rename() {
        let row: ShortBalanceRow = serde_json::from_value(serde_json::json!({
            "RPT_DUTY_OCCR_DD": "2020/01/10",
            "BAL_QTY": "5,489,240",
            "UNKNOWN": "x",
        }))
        .unwrap();
        assert_eq!(row.date, "2020/01/10");
        assert_eq!(row.balance_qty, "5,489,240");
        assert_eq!(row.balance_ratio, ""); // 누락 → default
    }

    #[test]
    fn foreign_holding_rename() {
        let row: ForeignHoldingRow = serde_json::from_value(serde_json::json!({
            "TRD_DD": "2021/01/15",
            "FORN_SHR_RT": "55.57",
        }))
        .unwrap();
        assert_eq!(row.date, "2021/01/15");
        assert_eq!(row.foreign_ratio, "55.57");
    }
}
