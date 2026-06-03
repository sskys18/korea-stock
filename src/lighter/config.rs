use crate::lighter::error::{LighterError, Result};

/// Lighter(zkLighter) 메인넷 REST Base URL.
///
/// 공식 SDK가 가리키는 호스트. 시세 엔드포인트는 키 없이 동작한다.
pub const DEFAULT_BASE_URL: &str = "https://mainnet.zklighter.elliot.ai";

/// Lighter 테스트넷 REST Base URL.
///
/// **거래 코드 개발·검증은 반드시 여기서 한다.** 단, 본 어댑터는 서명을 검증하지
/// 못해 data-only이므로 테스트넷에서도 서명 제출은 [`LighterError::SignerUnavailable`].
pub const TESTNET_BASE_URL: &str = "https://testnet.zklighter.elliot.ai";

/// Lighter 어댑터 설정.
///
/// 시세(`market`)는 키 없이 동작하므로 자격증명 필드는 모두 선택이다. 거래 서명에는
/// 계정 인덱스 + API 키 인덱스 + API 키 개인키가 필요하지만, 본 어댑터는 서명을
/// 검증하지 못해(data-only) 이 값들이 채워져 있어도 서명 제출은 거부된다. 구조체에
/// 보존하는 이유는 서명 구현이 추가될 때의 호환을 위해서다.
#[derive(Debug, Clone)]
pub struct LighterConfig {
    /// 계정 인덱스 (`account_index`). nextNonce·서명에 필요. 시세엔 불필요.
    pub account_index: Option<u64>,
    /// API 키 인덱스 (`api_key_index`, 0~255). 계정당 최대 256개 키.
    pub api_key_index: u8,
    /// API 키 개인키 (hex). zk 서명 키. **네트워크로 전송하지 않는다.**
    /// 비어 있으면 서명 호출 시 [`LighterError::Auth`].
    pub api_key_private_key: String,
    /// API Base URL. 기본 메인넷.
    pub base_url: String,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl LighterConfig {
    /// 자격증명으로 기본(메인넷) 설정 생성.
    pub fn new(
        account_index: u64,
        api_key_index: u8,
        api_key_private_key: impl Into<String>,
    ) -> Self {
        Self {
            account_index: Some(account_index),
            api_key_index,
            api_key_private_key: api_key_private_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            rate_limit: None,
        }
    }

    /// 키 없는 시세 전용 설정. 거래 액세서 호출 시 [`LighterError::Auth`].
    pub fn public() -> Self {
        Self {
            account_index: None,
            api_key_index: 0,
            api_key_private_key: String::new(),
            base_url: DEFAULT_BASE_URL.to_string(),
            rate_limit: None,
        }
    }

    /// 테스트넷으로 base_url 교체 (builder).
    pub fn testnet(mut self) -> Self {
        self.base_url = TESTNET_BASE_URL.to_string();
        self
    }

    /// 환경변수에서 설정 로드.
    /// `LIGHTER_ACCOUNT_INDEX` `LIGHTER_API_KEY_PRIVATE_KEY` 필수.
    /// `LIGHTER_API_KEY_INDEX`(선택, 기본 0), `LIGHTER_BASE_URL`(선택, 기본 메인넷),
    /// `LIGHTER_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| LighterError::Auth(format!("missing env var {name}")))
        }
        let account_index = var("LIGHTER_ACCOUNT_INDEX")?
            .parse()
            .map_err(|_| LighterError::Auth("invalid LIGHTER_ACCOUNT_INDEX".into()))?;
        let api_key_index = match std::env::var("LIGHTER_API_KEY_INDEX") {
            Ok(s) => s
                .parse()
                .map_err(|_| LighterError::Auth(format!("invalid LIGHTER_API_KEY_INDEX: {s}")))?,
            Err(_) => 0,
        };
        let base_url =
            std::env::var("LIGHTER_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let rate_limit = match std::env::var("LIGHTER_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| LighterError::Auth(format!("invalid LIGHTER_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            account_index: Some(account_index),
            api_key_index,
            api_key_private_key: var("LIGHTER_API_KEY_PRIVATE_KEY")?,
            base_url,
            rate_limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_has_no_creds() {
        let c = LighterConfig::public();
        assert!(c.account_index.is_none());
        assert!(c.api_key_private_key.is_empty());
        assert_eq!(c.base_url, DEFAULT_BASE_URL);
    }

    #[test]
    fn testnet_swaps_base_url() {
        let c = LighterConfig::new(42, 0, "0xdeadbeef").testnet();
        assert_eq!(c.base_url, TESTNET_BASE_URL);
        assert_eq!(c.account_index, Some(42));
    }
}
