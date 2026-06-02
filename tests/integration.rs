//! 모의투자 환경 통합 스모크 테스트.
//! 실행: KIS_* 환경변수 설정 후 `cargo test --test integration -- --ignored`

use kis_adapter::{Exchange, KisClient, KisConfig, Market};

fn client() -> Option<KisClient> {
    let config = KisConfig::from_env().ok()?;
    KisClient::new(config).ok()
}

/// 와이어 검증용 — KIS Api 에러(rt_cd≠0: 데이터 없음·계좌 미존재 등)는
/// "요청은 KIS에 도달" 으로 간주해 통과. 역직렬화/HTTP 에러만 실패 처리.
/// 계좌 종류·시장 상태에 의존하는 조회 TR에 사용.
fn assert_wire_ok<T>(r: kis_adapter::Result<T>, what: &str) {
    match r {
        Ok(_) => {}
        Err(kis_adapter::KisError::Api { rt_cd, msg_cd, msg }) => {
            eprintln!("{what}: KIS rt_cd={rt_cd} msg_cd={msg_cd} ({msg}) — 와이어 OK, 데이터 없음");
        }
        Err(e) => panic!("{what}: 와이어 실패 — {e}"),
    }
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn token_and_current_price() {
    let client = client().expect("KIS_* env vars + valid config");
    let ds = client.domestic_stock();
    let price = ds
        .current_price("005930", Market::Krx)
        .await
        .expect("current price call");
    assert!(!price.stck_prpr.is_empty(), "현재가 비어있지 않음");
    let again = ds
        .current_price("000660", Market::Krx)
        .await
        .expect("second call");
    assert!(!again.stck_prpr.is_empty());
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn balance_query() {
    let client = client().expect("KIS_* env vars");
    let ds = client.domestic_stock();
    let (_items, summary) = ds
        .balance_all(kis_adapter::domestic_stock::BalanceBasis::Default)
        .await
        .expect("balance call");
    assert!(!summary.is_empty(), "계좌 요약 1건 이상");
}

// ── Plan 2: 해외주식·선물옵션 스모크 ──────────────────────────────────

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn overseas_current_price() {
    let client = client().expect("KIS_* env vars");
    let os = client.overseas_stock();
    let price = os
        .current_price(kis_adapter::overseas_stock::OverseasExchange::Nasd, "AAPL")
        .await
        .expect("overseas current price call");
    assert!(!price.last.is_empty(), "현재가 비어있지 않음");
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn futureoption_current_price() {
    let client = client().expect("KIS_* env vars");
    let fo = client.futureoption();
    let (price, raw) = fo
        .current_price(kis_adapter::futureoption::MarketDiv::IndexFuture, "101W09")
        .await
        .expect("futureoption current price call");
    assert!(raw.get("output1").is_some() || !price.futs_prpr.is_empty());
}

#[tokio::test]
#[ignore = "requires KIS_* credentials + market hours"]
async fn realtime_subscribe_one() {
    use kis_adapter::{RealtimeEvent, SubscriptionKind};

    let config = KisConfig::from_env().expect("KIS_* env vars");
    let client = KisClient::new(config).expect("client");
    let mut rt = client.realtime().await.expect("realtime connect");
    let mut events = rt.take_events().expect("events");

    let _handle = rt
        .subscribe(SubscriptionKind::DomesticTrade(Market::Krx), "005930")
        .await
        .expect("subscribe");

    let got = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match events.recv().await {
                Some(RealtimeEvent::DomesticTrade { .. }) => return true,
                Some(_) => continue,
                None => return false,
            }
        }
    })
    .await;
    if let Ok(false) = got {
        panic!("event channel closed unexpectedly");
    }
}

