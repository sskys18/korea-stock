//! Binance USDM Futures 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! KIS/Toss 어댑터와 동일 크레이트 내 형제 모듈. 2026-06-02 Binance Futures가
//! 상장한 한국 대형주 무기한 선물(USDT 마진, 20x, 8h 펀딩)을 대상으로 한다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]).
//!
//! 설계는 Toss를 미러링한다 — 형제 모듈, 독립 에러([`BinanceError`]), 액세서 패턴.
//! 차이는 인증이다: OAuth 토큰 대신 요청마다 HMAC-SHA256 서명(`api_secret`)을
//! 붙인다. 시세(`market`)는 키 없이, 거래(`trade`)는 서명으로 호출한다.
//!
//! ```ignore
//! use korea_stock::global::binance::{BinanceClient, BinanceConfig, SAMSUNG};
//! use korea_stock::global::binance::trade::{OrderRequest, Side};
//!
//! // 시세 — 키 불필요.
//! let pub_client = BinanceClient::new(BinanceConfig::public())?;
//! let idx = pub_client.market().premium_index(SAMSUNG).await?;
//! println!("mark={} funding={}", idx.mark_price, idx.last_funding_rate);
//!
//! // 거래 — 테스트넷에서 검증 후 운영.
//! let client = BinanceClient::new(BinanceConfig::from_env()?.testnet())?;
//! let order = OrderRequest::limit(SAMSUNG, Side::Buy, "3", "65.00");
//! let resp = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{BinanceClient, RawRequest};
pub use config::{BinanceConfig, DEFAULT_BASE_URL, TESTNET_BASE_URL};
pub use error::{BinanceError, Result as BinanceResult};

/// 삼성전자 무기한 선물 심볼.
pub const SAMSUNG: &str = "SAMSUNGUSDT";
/// SK하이닉스 무기한 선물 심볼.
pub const SK_HYNIX: &str = "SKHYNIXUSDT";
/// 현대차 무기한 선물 심볼.
pub const HYUNDAI: &str = "HYUNDAIUSDT";

/// Binance가 상장한 한국 주식 무기한 선물 심볼 전체.
///
/// 출처: 2026-06-02 Binance Futures 공지(USDT-margined TradFi perpetuals).
/// `market().markets()` 결과를 이 목록으로 필터하면 KR 종목만 추린다.
/// **검증 권장:** 운영 전 `exchangeInfo`로 심볼 존재·`status=="TRADING"` 확인.
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → Binance 심볼. 미상장 종목은 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => return None, // Binance는 KR 지수 perp 미상장
    })
}
