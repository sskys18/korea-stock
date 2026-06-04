use std::path::PathBuf;

use crate::domestic::kis::error::{KisError, Result};

/// 실전투자 / 모의투자 환경.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Real,
    Mock,
}

impl Environment {
    /// REST API 베이스 URL.
    pub fn rest_base(&self) -> &'static str {
        match self {
            Environment::Real => "https://openapi.koreainvestment.com:9443",
            Environment::Mock => "https://openapivts.koreainvestment.com:29443",
        }
    }

    /// 실시간 WebSocket 베이스 URL (Plan 3에서 사용).
    pub fn ws_base(&self) -> &'static str {
        match self {
            Environment::Real => "ws://ops.koreainvestment.com:21000",
            Environment::Mock => "ws://ops.koreainvestment.com:31000",
        }
    }

    /// REST 레이트리밋 (요청/초). 실전 20, 모의 2.
    pub fn rate_limit(&self) -> u32 {
        match self {
            Environment::Real => 20,
            Environment::Mock => 2,
        }
    }
}

/// 어댑터 설정. app key/secret/계좌번호는 호출자가 주입.
#[derive(Debug, Clone)]
pub struct KisConfig {
    pub app_key: String,
    pub app_secret: String,
    /// 종합계좌번호 앞 8자리 (CANO).
    pub account_no: String,
    /// 계좌상품코드 뒤 2자리 (ACNT_PRDT_CD), 예: "01".
    pub account_product: String,
    pub environment: Environment,
    /// 토큰 캐시 파일 경로. None이면 메모리 캐시만.
    pub token_cache_path: Option<PathBuf>,
    /// POST 주문 호출 시 hashkey 발급·첨부 여부. 기본 false (KIS 비강제).
    pub use_hashkey: bool,
}

impl KisConfig {
    /// 환경변수에서 설정 로드.
    /// `KIS_APP_KEY` `KIS_APP_SECRET` `KIS_ACCOUNT_NO` `KIS_ACCOUNT_PRODUCT`
    /// `KIS_ENV`(real|mock, 기본 mock). 토큰 캐시는 기본 경로 사용.
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name).map_err(|_| KisError::Auth(format!("missing env var {name}")))
        }
        let environment = match std::env::var("KIS_ENV").as_deref() {
            Ok("real") => Environment::Real,
            Ok("mock") | Err(_) => Environment::Mock,
            Ok(other) => return Err(KisError::Auth(format!("invalid KIS_ENV: {other}"))),
        };
        Ok(Self {
            app_key: var("KIS_APP_KEY")?,
            app_secret: var("KIS_APP_SECRET")?,
            account_no: var("KIS_ACCOUNT_NO")?,
            account_product: var("KIS_ACCOUNT_PRODUCT")?,
            environment,
            token_cache_path: Self::default_token_cache_path(),
            use_hashkey: false,
        })
    }

    /// 기본 토큰 캐시 경로: `~/.kis/token.json`.
    pub fn default_token_cache_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".kis").join("token.json"))
    }
}
