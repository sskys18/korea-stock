//! Toobit USDT-M Perp 한국주식 무기한선물 시세 예제 (키 불필요).
//! 실행: `cargo run --example toobit_kr_quote`
//!
//! 삼성전자·SK하이닉스·현대차 무기한 스왑의 최근가·마크가·펀딩비·호가를 조회한다.

use korea_stock::global::toobit::{ToobitClient, ToobitConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = ToobitClient::new(ToobitConfig::public())?;
    let md = client.market();

    for symbol in KR_SYMBOLS {
        let ticker = md.ticker_24hr(symbol).await?;
        let mark = md.mark_price(symbol).await?;
        let funding = md.funding_rate(symbol).await?;
        let book = md.depth(symbol, Some(5)).await?;
        println!(
            "[{symbol}] last={} mark={} funding={}(주기 {}) 다음펀딩(ms)={}",
            ticker.last_price, mark.price, funding.rate, funding.period, funding.next_funding_time
        );
        println!(
            "  매도1 {} / 매수1 {}",
            book.asks.first().map(|e| e[0].as_str()).unwrap_or("-"),
            book.bids.first().map(|e| e[0].as_str()).unwrap_or("-"),
        );
    }

    Ok(())
}
