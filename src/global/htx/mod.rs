//! HTX(Huobi) USDT-M Linear Swap 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! KIS/Toss/Binance/MEXC 어댑터와 동일 크레이트 내 형제 모듈(공유 트레이트 없음).
//! HTX가 상장한 한국 대형주 무기한 선물(USDT 마진, linear swap)을 대상으로 한다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]) — 세 종목 모두
//! 라이브 `swap_contract_info`로 확인됨(2026-06-04, `contract_status==1` 거래중).
//!
//! 설계는 Binance 어댑터를 미러링하되 **인증이 구조적으로 다르다**:
//!  1. 서명은 HMAC-SHA256이되 **Base64**(hex 아님)이며, prehash가
//!     `METHOD\nHOST\nPATH\ncanonical_query`(GET-스타일 정규화 문자열)다 —
//!     **POST 주문도 본문이 아니라 이 정규화 문자열을 서명**한다.
//!  2. 서명 파라미터(`AccessKeyId`/`SignatureMethod`/`SignatureVersion`/`Timestamp`/
//!     `Signature`)는 **쿼리 스트링**으로 가고, JSON 본문은 서명하지 않는다.
//!  3. `Timestamp`는 UTC `YYYY-MM-DDTHH:MM:SS`이며 콜론이 `%3A`로 url-encode된다.
//!  4. 에러를 HTTP status가 아니라 **본문 `status`/`err_code`**로 판정한다(KIS·MEXC 방식).
//!
//! 시세(`market`)는 키 없이, 거래(`trade`)는 서명으로 호출한다.
//!
//! **서명 검증 상태:** HTX 공식 문서는 access key·secret을 마스킹(`e2xxxxxx…`)해
//! 검증 가능한 숫자 테스트 벡터를 공개하지 않는다. 따라서 본 어댑터의 prehash+HMAC를
//! **독립 도구(openssl)로 사전계산한 Base64 HMAC-SHA256과 대조**하는 self-consistency
//! 테스트로 고정했다(`client::tests::sign_matches_openssl_cross_tool_vector`). 라이브
//! 키 end-to-end 주문은 **미검증**(키 미보유).
//!
//! ```ignore
//! use korea_stock::global::htx::{HtxClient, HtxConfig, SAMSUNG};
//! use korea_stock::global::htx::trade::{OrderRequest, Direction, Offset};
//!
//! // 시세 — 키 불필요.
//! let pub_client = HtxClient::new(HtxConfig::public())?;
//! let d = pub_client.market().detail_merged(SAMSUNG).await?;
//! let f = pub_client.market().funding_rate(SAMSUNG).await?;
//! println!("last={} funding={} ask={} bid={}", d.close, f.funding_rate, d.ask_price(), d.bid_price());
//!
//! // 거래 — 키 필요(서명). 검증은 최소 수량·먼 지정가로.
//! let client = HtxClient::new(HtxConfig::from_env()?)?;
//! let order = OrderRequest::limit(SAMSUNG, Direction::Buy, Offset::Open, 1, 5, "200.00");
//! let resp = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{HtxClient, RawRequest};
pub use config::{HtxConfig, DEFAULT_BASE_URL, DEFAULT_HOST};
pub use error::{HtxError, Result as HtxResult};

/// 삼성전자 무기한 선물 계약 코드.
///
/// **라이브 검증됨[High] (2026-06-04):** `GET /linear-swap-api/v1/swap_contract_info`의
/// `contract_code` 필드가 `SAMSUNG-USDT`이고 `contract_status==1`(거래중)임을 확인.
pub const SAMSUNG: &str = "SAMSUNG-USDT";
/// SK하이닉스 무기한 선물 계약 코드. 라이브 검증됨[High] (2026-06-04, `contract_status==1`).
pub const SK_HYNIX: &str = "SKHYNIX-USDT";
/// 현대차 무기한 선물 계약 코드. 라이브 검증됨[High] (2026-06-04, `contract_status==1`).
pub const HYUNDAI: &str = "HYUNDAI-USDT";

/// HTX가 상장한 한국 주식 무기한 선물 계약 코드.
///
/// 출처: 라이브 `GET /linear-swap-api/v1/swap_contract_info` (2026-06-04). 세 종목 모두
/// `contract_status==1`(거래중)으로 확인. HTX KR perp는 `{TICKER}-USDT` 표기를 쓴다.
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → HTX 계약 코드. 미상장 종목은 `None`.
///
/// HTX는 KR 대형주 3종(삼성전자·SK하이닉스·현대차)만 상장했고 KOSPI200 지수
/// perp는 없으므로 [`crate::KrStock::Kospi200`]은 `None`을 돌려준다.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => return None, // HTX는 KR 지수 perp 미상장
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kr_symbols_match_live_contract_info() {
        assert_eq!(SAMSUNG, "SAMSUNG-USDT");
        assert_eq!(SK_HYNIX, "SKHYNIX-USDT");
        assert_eq!(HYUNDAI, "HYUNDAI-USDT");
        assert_eq!(KR_SYMBOLS, [SAMSUNG, SK_HYNIX, HYUNDAI]);
        assert!(KR_SYMBOLS.iter().all(|s| s.ends_with("-USDT")));
    }

    #[test]
    fn symbol_maps_three_stocks_and_none_for_kospi() {
        use crate::KrStock;
        assert_eq!(symbol(KrStock::SamsungElec), Some(SAMSUNG));
        assert_eq!(symbol(KrStock::SkHynix), Some(SK_HYNIX));
        assert_eq!(symbol(KrStock::HyundaiMotor), Some(HYUNDAI));
        assert_eq!(symbol(KrStock::Kospi200), None);
    }
}
