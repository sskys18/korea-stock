//! 한국투자증권(KIS) OpenAPI 어댑터.
//!
//! 국내주식·해외주식·국내선물옵션 거래/조회 + 실시간 WebSocket 시세를 제공한다.
//! 토스증권 어댑터(`crate::domestic::toss`)와 동일 크레이트 내 형제 모듈로, 공통
//! 레이트리미터(`crate::ratelimit`)를 공유한다. 설계 근거는
//! `docs/specs/2026-05-22-kis-adapter-design.md` 참조.
//!
//! ```ignore
//! use korea_stock::{KisClient, KisConfig};
//!
//! let client = KisClient::new(KisConfig::from_env()?)?;
//! let price = client.domestic_stock().current_price("005930", Default::default()).await?;
//! ```

mod auth;
mod client;
mod config;
mod error;
mod trid;

pub mod domestic_stock;
#[cfg(feature = "external")]
pub mod external;
pub mod futureoption;
pub mod overseas_stock;
pub mod realtime;

pub use client::{KisClient, KisResponse, RawRequest};
pub use config::{Environment, KisConfig};
pub use domestic_stock::{Exchange, Market, RankBy};
pub use error::{KisError, Result};
pub use realtime::{
    OrderNotice, OverseasTrade, RealtimeClient, RealtimeEvent, StockAsking, StockTrade,
    SubscriptionHandle, SubscriptionKind,
};
