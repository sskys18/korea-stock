//! Phemex Perpetual v2(USDT 마진 무기한 선물) 어댑터 — 한국 주식 무기한 선물(KR-stock perps).
//!
//! KIS/Toss/Binance/MEXC 어댑터와 동일 크레이트 내 형제 모듈(공유 트레이트 없음).
//! Phemex가 상장한 한국 대형주 무기한 선물(USDT 마진, `PerpetualV2`)을 대상으로 한다:
//! 삼성전자([`SAMSUNG`])·SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]) — 세 종목 모두
//! 라이브 `GET /public/products`(`perpProductsV2`)에서 `status=="Listed"`로 확인됨
//! (2026-06-04). `priceScale==0`이므로 Phemex 클래식 계약의 스케일 정수(Ep/Ev)가
//! **아니라** 평문 문자열 가격(`Rp`/`Rq`/`Rr` suffix = real value)을 쓴다.
//!
//! 설계는 Binance/MEXC 어댑터를 미러링하되 **인증이 구조적으로 다르다**:
//!  1. 서명을 헤더로 보낸다 — `x-phemex-access-token`(API key), `x-phemex-request-expiry`
//!     (epoch 초), `x-phemex-request-signature`.
//!  2. 서명 대상 문자열이 `URLPath + QueryString + Expiry + body`이다(구분자 없이 연결).
//!     GET/DELETE는 query(정렬·인코딩 없이 `?` 제외한 원문)를, POST/PUT은 JSON 본문
//!     원문을 붙인다.
//!  3. **HMAC 키는 `Base64::urlDecode(API Secret)`** — 시크릿을 base64url 디코드한
//!     바이트열을 키로 쓴다(Binance/MEXC는 시크릿 원문을 키로 사용).
//!
//! 도메인별 응답 envelope이 다르다 — 거래/계좌는 `{ code, data, msg }`(`code==0` 성공),
//! 시세는 `{ error, id, result }`. 둘 다 본문으로 성공/실패를 판정한다.
//!
//! 시세(`market`)는 키 없이, 거래(`trade`)는 서명으로 호출한다.
//!
//! ```ignore
//! use korea_stock::global::phemex::{PhemexClient, PhemexConfig, SAMSUNG};
//! use korea_stock::global::phemex::trade::{OrderRequest, Side, PosSide};
//!
//! // 시세 — 키 불필요.
//! let pub_client = PhemexClient::new(PhemexConfig::public())?;
//! let t = pub_client.market().ticker(SAMSUNG).await?;
//! println!("last={} mark={} funding={}", t.last_rp, t.mark_rp, t.funding_rate_rr);
//!
//! // 거래 — 테스트넷에서 검증 후 운영.
//! let client = PhemexClient::new(PhemexConfig::from_env()?.testnet())?;
//! let order = OrderRequest::limit(SAMSUNG, Side::Buy, PosSide::Long, "1", "235.0");
//! let resp = client.trade().place(&order).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{PhemexClient, RawRequest};
pub use config::{PhemexConfig, DEFAULT_BASE_URL, TESTNET_BASE_URL};
pub use error::{PhemexError, Result as PhemexResult};

/// 삼성전자 무기한 선물 심볼.
///
/// **라이브 검증됨[High] (2026-06-04):** `GET /public/products`의 `perpProductsV2`에
/// `symbol=="SAMSUNGUSDT"`, `type=="PerpetualV2"`, `status=="Listed"`,
/// `settleCurrency=="USDT"`, `priceScale==0`(평문 가격), `tickSize=="0.01"`,
/// `qtyStepSize=="0.01"`로 확인. `GET /md/v3/ticker/24hr?symbol=SAMSUNGUSDT`로 시세
/// 정상 응답도 확인.
pub const SAMSUNG: &str = "SAMSUNGUSDT";
/// SK하이닉스 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04, `status=="Listed"`).
pub const SK_HYNIX: &str = "SKHYNIXUSDT";
/// 현대차 무기한 선물 심볼. 라이브 검증됨[High] (2026-06-04, `status=="Listed"`).
pub const HYUNDAI: &str = "HYUNDAIUSDT";

/// Phemex가 상장한 한국 주식 무기한 선물 심볼 전체.
///
/// 출처: 라이브 `GET /public/products` → `perpProductsV2` (2026-06-04). 세 종목 모두
/// `status=="Listed"`. `market().contracts()` 결과를 이 목록으로 필터하면 KR 종목만 추린다.
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → Phemex 심볼. 미상장 종목은 `None`.
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => return None, // Phemex는 KR 지수 perp 미상장
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 라이브 `GET /public/products`(`perpProductsV2`, 2026-06-04)에서 확정한 심볼.
    #[test]
    fn kr_symbols_match_live_products() {
        assert_eq!(SAMSUNG, "SAMSUNGUSDT");
        assert_eq!(SK_HYNIX, "SKHYNIXUSDT");
        assert_eq!(HYUNDAI, "HYUNDAIUSDT");
        assert_eq!(KR_SYMBOLS, [SAMSUNG, SK_HYNIX, HYUNDAI]);
        // 모든 KR 심볼은 USDT-마진 perp이므로 `USDT`로 끝난다.
        assert!(KR_SYMBOLS.iter().all(|s| s.ends_with("USDT")));
    }
}
