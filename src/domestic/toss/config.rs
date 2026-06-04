use std::path::PathBuf;

use crate::domestic::toss::error::{Result, TossError};

/// 토스증권 Open API 기본 Base URL.
pub const DEFAULT_BASE_URL: &str = "https://openapi.tossinvest.com";

/// 토스 어댑터 설정. client id/secret은 호출자가 주입.
///
/// KIS와 달리 실전/모의 `Environment` 분기가 없다 — 스펙 `servers`에 URL 1개뿐이다.
#[derive(Debug, Clone)]
pub struct TossConfig {
    /// 발급받은 클라이언트 ID.
    pub client_id: String,
    /// 발급받은 클라이언트 시크릿.
    pub client_secret: String,
    /// API Base URL. 기본 `https://openapi.tossinvest.com`.
    pub base_url: String,
    /// 토큰 캐시 파일 경로. None이면 메모리 캐시만.
    pub token_cache_path: Option<PathBuf>,
    /// 선택적 클라이언트측 글로벌 레이트 캡 (요청/초).
    ///
    /// 토스 레이트리밋은 그룹별이며 응답 헤더로만 통보되므로 정적으로 알 수 없다.
    /// 권위 throttle은 429 + `Retry-After` 반응형 재시도(client.rs). 이 값은 보조적
    /// 사전 캡일 뿐 — `None`이면 캡 없이 429 루프에만 의존한다.
    pub rate_limit: Option<u32>,
}

impl TossConfig {
    /// client id/secret로 기본 설정 생성. base_url·캐시 경로는 기본값.
    pub fn new(client_id: impl Into<String>, client_secret: impl Into<String>) -> Self {
        Self {
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            token_cache_path: Self::default_token_cache_path(),
            rate_limit: None,
        }
    }

    /// 환경변수에서 설정 로드.
    /// `TOSS_CLIENT_ID` `TOSS_CLIENT_SECRET` 필수.
    /// `TOSS_BASE_URL`(선택, 기본 prod), `TOSS_RATE_LIMIT`(선택, req/s).
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| TossError::Auth(format!("missing env var {name}")))
        }
        let base_url = std::env::var("TOSS_BASE_URL").unwrap_or_else(|_| DEFAULT_BASE_URL.to_string());
        let rate_limit = match std::env::var("TOSS_RATE_LIMIT") {
            Ok(s) => Some(
                s.parse()
                    .map_err(|_| TossError::Auth(format!("invalid TOSS_RATE_LIMIT: {s}")))?,
            ),
            Err(_) => None,
        };
        Ok(Self {
            client_id: var("TOSS_CLIENT_ID")?,
            client_secret: var("TOSS_CLIENT_SECRET")?,
            base_url,
            token_cache_path: Self::default_token_cache_path(),
            rate_limit,
        })
    }

    /// 기본 토큰 캐시 경로: `~/.toss/token.json`.
    pub fn default_token_cache_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".toss").join("token.json"))
    }
}
