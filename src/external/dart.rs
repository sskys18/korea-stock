//! OpenDART(opendart.fss.or.kr) 클라이언트 — 전자공시.
//!
//! 공시검색 `list.json`. 자체 인증키(`crtfc_key`) 필요 — KIS 토큰과 무관.
//! 응답 필드는 OpenDART 공식 명세(공시검색 API) 기준.
//!
//! **와이어 미검증** — 컴파일·`#[serde(default)]` 내성까지만 보장. 실사용 시
//! 유효한 `crtfc_key`로 런타임 검증 필요.

use serde::Deserialize;

use crate::error::{KisError, Result};

const LIST_URL: &str = "https://opendart.fss.or.kr/api/list.json";

/// OpenDART 클라이언트. `crtfc_key`는 dart.fss.or.kr에서 발급.
pub struct DartClient {
    http: reqwest::Client,
    api_key: String,
}

impl DartClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: api_key.into(),
        }
    }

    /// 공시검색 — `list.json`.
    ///
    /// `corp_code`: DART 고유 8자리 회사코드(KRX 종목코드 아님; None이면 전체).
    /// `bgn_de`/`end_de`: 접수일자 범위 YYYYMMDD.
    pub async fn disclosures(
        &self,
        corp_code: Option<&str>,
        bgn_de: &str,
        end_de: &str,
    ) -> Result<DisclosureList> {
        let mut query: Vec<(&str, &str)> = vec![
            ("crtfc_key", self.api_key.as_str()),
            ("bgn_de", bgn_de),
            ("end_de", end_de),
            ("page_count", "100"),
        ];
        if let Some(cc) = corp_code {
            query.push(("corp_code", cc));
        }
        let resp = self
            .http
            .get(LIST_URL)
            .query(&query)
            .send()
            .await?
            .error_for_status()?;
        let list: DisclosureList = resp.json().await?;
        // status "000" = 정상. 그 외는 에러로 표면화.
        if list.status != "000" {
            return Err(KisError::External(format!(
                "DART status={} msg={}",
                list.status, list.message
            )));
        }
        Ok(list)
    }
}

/// 공시검색 응답 — `list.json`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct DisclosureList {
    pub status: String,
    pub message: String,
    pub page_no: u32,
    pub page_count: u32,
    pub total_count: u32,
    pub total_page: u32,
    pub list: Vec<Disclosure>,
}

/// 공시 1건 — `list[]`. OpenDART 공시검색 명세 필드.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Disclosure {
    /// 고유번호(회사 8자리).
    pub corp_code: String,
    /// 종목명(법인명).
    pub corp_name: String,
    /// 종목코드(6자리, 상장사).
    pub stock_code: String,
    /// 법인구분 Y(유가)/K(코스닥)/N(코넥스)/E(기타).
    pub corp_cls: String,
    /// 보고서명.
    pub report_nm: String,
    /// 접수번호(14자리).
    pub rcept_no: String,
    /// 공시 제출인명.
    pub flr_nm: String,
    /// 접수일자 YYYYMMDD.
    pub rcept_dt: String,
    /// 비고.
    pub rm: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disclosure_serde_default() {
        let one: Disclosure = serde_json::from_value(serde_json::json!({
            "corp_name": "삼성전자",
            "report_nm": "주요사항보고서",
            "rcept_no": "20240101000123",
        }))
        .unwrap();
        assert_eq!(one.corp_name, "삼성전자");
        assert_eq!(one.stock_code, ""); // 누락 → default
    }

    #[test]
    fn list_envelope_serde() {
        let list: DisclosureList = serde_json::from_value(serde_json::json!({
            "status": "000",
            "message": "정상",
            "total_count": 2,
            "list": [{"report_nm": "사업보고서"}],
        }))
        .unwrap();
        assert_eq!(list.status, "000");
        assert_eq!(list.total_count, 2);
        assert_eq!(list.list.len(), 1);
    }
}
