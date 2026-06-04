//! 한국투자증권(KIS) + 토스증권(Toss) 증권사 어댑터와, 한국 주식 무기한선물(perp)을
//! 상장한 암호화폐 거래소·DEX 어댑터(Binance·Hyperliquid·Lighter·MEXC)를 제공하는
//! Rust 비동기 어댑터.
//!
//! 모든 venue는 독립 형제 모듈이다(`kis`, `toss`, `binance`, `hyperliquid`,
//! `lighter`, `mexc`; 공유 트레이트 없음). 브로커 비종속 레이트리미터(`ratelimit`)를
//! 공유한다. perp venue 4종은 2026년 상장된 삼성전자·SK하이닉스·현대차 무기한선물을
//! 대상으로 하며, 능력 매트릭스·검증 상태는
//! `docs/specs/2026-06-02-kr-perp-venues-design.md` 참조.

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
