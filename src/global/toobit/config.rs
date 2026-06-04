use crate::global::toobit::error::{Result, ToobitError};

/// Toobit 운영 Base URL. 시세(`/quote/v1/*`)·거래(`/api/v1/*`) 모두 이 호스트.
///
/// Toobit은 공개된 별도 testnet 호스트가 없어(공식 문서 미기재) `testnet()` 빌더를
/// 제공하지 않는다. binance 템플릿과 달리 단일 base만 둔다.
pub const DEFAULT_BASE_URL: &str = "https://api.toobit.com";

/// Toobit 어댑터 설정. api key/secret은 호출자가 주입.
///
/// 시세 엔드포인트(`market`)는 키 없이도 동작하므로 `api_key`/`api_secret`은
/// 빈 문자열을 허용한다. 서명 엔드포인트(`trade`) 호출 시점에 비어 있으면
/// [`ToobitError::Auth`]로 거부한다.
#[derive(Debug, Clone)]
pub struct ToobitConfig {
    /// 발급받은 API Key. `X-BB-APIKEY` 헤더로 전송.
    pub api_key: String,
    /// 발급받은 API Secret. HMAC-SHA256 서명 키. **네트워크로 전송하지 않는다.**
    pub api_secret: String,
    /// API Base URL. 기본 운영 `https://api.toobit.com`.
    pub base_url: String,
    /// 서명 요청 유효시간(ms). 서버 수신 시각이 `timestamp + recvWindow`를 넘으면
    /// 거부(-1021). 기본 5000.
    pub recv_window: u64,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형
    /// 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl ToobitConfig {
    /// api key/secret로 기본(운영) 설정 생성.
    pub fn new(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            recv_window: 5000,
            rate_limit: None,
        }
    }

    /// 키 없는 시세 전용 설정. 서명 엔드포인트 호출 시 [`ToobitError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new(), String::new())
    }

    /// 환경변수에서 설정 로드.
    /// `TOOBIT_API_KEY` `TOOBIT_API_SECRET` 필수.
    /// `TOOBIT_BASE_URL`(선택, 기본 운영), `TOOBIT_RECV_WINDOW`(선택, ms),
    /// `TOOBIT_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| ToobitError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("TOOBIT_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let recv_window = match std::env::var("TOOBIT_RECV_WINDOW") {
            Ok(s) => s
                .parse()
                .map_err(|_| ToobitError::Auth(format!("invalid TOOBIT_RECV_WINDOW: {s}")))?,
            Err(_) => 5000,
        };
        let rate_limit = match std::env::var("TOOBIT_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| ToobitError::Auth(format!("invalid TOOBIT_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            api_key: var("TOOBIT_API_KEY")?,
            api_secret: var("TOOBIT_API_SECRET")?,
            base_url,
            recv_window,
            rate_limit,
        })
    }
}
