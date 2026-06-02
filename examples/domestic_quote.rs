//! 현재가 조회 예제. 실행: KIS_* 환경변수 설정 후
//! `cargo run --example domestic_quote -- 005930`

use kis_adapter::{KisClient, KisConfig, Market};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let code = std::env::args().nth(1).unwrap_or_else(|| "005930".into());

    let client = KisClient::new(KisConfig::from_env()?)?;
    let ds = client.domestic_stock();

    let price = ds.current_price(&code, Market::Krx).await?;
    println!("종목 {code} 현재가: {}", price.stck_prpr);
    println!("전일대비: {}", price.prdy_vrss);

    let (asking, expected) = ds.asking_price(&code, Market::Krx).await?;
    println!("매도1호가: {} / 매수1호가: {}", asking.askp1, asking.bidp1);
    println!("예상체결가: {}", expected.antc_cnpr);

    Ok(())
}
