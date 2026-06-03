//! 한국투자증권(KIS) + 토스증권(Toss) + Binance Futures OpenAPI Rust 어댑터.
//!
//! 증권사·거래소 어댑터를 형제 모듈로 제공하며(`kis`, `toss`, `binance`),
//! 브로커 비종속 레이트리미터(`ratelimit`)를 공유한다. `binance`는 2026-06-02
//! 상장된 한국 주식 무기한 선물(삼성전자·SK하이닉스·현대차)을 대상으로 한다.

pub mod binance;
pub mod hyperliquid;
pub mod kis;
pub mod lighter;
pub mod mexc;
pub mod toss;

mod ratelimit;

pub use kis::{
    Environment, Exchange, KisClient, KisConfig, KisError, KisResponse, Market, OrderNotice,
    OverseasTrade, RankBy, RawRequest, RealtimeClient, RealtimeEvent, Result, StockAsking,
    StockTrade, SubscriptionHandle, SubscriptionKind,
};
pub use toss::{RawRequest as TossRawRequest, TossClient, TossConfig, TossError, TossResponse};
pub use binance::{
    BinanceClient, BinanceConfig, BinanceError, RawRequest as BinanceRawRequest, KR_SYMBOLS,
};
pub use hyperliquid::{
    HyperliquidClient, HyperliquidConfig, HyperliquidError,
    RawRequest as HyperliquidRawRequest, KR_SYMBOLS as HYPERLIQUID_KR_SYMBOLS,
};
pub use mexc::{MexcClient, MexcConfig, MexcError, RawRequest as MexcRawRequest};
