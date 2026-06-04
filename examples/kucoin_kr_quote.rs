//! KuCoin Futures 한국주식 무기한선물(USDT-마진 perp) 시세 예제 (키 불필요).
//! 실행: `cargo run --example kucoin_kr_quote`
//!
//! 삼성전자·SK하이닉스·현대차 무기한선물의 마크가·지수가·펀딩비(계약 상세)와
//! 최우선 매수/매도 호가(티커)를 조회한다. KuCoin은 마크/펀딩을 `/contracts/{symbol}`에,
//! 최우선 호가를 `/ticker`에 담으므로 두 호출을 합쳐 한 줄로 출력한다.

use korea_stock::global::kucoin::{KucoinClient, KucoinConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = KucoinClient::new(KucoinConfig::public())?;
    let md = client.market();

    for symbol in KR_SYMBOLS {
        let c = md.contract(symbol).await?;
        let t = md.ticker(symbol).await?;
        println!(
            "[{symbol}] status={} mark={} index={} funding={} tick={} mult={}",
            c.status, c.mark_price, c.index_price, c.funding_fee_rate, c.tick_size, c.multiplier
        );
        println!(
            "  매도1 {} (x{}) / 매수1 {} (x{}) last={}",
            t.best_ask_price, t.best_ask_size, t.best_bid_price, t.best_bid_size, t.price
        );
    }

    Ok(())
}
