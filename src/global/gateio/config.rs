use crate::global::gateio::error::{GateioError, Result};

/// Gate.io APIv4 운영 Base URL.
pub const DEFAULT_BASE_URL: &str = "https://api.gateio.ws";

/// APIv4 공통 경로 프리픽스. 서명 대상 PATH는 이 프리픽스를 포함한 전체 경로다.
pub const API_PREFIX: &str = "/api/v4";

/// Gate.io 어댑터 설정. api key/secret은 호출자가 주입.
///
/// 시세 엔드포인트(`market`)는 키 없이도 동작하므로 `api_key`/`api_secret`은
/// 빈 문자열을 허용한다. 서명 엔드포인트(`trade`) 호출 시점에 비어 있으면
/// [`GateioError::Auth`]로 거부한다.
///
/// Gate에는 Binance류의 `recv_window`가 없다 — 대신 서명 `Timestamp`(초)가
/// 서버 시각과 60초 이상 벌어지면 거부된다(고정 윈도).
#[derive(Debug, Clone)]
pub struct GateioConfig {
    /// 발급받은 API Key. `KEY` 헤더로 전송.
    pub api_key: String,
    /// 발급받은 API Secret. HMAC-SHA512 서명 키. **네트워크로 전송하지 않는다.**
    pub api_secret: String,
    /// API Base URL. 기본 운영 `https://api.gateio.ws`.
    pub base_url: String,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형
    /// 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl GateioConfig {
    /// api key/secret로 기본(운영) 설정 생성.
    pub fn new(api_key: impl Into<String>, api_secret: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            api_secret: api_secret.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            rate_limit: None,
        }
    }

    /// 키 없는 시세 전용 설정. 서명 엔드포인트 호출 시 [`GateioError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new(), String::new())
    }

    /// 환경변수에서 설정 로드.
    /// `GATEIO_API_KEY` `GATEIO_API_SECRET` 필수.
    /// `GATEIO_BASE_URL`(선택, 기본 운영), `GATEIO_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| GateioError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("GATEIO_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let rate_limit = match std::env::var("GATEIO_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| GateioError::Auth(format!("invalid GATEIO_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            api_key: var("GATEIO_API_KEY")?,
            api_secret: var("GATEIO_API_SECRET")?,
            base_url,
            rate_limit,
        })
    }
}
