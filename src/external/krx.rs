//! KRX MDC(data.krx.co.kr) 클라이언트 — KIS 미제공 데이터.
//!
//! 공매도 *잔고*(outstanding)·외국인 보유량 추이는 KIS REST에 없어 KRX MDC에서 조회.
//!
//! ## 인증 (2024~ KRX 변경)
//! KRX는 MDC `getJsonData.cmd`를 **회원 로그인 세션** 뒤로 옮겼다(미인증 시 `LOGOUT` 400).
//! 무료 KRX 계정(`data.krx.co.kr` 가입)으로 폼 로그인 후 JSESSIONID 쿠키로 조회.
//! 흐름(pykrx `comm/auth.py` 역공학, OTP·crypto 아님):
//!   1. GET `MDCCOMS001.cmd` + `login.jsp` → 초기 쿠키
//!   2. POST `MDCCOMS001D1.cmd` {mbrId, pw} → `_error_code` CD001=정상 / CD011=중복(skipDup=Y 재전송)
//!   3. POST `getJsonData.cmd` {bld, ...} → 데이터 (세션 ~1h, 만료 시 재로그인)
//!
//! bld 코드·응답 컬럼은 pykrx 소스/cassette에서 추출(추측 없음):
//! - 개별종목 공매도 잔고: `dbms/MDC/STAT/srt/MDCSTAT30502`, 응답 키 `OutBlock_1`.
//! - 외국인 보유량 개별추이: `dbms/MDC/STAT/standard/MDCSTAT03702`, 응답 키 `output`.

use std::time::Duration;

use serde::Deserialize;
use tokio::sync::Mutex;

use crate::error::{KisError, Result};

const MDC_URL: &str = "https://data.krx.co.kr/comm/bldAttendant/getJsonData.cmd";
const LOGIN_PAGE: &str = "https://data.krx.co.kr/contents/MDC/COMS/client/MDCCOMS001.cmd";
const LOGIN_JSP: &str =
    "https://data.krx.co.kr/contents/MDC/COMS/client/view/login.jsp?site=mdc";
