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
//! 1. **시세는 키 불필요**(REST 공개). `market()`·[`realtime`]은 완전 구현이다.
//! 2. **거래 서명 — 암호 코어는 검증, 와이어 봉투는 미검증(게이트)**. Lighter 주문
//!    서명은 zk 친화 해시(Poseidon2/plonky2) + Schnorr over ECgFp5(GFp5 위 소수위
//!    곡선) 커스텀 스킴이고, 공식 SDK(`lighter-python`)는 **네이티브 바이너리
//!    (`lighter-go`)를 FFI로 호출**해 서명한다. 본 어댑터는 이를 [`sign`]에 순수
//!    Rust로 포팅했다.
//!    - **검증됨:** `poseidon_crypto/signature/schnorr/schnorr_test.go`
//!      `TestComparativeSchnorrSignAndVerify`의 결정적 벡터(sk,msg,k)→(S,E) 3케이스를
//!      `sign` `#[cfg(test)]`에 박아 통과 확인했고, 그 벡터가 **upstream Go 소스와
//!      바이트 일치**함을 독립 대조했다(Schnorr sign+verify가 GFp5·Poseidon2·scalar·
//!      곡선 연산을 독립 경로로 전이 검증). → 암호 프리미티브는 정확하다.
//!    - **미검증:** `tx_info` JSON 와이어 봉투·십진 스케일링·`chain_id`·order_expiry
//!      확장은 불투명한 native `.so`가 만들며 공식 픽스처가 없다. 메시지에 어떤 필드가
//!      어떤 바이트 레이아웃으로 들어가는지 종단 확정 불가. 따라서 **end-to-end 서명
//!      제출**은 [`LighterConfig::allow_unverified_signing`] 게이트(기본 false) 뒤에
//!      있고, 켜기 전 `scripts/lighter_capture_vector.md`로 공식 SDK 출력과 대조해야
//!      한다. 상태: **암호 검증 / 봉투 미검증(게이트)**.
//!
//! ```ignore
//! use korea_stock::global::lighter::{LighterClient, LighterConfig, SAMSUNGUSD};
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
pub(crate) mod sign;

pub mod market;
pub mod realtime;
pub mod trade;

pub use client::{LighterClient, RawRequest};
pub use config::{LighterConfig, DEFAULT_BASE_URL, TESTNET_BASE_URL, TESTNET_CHAIN_ID};
pub use error::{LighterError, Result as LighterResult};
pub use realtime::{BookEvent, LighterRealtime};

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

/// [`crate::KrStock`] → Lighter 심볼(활성 USD-마진 변형). 미상장 종목은 `None`.
///
/// 베어 심볼이 아닌 체결되는 `*USD` 마켓을 돌려준다.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNGUSD,
        SkHynix => SKHYNIXUSD,
        HyundaiMotor => HYUNDAIUSD,
        // Korea-composite 지수(market_id=142). **현재 inactive** — 심볼은 상장돼
        // 있으나 체결되지 않는다(bitunix PREVIEW와 동일하게 Some 유지 + 상태 별도확인).
        Kospi200 => KR_COMPOSITE,
    })
}
