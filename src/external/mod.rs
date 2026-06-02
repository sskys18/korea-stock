//! 외부 소스 — KIS 미제공 데이터(공매도 잔고·외국인 보유량·전자공시).
//!
//! `feature = "external"`로 게이트. KIS 인증과 무관한 독립 클라이언트:
//! - [`KrxClient`] — KRX MDC(공매도 잔고, 외국인 보유량). **무료 KRX 계정** 필요(폼 로그인 세션).
//! - [`DartClient`] — OpenDART(전자공시). 자체 `crtfc_key` 필요.
//!
//! 응답 struct는 컴파일·serde 내성 보장. 와이어 검증은 `--ignored` 라이브 테스트로 수행
//! (`cargo test --features external -- --ignored`).

mod dart;
mod krx;

pub use dart::{DartClient, Disclosure, DisclosureList};
pub use krx::{ForeignHoldingRow, KrxClient, ShortBalanceRow};

/// 외부 소스 설정. KIS `KisConfig`와 분리.
#[derive(Debug, Clone, Default)]
pub struct ExternalConfig {
    /// OpenDART 인증키. 미설정 시 `DartClient` 사용 불가.
    pub dart_api_key: Option<String>,
    /// KRX 로그인 ID. 미설정 시 `KrxClient` 사용 불가.
    pub krx_id: Option<String>,
    /// KRX 로그인 PW.
    pub krx_pw: Option<String>,
}
