//! HTX(Huobi) USDT-M Linear Swap 한국주식 무기한선물 시세 예제 (키 불필요).
//! 실행: `cargo run --example htx_kr_quote`
//!
//! 삼성전자·SK하이닉스·현대차 무기한선물의 last/mark·펀딩비·호가(bid/ask)를 조회한다.

use korea_stock::global::htx::{HtxClient, HtxConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = HtxClient::new(HtxConfig::public())?;
    let md = client.market();

    for symbol in KR_SYMBOLS {
        let detail = md.detail_merged(symbol).await?;
        let funding = md.funding_rate(symbol).await?;
        // HTX는 별도 mark price를 merged tick에 싣지 않는다(last=close가 기준가).
        println!(
            "[{symbol}] last={} (mark≈last) funding={} ts={}",
            detail.close, funding.funding_rate, detail.ts
        );
        println!(
            "  ask1 {} / bid1 {}  (24h high={} low={} vol={})",
            detail.ask_price(),
            detail.bid_price(),
            detail.high,
            detail.low,
            detail.vol,
        );
    }

    Ok(())
}
