use crate::global::bitget::error::{BitgetError, Result};

/// Bitget v2 운영 Base URL (시세·거래 공통 호스트).
pub const DEFAULT_BASE_URL: &str = "https://api.bitget.com";

/// Bitget 어댑터 설정. api key/secret/passphrase는 호출자가 주입.
///
/// 시세 엔드포인트(`market`)는 키 없이도 동작하므로 `api_key`/`api_secret`/`passphrase`는
/// 빈 문자열을 허용한다. 서명 엔드포인트(`trade`) 호출 시점에 비어 있으면
/// [`BitgetError::Auth`]로 거부한다.
///
/// **Bybit/MEXC와 다른 점:** Bitget 서명은 HMAC-SHA256 외에 **passphrase**(키 발급 시
/// 설정한 문자열)를 `ACCESS-PASSPHRASE` 헤더로 함께 보낸다. passphrase는 서명 문자열에는
/// 포함되지 않고 헤더로만 전송한다.
#[derive(Debug, Clone)]
pub struct BitgetConfig {
    /// 발급받은 API Key. `ACCESS-KEY` 헤더로 전송.
    pub api_key: String,
    /// 발급받은 API Secret. HMAC-SHA256 서명 키. **네트워크로 전송하지 않는다.**
    pub api_secret: String,
    /// 키 발급 시 설정한 passphrase. `ACCESS-PASSPHRASE` 헤더로 전송.
    pub passphrase: String,
    /// API Base URL. 기본 운영 `https://api.bitget.com`.
    pub base_url: String,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl BitgetConfig {
    /// api key/secret/passphrase로 기본(운영) 설정 생성.
    pub fn new(
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        passphrase: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            passphrase: passphrase.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            rate_limit: None,
        }
    }

    /// 키 없는 시세 전용 설정. 서명 엔드포인트 호출 시 [`BitgetError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new(), String::new(), String::new())
    }

    /// 환경변수에서 설정 로드.
    /// `BITGET_API_KEY` `BITGET_API_SECRET` `BITGET_API_PASSPHRASE` 필수.
    /// `BITGET_BASE_URL`(선택, 기본 운영), `BITGET_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| BitgetError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("BITGET_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let rate_limit = match std::env::var("BITGET_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| BitgetError::Auth(format!("invalid BITGET_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            api_key: var("BITGET_API_KEY")?,
            api_secret: var("BITGET_API_SECRET")?,
            passphrase: var("BITGET_API_PASSPHRASE")?,
            base_url,
            rate_limit,
        })
    }
}
