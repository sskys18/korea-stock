//! 한국투자증권(KIS) + 토스증권(Toss) 증권사 어댑터와, 한국 주식 무기한선물(perp)을
//! 상장한 암호화폐 거래소·DEX 어댑터를 제공하는 Rust 비동기 어댑터.
//!
//! venue는 두 갈래로 묶인다: 국내 증권사([`domestic`] — `kis`·`toss`)와 글로벌
//! perp 거래소·DEX([`global`]). global CEX는 `binance`·`bybit`·`bitget`·`kucoin`·
//! `gateio`·`bingx`·`mexc`·`htx`·`phemex`·`bitunix`·`toobit`·`weex`(12), DEX는
//! `hyperliquid`·`lighter`·`aster`(BNB)·`pacifica`(Solana)(4).
//! 공유 트레이트는 없다(서명·주문모델 상이). 브로커 비종속 레이트리미터(`ratelimit`)와
//! 종목 어휘([`KrStock`])만 공유한다 — 각 venue가 `global::<venue>::symbol(KrStock)`로
//! 자기 심볼에 매핑한다. perp venue는 2026년 상장된 삼성전자·SK하이닉스·현대차(일부는
//! KOSPI200 지수) 무기한선물을 대상으로 하며, 능력 매트릭스·검증 상태는
//! `docs/specs/2026-06-02-kr-perp-venues-design.md` 참조.

pub mod domestic;
pub mod global;
pub mod kr_stock;

mod ratelimit;

pub use kr_stock::KrStock;

pub use domestic::kis::{
    Environment, Exchange, KisClient, KisConfig, KisError, KisResponse, Market, OrderNotice,
    OverseasTrade, RankBy, RawRequest, RealtimeClient, RealtimeEvent, Result, StockAsking,
    StockTrade, SubscriptionHandle, SubscriptionKind,
};
pub use domestic::toss::{
    RawRequest as TossRawRequest, TossClient, TossConfig, TossError, TossResponse,
};
pub use global::binance::{
    BinanceClient, BinanceConfig, BinanceError, RawRequest as BinanceRawRequest, KR_SYMBOLS,
};
pub use global::hyperliquid::{
    HyperliquidClient, HyperliquidConfig, HyperliquidError,
    RawRequest as HyperliquidRawRequest, KR_SYMBOLS as HYPERLIQUID_KR_SYMBOLS,
};
pub use global::mexc::{MexcClient, MexcConfig, MexcError, RawRequest as MexcRawRequest};
pub use global::bybit::{
    BybitClient, BybitConfig, BybitError, RawRequest as BybitRawRequest,
    KR_SYMBOLS as BYBIT_KR_SYMBOLS,
};
pub use global::bitunix::{
    BitunixClient, BitunixConfig, BitunixError, RawRequest as BitunixRawRequest,
    KR_SYMBOLS as BITUNIX_KR_SYMBOLS,
};
pub use global::htx::{
    HtxClient, HtxConfig, HtxError, RawRequest as HtxRawRequest, KR_SYMBOLS as HTX_KR_SYMBOLS,
};
pub use global::phemex::{
    PhemexClient, PhemexConfig, PhemexError, RawRequest as PhemexRawRequest,
    KR_SYMBOLS as PHEMEX_KR_SYMBOLS,
};
pub use global::toobit::{
    RawRequest as ToobitRawRequest, ToobitClient, ToobitConfig, ToobitError,
    KR_SYMBOLS as TOOBIT_KR_SYMBOLS,
};
pub use global::weex::{
    RawRequest as WeexRawRequest, WeexClient, WeexConfig, WeexError,
    KR_SYMBOLS as WEEX_KR_SYMBOLS,
};
pub use global::bitget::{
    BitgetClient, BitgetConfig, BitgetError, RawRequest as BitgetRawRequest,
    KR_SYMBOLS as BITGET_KR_SYMBOLS,
};
pub use global::kucoin::{
    KucoinClient, KucoinConfig, KucoinError, RawRequest as KucoinRawRequest,
    KR_SYMBOLS as KUCOIN_KR_SYMBOLS,
};
pub use global::gateio::{
    GateioClient, GateioConfig, GateioError, RawRequest as GateioRawRequest,
    KR_SYMBOLS as GATEIO_KR_SYMBOLS,
};
pub use global::bingx::{
    BingxClient, BingxConfig, BingxError, RawRequest as BingxRawRequest,
    KR_SYMBOLS as BINGX_KR_SYMBOLS,
};
pub use global::aster::{
    AsterClient, AsterConfig, AsterError, RawRequest as AsterRawRequest,
    KR_SYMBOLS as ASTER_KR_SYMBOLS,
};
pub use global::pacifica::{
    PacificaClient, PacificaConfig, PacificaError, RawRequest as PacificaRawRequest,
    KR_SYMBOLS as PACIFICA_KR_SYMBOLS,
};
