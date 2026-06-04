//! Phemex Perpetual v2 한국주식 무기한선물 시세 예제 (키 불필요).
//! 실행: `cargo run --example phemex_kr_quote`
//!
//! 삼성전자·SK하이닉스·현대차 무기한선물의 최근가·마크가·펀딩비·1호가를 조회한다.
//! Phemex v2 perp은 평문 문자열 가격(`Rp`/`Rr` real value)이라 스케일 변환이 없다.

use korea_stock::global::phemex::{PhemexClient, PhemexConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = PhemexClient::new(PhemexConfig::public())?;
    let md = client.market();

    // 상품 메타: priceScale/tickSize/qtyStepSize (스케일 변환 여부 증명).
    let products = md.contracts().await?;
    for symbol in KR_SYMBOLS {
        if let Some(p) = products.iter().find(|p| p.symbol == symbol) {
            println!(
                "[{symbol}] type={} status={} priceScale={} tick={} qtyStep={}",
                p.product_type, p.status, p.price_scale, p.tick_size, p.qty_step_size
            );
        }
    }
    println!();

    for symbol in KR_SYMBOLS {
        let t = md.ticker(symbol).await?;
        // 1호가는 ticker에 포함되나, 호가창 깊이도 한 번 확인(엔드포인트 동작 증명).
        let ob = md.orderbook(symbol).await?;
        println!(
            "[{symbol}] last={} mark={} index={} funding={} predFunding={}",
            t.last(),
            t.mark_rp,
            t.index_rp,
            t.funding_rate_rr,
            t.pred_funding_rate_rr,
        );
        println!(
            "  ticker bid/ask = {} / {}   book bid/ask = {} / {}",
            t.bid_rp,
            t.ask_rp,
            ob.best_bid().unwrap_or("-"),
            ob.best_ask().unwrap_or("-"),
        );
    }

    Ok(())
}