// ── 조회 TR 와이어 검증 (읽기 전용 — 주문 TR은 실거래라 제외) ─────────

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn domestic_asking_price() {
    let client = client().expect("KIS_* env vars");
    let (asking, expected) = client
        .domestic_stock()
        .asking_price("005930", Market::Krx)
        .await
        .expect("asking price call");
    assert!(!asking.askp1.is_empty() || !expected.antc_cnpr.is_empty());
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn domestic_period_price() {
    use kis_adapter::domestic_stock::Period;
    let client = client().expect("KIS_* env vars");
    let (_summary, candles) = client
        .domestic_stock()
        .period_price(
            "005930",
            "20260401",
            "20260522",
            Period::Daily,
            true,
            Market::Krx,
        )
        .await
        .expect("period price call");
    assert!(!candles.is_empty(), "일봉 1건 이상");
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn domestic_minute_chart() {
    let client = client().expect("KIS_* env vars");
    let (_summary, candles) = client
        .domestic_stock()
        .minute_chart("005930", "100000", true, Market::Krx)
        .await
        .expect("minute chart call");
    let _ = candles;
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn domestic_buyable() {
    use kis_adapter::domestic_stock::OrderType;
    let client = client().expect("KIS_* env vars");
    let info = client
        .domestic_stock()
        .buyable("005930", 70000, OrderType::Market)
        .await
        .expect("buyable call");
    assert!(!info.nrcvb_buy_qty.is_empty() || !info.ord_psbl_cash.is_empty());
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn domestic_daily_conclusions() {
    use kis_adapter::domestic_stock::SellBuy;
    let client = client().expect("KIS_* env vars");
    let page = client
        .domestic_stock()
        .daily_conclusions("20260401", "20260522", SellBuy::All, None, Exchange::Krx)
        .await
        .expect("daily conclusions call");
    let _ = page.data;
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn domestic_revisable_orders() {
    use kis_adapter::domestic_stock::SellBuy;
    let client = client().expect("KIS_* env vars");
    let page = client
        .domestic_stock()
        .revisable_orders(SellBuy::All, None)
        .await
        .expect("revisable orders call");
    let _ = page.data;
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn overseas_balance() {
    use kis_adapter::overseas_stock::OverseasExchange;
    let client = client().expect("KIS_* env vars");
    let (_items, _summary) = client
        .overseas_stock()
        .balance_all(OverseasExchange::Nasd)
        .await
        .expect("overseas balance call");
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn overseas_period_price() {
    use kis_adapter::overseas_stock::{OverseasExchange, OverseasPeriod};
    let client = client().expect("KIS_* env vars");
    let page = client
        .overseas_stock()
        .period_price(
            OverseasExchange::Nasd,
            "AAPL",
            OverseasPeriod::Daily,
            "",
            true,
            false,
        )
        .await
        .expect("overseas period price call");
    let _ = page.data;
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn overseas_unfilled_orders() {
    use kis_adapter::overseas_stock::OverseasExchange;
    let client = client().expect("KIS_* env vars");
    let page = client
        .overseas_stock()
        .unfilled_orders(OverseasExchange::Nasd, None)
        .await
        .expect("overseas unfilled call");
    let _ = page.data;
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn overseas_conclusions() {
    use kis_adapter::overseas_stock::{FilledFilter, OverseasSellBuy};
    let client = client().expect("KIS_* env vars");
    let list = client
        .overseas_stock()
        .conclusions_all(
            "20260401",
            "20260522",
            OverseasSellBuy::All,
            FilledFilter::All,
        )
        .await
        .expect("overseas conclusions call");
    let _ = list;
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn futureoption_asking_price() {
    use kis_adapter::futureoption::MarketDiv;
    let client = client().expect("KIS_* env vars");
    let (_price, raw) = client
        .futureoption()
        .asking_price(MarketDiv::IndexFuture, "101W09")
        .await
        .expect("futureoption asking price call");
    assert!(raw.get("output1").is_some() || raw.get("output2").is_some());
}

// 선물옵션 계좌·체결·주문가능 TR은 선물옵션 거래계좌가 필요하다. 주식 전용
// 계좌면 KIS가 rt_cd≠0(계좌 미존재·데이터 없음)을 반환 — 와이어 도달은 검증되나
// 응답 struct 필드는 선물옵션 계좌로만 완전 검증 가능.
#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn futureoption_balance() {
    let client = client().expect("KIS_* env vars");
    let r = client.futureoption().balance("01", "1", None).await;
    assert_wire_ok(r, "futureoption balance");
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn futureoption_conclusions() {
    use kis_adapter::futureoption::{CcnlFilter, CcnlSellBuy};
    let client = client().expect("KIS_* env vars");
    let r = client
        .futureoption()
        .conclusions_all("20260401", "20260522", CcnlSellBuy::All, CcnlFilter::All)
        .await;
    assert_wire_ok(r, "futureoption conclusions");
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn futureoption_buyable() {
    use kis_adapter::futureoption::SellBuy;
    let client = client().expect("KIS_* env vars");
    let r = client
        .futureoption()
        .buyable("101W09", SellBuy::Buy, 300.0, "01")
        .await;
    assert_wire_ok(r, "futureoption buyable");
}

// 실주문 사이클 — 미체결 보장(-10% 지정가) → revise(-15%) → cancel.
// 가드: `KIS_LIVE_ORDER_TEST=1` 필수. 시장 시간 내(09:00~15:25 KST) 실행.
// 비용: 미체결이므로 체결 수수료 0원. 주문 자체는 KIS 무료.
// 종목: KODEX 200 (069500). 호가단위 5원(5000원 이상).
#[tokio::test]
#[ignore = "live order — set KIS_LIVE_ORDER_TEST=1, market hours only"]
async fn live_order_unfilled_cycle() {
    if std::env::var("KIS_LIVE_ORDER_TEST").as_deref() != Ok("1") {
        panic!("set KIS_LIVE_ORDER_TEST=1 to enable live order test");
    }
    use kis_adapter::domestic_stock::{OrderReq, OrderType, ReviseCancelReq};

    let client = client().expect("KIS_* env vars");
    let stock = "069500"; // KODEX 200

    // 1. 현재가
    let q = client
        .domestic_stock()
        .current_price(stock, Market::Krx)
        .await
        .expect("current price");
    let cur: u64 = q.stck_prpr.trim().parse().expect("price parse");
    assert!(cur > 5000, "tick size assumption: 5원 단위 종목");

    // 호가단위 5원 정렬
    let round5 = |p: u64| (p / 5) * 5;
    let bid_far = round5((cur as f64 * 0.90) as u64);
    let bid_lower = round5((cur as f64 * 0.85) as u64);
    eprintln!("CUR={cur} FAR={bid_far} LOWER={bid_lower}");

    // 2. 매수 (미체결 보장)
    let buy = client
        .domestic_stock()
        .buy(OrderReq::new(
            stock,
            OrderType::Limit,
            1,
            bid_far,
        ))
        .await
        .expect("buy order");
    eprintln!(
        "BUY: orgno={} odno={} t={}",
        buy.krx_fwdg_ord_orgno, buy.odno, buy.ord_tmd
    );
    assert!(!buy.odno.is_empty(), "주문번호 발급");

    // 3. 정정 (-15%, 잔량)
    let rev = client
        .domestic_stock()
        .revise(ReviseCancelReq {
            krx_fwdg_ord_orgno: buy.krx_fwdg_ord_orgno.clone(),
            orig_order_no: buy.odno.clone(),
            order_type: OrderType::Limit,
            quantity: 1,
            price: bid_lower,
            all: false,
            exchange: Exchange::Krx,
        })
        .await
        .expect("revise order");
    eprintln!("REVISE: odno={}", rev.odno);
    assert!(!rev.odno.is_empty(), "정정 주문번호");

    // 4. 취소 — 정정 결과 odno 사용, 잔량 전부
    let cancel = client
        .domestic_stock()
        .cancel(ReviseCancelReq {
            krx_fwdg_ord_orgno: rev.krx_fwdg_ord_orgno.clone(),
            orig_order_no: rev.odno.clone(),
            order_type: OrderType::Limit,
            quantity: 1,
            price: bid_lower,
            all: true,
            exchange: Exchange::Krx,
        })
        .await
        .expect("cancel order");
    eprintln!("CANCEL: odno={}", cancel.odno);
    assert!(!cancel.odno.is_empty(), "취소 주문번호");
}
