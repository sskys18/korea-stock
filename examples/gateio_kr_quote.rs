//! Gate.io APIv4 USDT Futures 한국주식 무기한선물 시세 예제 (키 불필요).
//! 실행: `cargo run --example gateio_kr_quote`
//!
//! 삼성전자·SK하이닉스·현대차 무기한선물의 mark/funding/bid/ask를 라이브
//! 공개 API에서 조회한다. Gate `tickers`는 mark·funding·최우선호가를 한 호출에
//! 담아 4개 필드를 모두 충족한다. order_book으로 호가1단을 교차 확인한다.

use korea_stock::global::gateio::{GateioClient, GateioConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = GateioClient::new(GateioConfig::public())?;
    let md = client.market();

    for symbol in KR_SYMBOLS {
        let t = md.ticker(symbol).await?;
        let book = md.order_book(symbol, Some(5)).await?;
        let c = md.contract(symbol).await?;
        println!(
            "[{symbol}] last={} mark={} index={} funding={} (cycle {}s, qmult {})",
            t.last, t.mark_price, t.index_price, t.funding_rate, c.funding_interval, c.quanto_multiplier
        );
        println!(
            "  ticker bid/ask {} / {}   book1 bid/ask {} / {}   status={}",
            t.highest_bid,
            t.lowest_ask,
            book.bids.first().map(|e| e.p.as_str()).unwrap_or("-"),
            book.asks.first().map(|e| e.p.as_str()).unwrap_or("-"),
            c.status,
        );
    }

    Ok(())
}
