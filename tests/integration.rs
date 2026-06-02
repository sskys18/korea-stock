//! 모의투자 환경 통합 스모크 테스트.
//! 실행: KIS_* 환경변수 설정 후 `cargo test --test integration -- --ignored`

use korea_stock::{Exchange, KisClient, KisConfig, Market, RankBy};

fn client() -> Option<KisClient> {
    let config = KisConfig::from_env().ok()?;
    KisClient::new(config).ok()
}

/// 와이어 검증용 — KIS Api 에러(rt_cd≠0: 데이터 없음·계좌 미존재 등)는
/// "요청은 KIS에 도달" 으로 간주해 통과. 역직렬화/HTTP 에러만 실패 처리.
/// 계좌 종류·시장 상태에 의존하는 조회 TR에 사용.
fn assert_wire_ok<T>(r: korea_stock::Result<T>, what: &str) {
    match r {
        Ok(_) => {}
        Err(korea_stock::KisError::Api { rt_cd, msg_cd, msg }) => {
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
        .balance_all(korea_stock::kis::domestic_stock::BalanceBasis::Default)
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
        .current_price(korea_stock::kis::overseas_stock::OverseasExchange::Nasd, "AAPL")
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
        .current_price(korea_stock::kis::futureoption::MarketDiv::IndexFuture, "101W09")
        .await
        .expect("futureoption current price call");
    assert!(raw.get("output1").is_some() || !price.futs_prpr.is_empty());
}

#[tokio::test]
#[ignore = "requires KIS_* credentials + market hours"]
async fn realtime_subscribe_one() {
    use korea_stock::{RealtimeEvent, SubscriptionKind};

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

// ── NXT / 통합 와이어 검증 ─────────────────────────────────────────────

/// NXT/통합 per-TR 실지원 맵 감사 — 공식 샘플 주석 대신 실API rt_cd로 확정.
/// 각 메서드 × {Nxt, Unified} 호출해 rt_cd/msg 출력. 패닉 없이 전부 로깅.
#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn nxt_unified_support_audit() {
    use korea_stock::kis::domestic_stock::{MarketClass, Period, RankBy};
    let client = client().expect("KIS_* env vars");
    let ds = client.domestic_stock();
    fn rep<T>(name: &str, mkt: Market, r: korea_stock::Result<T>) {
        match r {
            Ok(_) => eprintln!("  [OK]   {name} {mkt:?}"),
            Err(korea_stock::KisError::Api { rt_cd, msg_cd, msg }) => {
                eprintln!("  [API]  {name} {mkt:?} rt_cd={rt_cd} {msg_cd} {msg}")
            }
            Err(e) => eprintln!("  [WIRE-FAIL] {name} {mkt:?} {e}"),
        }
    }
    for m in [Market::Nxt, Market::Unified] {
        rep("current_price", m, ds.current_price("005930", m).await);
        rep("asking_price", m, ds.asking_price("005930", m).await);
        rep("period_price", m, ds.period_price("005930", "20260501", "20260602", Period::Daily, true, m).await);
        rep("minute_chart", m, ds.minute_chart("005930", "100000", true, m).await);
        rep("investor_trend_daily", m, ds.investor_trend_daily("005930", "20260602", false, m).await);
        rep("program_trade_today", m, ds.program_trade_today(MarketClass::Kospi, None, m).await);
        rep("program_trade_daily", m, ds.program_trade_daily(MarketClass::Kospi, "20260501", "20260602", m).await);
        rep("volume_rank", m, ds.volume_rank(m, RankBy::TradingAmount).await);
    }
    eprintln!("→ [API rt_cd≠0 with INVALID FID_COND_MRKT_DIV_CODE] = 해당 TR은 그 market 미지원");
}

/// NXT 실시간 호가(H0NXASP0, 65필드) decode 검증.
/// 구독 수락 확인 + 프레임이 오면 65필드 레이아웃 decode 정합성 확인.
/// 장중(08:00~20:00 KST)에 실데이터 검증됨. 장 마감 시 프레임 없으면 skip 로그.
#[tokio::test]
#[ignore = "requires KIS_* credentials + NXT market hours (08:00~20:00 KST)"]
async fn realtime_nxt_asking_decode() {
    use korea_stock::{RealtimeEvent, SubscriptionKind};

    let config = KisConfig::from_env().expect("KIS_* env vars");
    let client = KisClient::new(config).expect("client");
    let mut rt = client.realtime().await.expect("realtime connect");
    let mut events = rt.take_events().expect("events");

    // NXT 호가 구독 — tr_id H0NXASP0 합성 확인.
    let _h = rt
        .subscribe(SubscriptionKind::DomesticAsking(Market::Nxt), "005930")
        .await
        .expect("NXT asking subscribe");

    let frame = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            match events.recv().await {
                Some(RealtimeEvent::DomesticAsking { tr_id, data, .. }) => {
                    return Some((tr_id, data))
                }
                Some(RealtimeEvent::Reconnecting) | Some(RealtimeEvent::Reconnected) => continue,
                Some(_) => continue,
                None => return None,
            }
        }
    })
    .await;

    match frame {
        Ok(Some((tr_id, d))) => {
            eprintln!("NXT asking frame: tr_id={tr_id} askp1={} bidp1={}", d.askp1, d.bidp1);
            assert!(tr_id.starts_with("H0NX") || tr_id.starts_with("H0UN"), "NXT tr_id");
            // 65필드 레이아웃 decode 정합성 — 핵심 필드 + 중간가 tail 접근.
            assert!(!d.askp1.is_empty() || !d.bidp1.is_empty(), "호가 비어있지 않음");
            eprintln!(
                "중간가 tail: kmid_prc={} nmid_prc={} (NXT 65필드)",
                d.kmid_prc, d.nmid_prc
            );
        }
        Ok(None) => panic!("event channel closed"),
        Err(_) => eprintln!("프레임 없음 — 장 마감(NXT 08:00~20:00 KST)으로 추정. 구독 수락은 확인됨."),
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
    use korea_stock::kis::domestic_stock::Period;
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
    use korea_stock::kis::domestic_stock::OrderType;
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
    use korea_stock::kis::domestic_stock::SellBuy;
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
    use korea_stock::kis::domestic_stock::SellBuy;
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
    use korea_stock::kis::overseas_stock::OverseasExchange;
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
    use korea_stock::kis::overseas_stock::{OverseasExchange, OverseasPeriod};
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
    use korea_stock::kis::overseas_stock::OverseasExchange;
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
    use korea_stock::kis::overseas_stock::{FilledFilter, OverseasSellBuy};
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
    use korea_stock::kis::futureoption::MarketDiv;
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
    use korea_stock::kis::futureoption::{CcnlFilter, CcnlSellBuy};
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
    use korea_stock::kis::futureoption::SellBuy;
    let client = client().expect("KIS_* env vars");
    let r = client
        .futureoption()
        .buyable("101W09", SellBuy::Buy, 300.0, "01")
        .await;
    assert_wire_ok(r, "futureoption buyable");
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn domestic_volume_rank() {
    let client = client().expect("KIS_* env vars");
    let ds = client.domestic_stock();
    // KRX·NXT는 지원 — 거래대금순(순환매 레이더 스캐너).
    for m in [Market::Krx, Market::Nxt] {
        let rows = ds.volume_rank(m, RankBy::TradingAmount).await;
        match rows {
            Ok(items) => {
                eprintln!("VOLUME_RANK({m:?}): {} rows", items.len());
                if let Some(top) = items.first() {
                    eprintln!("  #1 {} {} 거래대금={}", top.mksc_shrn_iscd, top.hts_kor_isnm, top.acml_tr_pbmn);
                }
            }
            Err(e) => assert_wire_ok::<()>(Err(e), "volume_rank"),
        }
    }
    // 통합(Unified)은 KIS 미지원(OPSQ2001) — 어댑터가 호출 전 가드(2026-06-02 실API 확인).
    let un = ds.volume_rank(Market::Unified, RankBy::TradingAmount).await;
    assert!(
        matches!(un, Err(korea_stock::KisError::Decode(_))),
        "volume_rank Unified은 가드돼야 함"
    );
}
