//! 실시간 체결가 구독 예제.
//! 실행: KIS_* 환경변수 설정 후
//! `cargo run --example kis_realtime_feed -- 005930 000660`
//! (인자 없으면 005930 기본. Ctrl-C로 종료.)

use korea_stock::{KisClient, KisConfig, Market, RealtimeEvent, SubscriptionKind};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let codes: Vec<String> = {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.is_empty() {
            vec!["005930".to_string()]
        } else {
            args
        }
    };

    let client = KisClient::new(KisConfig::from_env()?)?;
    let mut rt = client.realtime().await?;
    let mut events = rt.take_events().expect("events receiver");

    let mut handles = Vec::new();
    for code in &codes {
        handles.push(
            rt.subscribe(SubscriptionKind::DomesticTrade(Market::Krx), code)
                .await?,
        );
        println!("구독: {code}");
    }

    println!("실시간 수신 시작 — Ctrl-C로 종료");
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("종료");
                break;
            }
            ev = events.recv() => {
                match ev {
                    Some(RealtimeEvent::DomesticTrade { tr_key, data, .. }) => {
                        println!(
                            "[{tr_key}] {} 현재가 {} 거래량 {}",
                            data.stck_cntg_hour, data.stck_prpr, data.cntg_vol
                        );
                    }
                    Some(RealtimeEvent::Lagged(n)) => {
                        eprintln!("경고: {n}건 유실 (채널 포화)");
                    }
                    Some(RealtimeEvent::Reconnecting) => println!("재연결 중..."),
                    Some(RealtimeEvent::Reconnected) => println!("재연결·재구독 완료"),
                    Some(_) => {}
                    None => {
                        println!("연결 종료됨");
                        break;
                    }
                }
            }
        }
    }
    drop(handles);
    Ok(())
}
