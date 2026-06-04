//! 시세 도메인 (키 불필요) — 마켓정보·마크가·펀딩·호가·캔들.
//!
//! Pacifica 공개 GET 엔드포인트(`/api/v1/*`). 가격·수량은 정밀도 보존 위해 String.
//! 필드명은 2026-06-04 라이브 응답으로 확정했다.

use serde::Deserialize;

use crate::global::pacifica::client::{ApiCall, PacificaClient};
use crate::global::pacifica::error::{PacificaError, Result};

/// 마켓 정보 1건 (`GET /api/v1/info` → data[]).
///
/// tick/lot/레버리지·펀딩 메타. 라이브 필드(2026-06-04): `symbol`·`tick_size`·`lot_size`
/// ·`max_leverage`·`funding_rate`·`next_funding_rate`·`instrument_type`·`base_asset` 등.
#[derive(Debug, Clone, Deserialize)]
pub struct MarketInfo {
    pub symbol: String,
    /// 호가 단위.
    pub tick_size: String,
    /// 수량 단위.
    pub lot_size: String,
    pub max_leverage: u32,
    /// 격리마진 전용 여부.
    #[serde(default)]
    pub isolated_only: bool,
    pub min_order_size: String,
    pub max_order_size: String,
    /// 현재 펀딩비율.
    pub funding_rate: String,
    /// 다음 펀딩비율(예측).
    pub next_funding_rate: String,
    /// 상장 시각(epoch ms).
    pub created_at: i64,
    /// 상품 유형(예 "perpetual").
    pub instrument_type: String,
    pub base_asset: String,
}

/// 가격 지표 1건 (`GET /api/v1/info/prices` → data[]).
///
/// 마크가·중간가·오라클가·펀딩·미결제약정·24h거래량. 라이브 필드(2026-06-04):
/// `mark`·`mid`·`oracle`·`funding`·`next_funding`·`open_interest`·`volume_24h`·
/// `yesterday_price`·`symbol`·`timestamp`.
#[derive(Debug, Clone, Deserialize)]
pub struct PriceInfo {
    pub symbol: String,
    /// 마크가 (청산·손익 기준).
    pub mark: String,
    /// 중간가 (best bid/ask 중앙).
    pub mid: String,
    /// 오라클가 (현물 인덱스).
    pub oracle: String,
    /// 현재 펀딩비율.
    pub funding: String,
    /// 다음 펀딩비율(예측).
    pub next_funding: String,
    /// 미결제약정.
    pub open_interest: String,
    /// 24시간 거래량.
    #[serde(rename = "volume_24h")]
    pub volume_24h: String,
    /// 전일 종가("-1"이면 미산정).
    pub yesterday_price: String,
    /// 시세 시각(epoch ms).
    pub timestamp: i64,
}

/// 호가 한 단계 (`{p, a, n}`). price·amount·num-orders.
#[derive(Debug, Clone, Deserialize)]
pub struct Level {
    /// 가격.
    #[serde(rename = "p")]
    pub price: String,
    /// 잔량(수량).
    #[serde(rename = "a")]
    pub amount: String,
    /// 해당 가격 주문 수.
    #[serde(rename = "n")]
    pub num_orders: u32,
}

/// 호가창 원본 (`GET /api/v1/book?symbol=...` → data). `l[0]`=매수, `l[1]`=매도.
#[derive(Debug, Clone, Deserialize)]
struct OrderBookRaw {
    #[serde(rename = "s")]
    symbol: String,
    /// `[bids, asks]`. bids 내림차순, asks 오름차순.
    l: Vec<Vec<Level>>,
    #[serde(rename = "t")]
    timestamp: i64,
}

/// 호가창 (정리됨). bids 내림차순, asks 오름차순.
#[derive(Debug, Clone)]
pub struct OrderBook {
    pub symbol: String,
    /// 매수호가(높은 가격순).
    pub bids: Vec<Level>,
    /// 매도호가(낮은 가격순).
    pub asks: Vec<Level>,
    /// 시각(epoch ms).
    pub timestamp: i64,
}

