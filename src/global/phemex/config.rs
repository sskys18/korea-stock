use crate::global::phemex::error::{PhemexError, Result};

/// Phemex 운영 Base URL (시세·거래 공용 호스트).
pub const DEFAULT_BASE_URL: &str = "https://api.phemex.com";

/// Phemex 테스트넷 Base URL.
///
/// 테스트넷 키 발급: <https://testnet.phemex.com>. **거래 코드 개발·검증은 반드시
/// 여기서 한다.** 시세 데이터는 운영과 다를 수 있다.
pub const TESTNET_BASE_URL: &str = "https://testnet-api.phemex.com";

/// Phemex 어댑터 설정. api key/secret은 호출자가 주입.
///
/// 시세 엔드포인트(`market`)는 키 없이도 동작하므로 `api_key`/`api_secret`은
/// 빈 문자열을 허용한다. 서명 엔드포인트(`trade`) 호출 시점에 비어 있으면
/// [`PhemexError::Auth`]로 거부한다.
#[derive(Debug, Clone)]
pub struct PhemexConfig {
    /// 발급받은 API Key(id 필드). `x-phemex-access-token` 헤더로 전송.
    pub api_key: String,
    /// 발급받은 API Secret. HMAC-SHA256 서명 키(**base64url 디코드 후** 키로 사용).
    /// **네트워크로 전송하지 않는다.**
    pub api_secret: String,
    /// API Base URL. 기본 운영 `https://api.phemex.com`.
    pub base_url: String,
    /// 서명 요청 유효시간(초). `x-phemex-request-expiry = Now() + request_expiry_secs`로
    /// 보내며, 서버가 이 시각을 넘겨 수신하면 거부한다. 기본 60.
    pub request_expiry_secs: u64,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형
    /// 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl PhemexConfig {
    /// api key/secret로 기본(운영) 설정 생성.
    pub fn new(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            request_expiry_secs: 60,
            rate_limit: None,
        }
    }

    /// 키 없는 시세 전용 설정. 서명 엔드포인트 호출 시 [`PhemexError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new(), String::new())
    }

    /// 테스트넷으로 base_url 교체 (builder).
    pub fn testnet(mut self) -> Self {
        self.base_url = TESTNET_BASE_URL.to_string();
        self
    }

    /// 환경변수에서 설정 로드.
    /// `PHEMEX_API_KEY` `PHEMEX_API_SECRET` 필수.
    /// `PHEMEX_BASE_URL`(선택, 기본 운영), `PHEMEX_REQUEST_EXPIRY`(선택, 초),
    /// `PHEMEX_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| PhemexError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("PHEMEX_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let request_expiry_secs = match std::env::var("PHEMEX_REQUEST_EXPIRY") {
            Ok(s) => s
                .parse()
                .map_err(|_| PhemexError::Auth(format!("invalid PHEMEX_REQUEST_EXPIRY: {s}")))?,
            Err(_) => 60,
        };
        let rate_limit = match std::env::var("PHEMEX_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| PhemexError::Auth(format!("invalid PHEMEX_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            api_key: var("PHEMEX_API_KEY")?,
            api_secret: var("PHEMEX_API_SECRET")?,
            base_url,
            request_expiry_secs,
            rate_limit,
        })
    }
}
