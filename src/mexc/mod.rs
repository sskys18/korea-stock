//! MEXC Contract(USDT 무기한 선물) 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! KIS/Toss/Binance 어댑터와 동일 크레이트 내 형제 모듈(공유 트레이트 없음).
//! MEXC가 상장한 한국 대형주 무기한 선물(USDT 마진, 24/7, 20x)을 대상으로 한다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`]). (현대차는 본 세션 기준 MEXC
//! Contract 상장 미확인 — [`HYUNDAI`]는 동일 표기 규칙으로 예비 정의만 둔다.)
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
/// MEXC Contract 표준 표기는 언더스코어형(예: `BTC_USDT`)이며, 본 KR 종목도
/// 동일 규칙을 따른다. **검증 권장[Medium]:** 본 세션에서 `contract/detail`로
/// 정확한 문자열을 확정하지 못했다. 운영 전 `market().contracts()`(=`/contract/detail`)
/// 결과에서 `state==0`(거래중) 및 정확한 `symbol` 문자열을 반드시 확인할 것.
pub const SAMSUNG: &str = "SAMSUNG_USDT";
/// SK하이닉스 무기한 선물 심볼. (검증 주의사항은 [`SAMSUNG`] 참고.)
pub const SK_HYNIX: &str = "SKHYNIX_USDT";
/// 현대차 무기한 선물 심볼 (예비 — MEXC Contract 상장 미확인). 검증 후 사용.
pub const HYUNDAI: &str = "HYUNDAI_USDT";

/// MEXC가 상장한 한국 주식 무기한 선물 심볼.
///
/// 출처: MEXC 공지(Samsung/SK Hynix Futures 상장). **운영 전 검증 필수** —
/// 정확한 심볼 문자열은 `market().contracts()`로 대조한다. [`HYUNDAI`]는 미확인이라
/// 본 배열에서 제외한다.
pub const KR_SYMBOLS: [&str; 2] = [SAMSUNG, SK_HYNIX];
