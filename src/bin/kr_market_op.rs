//! Live KRX/NXT market-operation (장운영) capture for the tree-capital-marker bot's
//! circuit-breaker detector. Subscribes to the realtime 장운영 feed and prints each
//! event as one JSON line. Market-wide events arrive on the Unified (UN, empty tr_key)
//! stream; the KRX per-market stream is also subscribed for comparison.
//!
//! Read-only (quote-side realtime only, no orders). Usage: `kr_market_op` with KIS_*
//! set. Ctrl-C to stop. Fields printed cover every candidate CB signal so the exact
//! circuit-breaker indicator (장운영구분코드 value and/or 거래정지사유 text) can be
//! confirmed against real data.

use korea_stock::{KisClient, KisConfig, Market, RealtimeEvent, SubscriptionKind};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KisClient::new(KisConfig::from_env()?)?;
    let mut rt = client.realtime().await?;
    let mut events = rt.take_events().expect("events receiver");

    let mut handles = Vec::new();
    handles.push(
        rt.subscribe(SubscriptionKind::MarketOperation(Market::Unified), "")
            .await?,
    );
    handles.push(
        rt.subscribe(SubscriptionKind::MarketOperation(Market::Krx), "")
            .await?,
    );
    eprintln!("subscribed 장운영 (Unified + KRX); waiting for events (Ctrl-C to stop)...");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => { eprintln!("stop"); break; }
            ev = events.recv() => match ev {
                Some(RealtimeEvent::MarketOperation { tr_id, tr_key, data }) => {
                    let line = serde_json::json!({
                        "tr_id": tr_id,
                        "tr_key": tr_key,
                        "trht_yn": data.trht_yn,
                        "reason": data.tr_susp_reas_cntt,
                        "mkop_cls_code": data.mkop_cls_code,
                        "antc_mkop_cls_code": data.antc_mkop_cls_code,
                        "mrkt_trtm_cls_code": data.mrkt_trtm_cls_code,
                        "iscd_stat_cls_code": data.iscd_stat_cls_code,
                        "vi_cls_code": data.vi_cls_code,
                        "exch_cls_code": data.exch_cls_code,
                    });
                    println!("{line}");
                }
                Some(RealtimeEvent::Reconnecting) => eprintln!("reconnecting..."),
                Some(RealtimeEvent::Reconnected) => eprintln!("reconnected"),
                Some(RealtimeEvent::Lagged(n)) => eprintln!("lagged {n}"),
                Some(_) => {}
                None => { eprintln!("connection closed"); break; }
            }
        }
    }
    drop(handles);
    Ok(())
}
