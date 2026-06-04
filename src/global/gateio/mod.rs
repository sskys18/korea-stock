//! Gate.io APIv4 USDT Futures 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! KIS/Toss 및 다른 global venue와 동일 크레이트 내 형제 모듈. 2026년
//! Gate.io가 상장한 한국 대형주 무기한 선물(USDT settle)을 대상으로 한다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]).
//!
//! 설계는 binance를 미러링한다 — 형제 모듈, 독립 에러([`GateioError`]), 액세서 패턴.
//! 인증은 Gate APIv4 **HMAC-SHA512**다: `METHOD\nPATH\nQUERY\nHEX(SHA512(body))\nTS`
//! 서명 문자열을 시크릿으로 HMAC-SHA512 하여 `KEY`/`Timestamp`/`SIGN` 헤더로 보낸다.
//! 시세(`market`)는 키 없이, 거래(`trade`)는 서명으로 호출한다.
//!
//! **Gate 특유점:** 선물 수량은 *계약(contract) 수*다. base 자산 수량 =
//! `size × quanto_multiplier` (KR 종목은 0.01). `size`는 부호 있는 정수 —
//! 양수=롱, 음수=숏. 시장가는 `price="0"` + `tif="ioc"`.
//!
//! ```ignore
//! use korea_stock::global::gateio::{GateioClient, GateioConfig, SAMSUNG};
//! use korea_stock::global::gateio::trade::{OrderRequest, Side};
//!
//! // 시세 — 키 불필요.
//! let pub_client = GateioClient::new(GateioConfig::public())?;
//! let t = pub_client.market().ticker(SAMSUNG).await?;
//! println!("mark={} funding={}", t.mark_price, t.funding_rate);
//!
//! // 거래 — 키 필요.
//! let client = GateioClient::new(GateioConfig::from_env()?)?;
//! let order = OrderRequest::limit(SAMSUNG, Side::Buy, 3, "230.00");
//! let resp = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{GateioClient, RawRequest};
pub use config::{GateioConfig, API_PREFIX, DEFAULT_BASE_URL};
pub use error::{GateioError, Result as GateioResult};

/// 삼성전자 무기한 선물 심볼.
pub const SAMSUNG: &str = "SAMSUNG_USDT";
/// SK하이닉스 무기한 선물 심볼.
pub const SK_HYNIX: &str = "SKHYNIX_USDT";
/// 현대차 무기한 선물 심볼.
pub const HYUNDAI: &str = "HYUNDAI_USDT";

/// Gate.io가 상장한 한국 주식 무기한 선물 심볼 전체.
///
/// 출처: Gate APIv4 `GET /api/v4/futures/usdt/contracts` (live 2026-06-04).
/// `market().contracts()` 결과를 이 목록으로 필터하면 KR 종목만 추린다.
/// **검증 권장:** 운영 전 단일 contract 조회로 존재·`status=="trading"` 확인.
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → Gate.io 심볼. 미상장 종목/지수는 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => return None, // Gate는 KR 지수 perp 미상장
    })
}
