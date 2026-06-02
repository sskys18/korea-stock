//! 한국투자증권(KIS) + 토스증권(Toss) OpenAPI Rust 어댑터.
//!
//! 두 증권사 어댑터를 형제 모듈(`kis`, `toss`)로 제공하며, 브로커 비종속
//! 레이트리미터(`ratelimit`)를 공유한다.

pub mod kis;
pub mod toss;

mod ratelimit;

pub use kis::{
    Environment, Exchange, KisClient, KisConfig, KisError, KisResponse, Market, OrderNotice,
    OverseasTrade, RankBy, RawRequest, RealtimeClient, RealtimeEvent, Result, StockAsking,
    StockTrade, SubscriptionHandle, SubscriptionKind,
};
pub use toss::{RawRequest as TossRawRequest, TossClient, TossConfig, TossError, TossResponse};
