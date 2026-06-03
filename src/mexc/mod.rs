//! MEXC Contract(USDT 무기한 선물) 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! KIS/Toss/Binance 어댑터와 동일 크레이트 내 형제 모듈(공유 트레이트 없음).
//! MEXC가 상장한 한국 대형주 무기한 선물(USDT 마진, 24/7, 20x)을 대상으로 한다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]) — 세 종목 모두
//! 라이브 `contract/detail`로 확인됨(2026-06-04, `state==0` 거래중).
//!
//! 설계는 Binance 어댑터를 미러링하되 **세 지점이 구조적으로 다르다**:
//!  1. 인증을 query가 아니라 **헤더**(`ApiKey`/`Request-Time`/`Signature`)로 보낸다.
//!  2. 서명 대상 문자열이 `accessKey + reqTime + paramString`이다(Binance는 query만 서명).
//!     GET/DELETE는 파라미터를 **키 사전순 정렬** 후 `k=v&k=v`, POST는 **JSON 본문 원문**.
//!  3. 에러를 HTTP status가 아니라 **본문 `success`/`code`**로 판정한다(KIS 방식).
//!
//! 시세(`market`)는 키 없이, 거래(`trade`)는 서명으로 호출한다.
//!
//! **거래 제약(중요):** MEXC는 2022-07-25부로 대부분 계정에서 Contract **신규 주문
//! 제출/취소 엔드포인트를 점검(중단)** 상태로 두었다. 조회(주문/포지션/자산)는 동작한다.
//! 본 어댑터는 HMAC 서명을 정상 구현(doc-vector 테스트 포함)하고 주문 제출도 **실제 서명
//! 호출**로 구현하므로, 서버가 막혀 있으면 MEXC의 점검 에러가 그대로 반환된다.
//!
//! ```ignore
//! use korea_stock::mexc::{MexcClient, MexcConfig, SAMSUNG};
//! use korea_stock::mexc::trade::{OrderRequest, Side, OpenType};
//!
//! // 시세 — 키 불필요.
//! let pub_client = MexcClient::new(MexcConfig::public())?;
//! let t = pub_client.market().ticker(SAMSUNG).await?;
//! println!("last={} funding={}", t.last_price, t.funding_rate);
//!
//! // 거래(조회) — 키 필요. 주문 제출은 MEXC 점검으로 막혀 있을 수 있다.
//! let client = MexcClient::new(MexcConfig::from_env()?)?;
//! let assets = client.trade().assets().await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{MexcClient, RawRequest};
pub use config::{MexcConfig, DEFAULT_BASE_URL};
pub use error::{MexcError, Result as MexcResult};

/// 삼성전자 무기한 선물 심볼.
///
/// **라이브 검증됨[High] (2026-06-04):** `GET /api/v1/contract/detail`의 `symbol`
/// 필드가 `SAMSUNGSTOCK_USDT`이고 `state==0`(거래중)임을 확인. `ticker?symbol=...`로
/// 시세 정상 응답(`success=true`)도 확인. 주의 — `displayNameEn`은 `SAMSUNG_USDT`로
/// 표기되나 이는 *표시명*이며, API가 요구하는 실제 심볼 문자열은 baseCoin에 `STOCK`이
/// 붙은 `SAMSUNGSTOCK_USDT`다(이전 추정치 `SAMSUNG_USDT`는 표시명을 심볼로 오인한 것).
pub const SAMSUNG: &str = "SAMSUNGSTOCK_USDT";
/// SK하이닉스 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04, `state==0`).
/// (`displayNameEn`=`SKHYNIX_USDT`이나 실제 심볼은 [`SAMSUNG`]과 동일 규칙.)
pub const SK_HYNIX: &str = "SKHYNIXSTOCK_USDT";
/// 현대차 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04, `state==0`).
/// 이전 세션에서 "상장 미확인"으로 제외했으나, `contract/detail`에 `HYUNDAISTOCK_USDT`로
/// 실재하며 거래중임을 확인했다(`displayNameEn`=`HYUNDAI_USDT`).
pub const HYUNDAI: &str = "HYUNDAISTOCK_USDT";

/// MEXC가 상장한 한국 주식 무기한 선물 심볼.
///
/// 출처: 라이브 `GET /api/v1/contract/detail` (2026-06-04). 세 종목 모두 `state==0`
/// (거래중)으로 확인. MEXC의 미국·한국 주식 perp는 baseCoin에 `STOCK` 접미가 붙는
/// 표기를 쓴다(예: `AAPLSTOCK_USDT`, `SAMSUNGSTOCK_USDT`).
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

#[cfg(test)]
mod tests {
    use super::*;

    /// 라이브 `GET /api/v1/contract/detail` (2026-06-04)에서 확정한 `symbol` 문자열.
    /// 세 종목 모두 `state==0`(거래중)이며 `ticker?symbol=...`로 응답 확인됨.
    #[test]
    fn kr_symbols_match_live_contract_detail() {
        assert_eq!(SAMSUNG, "SAMSUNGSTOCK_USDT");
        assert_eq!(SK_HYNIX, "SKHYNIXSTOCK_USDT");
        assert_eq!(HYUNDAI, "HYUNDAISTOCK_USDT");
        assert_eq!(KR_SYMBOLS, [SAMSUNG, SK_HYNIX, HYUNDAI]);
        // 모든 KR 심볼은 `STOCK_USDT`로 끝난다(MEXC 주식 perp 표기 규칙).
        assert!(KR_SYMBOLS.iter().all(|s| s.ends_with("STOCK_USDT")));
    }
}
