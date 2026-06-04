//! Bitget v2 USDT-M 무기한 선물(mix perp) 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! KIS/Toss/Binance/Bybit/MEXC 어댑터와 동일 크레이트 내 형제 모듈(공유 트레이트 없음).
//! Bitget이 상장한 한국 대형주 무기한 선물(USDT 마진, `productType=USDT-FUTURES`, 20x,
//! 8h 펀딩)을 대상으로 한다: 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차
//! ([`HYUNDAI`]).
//!
//! 파일 구조(mod/config/error/client/market/trade)는 Binance 어댑터에서 미러링하되,
//! **서명·에러 메커니즘은 Bybit/MEXC에 가깝다** — 단, Bitget 고유의 차이가 있다:
//!  1. 인증을 query가 아니라 **헤더**(`ACCESS-KEY`/`ACCESS-SIGN`/`ACCESS-TIMESTAMP`/
//!     `ACCESS-PASSPHRASE`)로 보내며, **passphrase**(키 발급 시 설정)를 추가로 요구한다.
//!  2. 서명 대상이 `timestamp + method.upper + requestPath + (?sortedQuery | rawBody)`,
//!     결과는 **Base64(HMAC-SHA256)**(Binance/Bybit의 hex와 다름). 서명 GET의 query는
//!     **키 사전순 정렬**, POST는 JSON 본문 원문을 서명한다.
//!  3. 에러를 HTTP status가 아니라 **본문 envelope `code`/`msg`**로 판정한다.
//!     `code=="00000"`이 성공, 그 외는 [`BitgetError::Api`].
//!  4. 호가창(merge-depth)은 가격/수량을 **JSON 숫자**로 내려준다(티커는 문자열) —
//!     [`market::Level`]이 숫자/문자열을 모두 받아 정밀도 보존 String으로 정규화한다.
//!
//! 시세(`market`)는 키 없이, 거래(`trade`)는 서명으로 호출한다. 티커
//! (`/api/v2/mix/market/ticker`) 한 호출이 마크가·지수가·펀딩비·최우선호가를 모두 담는다.
//!
//! ```ignore
//! use korea_stock::global::bitget::{BitgetClient, BitgetConfig, SAMSUNG};
//! use korea_stock::global::bitget::trade::{OrderRequest, Side};
//!
//! // 시세 — 키 불필요.
//! let pub_client = BitgetClient::new(BitgetConfig::public())?;
//! let t = pub_client.market().ticker(SAMSUNG).await?;
//! println!("mark={} funding={}", t.mark_price, t.funding_rate);
//!
//! // 거래 — passphrase 포함 키 필요. 운영 전 소액 검증.
//! let client = BitgetClient::new(BitgetConfig::from_env()?)?;
//! let order = OrderRequest::limit(SAMSUNG, Side::Buy, "0.1", "234.00");
//! let ack = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{BitgetClient, RawRequest};
pub use config::{BitgetConfig, DEFAULT_BASE_URL};
pub use error::{BitgetError, Result as BitgetResult};

/// 삼성전자 무기한 선물 심볼. **라이브 검증됨[High] (2026-06-04):**
/// `GET /api/v2/mix/market/ticker?symbol=SAMSUNGUSDT&productType=USDT-FUTURES`가
/// 정상 응답(`code=="00000"`)함을 확인.
pub const SAMSUNG: &str = "SAMSUNGUSDT";
/// SK하이닉스 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04).
pub const SK_HYNIX: &str = "SKHYNIXUSDT";
/// 현대차 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04).
pub const HYUNDAI: &str = "HYUNDAIUSDT";

/// Bitget이 상장한 한국 주식 무기한 선물 심볼 전체 (USDT-FUTURES mix perp).
///
/// `market().contracts()` 결과를 이 목록으로 필터하면 KR 종목만 추린다.
/// **검증 권장:** 운영 전 `contracts`로 심볼 존재·`symbolStatus=="normal"` 확인.
/// Bitget은 KOSPI200 등 한국 지수 perp를 상장하지 않는다(개별 종목만).
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → Bitget 심볼. 미상장 종목은 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => return None, // Bitget는 KR 지수 perp 미상장
    })
}
