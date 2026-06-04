//! WEEX Contract(USDT 무기한 선물) 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! Binance/MEXC 어댑터와 동일 크레이트 내 형제 모듈(공유 트레이트 없음). WEEX가
//! 상장한 한국 대형주 무기한 선물(USDT 마진, 20x)을 대상으로 한다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]) — 세 종목 모두
//! `cmt_` 접두어 + 소문자 표기(`cmt_samsungusdt` 등).
//!
//! 설계는 Binance를 미러링한다 — 형제 모듈, 독립 에러([`WeexError`]), 액세서 패턴.
//! 차이는 서명이다: WEEX는 Bitget 계열 스킴으로 `ACCESS-KEY/SIGN/TIMESTAMP/
//! PASSPHRASE` 헤더 + `Base64(HMAC_SHA256(secret, timestamp+METHOD+path[?query]+
//! body))`. **패스프레이즈 필수.** 시세(`market`)는 키 없이, 거래(`trade`)는 서명.
//!
//! ```ignore
//! use korea_stock::global::weex::{WeexClient, WeexConfig, SAMSUNG};
//! use korea_stock::global::weex::trade::{OrderRequest, Side, PositionSide};
//!
//! // 시세 — 키 불필요.
//! let pub_client = WeexClient::new(WeexConfig::public())?;
//! let t = pub_client.market().ticker(SAMSUNG).await?;
//! println!("last={} mark={} index={}", t.last, t.mark_price, t.index_price);
//!
//! // 거래 — 키+패스프레이즈 필요(운영 키만 존재, 테스트넷 없음).
//! let client = WeexClient::new(WeexConfig::from_env()?)?;
//! let order = OrderRequest::limit(SAMSUNG, Side::Buy, PositionSide::Long, "3", "230.00");
//! let resp = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{RawRequest, WeexClient};
pub use config::{WeexConfig, DEFAULT_BASE_URL};
pub use error::{Result as WeexResult, WeexError};

/// 삼성전자 무기한 선물 심볼 (시세).
pub const SAMSUNG: &str = "cmt_samsungusdt";
/// SK하이닉스 무기한 선물 심볼 (시세).
pub const SK_HYNIX: &str = "cmt_skhynixusdt";
/// 현대차 무기한 선물 심볼 (시세).
pub const HYUNDAI: &str = "cmt_hyundaiusdt";

/// WEEX가 상장한 한국 주식 무기한 선물 심볼 전체 (시세 표기).
///
/// 출처: `GET /capi/v2/market/contracts` 라이브 검증(2026-06-04).
/// `market().contracts()` 결과를 이 목록으로 필터하면 KR 종목만 추린다.
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → WEEX 시세 심볼. 미상장 종목/지수는 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => return None, // WEEX는 KR 지수 perp 미상장
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KrStock;

    #[test]
    fn symbol_maps_three_stocks_index_none() {
        assert_eq!(symbol(KrStock::SamsungElec), Some(SAMSUNG));
        assert_eq!(symbol(KrStock::SkHynix), Some(SK_HYNIX));
        assert_eq!(symbol(KrStock::HyundaiMotor), Some(HYUNDAI));
        assert_eq!(symbol(KrStock::Kospi200), None);
    }

    #[test]
    fn kr_symbols_have_cmt_prefix() {
        for s in KR_SYMBOLS {
            assert!(s.starts_with("cmt_"), "{s} missing cmt_ prefix");
            assert!(s.ends_with("usdt"));
        }
    }
}
