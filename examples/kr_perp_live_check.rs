//! 4 venue(Binance·Hyperliquid·Lighter·MEXC) 한국주식 perp **메인넷 public 검증**.
//! 키 불필요 — 공개 엔드포인트만. 심볼 실존·시세 응답·메타 해석을 라이브로 확인.
//! 실행: `cargo run --example kr_perp_live_check`
//!
//! 검증 항목:
//!  - 각 venue KR 심볼이 메인넷에 실재하고 호가/마크가가 응답하는가
//!  - HL: universe 메타에서 asset id가 하드코딩 상수와 일치하는가(재정렬 가드)
//!  - Lighter: 심볼→market_id 해석 + orderBookDetails status=active 인가
//!  - MEXC: contract state(=0 거래중) + ticker 응답
//!
//! 실주문 왕복은 venue API 키가 없어 제외(.env에 KIS만 존재).

use korea_stock::global::{binance, hyperliquid, lighter, mexc};

#[tokio::main]
async fn main() {
    let mut pass = 0u32;
    let mut fail = 0u32;
    macro_rules! ok {
        ($($a:tt)*) => {{ pass += 1; println!("  ✅ {}", format!($($a)*)); }};
    }
    macro_rules! bad {
        ($($a:tt)*) => {{ fail += 1; println!("  ❌ {}", format!($($a)*)); }};
    }

    // ---- Binance USDM Futures ----
    println!("\n=== Binance (fapi.binance.com) ===");
    match binance::BinanceClient::new(binance::BinanceConfig::public()) {
        Ok(c) => {
            let md = c.market();
            for s in binance::KR_SYMBOLS {
                match md.premium_index(s).await {
                    Ok(idx) => match md.depth(s, Some(5)).await {
                        Ok(book) => ok!(
                            "{s}: mark={} index={} funding={} bid1={} ask1={}",
                            idx.mark_price,
                            idx.index_price,
                            idx.last_funding_rate,
                            book.bids.first().map(|e| e[0].as_str()).unwrap_or("-"),
                            book.asks.first().map(|e| e[0].as_str()).unwrap_or("-"),
                        ),
                        Err(e) => bad!("{s}: depth 실패 {e}"),
                    },
                    Err(e) => bad!("{s}: premium_index 실패 {e}"),
                }
            }
        }
        Err(e) => bad!("client 생성 실패 {e}"),
    }

    // ---- Hyperliquid (dex=xyz) ----
    println!("\n=== Hyperliquid (api.hyperliquid.xyz, dex=xyz) ===");
    match hyperliquid::HyperliquidClient::new(hyperliquid::HyperliquidConfig::public()) {
        Ok(c) => {
            let md = c.market();
            let consts = [
                (hyperliquid::SAMSUNG, hyperliquid::SAMSUNG_ASSET),
                (hyperliquid::SK_HYNIX, hyperliquid::SK_HYNIX_ASSET),
                (hyperliquid::HYUNDAI, hyperliquid::HYUNDAI_ASSET),
            ];
            for (coin, const_id) in consts {
                match md.asset_id(hyperliquid::DEX, coin).await {
                    Ok(live_id) => {
                        if live_id == const_id {
                            match md.l2_book(coin).await {
                                Ok(b) => ok!(
                                    "{coin}: asset_id={live_id}(상수일치) bid1={} ask1={}",
                                    b.bids().first().map(|l| l.px.as_str()).unwrap_or("-"),
                                    b.asks().first().map(|l| l.px.as_str()).unwrap_or("-"),
                                ),
                                Err(e) => bad!("{coin}: l2_book 실패 {e}"),
                            }
                        } else {
                            bad!("{coin}: asset_id 불일치 live={live_id} 상수={const_id} (universe 재정렬! 상수 갱신 필요)");
                        }
                    }
                    Err(e) => bad!("{coin}: asset_id 해석 실패 {e}"),
                }
            }
        }
        Err(e) => bad!("client 생성 실패 {e}"),
    }

    // ---- Lighter (mainnet.zklighter) ----
    println!("\n=== Lighter (mainnet.zklighter.elliot.ai) ===");
    match lighter::LighterClient::new(lighter::LighterConfig::public()) {
        Ok(c) => {
            let md = c.market();
            for s in lighter::KR_SYMBOLS {
                match md.market_id(s).await {
                    Ok(id) => match md.order_book_details(id).await {
                        Ok(d) => {
                            if d.status == "active" {
                                ok!(
                                    "{s}: market_id={id} status={} last={} OI={}",
                                    d.status,
                                    d.last_trade_price,
                                    d.open_interest
                                );
                            } else {
                                bad!("{s}: market_id={id} status={}(비활성)", d.status);
                            }
                        }
                        Err(e) => bad!("{s}: order_book_details 실패 {e}"),
                    },
                    Err(e) => bad!("{s}: market_id 해석 실패 {e}"),
                }
            }
        }
        Err(e) => bad!("client 생성 실패 {e}"),
    }

    // ---- MEXC Futures (contract.mexc.com) ----
    println!("\n=== MEXC (contract.mexc.com) ===");
    match mexc::MexcClient::new(mexc::MexcConfig::public()) {
        Ok(c) => {
            let md = c.market();
            for s in mexc::KR_SYMBOLS {
                match md.contract(s).await {
                    Ok(ct) => match md.ticker(s).await {
                        Ok(t) => ok!(
                            "{s}: state={}({}) last={} fair={} funding={}",
                            ct.state,
                            if ct.state == 0 { "거래중" } else { "비정상" },
                            t.last_price,
                            t.fair_price,
                            t.funding_rate
                        ),
                        Err(e) => bad!("{s}: ticker 실패 {e}"),
                    },
                    Err(e) => bad!("{s}: contract 조회 실패 {e}"),
                }
            }
        }
        Err(e) => bad!("client 생성 실패 {e}"),
    }

    println!("\n=== 결과: {pass} pass / {fail} fail ===");
    if fail > 0 {
        std::process::exit(1);
    }
}
