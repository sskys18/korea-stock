//! Aster perp DEX 한국주식 무기한선물 시세 예제 (키 불필요).
//! 실행: `cargo run --example aster_kr_quote`
//!
//! 삼성전자·SK하이닉스 무기한선물의 마크가·펀딩비·호가를 조회한다(현대차 미상장).
//! Aster fapi는 Binance USDM Futures 호환 표면이라 binance 예제와 동일한 흐름.

use korea_stock::global::aster::{AsterClient, AsterConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = AsterClient::new(AsterConfig::public())?;
    let md = client.market();

    for symbol in KR_SYMBOLS {
        let idx = md.premium_index(symbol).await?;
        let book = md.depth(symbol, Some(5)).await?;
        println!(
            "[{symbol}] mark={} index={} funding={} 다음펀딩(ms)={}",
            idx.mark_price, idx.index_price, idx.last_funding_rate, idx.next_funding_time
        );
        println!(
            "  매도1 {} / 매수1 {}",
            book.asks.first().map(|e| e[0].as_str()).unwrap_or("-"),
            book.bids.first().map(|e| e[0].as_str()).unwrap_or("-"),
        );
    }

    Ok(())
}
