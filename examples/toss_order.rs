//! 토스증권 계좌·주문 흐름 예제 (기본은 실주문 미발생 — 안전).
//! 실행: TOSS_CLIENT_ID / TOSS_CLIENT_SECRET 환경변수 설정 후
//! `cargo run --example toss_order`
//!
//! 계좌 헤더 흐름을 end-to-end로 보여준다:
//!   1) `accounts().list()` 로 accountSeq 획득 (계좌 헤더 불필요)
//!   2) accountSeq를 보유한 **계좌 스코프 액세서**로 매수가능금액·보유주식 조회
//!   3) 주문 생성 호출 시연
//!
//! 핵심: `X-Tossinvest-Account` 헤더는 `client.order(seq)` / `client.asset(seq)` /
//! `client.order_info(seq)` 액세서가 accountSeq를 구조적으로 보유해 자동 주입한다.
//! seq 없이 계좌 스코프 호출을 만들 방법 자체가 컴파일 단계에서 존재하지 않으므로
//! 헤더 누락이 불가능하다.
//!
//! 안전장치: 실제 매수 주문은 `TOSS_LIVE_ORDER=1` 일 때만 전송한다. 미설정 시
//! 주문 파라미터만 구성해 dry-run 안내를 출력하고 호출하지 않는다 (실주문 오발 방지).

use korea_stock::domestic::toss::order::{OrderCreate, OrderType, Side};
use korea_stock::{TossClient, TossConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let client = TossClient::new(TossConfig::from_env()?)?;

    // 1) 계좌 목록 — 계좌 헤더 불필요(seq-free 액세서).
    let accounts = client.accounts().list().await?;
    let Some(account) = accounts.first() else {
        println!("계좌가 없습니다.");
        return Ok(());
    };
    let seq = account.account_seq;
    println!(
        "계좌 {} (seq={}, type={})",
        account.account_no, seq, account.account_type
    );

    // 2) 계좌 스코프 액세서 — seq를 보유해 X-Tossinvest-Account 자동 주입.
    //    아래 호출들은 seq를 다시 넘기지 않는다. 액세서가 이미 들고 있다.
    let buying = client.order_info(seq).buying_power("KRW").await?;
    println!(
        "매수가능금액: {} {}",
        buying.cash_buying_power, buying.currency
    );

    let holdings = client.asset(seq).holdings(None).await?;
    println!("보유종목 {}건", holdings.items.len());
    for it in &holdings.items {
        println!("  {} {} 보유 {}주", it.symbol, it.name, it.quantity);
    }

    // 3) 주문 생성 시연 — 삼성전자 1주 지정가 매수.
    //    OrderCreate::Quantity 변형은 수량 기반 주문의 불변식(price는 LIMIT 필수)을 타입으로 표현.
    let order = OrderCreate::Quantity {
        symbol: "005930".into(),
        side: Side::Buy,
        order_type: OrderType::Limit,
        quantity: "1".into(),
        price: Some("60000".into()),
        time_in_force: None,
        client_order_id: None,
        confirm_high_value_order: false,
    };

    let live = std::env::var("TOSS_LIVE_ORDER").as_deref() == Ok("1");
    if live {
        // 계좌 스코프 order(seq) 액세서가 헤더를 붙여 실제 전송.
        let resp = client.order(seq).create(order).await?;
        println!("주문 전송됨: orderId={}", resp.order_id);
    } else {
        println!("[dry-run] TOSS_LIVE_ORDER=1 미설정 — 주문 미전송.");
        println!("[dry-run] 구성된 주문: {order:?}");
        println!("[dry-run] 실제 전송하려면: TOSS_LIVE_ORDER=1 cargo run --example toss_order");
    }

    Ok(())
}
