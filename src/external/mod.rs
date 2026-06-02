//! 외부 소스 — KIS 미제공 데이터(공매도 잔고·외국인 보유량·전자공시).
//!
//! `feature = "external"`로 게이트. KIS 인증과 무관한 독립 클라이언트:
//! - [`KrxClient`] — KRX MDC(공매도 잔고, 외국인 보유량). 인증 불필요.
//! - [`DartClient`] — OpenDART(전자공시). 자체 `crtfc_key` 필요.
//!
//! 모든 응답 struct는 **와이어 미검증** — 컴파일·serde 내성까지만 보장.

mod dart;
mod krx;

pub use dart::{Disclosure, DisclosureList, DartClient};
pub use krx::{ForeignHoldingRow, KrxClient, ShortBalanceRow};

/// 외부 소스 설정. KIS `KisConfig`와 분리.
#[derive(Debug, Clone, Default)]
pub struct ExternalConfig {
    /// OpenDART 인증키. 미설정 시 `DartClient` 사용 불가.
    pub dart_api_key: Option<String>,
}