const LOGIN_URL: &str = "https://data.krx.co.kr/contents/MDC/COMS/client/MDCCOMS001D1.cmd";
const REFERER: &str = "https://data.krx.co.kr/contents/MDC/MDI/outerLoader/index.cmd";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
     (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
const BLD_SHORT_BALANCE: &str = "dbms/MDC/STAT/srt/MDCSTAT30502";
const BLD_FOREIGN_HOLDING: &str = "dbms/MDC/STAT/standard/MDCSTAT03702";

/// KRX MDC 클라이언트. KIS 인증과 무관 — 독립 쿠키 세션.
///
/// 무료 KRX 계정 필요. [`KrxClient::from_env`]는 `KRX_ID`/`KRX_PW`를 사용.
pub struct KrxClient {
    http: reqwest::Client,
    login_id: String,
    login_pw: String,
    /// 세션 세대. 0=미로그인. 로그인 직렬화 + 동시 재로그인 방지용.
    /// 한 태스크가 `LOGOUT`을 만나면 자신이 본 세대와 비교해, 다른 태스크가
    /// 이미 갱신했으면 그 세션을 재사용하고 중복 로그인하지 않는다.
    generation: Mutex<u64>,
}

impl KrxClient {
    /// 자격증명으로 생성. HTTP는 쿠키 저장 활성화.
    pub fn new(login_id: impl Into<String>, login_pw: impl Into<String>) -> Result<Self> {
        let http = reqwest::Client::builder()
            .cookie_store(true)
            .timeout(Duration::from_secs(15))
            .user_agent(USER_AGENT)
            .build()?;
        Ok(Self {
            http,
            login_id: login_id.into(),
            login_pw: login_pw.into(),
            generation: Mutex::new(0),
        })
    }

    /// 환경변수 `KRX_ID`/`KRX_PW`로 생성.
    pub fn from_env() -> Result<Self> {
        let id = std::env::var("KRX_ID")
            .map_err(|_| KisError::External("KRX_ID env not set".into()))?;
        let pw = std::env::var("KRX_PW")
            .map_err(|_| KisError::External("KRX_PW env not set".into()))?;
        Self::new(id, pw)
    }

    /// warmup GET → 초기 쿠키. 실패해도 로그인 단계에서 재확인되므로 무해.
    async fn warmup(&self) -> Result<()> {
        self.http.get(LOGIN_PAGE).send().await?;
        self.http
            .get(LOGIN_JSP)
            .header("Referer", LOGIN_PAGE)
            .send()
            .await?;
        Ok(())
    }

    /// 폼 로그인. `_error_code` CD001=정상. CD011(중복) 시 skipDup=Y 재전송.
    async fn login(&self) -> Result<()> {
        self.warmup().await?;
        let do_post = |skip_dup: bool| {
            let mut form: Vec<(&str, &str)> = vec![
                ("mbrNm", ""),
                ("telNo", ""),
                ("di", ""),
                ("certType", ""),
                ("mbrId", self.login_id.as_str()),
                ("pw", self.login_pw.as_str()),
            ];
            if skip_dup {
                form.push(("skipDup", "Y"));
            }
            self.http
                .post(LOGIN_URL)
                .header("Referer", LOGIN_PAGE)
                .form(&form)
                .send()
        };

        let body: serde_json::Value = do_post(false).await?.json().await?;
        let code = body
            .get("_error_code")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let final_body = if code == "CD011" {
            do_post(true).await?.json::<serde_json::Value>().await?
        } else {
            body
        };
        let code = final_body
            .get("_error_code")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if code != "CD001" {
            let msg = final_body
                .get("_error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            return Err(KisError::External(format!(
                "KRX login failed: code={code} msg={msg}"
            )));
        }
        Ok(())
    }

    /// 미로그인 시 로그인(직렬화). 호출 시점의 세션 세대를 반환.
    /// 잠금을 로그인 네트워크 호출 동안 유지 → 동시 호출은 단일 로그인 공유.
    async fn ensure_login(&self) -> Result<u64> {
        let mut g = self.generation.lock().await;
        if *g == 0 {
            self.login().await?;
            *g = 1;
        }
        Ok(*g)
    }

    /// `LOGOUT` 후 재로그인 — 단, 본 세대(`seen_gen`)가 그대로일 때만.
    /// 다른 태스크가 이미 갱신(세대 증가)했으면 그 세션을 재사용(중복 로그인 회피).
    async fn relogin_if_stale(&self, seen_gen: u64) -> Result<()> {
        let mut g = self.generation.lock().await;
        if *g == seen_gen {
            self.login().await?;
            *g += 1;
        }
        Ok(())
    }

    /// `getJsonData.cmd` POST 1회. 본문 텍스트 반환(LOGOUT 감지용).
    async fn post_json(&self, bld: &str, extra: &[(&str, &str)]) -> Result<String> {
        let mut form: Vec<(&str, &str)> = vec![("bld", bld)];
        form.extend_from_slice(extra);
        let resp = self
            .http
            .post(MDC_URL)
            .header("Referer", REFERER)
            .header("X-Requested-With", "XMLHttpRequest")
            .form(&form)
            .send()
            .await?;
        Ok(resp.text().await?)
    }

    /// MDC 조회. 로그인 보장 + `LOGOUT`(세션만료) 시 1회 재로그인 후 재시도.
    async fn fetch<T: serde::de::DeserializeOwned>(
        &self,
        bld: &str,
        extra: &[(&str, &str)],
        result_key: &str,
    ) -> Result<T> {
        let cur_gen = self.ensure_login().await?;
        let mut text = self.post_json(bld, extra).await?;
        if text.trim() == "LOGOUT" {
            // 세션 만료 — 본 세대 기준 재로그인(중복 방지) 후 1회 재시도.
            self.relogin_if_stale(cur_gen).await?;
            text = self.post_json(bld, extra).await?;
        }
        if text.trim() == "LOGOUT" {
            return Err(KisError::External(
                "KRX returned LOGOUT after re-login (check KRX_ID/KRX_PW)".into(),
            ));
        }
        let mut body: serde_json::Value = serde_json::from_str(&text)?;
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

    #[test]
    fn client_builds() {
        assert!(KrxClient::new("id", "pw").is_ok());
    }

    // 실거래 와이어 검증 — KRX_ID/KRX_PW + 네트워크 필요.
    //   cargo test --features external -- --ignored krx_live
    #[tokio::test]
    #[ignore = "requires KRX_ID/KRX_PW + network"]
    async fn krx_live_short_balance() {
        let krx = KrxClient::from_env().expect("set KRX_ID/KRX_PW");
        // 삼성전자 ISIN, 짧은 구간.
        let rows = krx
            .short_balance("20240102", "20240110", "KR7005930003")
            .await
            .expect("short_balance call");
        assert!(!rows.is_empty(), "expected non-empty short balance rows");
        assert!(!rows[0].date.is_empty(), "row date populated");
        assert!(!rows[0].balance_qty.is_empty(), "balance_qty populated");
    }

    #[tokio::test]
    #[ignore = "requires KRX_ID/KRX_PW + network"]
    async fn krx_live_foreign_holding() {
        let krx = KrxClient::from_env().expect("set KRX_ID/KRX_PW");
        let rows = krx
            .foreign_holding("20240102", "20240110", "KR7005930003")
            .await
            .expect("foreign_holding call");
        assert!(!rows.is_empty(), "expected non-empty foreign holding rows");
        assert!(!rows[0].date.is_empty(), "row date populated");
        assert!(
            !rows[0].foreign_ratio.is_empty(),
            "foreign_ratio populated"
        );
    }
}
