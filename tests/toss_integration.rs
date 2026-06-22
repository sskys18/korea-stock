//! 토스증권 라이브 스모크 — **조회 전용**. 주문 write(create/modify/cancel)는
//! 제외한다: 토스는 모의투자 환경이 없어 실주문=실자금이기 때문.
//! 실행: TOSS_CLIENT_ID / TOSS_CLIENT_SECRET 설정 후
//!   `cargo test --test toss_integration -- --ignored --nocapture`

use korea_stock::domestic::toss::market_data::{CandleInterval, CandleReq};
use korea_stock::domestic::toss::order::{OrderListReq, OrderStatusFilter};
use korea_stock::{TossClient, TossConfig, TossError};

fn client() -> Option<TossClient> {
    TossClient::new(TossConfig::from_env().ok()?).ok()
}

/// 서버 도달(=`Api` 비즈니스 에러: 데이터 없음·심볼 미존재 등)은 와이어 OK로 통과,
/// 전송/디코드 실패만 패닉. KIS `assert_wire_ok`의 토스판.
fn wire_ok<T>(r: Result<T, TossError>, what: &str) {
    match r {
        Ok(_) => eprintln!("  [OK]   {what}"),
        Err(TossError::Api {
            status,
            code,
            message,
            ..
        }) => eprintln!("  [API]  {what} http={status} code={code} ({message}) — 와이어 OK"),
        Err(e) => panic!("{what}: 와이어 실패 — {e}"),
    }
}

/// 시세 도메인 5종 — orderbook·prices·trades·price_limits·candles.
#[tokio::test]
#[ignore = "requires TOSS_* credentials"]
async fn market_data_all() {
    let c = client().expect("TOSS_* env vars + valid config");
    let md = c.market_data();

    let prices = md.prices(&["005930", "AAPL"]).await.expect("prices");
    assert!(!prices.is_empty(), "현재가 1건 이상");
    eprintln!("  [OK]   prices: {} symbols", prices.len());

    md.orderbook("005930").await.expect("orderbook KR");
    eprintln!("  [OK]   orderbook 005930");
    md.orderbook("AAPL").await.expect("orderbook US");
    eprintln!("  [OK]   orderbook AAPL");

    wire_ok(md.trades("005930", Some(10)).await, "trades 005930");
    wire_ok(md.price_limits("005930").await, "price_limits 005930");
    wire_ok(
        md.candles(CandleReq::new("005930", CandleInterval::Day1)).await,
        "candles 005930 1d",
    );
}

/// 종목정보(stocks·warnings) + 시장정보(exchange_rate·calendar KR/US).
#[tokio::test]
#[ignore = "requires TOSS_* credentials"]
async fn stock_and_market_info() {
    let c = client().expect("TOSS_* env vars");

    let si = c.stock_info();
    wire_ok(si.stocks(&["005930", "AAPL"]).await, "stocks");
    wire_ok(si.warnings("005930").await, "warnings 005930");

    let mi = c.market_info();
    wire_ok(mi.exchange_rate("USD", "KRW", None).await, "exchange_rate USD/KRW");
    wire_ok(mi.calendar_kr(None).await, "calendar_kr");
    wire_ok(mi.calendar_us(None).await, "calendar_us");
}

/// 계좌 스코프 조회 — accounts·holdings·buying_power·sellable_quantity·
/// commissions·order.list(Open)·order.get(있을 때만). 헤더 자동 주입 경로 검증.
#[tokio::test]
#[ignore = "requires TOSS_* credentials + 실계좌"]
async fn account_scoped_reads() {
    let c = client().expect("TOSS_* env vars");

    let accounts = c.accounts().list().await.expect("accounts list");
    let Some(account) = accounts.first() else {
        eprintln!("  [SKIP] 계좌 없음 — 계좌 스코프 호출 생략");
        return;
    };
    let seq = account.account_seq;
    eprintln!("  [OK]   accounts: seq={seq} type={}", account.account_type);

    wire_ok(c.asset(seq).holdings(None).await, "holdings");
    wire_ok(c.order_info(seq).buying_power("KRW").await, "buying_power KRW");
    wire_ok(
        c.order_info(seq).sellable_quantity("005930").await,
        "sellable_quantity 005930",
    );
    wire_ok(c.order_info(seq).commissions().await, "commissions");

    // 주문 목록(Open) — get 검증용 order_id 확보 겸.
    let list = c
        .order(seq)
        .list(OrderListReq::new(OrderStatusFilter::Open))
        .await;
    match list {
        Ok(page) => {
            eprintln!("  [OK]   order.list(Open): {} orders", page.orders.len());
            // 미체결 주문이 있으면 get까지 검증, 없으면 skip(주문 write 안 함).
            if let Some(o) = page.orders.first() {
                wire_ok(c.order(seq).get(&o.order_id).await, "order.get");
            } else {
                eprintln!("  [SKIP] order.get — Open 주문 없음(write 미수행)");
            }
        }
        Err(e) => wire_ok::<()>(Err(e), "order.list(Open)"),
    }
}
