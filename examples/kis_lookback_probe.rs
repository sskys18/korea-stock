//! 1분봉 by-date lookback 천장 실측. 실행: KIS_* 설정 후
//! `cargo run --example kis_lookback_probe`
//!
//! 005930(삼성)에 대해 과거일들로 minute_chart_by_date 호출 →
//! 반환 봉 수 + 최신/최古 시각 출력. 봉 0 = 그 날짜는 API 한계 밖.

use korea_stock::{KisClient, KisConfig, Market};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KisClient::new(KisConfig::from_env()?)?;
    let ds = client.domestic_stock();
    let code = "005930";
    // 오늘 2026-06-02 기준 T-3 / T-30 / T-90 / T-180 / T-365 후보(거래일 근사).
    let dates = [
        "20260528", "20260430", "20260303", "20251201", "20250602",
    ];
    println!("date      | bars | earliest | latest");
    for d in dates {
        match ds
            .minute_chart_by_date(code, d, "153000", true, Market::Krx)
            .await
        {
            Ok((_s, candles)) => {
                let n = candles.len();
                let lo = candles.iter().map(|c| c.stck_cntg_hour.as_str()).min().unwrap_or("-");
                let hi = candles.iter().map(|c| c.stck_cntg_hour.as_str()).max().unwrap_or("-");
                println!("{d} | {n:>4} | {lo} | {hi}");
            }
            Err(e) => println!("{d} | ERR  | {e}"),
        }
    }
    Ok(())
}
