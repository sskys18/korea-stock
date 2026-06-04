//! KuCoin Futures(USDT-마진 무기한 선물) 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! Binance/Bybit/MEXC 등과 동일 크레이트 내 형제 모듈(공유 트레이트 없음).
//! KuCoin Futures가 상장한 한국 대형주 무기한 선물을 대상으로 한다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]).
//! KuCoin 심볼은 끝에 **`M`**(margin/perp)이 붙는다(`SAMSUNGUSDTM`).
//!
//! 설계는 파일 구조(mod/config/error/client/market/trade)를 Binance 어댑터에서
//! 미러링하되, **서명·에러 메커니즘은 Bybit에 가깝되 다음 점이 다르다**:
//!  1. 인증을 **헤더**(`KC-API-KEY`/`KC-API-SIGN`/`KC-API-TIMESTAMP`/
//!     `KC-API-PASSPHRASE`/`KC-API-KEY-VERSION: 2`)로 보낸다.
//!  2. 서명 대상 prehash가 `timestamp + METHOD(대문자) + endpoint(?query 포함) + body`이며,
//!     `KC-API-SIGN = base64(HMAC_SHA256(secret, prehash))`. v2 패스프레이즈는
//!     `base64(HMAC_SHA256(secret, rawPassphrase))`. (Bybit는 hex, KuCoin은 **base64**.)
//!  3. 에러를 본문 envelope **문자열 `code`**로 판정한다. `code=="200000"`이 성공,
//!     그 외는 [`KucoinError::Api`]. (Bybit의 정수 `retCode==0`와 달리 **문자열**.)
//!
//! 시세(`market`)는 키 없이, 거래(`trade`)는 서명으로 호출한다. 마크가·지수가·펀딩비는
//! 계약 상세(`/contracts/{symbol}`)에, 최우선 호가는 티커(`/ticker`)에 담긴다.
//!
//! ```ignore
//! use korea_stock::global::kucoin::{KucoinClient, KucoinConfig, SAMSUNG};
//! use korea_stock::global::kucoin::trade::{OrderRequest, Side};
//!
//! // 시세 — 키 불필요.
//! let pub_client = KucoinClient::new(KucoinConfig::public())?;
//! let c = pub_client.market().contract(SAMSUNG).await?;
//! println!("mark={} funding={}", c.mark_price, c.funding_fee_rate);
//!
//! // 거래 — 실자금. 키/시크릿/패스프레이즈 필요.
//! let client = KucoinClient::new(KucoinConfig::from_env()?)?;
//! let order = OrderRequest::limit(SAMSUNG, Side::Buy, 3, "233.00");
//! let ack = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{KucoinClient, RawRequest};
pub use config::{KucoinConfig, DEFAULT_BASE_URL};
pub use error::{KucoinError, Result as KucoinResult};

/// 삼성전자 무기한 선물 심볼. **라이브 검증됨[High] (2026-06-04):**
/// `GET /api/v1/contracts/active`에 `status="Open"`으로 존재함을 확인.
pub const SAMSUNG: &str = "SAMSUNGUSDTM";
/// SK하이닉스 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04).
pub const SK_HYNIX: &str = "SKHYNIXUSDTM";
/// 현대차 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04).
pub const HYUNDAI: &str = "HYUNDAIUSDTM";

/// KuCoin Futures가 상장한 한국 주식 무기한 선물 심볼 전체 (USDT-마진 perp).
///
/// `market().contracts()` 결과를 이 목록으로 필터하면 KR 종목만 추린다.
/// **검증 권장:** 운영 전 `contracts/active`로 심볼 존재·`status=="Open"` 확인.
/// KuCoin은 KOSPI200 등 한국 지수 perp를 상장하지 않는다(개별 종목만).
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → KuCoin 심볼. 미상장 종목은 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => return None, // KuCoin은 KR 지수 perp 미상장
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kr_symbols_match_live_contracts_active() {
        // `GET /api/v1/contracts/active` (2026-06-04) 라이브 확인값과 상수 대조.
        assert_eq!(SAMSUNG, "SAMSUNGUSDTM");
        assert_eq!(SK_HYNIX, "SKHYNIXUSDTM");
        assert_eq!(HYUNDAI, "HYUNDAIUSDTM");
        assert_eq!(KR_SYMBOLS, [SAMSUNG, SK_HYNIX, HYUNDAI]);
        // KuCoin 선물 심볼은 끝에 M(margin/perp)이 붙는다.
        assert!(KR_SYMBOLS.iter().all(|s| s.ends_with("USDTM")));
    }

    #[test]
    fn symbol_maps_three_stocks_and_none_for_kospi() {
        use crate::KrStock;
        assert_eq!(symbol(KrStock::SamsungElec), Some(SAMSUNG));
        assert_eq!(symbol(KrStock::SkHynix), Some(SK_HYNIX));
        assert_eq!(symbol(KrStock::HyundaiMotor), Some(HYUNDAI));
        assert_eq!(symbol(KrStock::Kospi200), None);
    }
}
