//! Binance USDM Futures 한국주식 무기한선물 주문 예제 (HMAC 서명 필요).
//! 실행: BINANCE_API_KEY / BINANCE_API_SECRET 설정 후
//! `cargo run --example binance_kr_order`
//!
//! **테스트넷 강제:** 실거래 사고 방지를 위해 이 예제는 testnet base_url을 쓴다.
//! testnet 키는 <https://testnet.binancefuture.com> 에서 발급한다.
//! 운영 전환은 `.testnet()` 호출을 제거하면 된다 — 그 즉시 실자금이 움직인다.

use korea_stock::binance::trade::{OrderRequest, Side};
use korea_stock::binance::{BinanceClient, BinanceConfig, SAMSUNG};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = BinanceClient::new(BinanceConfig::from_env()?.testnet())?;
    let trade = client.trade();

    // 잔고 확인.
    for b in trade.balances().await? {
        if b.balance != "0.00000000" {
            println!("잔고 {} = {} (가용 {})", b.asset, b.balance, b.available_balance);
        }
    }

    // 레버리지 설정 (예: 10x).
    let lev = trade.set_leverage(SAMSUNG, 10).await?;
    println!("레버리지 {} = {}x (max notional {})", lev.symbol, lev.leverage, lev.max_notional_value);

    // 지정가 매수 주문 — 마크가 대비 충분히 낮게 걸어 미체결 상태 확인.
    let order = OrderRequest::limit(SAMSUNG, Side::Buy, "1", "1.00").client_order_id("kr-demo-1");
    let placed = trade.place(&order).await?;
    println!(
        "주문 접수: id={} status={} {}@{}",
        placed.order_id, placed.status, placed.orig_qty, placed.price
    );

    // 미체결 조회 후 취소.
    let open = trade.open_orders(Some(SAMSUNG)).await?;
    println!("미체결 {}건", open.len());
    let canceled = trade.cancel(SAMSUNG, placed.order_id).await?;
    println!("취소 완료: id={} status={}", canceled.order_id, canceled.status);

    // 포지션 확인.
    for p in trade.positions(Some(SAMSUNG)).await? {
        println!("포지션 {} amt={} entry={} uPnL={}", p.symbol, p.position_amt, p.entry_price, p.unrealized_profit);
    }

    Ok(())
}
