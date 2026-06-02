//! 한국투자증권(KIS) OpenAPI Rust 어댑터.

mod auth;
mod client;
mod config;
mod error;
mod ratelimit;
mod trid;

pub mod domestic_stock;
#[cfg(feature = "external")]
pub mod external;
pub mod futureoption;
pub mod overseas_stock;
pub mod realtime;

pub use client::{KisClient, KisResponse, RawRequest};
pub use config::{Environment, KisConfig};
pub use error::{KisError, Result};
pub use realtime::{
    OrderNotice, OverseasTrade, RealtimeClient, RealtimeEvent, StockAsking, StockTrade,
    SubscriptionHandle, SubscriptionKind,
};
