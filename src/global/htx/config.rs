use crate::global::htx::error::{HtxError, Result};

/// HTX USDT-M Linear Swap 운영 Base URL.
///
/// 시세·거래 모두 동일 호스트. 서명 prehash의 HOST 라인도 이 호스트의 도메인부
/// (`api.hbdm.com`)와 일치해야 한다([`HtxConfig::host`]).
pub const DEFAULT_BASE_URL: &str = "https://api.hbdm.com";

/// 서명 prehash에 쓰는 기본 호스트(도메인부, 스킴·포트 제외). HTX 서명은
/// prehash 2번째 라인에 **소문자 호스트**를 요구한다.
pub const DEFAULT_HOST: &str = "api.hbdm.com";

/// HTX 어댑터 설정. access key/secret은 호출자가 주입.
///
/// 시세 엔드포인트(`market`)는 키 없이도 동작하므로 `access_key`/`secret_key`는
/// 빈 문자열을 허용한다. 서명 엔드포인트(`trade`) 호출 시점에 비어 있으면
/// [`HtxError::Auth`]로 거부한다.
#[derive(Debug, Clone)]
pub struct HtxConfig {
    /// 발급받은 API Access Key. 서명 쿼리의 `AccessKeyId`로 전송.
    pub access_key: String,
    /// 발급받은 API Secret. HMAC-SHA256 서명 키. **네트워크로 전송하지 않는다.**
    pub secret_key: String,
    /// API Base URL. 기본 운영 `https://api.hbdm.com`.
    pub base_url: String,
    /// 서명 prehash HOST 라인. `base_url`과 분리해 두는 이유: prehash는 스킴·포트를
    /// 제외한 **순수 호스트**(소문자)를 요구하기 때문이다.
    pub host: String,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 반응형 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl HtxConfig {
    /// access key/secret로 기본(운영) 설정 생성.
    pub fn new(access_key: impl Into<String>, secret_key: impl Into<String>) -> Self {
        Self {
            access_key: access_key.into(),
            secret_key: secret_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            host: DEFAULT_HOST.to_string(),
            rate_limit: None,
        }
    }

    /// 키 없는 시세 전용 설정. 서명 엔드포인트 호출 시 [`HtxError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new(), String::new())
    }

    /// 환경변수에서 설정 로드.
    /// `HTX_API_KEY` `HTX_API_SECRET` 필수.
    /// `HTX_BASE_URL`(선택, 기본 운영), `HTX_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| HtxError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("HTX_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        // host는 base_url에서 도메인부를 추출(스킴·경로 제거), 실패 시 기본값.
        let host = base_url
            .split("://")
            .nth(1)
            .map(|rest| rest.split('/').next().unwrap_or(DEFAULT_HOST).to_string())
            .unwrap_or_else(|| DEFAULT_HOST.to_string());
        let rate_limit = match std::env::var("HTX_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| HtxError::Auth(format!("invalid HTX_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            access_key: var("HTX_API_KEY")?,
            secret_key: var("HTX_API_SECRET")?,
            base_url,
            host,
            rate_limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_has_empty_keys() {
        let c = HtxConfig::public();
        assert!(c.access_key.is_empty());
        assert!(c.secret_key.is_empty());
        assert_eq!(c.base_url, DEFAULT_BASE_URL);
        assert_eq!(c.host, DEFAULT_HOST);
    }
}
