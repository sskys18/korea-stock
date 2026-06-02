//! 토스증권 시세 조회 예제 (토큰만 필요 — 계좌 헤더 불필요).
//! 실행: TOSS_CLIENT_ID / TOSS_CLIENT_SECRET 환경변수 설정 후
//! `cargo run --example toss_quote`
//!
//! 국내(005930)·미국(AAPL) 종목의 현재가(prices)와 호가(orderbook)를 조회한다.

use korea_stock::{TossClient, TossConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = TossClient::new(TossConfig::from_env()?)?;
    let md = client.market_data();

    // 현재가 — 복수 심볼 일괄 조회 (KR + US).
    let prices = md.prices(&["005930", "AAPL"]).await?;
    for p in &prices {
        println!("현재가 {} = {} {}", p.symbol, p.last_price, p.currency);
    }

    // 호가 — 국내 종목.
    let kr = md.orderbook("005930").await?;
    println!(
        "[005930] 매도1호가 {} / 매수1호가 {} ({})",
        kr.asks.first().map(|e| e.price.as_str()).unwrap_or("-"),
        kr.bids.first().map(|e| e.price.as_str()).unwrap_or("-"),
        kr.currency,
    );

    // 호가 — 미국 종목.
    let us = md.orderbook("AAPL").await?;
    println!(
        "[AAPL] 매도1호가 {} / 매수1호가 {} ({})",
        us.asks.first().map(|e| e.price.as_str()).unwrap_or("-"),
        us.bids.first().map(|e| e.price.as_str()).unwrap_or("-"),
        us.currency,
    );

    Ok(())
}
