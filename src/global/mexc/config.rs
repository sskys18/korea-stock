use crate::global::mexc::error::{MexcError, Result};

/// MEXC Contract(선물) 운영 Base URL.
///
/// 주의: 현물 API(`https://api.mexc.com`)와 **호스트가 다르다**. Contract API는
/// 전용 호스트를 쓴다. 별도 테스트넷(sandbox)은 제공되지 않으므로 `testnet()`은
/// base_url을 바꾸지 않고, 거래 코드는 최소 수량으로 운영에서 신중히 검증한다.
pub const DEFAULT_BASE_URL: &str = "https://contract.mexc.com";

/// MEXC 어댑터 설정. access key/secret은 호출자가 주입.
///
/// 시세 엔드포인트(`market`)는 키 없이도 동작하므로 `access_key`/`secret_key`는
/// 빈 문자열을 허용한다. 서명 엔드포인트(`trade`) 호출 시점에 비어 있으면
/// [`MexcError::Auth`]로 거부한다.
#[derive(Debug, Clone)]
pub struct MexcConfig {
    /// 발급받은 Access Key. `ApiKey` 헤더로 전송.
    pub access_key: String,
    /// 발급받은 Secret Key. HMAC-SHA256 서명 키. **네트워크로 전송하지 않는다.**
    pub secret_key: String,
    /// API Base URL. 기본 운영 `https://contract.mexc.com`.
    pub base_url: String,
    /// 서명 요청 유효시간(초). 서버 수신 시각이 `Request-Time + recv_window`를 넘으면
    /// 거부. `Recv-Window` 헤더로 전송. 기본 10, 최대 60.
    pub recv_window: u64,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl MexcConfig {
    /// access key/secret로 기본(운영) 설정 생성.
    pub fn new(access_key: impl Into<String>, secret_key: impl Into<String>) -> Self {
        Self {
            access_key: access_key.into(),
            secret_key: secret_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            recv_window: 10,
            rate_limit: None,
        }
    }

    /// 키 없는 시세 전용 설정. 서명 엔드포인트 호출 시 [`MexcError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new(), String::new())
    }

    /// MEXC Contract는 별도 테스트넷이 없다. base_url을 바꾸지 않고 `self`를 반환한다
    /// (형제 어댑터와의 호출 시그니처 일관성을 위한 no-op builder).
    pub fn testnet(self) -> Self {
        self
    }

    /// 환경변수에서 설정 로드.
    /// `MEXC_ACCESS_KEY` `MEXC_SECRET_KEY` 필수.
    /// `MEXC_CONTRACT_BASE_URL`(선택, 기본 운영), `MEXC_RECV_WINDOW`(선택, 초),
    /// `MEXC_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| MexcError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("MEXC_CONTRACT_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let recv_window = match std::env::var("MEXC_RECV_WINDOW") {
            Ok(s) => s
                .parse()
                .map_err(|_| MexcError::Auth(format!("invalid MEXC_RECV_WINDOW: {s}")))?,
            Err(_) => 10,
        };
        let rate_limit = match std::env::var("MEXC_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| MexcError::Auth(format!("invalid MEXC_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            access_key: var("MEXC_ACCESS_KEY")?,
            secret_key: var("MEXC_SECRET_KEY")?,
            base_url,
            recv_window,
            rate_limit,
        })
    }
}
