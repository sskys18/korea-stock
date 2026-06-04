use crate::global::pacifica::error::{PacificaError, Result};

/// Pacifica(Solana perp DEX) 메인넷 REST Base URL.
///
/// 공식 SDK `common/constants.py`가 가리키는 호스트(`/api/v1`는 경로가 붙는다).
/// 시세 엔드포인트(GET)는 키 없이 동작한다.
pub const DEFAULT_BASE_URL: &str = "https://api.pacifica.fi";

/// Pacifica 테스트넷 REST Base URL (SDK 주석의 `test-api`).
///
/// **거래 코드 개발·검증은 여기서 한다.** 단, 본 어댑터는 end-to-end 서명 수락을
/// 검증하지 못해(게이트) 테스트넷에서도 게이트가 닫혀 있으면 서명 제출은
/// [`PacificaError::SignerUnavailable`].
pub const TESTNET_BASE_URL: &str = "https://test-api.pacifica.fi";

/// 서명 만료창 기본값(ms). SDK 예제의 `expiry_window = 5_000`.
pub const DEFAULT_EXPIRY_WINDOW_MS: u64 = 5_000;

/// Pacifica 어댑터 설정.
///
/// 시세(`market`)는 키 없이 동작하므로 자격증명은 선택이다. 거래 서명에는 트레이더의
/// Solana 시크릿 키가 필요하다(base58 64바이트 또는 32바이트 seed). 본 어댑터는
/// 메시지 정규화·Ed25519 코어를 공식 SDK 골든 벡터로 검증했으나, 실주문 수락은
/// 미검증이라 [`allow_unverified_signing`](Self::allow_unverified_signing) 게이트
/// 뒤에 둔다.
#[derive(Debug, Clone)]
pub struct PacificaConfig {
    /// 트레이더 Solana 시크릿 키 (base58). 64바이트(seed‖pubkey) 또는 32바이트 seed.
    /// zk/Ed25519 서명 키. **네트워크로 전송하지 않는다.** 비어 있으면 서명 호출 시
    /// [`PacificaError::Auth`].
    pub solana_secret_key: String,
    /// API Base URL. 기본 메인넷 `https://api.pacifica.fi`.
    pub base_url: String,
    /// 서명 만료창(ms). 서버는 `timestamp + expiry_window` 이후 요청을 거부한다.
    /// 기본 [`DEFAULT_EXPIRY_WINDOW_MS`].
    pub expiry_window_ms: u64,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형 백오프에만 의존.
    pub rate_limit: Option<u32>,
    /// **미검증 서명 허용 게이트 (기본 false).**
    ///
    /// 메시지 정규화·Ed25519 코어는 공식 SDK와 교차검증됐으나(골든 벡터), 실키로
    /// 서버가 주문을 수락하는 end-to-end 경로는 미검증이다. 이 값이 `true`가 아니면
    /// [`crate::global::pacifica::trade::Trade::place`]·
    /// [`cancel`](crate::global::pacifica::trade::Trade::cancel)은
    /// [`PacificaError::SignerUnavailable`]로 거부한다. 실자금 사용 전 테스트넷에서
    /// 명시적으로 켜고 검증할 것.
    pub allow_unverified_signing: bool,
}

impl PacificaConfig {
    /// Solana 시크릿 키(base58)로 기본(메인넷) 설정 생성.
    pub fn new(solana_secret_key: impl Into<String>) -> Self {
        Self {
            solana_secret_key: solana_secret_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            expiry_window_ms: DEFAULT_EXPIRY_WINDOW_MS,
            rate_limit: None,
            allow_unverified_signing: false,
        }
    }

    /// 키 없는 시세 전용 설정. 거래 액세서 호출 시 [`PacificaError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new())
    }

    /// 테스트넷으로 base_url 교체 (builder).
    pub fn testnet(mut self) -> Self {
        self.base_url = TESTNET_BASE_URL.to_string();
        self
    }

    /// 미검증 서명 허용 게이트를 켠다 (builder). 검증 전 사용 금지.
    pub fn allow_unverified_signing(mut self, v: bool) -> Self {
        self.allow_unverified_signing = v;
        self
    }

    /// 환경변수에서 설정 로드.
    /// `PACIFICA_SOLANA_SECRET_KEY` 필수.
    /// `PACIFICA_BASE_URL`(선택, 기본 메인넷), `PACIFICA_EXPIRY_WINDOW`(선택, ms),
    /// `PACIFICA_RATE_LIMIT`(선택, req/s),
    /// `PACIFICA_ALLOW_UNVERIFIED_SIGNING`(선택, "1"/"true"면 게이트 오픈, 기본 false).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| PacificaError::Auth(format!("missing env var {name}")))
        }
        let base_url =
            std::env::var("PACIFICA_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let expiry_window_ms = match std::env::var("PACIFICA_EXPIRY_WINDOW") {
            Ok(s) => s
                .parse()
                .map_err(|_| PacificaError::Auth(format!("invalid PACIFICA_EXPIRY_WINDOW: {s}")))?,
            Err(_) => DEFAULT_EXPIRY_WINDOW_MS,
        };
        let rate_limit = match std::env::var("PACIFICA_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| PacificaError::Auth(format!("invalid PACIFICA_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        let allow_unverified_signing = matches!(
            std::env::var("PACIFICA_ALLOW_UNVERIFIED_SIGNING").as_deref(),
            Ok("1") | Ok("true") | Ok("TRUE")
        );
        Ok(Self {
            solana_secret_key: var("PACIFICA_SOLANA_SECRET_KEY")?,
            base_url,
            expiry_window_ms,
            rate_limit,
            allow_unverified_signing,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_has_no_key() {
        let c = PacificaConfig::public();
        assert!(c.solana_secret_key.is_empty());
        assert_eq!(c.base_url, DEFAULT_BASE_URL);
        assert!(!c.allow_unverified_signing);
    }

    #[test]
    fn testnet_swaps_base_url() {
        let c = PacificaConfig::new("abc").testnet();
        assert_eq!(c.base_url, TESTNET_BASE_URL);
        assert_eq!(c.solana_secret_key, "abc");
    }

    #[test]
    fn gate_builder_opens() {
        let c = PacificaConfig::new("abc").allow_unverified_signing(true);
        assert!(c.allow_unverified_signing);
    }
}
