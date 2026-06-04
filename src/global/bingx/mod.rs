//! BingX Perpetual Swap V2 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! Binance와 동일 크레이트 내 형제 모듈. 2026년 BingX가 TradFi perp 웨이브에 맞춰
//! 상장한 한국 대형주·지수 무기한 선물(USDT-M)을 대상으로 한다. BingX의 TradFi
//! 심볼 표기는 단일종목 `NCSK…2USD`, 지수 `NCSI…2USD` 규칙을 쓴다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·**KOSPI 지수([`KOSPI200`])**.
//! **현대차는 미상장.** (현물 ETF 프록시 `NCSIEWY2USD-USDT`는 별도 — KR 종목 매핑
//! 대상이 아니다.)
//!
//! 설계는 Binance를 미러링한다 — 형제 모듈, 독립 에러([`BingxError`]), 액세서 패턴.
//! 인증은 Binance식 HMAC-SHA256: 전체 파라미터(`timestamp` 포함)를 삽입순
//! queryString으로 직렬화하고 `signature=HEX(HMAC_SHA256(secret, queryString))`을
//! 덧붙이며 `X-BX-APIKEY` 헤더를 보낸다. 차이는 응답이 `{code,msg,data}` envelope로
//! 한 겹 감싸진다는 점(`code` 정수, 성공=0) — [`client`]에서 `data`를 풀어준다.
//! 시세(`market`)는 키 없이, 거래(`trade`)는 서명으로 호출한다.
//!
//! ```ignore
//! use korea_stock::global::bingx::{BingxClient, BingxConfig, SAMSUNG};
//! use korea_stock::global::bingx::trade::{OrderRequest, Side};
//!
//! // 시세 — 키 불필요.
//! let pub_client = BingxClient::new(BingxConfig::public())?;
//! let idx = pub_client.market().premium_index(SAMSUNG).await?;
//! println!("mark={} funding={}", idx.mark_price, idx.last_funding_rate);
//!
//! // 거래 — 운영 키로 (테스트넷 미제공).
//! let client = BingxClient::new(BingxConfig::from_env()?)?;
//! let order = OrderRequest::limit(SAMSUNG, Side::Buy, "0.01", "230.00");
//! let resp = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{BingxClient, RawRequest};
pub use config::{BingxConfig, DEFAULT_BASE_URL};
pub use error::{BingxError, Result as BingxResult};

/// 삼성전자 무기한 선물 심볼 (BingX `NCSK…2USD` 단일종목 표기).
pub const SAMSUNG: &str = "NCSKSAMSUNG2USD-USDT";
/// SK하이닉스 무기한 선물 심볼.
pub const SK_HYNIX: &str = "NCSKSKHYNIX2USD-USDT";
/// KOSPI 200 지수 무기한 선물 심볼 (BingX `NCSI…2USD` 지수 표기, displayName
/// `KR200-USDT`). BingX는 KR 지수 perp를 상장한 몇 안 되는 venue다.
pub const KOSPI200: &str = "NCSIKOSPI2USD-USDT";

/// BingX가 상장한 한국 주식·지수 무기한 선물 심볼 전체.
///
/// 출처: live API `GET /openApi/swap/v2/quote/contracts` (2026-06-04 검증).
/// **현대차는 BingX에 미상장**이라 목록에 없다. `market().contracts()` 결과를
/// 이 목록으로 필터하면 KR 종목만 추린다.
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, KOSPI200];

/// [`crate::KrStock`] → BingX 심볼. 미상장 종목은 `None`.
///
/// **주의:** BingX는 다른 venue와 달리 KOSPI 지수를 상장하므로 `Kospi200 => Some`이고,
/// 반대로 현대차는 미상장이라 `HyundaiMotor => None`이다.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => return None, // BingX는 현대차 perp 미상장
        Kospi200 => KOSPI200,        // BingX는 KOSPI 지수 perp 상장 (이 venue만의 예외)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::KrStock;

    #[test]
    fn kr_symbols_cover_listed_stocks() {
        assert_eq!(KR_SYMBOLS.len(), 3);
        assert!(KR_SYMBOLS.contains(&SAMSUNG));
        assert!(KR_SYMBOLS.contains(&SK_HYNIX));
        assert!(KR_SYMBOLS.contains(&KOSPI200));
    }

    #[test]
    fn symbol_mapping_hyundai_absent_kospi_present() {
        // BingX 고유: 현대차 미상장(None), KOSPI 지수 상장(Some).
        assert_eq!(symbol(KrStock::SamsungElec), Some(SAMSUNG));
        assert_eq!(symbol(KrStock::SkHynix), Some(SK_HYNIX));
        assert_eq!(symbol(KrStock::HyundaiMotor), None);
        assert_eq!(symbol(KrStock::Kospi200), Some(KOSPI200));
    }

    #[test]
    fn naming_convention_single_stock_vs_index() {
        // 단일종목은 NCSK 접두, 지수는 NCSI 접두.
        assert!(SAMSUNG.starts_with("NCSK"));
        assert!(SK_HYNIX.starts_with("NCSK"));
        assert!(KOSPI200.starts_with("NCSI"));
    }
}
