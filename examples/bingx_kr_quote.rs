//! BingX Perpetual Swap V2 한국주식 무기한선물 시세 예제 (키 불필요).
//! 실행: `cargo run --example bingx_kr_quote`
//!
//! 삼성전자·SK하이닉스·**KOSPI 지수**의 last·mark·index·funding·bid/ask를
//! 라이브 공개 API에서 조회한다. (BingX는 현대차 미상장, KOSPI 지수 상장.)

use korea_stock::global::bingx::{BingxClient, BingxConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = BingxClient::new(BingxConfig::public())?;
    let md = client.market();

    for symbol in KR_SYMBOLS {
        let t = md.ticker(symbol).await?;
        let idx = md.premium_index(symbol).await?;
        let book = md.depth(symbol, Some(5)).await?;
        let kind = if symbol.starts_with("NCSI") {
            "index"
        } else {
            "stock"
        };
        println!(
            "[{symbol}] ({kind}) last={} mark={} index={} funding={} (cycle {}h)",
            t.last_price,
            idx.mark_price,
            idx.index_price,
            idx.last_funding_rate,
            idx.funding_interval_hours,
        );
        println!(
            "  ticker bid/ask {} / {}   depth1 bid/ask {} / {}",
            t.bid_price,
            t.ask_price,
            book.bids.first().map(|e| e[0].as_str()).unwrap_or("-"),
            book.asks.first().map(|e| e[0].as_str()).unwrap_or("-"),
        );
    }

    Ok(())
}
