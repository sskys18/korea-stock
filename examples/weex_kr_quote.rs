//! WEEX Contract 한국주식 무기한선물 시세 예제 (키 불필요).
//! 실행: `cargo run --example weex_kr_quote`
//!
//! 삼성전자·SK하이닉스·현대차 무기한선물의 last·mark·index·funding·bid/ask를
//! 라이브 공개 API에서 조회한다.

use korea_stock::global::weex::{WeexClient, WeexConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = WeexClient::new(WeexConfig::public())?;
    let md = client.market();

    for symbol in KR_SYMBOLS {
        let t = md.ticker(symbol).await?;
        let f = md.funding_rate(symbol).await?;
        let book = md.depth(symbol, Some(15)).await?;
        println!(
            "[{symbol}] last={} mark={} index={} funding={} (cycle {}m)",
            t.last, t.mark_price, t.index_price, f.funding_rate, f.collect_cycle
        );
        println!(
            "  ticker bid/ask {} / {}   depth1 bid/ask {} / {}",
            t.best_bid,
            t.best_ask,
            book.bids.first().map(|e| e[0].as_str()).unwrap_or("-"),
            book.asks.first().map(|e| e[0].as_str()).unwrap_or("-"),
        );
    }

    Ok(())
}
