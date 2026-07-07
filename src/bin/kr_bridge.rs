//! One-shot KRX/NXT quote bridge for the tree-capital-marker Python bot.
//!
//! Usage: `kr_bridge <code> [market]` where `market` is `krx` | `nxt` | `unified`
//! (default `unified` = KRX+NXT 통합시세). Requires the `KIS_*` env vars
//! (`KisConfig::from_env`). Read-only: this bin ONLY calls the current-price
//! quote TR, never an order/account TR.
//!
//! On success prints exactly one JSON line to stdout and exits 0:
//!   {"ok":true,"code":"005930","market":"unified","price":295750.0,"prev_close":318000.0}
//! On failure prints one JSON line ({"ok":false,"error":"..."}) and exits 1
//! (usage errors exit 2), so the Python side can fail closed on a non-zero exit
//! or an `ok:false` payload without ever consuming a partial/garbled price.

use korea_stock::{KisClient, KisConfig, Market};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let code = match args.get(1) {
        Some(c) if !c.trim().is_empty() => c.trim().to_string(),
        _ => {
            eprintln!("usage: kr_bridge <code> [krx|nxt|unified]");
            std::process::exit(2);
        }
    };
    let market = match args.get(2).map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        None | Some("") | Some("unified") | Some("un") => Market::Unified,
        Some("krx") | Some("j") => Market::Krx,
        Some("nxt") | Some("nx") => Market::Nxt,
        Some(other) => {
            eprintln!("unknown market {other:?} (expected krx|nxt|unified)");
            std::process::exit(2);
        }
    };

    match quote(&code, market).await {
        Ok(json) => println!("{json}"),
        Err(err) => {
            println!("{}", serde_json::json!({ "ok": false, "error": err }));
            std::process::exit(1);
        }
    }
}

fn market_str(market: Market) -> &'static str {
    match market {
        Market::Krx => "krx",
        Market::Nxt => "nxt",
        Market::Unified => "unified",
    }
}

async fn quote(code: &str, market: Market) -> Result<String, String> {
    let config = KisConfig::from_env().map_err(|e| e.to_string())?;
    let client = KisClient::new(config).map_err(|e| e.to_string())?;
    let cp = client
        .domestic_stock()
        .current_price(code, market)
        .await
        .map_err(|e| e.to_string())?;

    let price = cp
        .price()
        .ok_or_else(|| format!("unparseable current price {:?}", cp.stck_prpr))?;
    // KIS returns 0 (not an error) for an unknown/halted code; reject it so the
    // Python side fails closed instead of treating 0 as a real price.
    if !(price > 0.0) {
        return Err(format!("non-positive price {price} for {code} (unknown/halted?)"));
    }
    // stck_sdpr = 주식 기준가 (regular-session base = prior close); best-effort.
    let prev_close: Option<f64> = cp.stck_sdpr.trim().parse().ok();

    Ok(serde_json::json!({
        "ok": true,
        "code": code,
        "market": market_str(market),
        "price": price,
        "prev_close": prev_close,
    })
    .to_string())
}
