use crate::global::weex::error::{Result, WeexError};

/// WEEX Contract(USDT 무기한 선물) 운영 Base URL.
///
/// 시세·거래 모두 같은 호스트를 쓴다. 시세는 `/capi/v2/market/*`,
/// 거래는 `POST /capi/v3/order` (Binance-스타일 필드).
pub const DEFAULT_BASE_URL: &str = "https://api-contract.weex.com";

/// WEEX 어댑터 설정. api key/secret/passphrase는 호출자가 주입.
///
/// 시세 엔드포인트(`market`)는 키 없이도 동작하므로 자격증명은 빈 문자열을
/// 허용한다. 서명 엔드포인트(`trade`) 호출 시점에 하나라도 비어 있으면
/// [`WeexError::Auth`]로 거부한다. **WEEX는 패스프레이즈가 필수**
/// (Bitget 계열 — `ACCESS-PASSPHRASE` 헤더).
#[derive(Debug, Clone)]
pub struct WeexConfig {
    /// 발급받은 API Key. `ACCESS-KEY` 헤더로 전송.
    pub api_key: String,
    /// 발급받은 API Secret. HMAC-SHA256 서명 키. **네트워크로 전송하지 않는다.**
    pub api_secret: String,
    /// API Key 생성 시 지정한 패스프레이즈. `ACCESS-PASSPHRASE` 헤더로 평문 전송.
    pub api_passphrase: String,
    /// API Base URL. 기본 운영 `https://api-contract.weex.com`.
    pub base_url: String,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형
    /// 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl WeexConfig {
    /// api key/secret/passphrase로 기본(운영) 설정 생성.
    pub fn new(
        api_key: impl Into<String>,
        api_secret: impl Into<String>,
        api_passphrase: impl Into<String>,
    ) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            api_passphrase: api_passphrase.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            rate_limit: None,
        }
    }

    /// 키 없는 시세 전용 설정. 서명 엔드포인트 호출 시 [`WeexError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new(), String::new(), String::new())
    }

    /// 환경변수에서 설정 로드.
    /// `WEEX_API_KEY` `WEEX_API_SECRET` `WEEX_API_PASSPHRASE` 필수.
    /// `WEEX_BASE_URL`(선택, 기본 운영), `WEEX_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| WeexError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("WEEX_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let rate_limit = match std::env::var("WEEX_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| WeexError::Auth(format!("invalid WEEX_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            api_key: var("WEEX_API_KEY")?,
            api_secret: var("WEEX_API_SECRET")?,
            api_passphrase: var("WEEX_API_PASSPHRASE")?,
            base_url,
            rate_limit,
        })
    }
}
