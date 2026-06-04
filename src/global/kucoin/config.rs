use crate::global::kucoin::error::{KucoinError, Result};

/// KuCoin Futures 운영 Base URL (시세·거래 공통 호스트).
pub const DEFAULT_BASE_URL: &str = "https://api-futures.kucoin.com";

/// KuCoin Futures 어댑터 설정. api key/secret/passphrase는 호출자가 주입.
///
/// 시세 엔드포인트(`market`)는 키 없이 동작하므로 자격증명은 빈 문자열을 허용한다.
/// 서명 엔드포인트(`trade`) 호출 시점에 비어 있으면 [`KucoinError::Auth`]로 거부한다.
///
/// KuCoin Futures 샌드박스는 폐지되었으므로 `testnet()` 헬퍼·테스트넷 base URL은 두지 않는다.
#[derive(Debug, Clone)]
pub struct KucoinConfig {
    /// 발급받은 API Key. `KC-API-KEY` 헤더로 전송.
    pub api_key: String,
    /// 발급받은 API Secret. HMAC-SHA256 서명·패스프레이즈 암호화 키. **전송하지 않는다.**
    pub api_secret: String,
    /// API Key 생성 시 지정한 패스프레이즈(원문). v2 키는 이를 secret으로 HMAC-SHA256 →
    /// base64한 값을 `KC-API-PASSPHRASE`로 전송한다(원문 자체는 전송하지 않는다).
    pub api_passphrase: String,
    /// API Base URL. 기본 운영 `https://api-futures.kucoin.com`.
    pub base_url: String,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl KucoinConfig {
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

    /// 키 없는 시세 전용 설정. 서명 엔드포인트 호출 시 [`KucoinError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new(), String::new(), String::new())
    }

    /// 환경변수에서 설정 로드.
    /// `KUCOIN_API_KEY` `KUCOIN_API_SECRET` `KUCOIN_API_PASSPHRASE` 필수.
    /// `KUCOIN_BASE_URL`(선택, 기본 운영), `KUCOIN_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| KucoinError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("KUCOIN_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let rate_limit = match std::env::var("KUCOIN_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| KucoinError::Auth(format!("invalid KUCOIN_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            api_key: var("KUCOIN_API_KEY")?,
            api_secret: var("KUCOIN_API_SECRET")?,
            api_passphrase: var("KUCOIN_API_PASSPHRASE")?,
            base_url,
            rate_limit,
        })
    }
}
