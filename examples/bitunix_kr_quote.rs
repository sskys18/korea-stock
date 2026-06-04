//! Bitunix Futures 한국주식 무기한선물 시세 예제 (키 불필요).
//! 실행: `cargo run --example bitunix_kr_quote`
//!
//! 삼성전자·SK하이닉스·현대차 무기한선물의 마크가·펀딩비·호가를 조회한다.
//!
//! **주의:** 세 KR 심볼은 2026-06-04 현재 `symbolStatus=="PREVIEW"`(상장 예고)다.
//! 따라서 `/tickers`엔 없고 `/depth` 호가창은 비어 있다. 마크가·펀딩은
//! `/funding_rate`로 조회한다. 호가가 비는 것은 실패가 아니라 PREVIEW의 정상 상태다.

use korea_stock::global::bitunix::{BitunixClient, BitunixConfig, KR_SYMBOLS};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    // 시세는 키 없이 조회 가능.
    let client = BitunixClient::new(BitunixConfig::public())?;
    let md = client.market();

    for symbol in KR_SYMBOLS {
        // PREVIEW 심볼도 마크가·펀딩을 응답하는 유일한 엔드포인트.
        let fr = md.funding_rate(symbol).await?;
        // 거래쌍 메타로 상장 상태 동반 출력(PREVIEW/OPEN).
        let pair = md.trading_pair(symbol).await?;
        let book = md.depth(symbol, Some(5)).await?;

        println!(
            "[{symbol}] status={} mark={} 최근={} funding={} 다음펀딩(ms)={}",
            pair.symbol_status, fr.mark_price, fr.last_price, fr.funding_rate, fr.next_funding_time
        );
        println!(
            "  매도1 {} / 매수1 {}  (PREVIEW 심볼은 호가가 비어 있을 수 있음)",
            book.asks.first().map(|e| e[0].as_str()).unwrap_or("-"),
            book.bids.first().map(|e| e[0].as_str()).unwrap_or("-"),
        );
    }

    Ok(())
}
