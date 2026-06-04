use crate::global::bitunix::error::{BitunixError, Result};

/// Bitunix Futures(fapi) 운영 Base URL.
pub const DEFAULT_BASE_URL: &str = "https://fapi.bitunix.com";

/// Bitunix 어댑터 설정. api key/secret은 호출자가 주입.
///
/// 시세 엔드포인트(`market`)는 키 없이도 동작하므로 `api_key`/`api_secret`은
/// 빈 문자열을 허용한다. 서명 엔드포인트(`trade`) 호출 시점에 비어 있으면
/// [`BitunixError::Auth`]로 거부한다.
#[derive(Debug, Clone)]
pub struct BitunixConfig {
    /// 발급받은 API Key. `api-key` 헤더로 전송.
    pub api_key: String,
    /// 발급받은 API Secret. 이중 SHA256 서명 키. **네트워크로 전송하지 않는다.**
    pub api_secret: String,
    /// API Base URL. 기본 운영 `https://fapi.bitunix.com`.
    pub base_url: String,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형
    /// 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl BitunixConfig {
    /// api key/secret로 기본(운영) 설정 생성.
    pub fn new(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            rate_limit: None,
        }
    }

    /// 키 없는 시세 전용 설정. 서명 엔드포인트 호출 시 [`BitunixError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new(), String::new())
    }

    /// 환경변수에서 설정 로드.
    /// `BITUNIX_API_KEY` `BITUNIX_API_SECRET` 필수.
    /// `BITUNIX_BASE_URL`(선택, 기본 운영), `BITUNIX_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| BitunixError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("BITUNIX_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let rate_limit = match std::env::var("BITUNIX_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| BitunixError::Auth(format!("invalid BITUNIX_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            api_key: var("BITUNIX_API_KEY")?,
            api_secret: var("BITUNIX_API_SECRET")?,
            base_url,
            rate_limit,
        })
    }
}
