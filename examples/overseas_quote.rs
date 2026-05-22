//! 해외주식 현재가 + 기간시세 예제.
//! 실행: KIS_* 환경변수 설정 후
//! `cargo run --example overseas_quote -- AAPL`

use kis_adapter::overseas_stock::{OverseasExchange, OverseasPeriod};
use kis_adapter::{KisClient, KisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let symbol = std::env::args().nth(1).unwrap_or_else(|| "AAPL".into());

    let client = KisClient::new(KisConfig::from_env()?)?;
    let os = client.overseas_stock();

    let price = os.current_price(OverseasExchange::Nasd, &symbol).await?;
    println!("{symbol} 현재가: {} ({})", price.last, price.rate);

    let page = os
        .period_price(
            OverseasExchange::Nasd,
            &symbol,
            OverseasPeriod::Daily,
            "",
            true,
        )
        .await?;
    let (summary, candles) = &page.data;
    println!(
        "종목 {} — 일봉 {}건 (다음페이지: {})",
        summary.rsym,
        candles.len(),
        page.has_next()
    );
    for c in candles.iter().take(5) {
        println!("  {} 종가 {} 거래량 {}", c.xymd, c.clos, c.tvol);
    }

    Ok(())
}
