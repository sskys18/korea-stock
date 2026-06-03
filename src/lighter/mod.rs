//! Lighter (zkLighter) 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! KIS/Toss/Binance 어댑터와 동일 크레이트 내 형제 모듈. Lighter는 Optimism 위
//! zk-rollup 오더북 DEX이며, KR 대형주 무기한 선물을 상장한 첫 DEX다(10x):
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]) 및
//! 한국종합지수([`KR_COMPOSITE`]).
//!
//! 설계는 Binance를 미러링한다 — 형제 모듈, 독립 에러([`LighterError`]), 액세서
//! 패턴, 정밀도 보존 String. 차이는 두 가지다:
//!
//! 1. **시세는 키 불필요**(REST 공개). `market()`은 완전 구현이다.
//! 2. **거래 서명은 검증 불가**. Lighter의 주문 서명은 zk 친화 해시(poseidon)+
//!    schnorr 류의 커스텀 스킴이고, 공식 SDK(`lighter-python`)·비공식
//!    `lighter-rust` 크레이트 **모두 네이티브 Go 바이너리(`lighter-go`)를 FFI로
//!    호출**해 서명한다. crates.io에 순수 Rust 구현이 없고, 이 세션에서 해시/서명
//!    알고리즘을 공식 문서로 재현 검증할 수 없었다. **서명을 날조하지 않는다**
//!    (HARD RULE 3). 따라서 [`trade`]는 타입드 요청/응답 구조체와 호출 경로까지
//!    완비하되, 실제 서명 제출은 [`LighterError::SignerUnavailable`]로 명확히 거부한다.
//!    상태: **data-only**.
//!
//! ```ignore
//! use korea_stock::lighter::{LighterClient, LighterConfig, SAMSUNGUSD};
//!
//! // 시세 — 키 불필요, 완전 동작.
//! let client = LighterClient::new(LighterConfig::public())?;
//! let m = client.market();
//! let id = m.market_id(SAMSUNGUSD).await?;           // 심볼 → market_id 해석
//! let book = m.order_book_orders(id, 10).await?;     // 호가 10단계
//! let candles = m.candles(id, "1h", start, end, 24).await?;
//! ```
//!
//! ## 심볼 주의
//! Lighter는 KR 종목당 두 마켓을 둔다: 베어 심볼(`SAMSUNG`)과 USD 마진 변형
//! (`SAMSUNGUSD`). **현재 활성·체결되는 쪽은 `*USD` 변형**이며, 베어 심볼은
//! `status=inactive`다(2026-06 기준 라이브 `orderBookDetails` 확인). 따라서
//! [`KR_SYMBOLS`]에는 활성 `*USD` 심볼을 담는다.

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{LighterClient, RawRequest};
pub use config::{LighterConfig, DEFAULT_BASE_URL, TESTNET_BASE_URL};
pub use error::{LighterError, Result as LighterResult};

/// 삼성전자 무기한 선물 심볼 (활성 USD-마진 변형, market_id=162).
pub const SAMSUNGUSD: &str = "SAMSUNGUSD";
/// SK하이닉스 무기한 선물 심볼 (활성 USD-마진 변형, market_id=161).
pub const SKHYNIXUSD: &str = "SKHYNIXUSD";
/// 현대차 무기한 선물 심볼 (활성 USD-마진 변형, market_id=160).
pub const HYUNDAIUSD: &str = "HYUNDAIUSD";

/// 삼성전자 베어 심볼 (market_id=140). **현재 inactive** — 체결되지 않는다.
/// 호환을 위해 노출하되 거래엔 [`SAMSUNGUSD`]를 쓴다.
pub const SAMSUNG: &str = "SAMSUNG";
/// SK하이닉스 베어 심볼 (market_id=143). **현재 inactive**.
pub const SK_HYNIX: &str = "SKHYNIX";
/// 현대차 베어 심볼 (market_id=141). **현재 inactive**.
pub const HYUNDAI: &str = "HYUNDAI";
/// 한국종합지수(Korean Composite) 무기한 (market_id=142). **현재 inactive**.
pub const KR_COMPOSITE: &str = "KRCOMP";

/// Lighter가 상장한 한국 주식 무기한 선물 심볼 — **활성·체결되는 마켓만**.
///
/// 출처: 2026-06 라이브 `GET /api/v1/orderBookDetails`. 베어 심볼
/// (`SAMSUNG`/`SKHYNIX`/`HYUNDAI`/`KRCOMP`)은 `status=inactive`라 제외한다.
/// **검증 권장:** 운영 전 `market().order_book_details(id)`로 `status=="active"` 확인.
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNGUSD, SKHYNIXUSD, HYUNDAIUSD];
