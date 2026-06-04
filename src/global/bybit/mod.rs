//! Bybit v5 USDT 무기한 선물(linear perp) 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! KIS/Toss/Binance/MEXC 어댑터와 동일 크레이트 내 형제 모듈(공유 트레이트 없음).
//! Bybit가 상장한 한국 대형주 무기한 선물(USDT 마진, `category=linear`)을 대상으로 한다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]).
//!
//! 설계는 파일 구조(mod/config/error/client/market/trade)를 Binance 어댑터에서
//! 미러링하되, **서명·에러 메커니즘은 MEXC 어댑터에 가깝다**:
//!  1. 인증을 query가 아니라 **헤더**(`X-BAPI-API-KEY`/`X-BAPI-TIMESTAMP`/
//!     `X-BAPI-RECV-WINDOW`/`X-BAPI-SIGN`)로 보낸다.
//!  2. 서명 대상 문자열이 `timestamp + api_key + recv_window + (queryString|rawBody)`이다.
//!     GET은 query 문자열(정렬 불요·삽입순 유지), POST는 **JSON 본문 원문**을 서명한다.
//!  3. 에러를 HTTP status가 아니라 **본문 envelope `retCode`/`retMsg`**로 판정한다.
//!     `retCode==0`이 성공, 그 외는 [`BybitError::Api`].
//!
//! 시세(`market`)는 키 없이, 거래(`trade`)는 서명으로 호출한다. 선형 티커
//! (`/v5/market/tickers`) 한 호출이 마크가·지수가·펀딩비·최우선호가를 모두 담는다.
//!
//! ```ignore
//! use korea_stock::global::bybit::{BybitClient, BybitConfig, SAMSUNG};
//! use korea_stock::global::bybit::trade::{OrderRequest, Side};
//!
//! // 시세 — 키 불필요.
//! let pub_client = BybitClient::new(BybitConfig::public())?;
//! let t = pub_client.market().ticker(SAMSUNG).await?;
//! println!("mark={} funding={}", t.mark_price, t.funding_rate);
//!
//! // 거래 — 테스트넷에서 검증 후 운영.
//! let client = BybitClient::new(BybitConfig::from_env()?.testnet())?;
//! let order = OrderRequest::limit(SAMSUNG, Side::Buy, "3", "65.00");
//! let ack = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{BybitClient, RawRequest};
pub use config::{BybitConfig, DEFAULT_BASE_URL, TESTNET_BASE_URL};
pub use error::{BybitError, Result as BybitResult};

/// 삼성전자 무기한 선물 심볼. **라이브 검증됨[High] (2026-06-04):**
/// `GET /v5/market/tickers?category=linear&symbol=SAMSUNGUSDT`가 정상 응답
/// (`retCode==0`)함을 확인.
pub const SAMSUNG: &str = "SAMSUNGUSDT";
/// SK하이닉스 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04).
pub const SK_HYNIX: &str = "SKHYNIXUSDT";
/// 현대차 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04).
pub const HYUNDAI: &str = "HYUNDAIUSDT";

/// Bybit가 상장한 한국 주식 무기한 선물 심볼 전체 (USDT linear perp).
///
/// `market().instruments()` 결과를 이 목록으로 필터하면 KR 종목만 추린다.
/// **검증 권장:** 운영 전 `instruments-info`로 심볼 존재·`status=="Trading"` 확인.
/// Bybit는 KOSPI200 등 한국 지수 perp를 상장하지 않는다(개별 종목만).
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → Bybit 심볼. 미상장 종목은 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => return None, // Bybit는 KR 지수 perp 미상장
    })
}
