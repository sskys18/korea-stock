//! 모의투자 환경 통합 스모크 테스트.
//! 실행: KIS_* 환경변수 설정 후 `cargo test --test integration -- --ignored`

use kis_adapter::{KisClient, KisConfig};

fn client() -> Option<KisClient> {
    let config = KisConfig::from_env().ok()?;
    KisClient::new(config).ok()
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn token_and_current_price() {
    let client = client().expect("KIS_* env vars + valid config");
    let ds = client.domestic_stock();
    let price = ds
        .current_price("005930")
        .await
        .expect("current price call");
    assert!(!price.stck_prpr.is_empty(), "현재가 비어있지 않음");
    let again = ds.current_price("000660").await.expect("second call");
    assert!(!again.stck_prpr.is_empty());
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn balance_query() {
    let client = client().expect("KIS_* env vars");
    let ds = client.domestic_stock();
    let (_items, summary) = ds.balance_all().await.expect("balance call");
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
        .subscribe(SubscriptionKind::DomesticTrade, "005930")
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
