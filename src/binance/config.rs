use crate::binance::error::{BinanceError, Result};

/// Binance USDM Futures(fapi) 운영 Base URL.
pub const DEFAULT_BASE_URL: &str = "https://fapi.binance.com";

/// Binance USDM Futures 테스트넷 Base URL.
///
/// 시세 엔드포인트는 운영과 데이터가 다르며, 주문은 모의 체결된다. 키 발급은
/// <https://testnet.binancefuture.com>. **거래 코드 개발·검증은 반드시 여기서 한다.**
pub const TESTNET_BASE_URL: &str = "https://testnet.binancefuture.com";

/// Binance 어댑터 설정. api key/secret은 호출자가 주입.
///
/// 시세 엔드포인트(`market`)는 키 없이도 동작하므로 `api_key`/`api_secret`은
/// 빈 문자열을 허용한다. 서명 엔드포인트(`trade`) 호출 시점에 비어 있으면
/// [`BinanceError::Auth`]로 거부한다.
#[derive(Debug, Clone)]
pub struct BinanceConfig {
    /// 발급받은 API Key. `X-MBX-APIKEY` 헤더로 전송.
    pub api_key: String,
    /// 발급받은 API Secret. HMAC-SHA256 서명 키. **네트워크로 전송하지 않는다.**
    pub api_secret: String,
    /// API Base URL. 기본 운영 `https://fapi.binance.com`.
    pub base_url: String,
    /// 서명 요청 유효시간(ms). 서버 수신 시각이 `timestamp + recv_window`를 넘으면
    /// 거부(-1021). 기본 5000, 최대 60000.
    pub recv_window: u64,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429/418 반응형
    /// 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl BinanceConfig {
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

    /// 키 없는 시세 전용 설정 (테스트넷/운영 시세 조회용). 서명 엔드포인트 호출 시
    /// [`BinanceError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new(), String::new())
    }

    /// 테스트넷으로 base_url 교체 (builder).
    pub fn testnet(mut self) -> Self {
        self.base_url = TESTNET_BASE_URL.to_string();
        self
    }

    /// 환경변수에서 설정 로드.
    /// `BINANCE_API_KEY` `BINANCE_API_SECRET` 필수.
    /// `BINANCE_FUTURES_BASE_URL`(선택, 기본 운영), `BINANCE_RECV_WINDOW`(선택, ms),
    /// `BINANCE_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| BinanceError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("BINANCE_FUTURES_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let recv_window = match std::env::var("BINANCE_RECV_WINDOW") {
            Ok(s) => s
                .parse()
                .map_err(|_| BinanceError::Auth(format!("invalid BINANCE_RECV_WINDOW: {s}")))?,
            Err(_) => 5000,
        };
        let rate_limit = match std::env::var("BINANCE_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| BinanceError::Auth(format!("invalid BINANCE_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            api_key: var("BINANCE_API_KEY")?,
            api_secret: var("BINANCE_API_SECRET")?,
            base_url,
            recv_window,
            rate_limit,
        })
    }
}
