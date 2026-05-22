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
