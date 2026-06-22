//! 토스 주문 — 매수가능금액 부족(insufficient funds) 에러를 노출하는 probe.
//!
//! `toss_order` 예제의 60000원 지정가는 005930 하한가 미달이라 가격 검증 단계에서
//! `price-out-of-range`로 먼저 거부된다 — funds 검사까지 도달하지 못한다. 이 probe는
//! **하한가**에 1주 매수를 시도한다: 가격 검증은 통과하되 (1) 하한가라 시장가 한참
//! 아래 → 체결 불가, (2) 매수가능금액 부족 → 접수 거부. 따라서 실체결 위험 없이
//! funds 에러만 노출한다. 만일 예상과 달리 접수되면 즉시 취소한다(안전망).
//!
//! 실행: `TOSS_LIVE_ORDER=1 cargo run --example toss_funds_probe`

use korea_stock::domestic::toss::order::{OrderCreate, OrderType, Side};
use korea_stock::{TossClient, TossConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = TossClient::new(TossConfig::from_env()?)?;
    let accounts = client.accounts().list().await?;
    let seq = accounts.first().expect("계좌 필요").account_seq;

    // 하한가 — 가격 검증을 통과하는 최저가. 매수 지정가를 여기에 두면 체결 불가.
    let limits = client.market_data().price_limits("005930").await?;
    let lower = limits.lower_limit_price.clone().expect("하한가(가격제한) 필요");
    let buying = client.order_info(seq).buying_power("KRW").await?;
    println!(
        "005930 하한가={lower} / 매수가능금액={} {}",
        buying.cash_buying_power, buying.currency
    );

    let order = OrderCreate::Quantity {
        symbol: "005930".into(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        quantity: "1".into(),
        price: Some(lower),
        time_in_force: None,
        client_order_id: None,
        confirm_high_value_order: false,
    };

    if std::env::var("TOSS_LIVE_ORDER").as_deref() != Ok("1") {
        println!("[dry-run] TOSS_LIVE_ORDER=1 미설정 — 미전송. order={order:?}");
        return Ok(());
    }

    match client.order(seq).create(order).await {
        Err(e) => println!("거부됨(예상대로 funds 에러 기대): {e}"),
        Ok(resp) => {
            // 예상 외 접수 — 즉시 취소(체결은 하한가라 불가하지만 안전망).
            println!("⚠️ 예상 외 접수: orderId={} — 즉시 취소", resp.order_id);
            let c = client.order(seq).cancel(&resp.order_id).await?;
            println!("취소 결과: {c:?}");
        }
    }
    Ok(())
}
