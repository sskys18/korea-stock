//! Bitunix Futures(USDT 무기한 선물) 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! KIS/Toss/Binance/MEXC 어댑터와 동일 크레이트 내 형제 모듈(공유 트레이트 없음).
//! Bitunix가 상장한 한국 대형주 무기한 선물(USDT 마진, 8h 펀딩)을 대상으로 한다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]).
//!
//! 설계는 MEXC 어댑터를 미러링한다 — **헤더 기반 인증** + **본문 `code`로 성공 판정**
//! (HTTP status 아님). 단 서명 방식이 다르다:
//!  - MEXC: HMAC-SHA256(`accessKey+reqTime+paramString`).
//!  - Bitunix: **순수 이중 SHA256**(HMAC 아님). `digest = SHA256(nonce+timestamp+
//!    apiKey+queryParams+body)` → `sign = SHA256(digest_hex + secret)`. 둘 다
//!    소문자 hex. `queryParams`는 키 ASCII 오름차순 정렬 후 **구분자 없이** k·v를
//!    이어붙인다(`{id:1,uid:200}` → `"id1uid200"`). POST면 queryParams="" + body=JSON.
//!
//! 시세(`market`)는 키 없이, 거래(`trade`)는 서명 헤더(`api-key`/`nonce`/`timestamp`/
//! `sign`)로 호출한다.
//!
//! **심볼 상태 주의[High] (2026-06-04):** 세 KR 심볼은 `trading_pairs`에 존재하나
//! `symbolStatus=="PREVIEW"`(상장 예고, 거래 미개시)다. 따라서 `/tickers`에 나타나지
//! 않고 `/depth` 호가창은 비어 있다. 마크가·펀딩은 `/funding_rate`만 라이브 응답한다.
//!
//! **거래 검증 상태:** 서명 알고리즘은 공식 문서(Go/Python 레퍼런스)로 검증된 이중
//! SHA256 구현이다. 다만 **어떤 주문도 라이브/테스트넷으로 실제 전송된 적 없다**
//! (키 미보유). 실자금 투입 전 사용자 키로 왕복 확인 필수.
//!
//! ```ignore
//! use korea_stock::global::bitunix::{BitunixClient, BitunixConfig, SAMSUNG};
//!
//! // 시세 — 키 불필요. (PREVIEW 심볼은 funding_rate로 마크가/펀딩 조회)
//! let pub_client = BitunixClient::new(BitunixConfig::public())?;
//! let fr = pub_client.market().funding_rate(SAMSUNG).await?;
//! println!("mark={} funding={}", fr.mark_price, fr.funding_rate);
//!
//! // 거래 — 키 필요. **라이브 미검증** — 실주문 전 testnet 왕복 확인.
//! let client = BitunixClient::new(BitunixConfig::from_env()?)?;
//! let acct = client.trade().account("USDT").await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{BitunixClient, RawRequest};
pub use config::{BitunixConfig, DEFAULT_BASE_URL};
pub use error::{BitunixError, Result as BitunixResult};

/// 삼성전자 무기한 선물 심볼.
///
/// **라이브 검증됨[High] (2026-06-04):** `GET /api/v1/futures/market/trading_pairs`에
/// `symbol=="SAMSUNGUSDT"`, `base=="SAMSUNG"`로 존재. 단 `symbolStatus=="PREVIEW"`
/// (상장 예고)라 `/tickers`·`/depth`엔 아직 데이터가 없고, `/funding_rate`만 마크가
/// (`markPrice`)를 반환한다.
pub const SAMSUNG: &str = "SAMSUNGUSDT";
/// SK하이닉스 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04, `symbolStatus=="PREVIEW"`).
pub const SK_HYNIX: &str = "SKHYNIXUSDT";
/// 현대차 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04, `symbolStatus=="PREVIEW"`).
pub const HYUNDAI: &str = "HYUNDAIUSDT";

/// Bitunix가 상장한 한국 주식 무기한 선물 심볼 전체.
///
/// 출처: 라이브 `GET /api/v1/futures/market/trading_pairs` (2026-06-04). 세 종목 모두
/// `symbolStatus=="PREVIEW"`(거래 미개시). 거래 개시 후엔 `symbolStatus=="OPEN"`으로
/// 바뀌며 `/tickers`·`/depth`에 데이터가 채워진다. **운영 전 상태 재확인 권장.**
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → Bitunix 심볼. 미상장 종목은 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => return None, // Bitunix는 KR 지수 perp 미상장
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 라이브 `GET /api/v1/futures/market/trading_pairs` (2026-06-04)에서 확정한 심볼.
    #[test]
    fn kr_symbols_match_live_trading_pairs() {
        assert_eq!(SAMSUNG, "SAMSUNGUSDT");
        assert_eq!(SK_HYNIX, "SKHYNIXUSDT");
        assert_eq!(HYUNDAI, "HYUNDAIUSDT");
        assert_eq!(KR_SYMBOLS, [SAMSUNG, SK_HYNIX, HYUNDAI]);
        // 모든 KR 심볼은 USDT 마진(`USDT`로 끝남).
        assert!(KR_SYMBOLS.iter().all(|s| s.ends_with("USDT")));
    }
}
