//! Toobit USDT-M Perpetual Swap 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! Binance/MEXC 어댑터와 동일 크레이트 내 형제 모듈. 2026년 Toobit이 상장한 한국
//! 대형주 무기한 스왑(USDT 마진)을 대상으로 한다: 삼성전자([`SAMSUNG`])·
//! SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]). 심볼은 `<UNDERLYING>-SWAP-USDT` 형식.
//!
//! 구조·네이밍은 binance 정본 템플릿을 미러링한다 — 형제 모듈, 독립 에러
//! ([`ToobitError`]), 액세서 패턴. 인증은 Binance-호환 HMAC-SHA256이다:
//! `signature = HMAC_SHA256(secret, totalParams)`, `totalParams = queryString + body`,
//! `X-BB-APIKEY` 헤더. 시세(`market`)는 키 없이, 거래(`trade`)는 서명으로 호출한다.
//!
//! ```ignore
//! use korea_stock::global::toobit::{ToobitClient, ToobitConfig, SAMSUNG};
//! use korea_stock::global::toobit::trade::{OrderRequest, Side};
//!
//! // 시세 — 키 불필요.
//! let pub_client = ToobitClient::new(ToobitConfig::public())?;
//! let mark = pub_client.market().mark_price(SAMSUNG).await?;
//! let ticker = pub_client.market().ticker_24hr(SAMSUNG).await?;
//! println!("mark={} last={}", mark.price, ticker.last_price);
//!
//! // 거래 — 키 필요(`from_env`). 실주문은 실자금을 움직인다.
//! let client = ToobitClient::new(ToobitConfig::from_env()?)?;
//! let order = OrderRequest::limit(SAMSUNG, Side::BuyOpen, "1", "230.00");
//! let resp = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{RawRequest, ToobitClient};
pub use config::{ToobitConfig, DEFAULT_BASE_URL};
pub use error::{Result as ToobitResult, ToobitError};

/// 삼성전자 무기한 스왑 심볼.
pub const SAMSUNG: &str = "SAMSUNG-SWAP-USDT";
/// SK하이닉스 무기한 스왑 심볼.
pub const SK_HYNIX: &str = "SKHYNIX-SWAP-USDT";
/// 현대차 무기한 스왑 심볼.
pub const HYUNDAI: &str = "HYUNDAI-SWAP-USDT";

/// Toobit이 상장한 한국 주식 무기한 스왑 심볼 전체.
///
/// 출처: Toobit `GET /api/v1/exchangeInfo` 라이브 심볼 목록(2026-06-04, `contracts[]`).
/// **검증 권장:** 운영 전 `exchangeInfo`로 심볼 존재·`status=="TRADING"` 확인.
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → Toobit 심볼. 미상장 종목은 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => return None, // Toobit은 KR 지수 perp 미상장
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KrStock;

    #[test]
    fn symbol_maps_three_stocks() {
        assert_eq!(symbol(KrStock::SamsungElec), Some(SAMSUNG));
        assert_eq!(symbol(KrStock::SkHynix), Some(SK_HYNIX));
        assert_eq!(symbol(KrStock::HyundaiMotor), Some(HYUNDAI));
    }

    #[test]
    fn kospi200_index_unlisted() {
        assert_eq!(symbol(KrStock::Kospi200), None);
    }

    #[test]
    fn kr_symbols_match_consts() {
        assert_eq!(KR_SYMBOLS, [SAMSUNG, SK_HYNIX, HYUNDAI]);
    }
}