/// 캔들(봉) 1건 (`GET /api/v1/kline` → data[]).
///
/// 라이브 필드(2026-06-04): `t`(시작ms)·`T`(종료ms)·`s`·`i`(간격)·`o`·`c`·`h`·`l`·
/// `v`(거래량)·`n`(체결수).
#[derive(Debug, Clone, Deserialize)]
pub struct Kline {
    /// 봉 시작 시각(epoch ms).
    #[serde(rename = "t")]
    pub open_time: i64,
    /// 봉 종료 시각(epoch ms).
    #[serde(rename = "T")]
    pub close_time: i64,
    #[serde(rename = "s")]
    pub symbol: String,
    /// 간격 코드(예 "1h").
    #[serde(rename = "i")]
    pub interval: String,
    #[serde(rename = "o")]
    pub open: String,
    #[serde(rename = "c")]
    pub close: String,
    #[serde(rename = "h")]
    pub high: String,
    #[serde(rename = "l")]
    pub low: String,
    /// 거래량.
    #[serde(rename = "v")]
    pub volume: String,
    /// 체결 건수.
    #[serde(rename = "n")]
    pub trades: i64,
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a PacificaClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a PacificaClient) -> Self {
        Self { client }
    }

    /// 전체 마켓 정보 목록 (`GET /api/v1/info`). KR 종목만 보려면
    /// [`crate::global::pacifica::KR_SYMBOLS`]로 필터.
    pub async fn markets(&self) -> Result<Vec<MarketInfo>> {
        self.client
            .call(ApiCall::get("/api/v1/info", vec![]))
            .await?
            .parse()
    }

    /// 단일 마켓 정보 (없으면 [`PacificaError::Decode`]).
    pub async fn market_info(&self, symbol: &str) -> Result<MarketInfo> {
        self.markets()
            .await?
            .into_iter()
            .find(|m| m.symbol == symbol)
            .ok_or_else(|| PacificaError::Decode(format!("symbol not found: {symbol}")))
    }

    /// 전체 가격 지표 목록 (`GET /api/v1/info/prices`).
    pub async fn prices(&self) -> Result<Vec<PriceInfo>> {
        self.client
            .call(ApiCall::get("/api/v1/info/prices", vec![]))
            .await?
            .parse()
    }

    /// 단일 심볼 가격 지표(마크가·펀딩 등). 없으면 [`PacificaError::Decode`].
    pub async fn price(&self, symbol: &str) -> Result<PriceInfo> {
        self.prices()
            .await?
            .into_iter()
            .find(|p| p.symbol == symbol)
            .ok_or_else(|| PacificaError::Decode(format!("price not found: {symbol}")))
    }

    /// 호가창 (`GET /api/v1/book?symbol=...`).
    pub async fn order_book(&self, symbol: &str) -> Result<OrderBook> {
        let raw: OrderBookRaw = self
            .client
            .call(ApiCall::get(
                "/api/v1/book",
                vec![("symbol".into(), symbol.into())],
            ))
            .await?
            .parse()?;
        let mut levels = raw.l.into_iter();
        let bids = levels.next().unwrap_or_default();
        let asks = levels.next().unwrap_or_default();
        Ok(OrderBook {
            symbol: raw.symbol,
            bids,
            asks,
            timestamp: raw.timestamp,
        })
    }

    /// 캔들 조회 (`GET /api/v1/kline`). `interval` 예 "1m"/"1h"/"1d", `[start_time,end_time]`은 epoch ms.
    pub async fn klines(
        &self,
        symbol: &str,
        interval: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<Kline>> {
        self.client
            .call(ApiCall::get(
                "/api/v1/kline",
                vec![
                    ("symbol".into(), symbol.into()),
                    ("interval".into(), interval.into()),
                    ("start_time".into(), start_time.to_string()),
                    ("end_time".into(), end_time.to_string()),
                ],
            ))
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn market_info_parses_live_shape() {
        let v = serde_json::json!({
            "symbol": "SAMSUNG", "tick_size": "0.01", "min_tick": "0",
            "max_tick": "10000000", "lot_size": "0.001", "max_leverage": 10,
            "isolated_only": false, "min_order_size": "10", "max_order_size": "1000000",
            "funding_rate": "0.0000125", "next_funding_rate": "0.0000125",
            "created_at": 1780468404816i64, "instrument_type": "perpetual",
            "base_asset": "SAMSUNG"
        });
        let m: MarketInfo = serde_json::from_value(v).unwrap();
        assert_eq!(m.symbol, "SAMSUNG");
        assert_eq!(m.tick_size, "0.01");
        assert_eq!(m.max_leverage, 10);
        assert_eq!(m.instrument_type, "perpetual");
    }

    #[test]
    fn price_info_parses_live_shape() {
        let v = serde_json::json!({
            "funding": "0.00045785", "mark": "1511.861905", "mid": "1516.7",
            "next_funding": "0.00059294", "open_interest": "49.4714",
            "oracle": "1503.375942", "symbol": "SKHYNIX", "timestamp": 1780550239078i64,
            "volume_24h": "183649.85334", "yesterday_price": "-1"
        });
        let p: PriceInfo = serde_json::from_value(v).unwrap();
        assert_eq!(p.symbol, "SKHYNIX");
        assert_eq!(p.mark, "1511.861905");
        assert_eq!(p.funding, "0.00045785");
        assert_eq!(p.volume_24h, "183649.85334");
    }

    #[test]
    fn order_book_splits_bids_asks() {
        let v = serde_json::json!({
            "s": "SAMSUNG",
            "l": [
                [{"p": "233.2", "a": "0.051", "n": 1}],
                [{"p": "234.26", "a": "1.099", "n": 1}]
            ],
            "t": 1780550247014i64
        });
        let raw: OrderBookRaw = serde_json::from_value(v).unwrap();
        let mut it = raw.l.into_iter();
        let bids = it.next().unwrap();
        let asks = it.next().unwrap();
        assert_eq!(bids[0].price, "233.2");
        assert_eq!(bids[0].amount, "0.051");
        assert_eq!(asks[0].price, "234.26");
        assert_eq!(asks[0].num_orders, 1);
    }

    #[test]
    fn kline_parses_renamed_fields() {
        let v = serde_json::json!({
            "t": 1780466400000i64, "T": 1780470000000i64, "s": "SAMSUNG", "i": "1h",
            "o": "255.37", "c": "253.93", "h": "255.37", "l": "250.43",
            "v": "0.684", "n": 127
        });
        let k: Kline = serde_json::from_value(v).unwrap();
        assert_eq!(k.open, "255.37");
        assert_eq!(k.close, "253.93");
        assert_eq!(k.close_time, 1780470000000);
        assert_eq!(k.trades, 127);
    }
}
