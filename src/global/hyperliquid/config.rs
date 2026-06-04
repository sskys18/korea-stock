use crate::global::hyperliquid::error::{HyperliquidError, Result};

/// Hyperliquid 운영 Base URL. `/info`·`/exchange` 공통.
pub const DEFAULT_BASE_URL: &str = "https://api.hyperliquid.xyz";

/// Hyperliquid 테스트넷 Base URL.
///
/// **거래 코드 개발·검증은 반드시 여기서 한다.** 테스트넷은 서명 `source`가
/// `"b"`(운영은 `"a"`)라서 동일 action도 서명이 달라진다 — base_url로 자동 선택된다.
/// 단, **HIP-3 Trade.xyz(`xyz`) 빌더 dex가 테스트넷에 존재하는지는 별도 확인**이
/// 필요하다(빌더 배포는 네트워크별).
pub const TESTNET_BASE_URL: &str = "https://api.hyperliquid-testnet.xyz";

/// Hyperliquid 어댑터 설정.
///
/// 시세(`/info`)는 키 없이 동작하므로 `secret_key`는 빈 문자열을 허용한다. 서명
/// 엔드포인트(`/exchange`) 호출 시점에 비어 있으면 [`HyperliquidError::Auth`]로
/// 거부한다.
///
/// **vault/agent 모델:** Hyperliquid는 마스터 지갑이 별도 *agent(API) wallet*에
/// 서명 권한을 위임할 수 있다. 이때 서명 키는 agent의 비밀키, 자금·포지션은
/// 마스터/볼트 주소 소속이다. `vault_address`가 `Some`이면 그 주소로 주문이
/// 라우팅되고(서브계정/볼트), `None`이면 서명 지갑 자신의 계정에 라우팅된다.
#[derive(Debug, Clone)]
pub struct HyperliquidConfig {
    /// secp256k1 비밀키(hex, "0x" 접두 허용). **네트워크로 전송하지 않는다.**
    /// 서명에만 사용. 시세 전용이면 빈 문자열 허용.
    pub secret_key: String,
    /// 선택적 vault/서브계정 주소("0x"+40hex). `Some`이면 action에 `vaultAddress`로
    /// 부착되고 서명 해시에도 포함된다.
    pub vault_address: Option<String>,
    /// API Base URL. 기본 운영 `https://api.hyperliquid.xyz`.
    pub base_url: String,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초). `None`이면 429 반응형
    /// 백오프에만 의존.
    pub rate_limit: Option<u32>,
}

impl HyperliquidConfig {
    /// 비밀키로 기본(운영) 설정 생성. vault 없음.
    pub fn new(secret_key: impl Into<String>) -> Self {
        Self {
            secret_key: secret_key.into(),
            vault_address: None,
            base_url: DEFAULT_BASE_URL.to_string(),
            rate_limit: None,
        }
    }

    /// 키 없는 시세 전용 설정. 서명 엔드포인트 호출 시 [`HyperliquidError::Auth`].
    pub fn public() -> Self {
        Self::new(String::new())
    }

    /// vault/서브계정 주소 지정 (builder).
    pub fn vault(mut self, address: impl Into<String>) -> Self {
        self.vault_address = Some(address.into());
        self
    }

    /// 테스트넷으로 base_url 교체 (builder). 서명 `source`가 자동으로 `"b"`가 된다.
    pub fn testnet(mut self) -> Self {
        self.base_url = TESTNET_BASE_URL.to_string();
        self
    }

    /// base_url이 운영 메인넷인지. 서명 `source`(`"a"`/`"b"`) 선택에 쓴다.
    pub fn is_mainnet(&self) -> bool {
        self.base_url == DEFAULT_BASE_URL
    }

    /// 환경변수에서 설정 로드.
    /// `HYPERLIQUID_SECRET_KEY` 필수.
    /// `HYPERLIQUID_VAULT_ADDRESS`(선택), `HYPERLIQUID_BASE_URL`(선택, 기본 운영),
    /// `HYPERLIQUID_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        let secret_key = std::env::var("HYPERLIQUID_SECRET_KEY")
            .map_err(|_| HyperliquidError::Auth("missing env var HYPERLIQUID_SECRET_KEY".into()))?;
        let vault_address = std::env::var("HYPERLIQUID_VAULT_ADDRESS").ok();
        let base_url =
            std::env::var("HYPERLIQUID_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let rate_limit = match std::env::var("HYPERLIQUID_RATE_LIMIT") {
            Ok(s) => Some(s.parse().map_err(|_| {
                HyperliquidError::Auth(format!("invalid HYPERLIQUID_RATE_LIMIT: {s}"))
            })?),
            Err(_) => None,
        };
        Ok(Self {
            secret_key,
            vault_address,
            base_url,
            rate_limit,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mainnet_detection_drives_source() {
        assert!(HyperliquidConfig::new("0x01").is_mainnet());
        assert!(!HyperliquidConfig::new("0x01").testnet().is_mainnet());
    }

    #[test]
    fn vault_builder_sets_address() {
        let c = HyperliquidConfig::new("0x01").vault("0xabc");
        assert_eq!(c.vault_address.as_deref(), Some("0xabc"));
    }
}
