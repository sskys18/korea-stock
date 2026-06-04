//! Aster perp DEX 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! Aster는 BNB Chain(멀티체인) 위의 perp DEX로, 오프체인 매칭 + 온체인 정산
//! 구조다. fapi 표면은 **Binance USDM Futures와 호환**(`/fapi/v1/*`, `/fapi/v2/*`)
//! 이라 시세·HMAC 서명 거래 경로가 Binance 어댑터를 거의 그대로 미러링한다.
//! 2026-06-04 기준 상장 한국 종목: 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`]).
//! **현대차·KOSPI200 미상장.**
//!
//! 인증은 두 경로가 존재한다:
//!  - **V1(Legacy) fapi: Binance 호환 API-key + HMAC-SHA256** — `X-MBX-APIKEY`
//!    헤더 + `signature` 쿼리 파라미터. **이 어댑터가 구현하는 경로.**
//!  - V3(Recommended) fapi: **EIP-712 온체인 지갑 서명**(`signer`/`user`/`nonce`).
//!    이 어댑터는 V3 EIP-712 경로를 **의도적으로 구현하지 않는다** — Binance
//!    호환 HMAC 표면만 사용한다(hyperliquid 모듈이 EIP-712 참조 구현).
//!
//! 시세(`market`)는 키 없이, 거래(`trade`)는 HMAC 서명으로 호출한다.
//!
//! ```ignore
//! use korea_stock::global::aster::{AsterClient, AsterConfig, SAMSUNG};
//! use korea_stock::global::aster::trade::{OrderRequest, Side};
//!
//! // 시세 — 키 불필요.
//! let pub_client = AsterClient::new(AsterConfig::public())?;
//! let idx = pub_client.market().premium_index(SAMSUNG).await?;
//! println!("mark={} funding={}", idx.mark_price, idx.last_funding_rate);
//!
//! // 거래 — HMAC 서명(api key/secret 필요).
//! let client = AsterClient::new(AsterConfig::from_env()?)?;
//! let order = OrderRequest::limit(SAMSUNG, Side::Buy, "3", "65.00");
//! let resp = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{AsterClient, RawRequest};
pub use config::{AsterConfig, DEFAULT_BASE_URL};
pub use error::{AsterError, Result as AsterResult};

/// 삼성전자 무기한 선물 심볼.
pub const SAMSUNG: &str = "SAMSUNGUSDT";
/// SK하이닉스 무기한 선물 심볼.
pub const SK_HYNIX: &str = "SKHYNIXUSDT";

/// Aster가 상장한 한국 주식 무기한 선물 심볼 전체 (length 2 — 현대차 미상장).
///
/// 출처: Aster fapi `exchangeInfo` 라이브(2026-06-04, 460+ 심볼 중 KR 2종).
/// **검증 권장:** 운영 전 `exchangeInfo`로 심볼 존재·`status=="TRADING"` 확인.
pub const KR_SYMBOLS: [&str; 2] = [SAMSUNG, SK_HYNIX];

/// [`crate::KrStock`] → Aster 심볼. 미상장 종목·지수는 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => return None, // Aster는 현대차 perp 미상장
        Kospi200 => return None,     // Aster는 KR 지수 perp 미상장
    })
}
