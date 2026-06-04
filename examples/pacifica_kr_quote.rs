//! Pacifica(Solana perp DEX) 한국주식 무기한선물 시세 예제 (키 불필요).
//! 실행: `cargo run --example pacifica_kr_quote`
//!
//! 삼성전자·SK하이닉스 무기한선물의 마크가·펀딩·최우선 호가를 라이브 공개 API에서 조회한다.
//! Pacifica는 현대차·KOSPI200을 상장하지 않는다(KR_SYMBOLS는 2종).

use korea_stock::global::pacifica::{PacificaClient, PacificaConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = PacificaClient::new(PacificaConfig::public())?;
    let md = client.market();

    for symbol in KR_SYMBOLS {
        let price = md.price(symbol).await?;
        let book = md.order_book(symbol).await?;
        let bid = book.bids.first();
        let ask = book.asks.first();
        println!(
            "[{symbol}] mark={} mid={} oracle={} funding={} next_funding={} oi={} vol24h={}",
            price.mark,
            price.mid,
            price.oracle,
            price.funding,
            price.next_funding,
            price.open_interest,
            price.volume_24h,
        );
        println!(
            "  bid1={} (x{})  ask1={} (x{})",
            bid.map(|l| l.price.as_str()).unwrap_or("-"),
            bid.map(|l| l.amount.as_str()).unwrap_or("-"),
            ask.map(|l| l.price.as_str()).unwrap_or("-"),
            ask.map(|l| l.amount.as_str()).unwrap_or("-"),
        );
    }

    Ok(())
}
