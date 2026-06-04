//! Bybit v5 한국주식 무기한선물(USDT linear perp) 시세 예제 (키 불필요).
//! 실행: `cargo run --example bybit_kr_quote`
//!
//! 삼성전자·SK하이닉스·현대차 무기한선물의 마크가·펀딩비·최우선 호가를 조회한다.
//! 선형 티커(`/v5/market/tickers`) 한 호출이 네 값을 모두 담는다.

use korea_stock::global::bybit::{BybitClient, BybitConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = BybitClient::new(BybitConfig::public())?;
    let md = client.market();

    for symbol in KR_SYMBOLS {
        let t = md.ticker(symbol).await?;
        println!(
            "[{symbol}] mark={} index={} last={} funding={} 다음펀딩(ms)={}",
            t.mark_price, t.index_price, t.last_price, t.funding_rate, t.next_funding_time
        );
        println!(
            "  매도1 {} (x{}) / 매수1 {} (x{})",
            t.ask1_price, t.ask1_size, t.bid1_price, t.bid1_size,
        );
    }

    Ok(())
}
