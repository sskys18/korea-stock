//! Pacifica 어댑터 — 한국 주식 무기한 선물(KR-stock perps), Solana perp DEX.
//!
//! KIS/Toss/Binance/Lighter 어댑터와 동일 크레이트 내 형제 모듈. Pacifica는 Solana
//! 위 CEX형 오더북 perp DEX이며, KR 대형주 무기한 선물 둘을 상장한다(2026-06-04 라이브):
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`]). **현대차·KOSPI200 미상장.**
//!
//! 설계는 Binance/Lighter를 미러링한다 — 형제 모듈, 독립 에러([`PacificaError`]), 액세서
//! 패턴, 정밀도 보존 String. 차이는 인증이다:
//!
//! 1. **시세는 키 불필요**(공개 GET). `market()`은 완전 구현이다 — 마켓정보·마크가·펀딩
//!    ·호가·캔들. 필드명은 2026-06-04 라이브 응답으로 확정했다.
//! 2. **거래 서명 — 메시지+암호 코어 검증 / e2e 미검증(게이트)**. Pacifica 주문은
//!    트레이더의 Solana 키(Ed25519)로 **정렬된 compact JSON** 메시지를 서명하고 base58로
//!    인코딩해 제출한다([`sign`]에 격리).
//!    - **검증됨:** [`sign::tests::matches_sdk_golden_vector`]가 공식 `python-sdk`의
//!      실제 `prepare_message` 정규화 문자열 + 고정 seed Ed25519 base58 서명을 **바이트
//!      일치**로 대조한다(이 세션에서 SDK 클론해 캡처). 메시지 레이아웃·키정렬·compact
//!      JSON·Ed25519·base58이 공식 구현과 일치함이 증명됐다.
//!    - **미검증:** 실키로 서버가 주문을 수락하는 end-to-end 경로는 키 없이 확인 불가.
//!      따라서 실제 제출은 [`PacificaConfig::allow_unverified_signing`] 게이트(기본 false)
//!      뒤에 두고, 켜기 전 테스트넷에서 검증해야 한다.
//!
//! ```ignore
//! use korea_stock::global::pacifica::{PacificaClient, PacificaConfig, SAMSUNG};
//!
//! // 시세 — 키 불필요, 완전 동작.
//! let client = PacificaClient::new(PacificaConfig::public())?;
//! let m = client.market();
//! let price = m.price(SAMSUNG).await?;             // 마크가·펀딩
//! let book = m.order_book(SAMSUNG).await?;         // 호가
//! println!("mark={} funding={} bid={}", price.mark, price.funding, book.bids[0].price);
//! ```

mod client;
mod config;
mod error;
mod sign;

pub mod market;
pub mod trade;

pub use client::{PacificaClient, RawRequest};
pub use config::{
    PacificaConfig, DEFAULT_BASE_URL, DEFAULT_EXPIRY_WINDOW_MS, TESTNET_BASE_URL,
};
pub use error::{PacificaError, Result as PacificaResult};

/// 삼성전자 무기한 선물 심볼.
pub const SAMSUNG: &str = "SAMSUNG";
/// SK하이닉스 무기한 선물 심볼.
pub const SK_HYNIX: &str = "SKHYNIX";

/// Pacifica가 상장한 한국 주식 무기한 선물 심볼 전체 — **현대차·지수 미상장**.
///
/// 출처: 2026-06-04 라이브 `GET /api/v1/info`(69 마켓). 현대차(`HYUNDAI`)는 없다.
/// **검증 권장:** 운영 전 `market().market_info(sym)`로 존재·`instrument_type=="perpetual"` 확인.
pub const KR_SYMBOLS: [&str; 2] = [SAMSUNG, SK_HYNIX];

/// [`crate::KrStock`] → Pacifica 심볼. 미상장 종목·지수는 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => return None, // Pacifica는 현대차 perp 미상장
        Kospi200 => return None,     // Pacifica는 KR 지수 perp 미상장
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KrStock;

    #[test]
    fn symbol_mapping() {
        assert_eq!(symbol(KrStock::SamsungElec), Some(SAMSUNG));
        assert_eq!(symbol(KrStock::SkHynix), Some(SK_HYNIX));
        assert_eq!(symbol(KrStock::HyundaiMotor), None);
        assert_eq!(symbol(KrStock::Kospi200), None);
    }

    #[test]
    fn kr_symbols_listed() {
        assert_eq!(KR_SYMBOLS, ["SAMSUNG", "SKHYNIX"]);
    }
}
