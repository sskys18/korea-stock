//! 한국 주식 무기한선물(perp)을 상장한 글로벌 암호화폐 거래소·DEX 어댑터.
//!
//! CEX: `binance`·`bybit`·`mexc`·`htx`·`phemex`·`bitunix`·`toobit`·`weex`.
//! DEX: `hyperliquid`·`lighter`.
//! 공유 트레이트 없음 — 종목 어휘([`crate::KrStock`])만 venue별 `symbol()`로 공유.

pub mod binance;
pub mod bitunix;
pub mod bybit;
pub mod htx;
pub mod hyperliquid;
pub mod lighter;
pub mod mexc;
pub mod phemex;
pub mod toobit;
pub mod weex;
