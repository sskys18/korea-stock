//! 한국투자증권(KIS) + 토스증권(Toss) 증권사 어댑터와, 한국 주식 무기한선물(perp)을
//! 상장한 암호화폐 거래소·DEX 어댑터(Binance·Hyperliquid·Lighter·MEXC)를 제공하는
//! Rust 비동기 어댑터.
//!
//! venue는 두 갈래로 묶인다: 국내 증권사([`domestic`] — `kis`·`toss`)와 글로벌
//! perp 거래소·DEX([`global`] — `binance`·`hyperliquid`·`lighter`·`mexc` …).
//! 공유 트레이트는 없다(서명·주문모델 상이). 브로커 비종속 레이트리미터(`ratelimit`)와
//! 종목 어휘([`KrStock`])만 공유한다. perp venue는 2026년 상장된 삼성전자·SK하이닉스·
//! 현대차 무기한선물을 대상으로 하며, 능력 매트릭스·검증 상태는
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
