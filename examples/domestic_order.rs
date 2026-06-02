//! 잔고 + 매수가능 조회 예제 (주문은 실행 안 함 — 안전).
//! 실행: `cargo run --example domestic_order`
//!
//! 실제 주문 호출에서 hashkey 관련 KIS 오류가 나면 `KisConfig.use_hashkey = true`로
//! 설정한 클라이언트로 재시도한다.

use kis_adapter::domestic_stock::{BalanceBasis, OrderType};
use kis_adapter::{KisClient, KisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let client = KisClient::new(KisConfig::from_env()?)?;
    let ds = client.domestic_stock();

    let (items, summary) = ds.balance_all(BalanceBasis::Default).await?;
    println!("보유종목 {}건", items.len());
    for it in &items {
        println!("  {} {} 보유 {}주", it.pdno, it.prdt_name, it.hldg_qty);
    }
    if let Some(s) = summary.first() {
        println!("총평가금액: {}", s.tot_evlu_amt);
    }

    let buyable = ds.buyable("005930", 70000, OrderType::Market).await?;
    println!("삼성전자 매수가능: {}주", buyable.nrcvb_buy_qty);

    Ok(())
}
