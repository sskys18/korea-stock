//! Lighter·MEXC 최우선 호가(top-of-book bid1/ask1) — 키 불필요.
//! `kr_perp_live_check`가 두 venue는 last/fair만 출력하므로, 실호가를 별도로 뽑는다.
//! 실행: `cargo run --example lighter_mexc_book`

use korea_stock::global::{lighter, mexc};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Lighter (mainnet.zklighter) ===");
    let lc = lighter::LighterClient::new(lighter::LighterConfig::public())?;
    let lm = lc.market();
    for s in lighter::KR_SYMBOLS {
        let id = lm.market_id(s).await?;
        let ob = lm.order_book_orders(id, 5).await?;
        let bid = ob.bids.first().map(|o| o.price.as_str()).unwrap_or("-");
        let ask = ob.asks.first().map(|o| o.price.as_str()).unwrap_or("-");
        println!("[{s}] (market_id={id}) bid1={bid} / ask1={ask}");
    }

    println!("\n=== MEXC (contract.mexc.com) ===");
    let mc = mexc::MexcClient::new(mexc::MexcConfig::public())?;
    let mm = mc.market();
    for s in mexc::KR_SYMBOLS {
        let d = mm.depth(s, Some(5)).await?;
        let bid = d.bids.first().map(|l| l.price.as_str()).unwrap_or("-");
        let ask = d.asks.first().map(|l| l.price.as_str()).unwrap_or("-");
        println!("[{s}] bid1={bid} / ask1={ask}");
    }
    Ok(())
}
