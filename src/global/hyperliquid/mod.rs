//! Hyperliquid(DEX) 어댑터 — 한국 주식 무기한 선물(KR-stock perps via HIP-3).
//!
//! KIS/Toss/Binance 어댑터와 동일 크레이트 내 형제 모듈. 독립 트레이트는 없다.
//! 대상은 Hyperliquid L1 위에 **Trade.xyz가 HIP-3로 배포한** 빌더 perp DEX
//! (`dex` 네임스페이스 `"xyz"`)의 한국 종목이다: 삼성전자([`SAMSUNG`])·
//! SK하이닉스([`SK_HYNIX`])·현대차([`HYUNDAI`]). 오라클은 KRX 원화가를
//! USD/KRW로 환산한 USD 가격이며, 24/7 거래·최대 10x·격리마진이다.
//!
//! 설계는 Binance를 미러링하되 두 축이 다르다:
//! 1. **전송:** 시세·거래 **모두 POST + JSON 바디**다(Binance의 GET+쿼리/HMAC와
//!    다름). 시세는 `POST /info`, 거래는 `POST /exchange`. 키 불필요한 `info`는
//!    [`market`]에서, 서명 필요한 `exchange`는 [`trade`]에서 호출한다.
//! 2. **서명:** HMAC이 아니라 **EIP-712 + secp256k1 ECDSA**다. action을 msgpack로
//!    직렬화→nonce/vault/expires 바이트 부착→keccak256 해시→`Agent` 타입드데이터
//!    서명. 자세한 명세는 [`crate::global::hyperliquid::client`]의 `sign_l1_action` 참조.
//!
//! ## HIP-3 자산 식별
//! HIP-3 빌더 perp는 coin을 `"{dex}:{ticker}"` 형식으로 참조한다(시세 호가/캔들).
//! 거래(action)에서는 **정수 asset id**를 쓰며, 빌더 dex 오프셋 공식은
//! `110000 + perp_dex_index*10000 + universe_index`다. Trade.xyz(`xyz`)는
//! `perpDexs()[1:]`의 첫 원소(i=0)라 오프셋 110000.
//! (출처: hyperliquid-python-sdk `info.py` `coin_to_asset` 빌드 루프.)
//!
//! ```ignore
//! use korea_stock::global::hyperliquid::{HyperliquidClient, HyperliquidConfig, SAMSUNG};
//! use korea_stock::global::hyperliquid::trade::{OrderRequest, Tif};
//!
//! // 시세 — 키 불필요.
//! let pub_client = HyperliquidClient::new(HyperliquidConfig::public())?;
//! let ctxs = pub_client.market().meta_and_asset_ctxs("xyz").await?;
//! let book = pub_client.market().l2_book(SAMSUNG).await?;
//!
//! // 거래 — 비밀키(개인 지갑/agent wallet) 필요. coin 이름으로 거는 place_by_coin이
//! // asset id를 라이브 meta에서 재도출해 wrong-instrument 사고를 막는다(운영 권장).
//! let client = HyperliquidClient::new(HyperliquidConfig::from_env()?)?;
//! let resp = client.trade().place_by_coin(DEX, SAMSUNG, true, "240.00", "1", Tif::Gtc).await?;
//! ```

mod client;
mod config;
mod error;

pub mod market;
pub mod trade;

pub use client::{HyperliquidClient, RawRequest};
pub use config::{HyperliquidConfig, DEFAULT_BASE_URL, TESTNET_BASE_URL};
pub use error::{HyperliquidError, Result as HyperliquidResult};

/// Trade.xyz가 HIP-3로 배포한 빌더 perp DEX 네임스페이스.
///
/// 시세 coin 식별자·`meta`/`metaAndAssetCtxs`의 `dex` 파라미터에 쓴다.
/// (출처: `POST /info {"type":"perpDexs"}` → 인덱스 1의 `name`.)
pub const DEX: &str = "xyz";

/// 삼성전자(KRX:005930) 무기한 선물 coin 식별자 — 시세 호가/캔들용.
///
/// 오라클: 삼성전자 보통주 원화가 × USD/KRW. (출처: trade.xyz 자산 디렉터리.)
pub const SAMSUNG: &str = "xyz:SMSN";
/// SK하이닉스(KRX:000660) 무기한 선물 coin 식별자.
pub const SK_HYNIX: &str = "xyz:SKHX";
/// 현대차(KRX:005380) 무기한 선물 coin 식별자.
pub const HYUNDAI: &str = "xyz:HYUNDAI";
/// KOSPI 200 지수 무기한 선물 coin 식별자.
pub const KOSPI200: &str = "xyz:KR200";

/// 삼성전자 거래용 정수 asset id (`110000 + universe_index 34`).
///
/// **검증 권장:** universe는 재정렬될 수 있다. 운영 전 `market().meta("xyz")`로
/// `SMSN`의 universe_index를 재확인하고 `110000 + idx`로 산출하라.
pub const SAMSUNG_ASSET: u64 = 110034;
/// SK하이닉스 거래용 정수 asset id (`110000 + universe_index 22`).
pub const SK_HYNIX_ASSET: u64 = 110022;
/// 현대차 거래용 정수 asset id (`110000 + universe_index 45`).
pub const HYUNDAI_ASSET: u64 = 110045;

/// Trade.xyz HIP-3에 상장된 한국 주식 무기한 선물 coin 식별자 전체.
///
/// 시세 조회용. 거래 시에는 [`SAMSUNG_ASSET`]/[`SK_HYNIX_ASSET`]/[`HYUNDAI_ASSET`]
/// 정수 id를 사용한다.
pub const KR_SYMBOLS: [&str; 3] = [SAMSUNG, SK_HYNIX, HYUNDAI];

/// [`crate::KrStock`] → Hyperliquid coin 식별자(`xyz:*`). 미상장 종목은 `None`.
///
/// 거래 시에는 coin 대신 [`SAMSUNG_ASSET`] 등 정수 asset id를 쓰거나
/// `market().asset_id("xyz", coin)`로 라이브 재도출하라(universe 재정렬 가드).
pub fn symbol(stock: crate::KrStock) -> Option<&'static str> {
    use crate::KrStock::*;
    Some(match stock {
        SamsungElec => SAMSUNG,
        SkHynix => SK_HYNIX,
        HyundaiMotor => HYUNDAI,
        Kospi200 => KOSPI200,
    })
}
