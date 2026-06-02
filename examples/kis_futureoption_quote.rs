//! 선물옵션 현재가 + 호가 예제.
//! 실행: `cargo run --example kis_futureoption_quote -- 101W09`

use korea_stock::kis::futureoption::MarketDiv;
use korea_stock::{KisClient, KisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let code = std::env::args().nth(1).unwrap_or_else(|| "101W09".into());

    let client = KisClient::new(KisConfig::from_env()?)?;
    let fo = client.futureoption();

    let (price, _raw) = fo.current_price(MarketDiv::IndexFuture, &code).await?;
    println!(
        "{} {} 현재가: {} ({})",
        code, price.hts_kor_isnm, price.futs_prpr, price.futs_prdy_ctrt
    );
    println!(
        "미결제약정: {}  델타: {}",
        price.hts_otst_stpl_qty, price.delta_val
    );

    let (asking, _raw) = fo.asking_price(MarketDiv::IndexFuture, &code).await?;
    println!(
        "매도1: {} / 매수1: {}",
        asking.futs_askp1, asking.futs_bidp1
    );

    Ok(())
}
