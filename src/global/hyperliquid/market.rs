//! 시세 도메인 (키 불필요) — `POST /info`. 마크가·펀딩·호가·캔들·메타.
//!
//! 모든 호출은 POST + JSON 바디다(`{"type": "...", ...}`). 가격·수량·펀딩비는
//! 정밀도 보존 위해 String. HIP-3 빌더 perp는 coin을 `"{dex}:{ticker}"`(예
//! `"xyz:SMSN"`)로 참조하고, `meta`/`metaAndAssetCtxs`는 `dex` 파라미터로 빌더
//! 네임스페이스를 선택한다.

use serde::Deserialize;
use serde_json::json;

use crate::global::hyperliquid::client::{parse_json, HyperliquidClient};
use crate::global::hyperliquid::error::{HyperliquidError, Result};

/// 빌더 dex의 정수 asset id 오프셋(`110000 + perp_dex_index*10000`).
///
/// Trade.xyz(`"xyz"`)는 `perpDexs()[1:]`의 i=0이라 110000. 다른 dex는 미지원이며
/// 잘못된 id로 엉뚱한 종목을 거래하는 사고를 막기 위해 추측 대신 에러를 낸다.
fn builder_asset_base(dex: &str) -> Result<u64> {
    match dex {
        "xyz" => Ok(110_000),
        other => Err(HyperliquidError::Decode(format!(
            "unknown builder dex '{other}': asset-id base not known; derive perpDexs index first"
        ))),
    }
}

/// `meta`의 universe에서 coin의 거래용 정수 asset id를 **런타임 재도출**한다.
///
/// universe 재정렬 시 하드코딩 상수([`crate::global::hyperliquid::SAMSUNG_ASSET`] 등)가
/// 엉뚱한 종목을 가리키는 wrong-instrument 사고를 차단한다. 순수 함수로 분리해
/// 네트워크 없이 단위 검증한다.
pub fn resolve_asset_id(meta: &Meta, dex: &str, coin: &str) -> Result<u64> {
    let idx = meta
        .universe
        .iter()
        .position(|a| a.name == coin)
        .ok_or_else(|| {
            HyperliquidError::Decode(format!("coin '{coin}' not found in dex '{dex}' universe"))
        })?;
    Ok(builder_asset_base(dex)? + idx as u64)
}

/// `meta.universe[]` 1건 — perp 자산 메타.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetMeta {
    /// coin 식별자. HIP-3는 `"xyz:SMSN"` 형태.
    pub name: String,
    /// 수량 소수 자릿수 (주문 `sz` 포맷 기준).
    pub sz_decimals: u32,
    /// 최대 레버리지.
    pub max_leverage: u32,
    /// 격리마진 전용 여부 (HIP-3 KR 종목은 true).
    #[serde(default)]
    pub only_isolated: bool,
}

/// `meta` 응답 — universe 배열.
#[derive(Debug, Clone, Deserialize)]
pub struct Meta {
    pub universe: Vec<AssetMeta>,
}

/// `metaAndAssetCtxs[1][]` 1건 — 자산 실시간 컨텍스트(마크/오라클/펀딩 등).
///
/// 배열 인덱스가 `meta.universe`의 인덱스와 1:1 대응한다(이름 필드는 없음).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetCtx {
    /// 현재 펀딩비율 (1시간 단위, 예: "0.0000056726").
    pub funding: String,
    /// 미결제약정 (계약 수).
    pub open_interest: String,
    /// 마크가.
    pub mark_px: String,
    /// 중간가 (없을 수 있음).
    #[serde(default)]
    pub mid_px: Option<String>,
    /// 오라클가 (KRX 원화가 × USD/KRW).
    pub oracle_px: String,
    /// 전일 종가.
    pub prev_day_px: String,
    /// 24h 명목 거래대금 (USD).
    pub day_ntl_vlm: String,
    /// 프리미엄 (마크가-오라클가 기반).
    #[serde(default)]
    pub premium: Option<String>,
}

/// `meta`(universe) + `assetCtxs`를 인덱스로 짝지은 한 종목.
#[derive(Debug, Clone)]
pub struct AssetSnapshot {
    pub meta: AssetMeta,
    pub ctx: AssetCtx,
}

/// `fundingHistory` 1건.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingHistory {
    pub coin: String,
    /// 해당 회차 펀딩비율.
    pub funding_rate: String,
    /// 프리미엄.
    pub premium: String,
    /// 정산 시각 (epoch ms).
    pub time: i64,
}

/// 호가 1레벨 (`l2Book.levels[side][]`).
#[derive(Debug, Clone, Deserialize)]
pub struct BookLevel {
    /// 가격.
    pub px: String,
    /// 잔량.
    pub sz: String,
    /// 해당 가격의 주문 수.
    pub n: u32,
}

/// 호가창 (`l2Book`). `levels[0]`=매수(높은가순), `levels[1]`=매도(낮은가순).
#[derive(Debug, Clone, Deserialize)]
pub struct L2Book {
    pub coin: String,
    /// 스냅샷 시각 (epoch ms).
    pub time: i64,
    /// `[bids, asks]` 2원소.
    pub levels: Vec<Vec<BookLevel>>,
}

impl L2Book {
    /// 매수 호가 (높은 가격순).
    pub fn bids(&self) -> &[BookLevel] {
        self.levels.first().map(Vec::as_slice).unwrap_or(&[])
    }
    /// 매도 호가 (낮은 가격순).
    pub fn asks(&self) -> &[BookLevel] {
        self.levels.get(1).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// 캔들(봉) 1건 (`candleSnapshot[]`). 이종 키 약어를 명시 필드로 받는다.
#[derive(Debug, Clone, Deserialize)]
pub struct Candle {
    /// 봉 시작 시각 (epoch ms).
    #[serde(rename = "t")]
    pub open_time: i64,
    /// 봉 종료 시각 (epoch ms).
    #[serde(rename = "T")]
    pub close_time: i64,
    /// coin 식별자.
    #[serde(rename = "s")]
    pub coin: String,
    /// 봉 단위 ("1h" 등).
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
    /// 거래량 (base).
    #[serde(rename = "v")]
    pub volume: String,
    /// 체결 수.
    #[serde(rename = "n")]
    pub trades: u32,
}

/// 캔들 봉 단위. 값을 우리가 통제하므로 enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CandleInterval {
    Min1,
    Min5,
    Min15,
    Hour1,
    Hour4,
    Day1,
}

impl CandleInterval {
    fn code(self) -> &'static str {
        match self {
            CandleInterval::Min1 => "1m",
            CandleInterval::Min5 => "5m",
            CandleInterval::Min15 => "15m",
            CandleInterval::Hour1 => "1h",
            CandleInterval::Hour4 => "4h",
            CandleInterval::Day1 => "1d",
        }
    }
}

/// 시세 도메인 액세서. `client.market()`으로 획득.
pub struct Market<'a> {
    client: &'a HyperliquidClient,
}

impl<'a> Market<'a> {
    pub(crate) fn new(client: &'a HyperliquidClient) -> Self {
        Self { client }
    }

    /// perp 메타(universe). `dex`는 빌더 네임스페이스(예 [`crate::global::hyperliquid::DEX`]).
    /// 기본 perp dex를 보려면 빈 문자열.
    pub async fn meta(&self, dex: &str) -> Result<Meta> {
        let body = json!({ "type": "meta", "dex": dex });
        parse_json(self.client.post_json("/info", &body).await?)
    }

    /// coin의 거래용 정수 asset id를 **라이브 meta에서 재도출**한다.
    ///
    /// 하드코딩 상수 대신 이 메서드로 주문 직전 asset id를 산출하면 universe 재정렬에
    /// 의한 wrong-instrument 사고를 막는다. [`crate::global::hyperliquid::trade::Trade::place_by_coin`]가
    /// 내부적으로 호출한다.
    pub async fn asset_id(&self, dex: &str, coin: &str) -> Result<u64> {
        let meta = self.meta(dex).await?;
        resolve_asset_id(&meta, dex, coin)
    }

    /// meta + 자산 컨텍스트(마크/오라클/펀딩)를 인덱스로 짝지어 반환.
    ///
    /// `metaAndAssetCtxs`는 `[meta, ctxs]` 2원소 배열이며 둘은 universe 순서로
    /// 1:1 대응한다. 이름→스냅샷 조회는 결과를 순회한다.
    pub async fn meta_and_asset_ctxs(&self, dex: &str) -> Result<Vec<AssetSnapshot>> {
        let body = json!({ "type": "metaAndAssetCtxs", "dex": dex });
        let raw = self.client.post_json("/info", &body).await?;
        let (meta, ctxs): (Meta, Vec<AssetCtx>) = parse_json(raw)?;
        Ok(meta
            .universe
            .into_iter()
            .zip(ctxs)
            .map(|(meta, ctx)| AssetSnapshot { meta, ctx })
            .collect())
    }

    /// 단일 coin의 마크/오라클/펀딩 스냅샷. (내부적으로 dex 전체를 받아 필터.)
    pub async fn asset_snapshot(&self, dex: &str, coin: &str) -> Result<Option<AssetSnapshot>> {
        Ok(self
            .meta_and_asset_ctxs(dex)
            .await?
            .into_iter()
            .find(|s| s.meta.name == coin))
    }

    /// 전체 mid 가격 맵 (`allMids`). 키=coin 식별자, 값=mid 가격 문자열.
    pub async fn all_mids(&self, dex: &str) -> Result<std::collections::HashMap<String, String>> {
        let body = json!({ "type": "allMids", "dex": dex });
        parse_json(self.client.post_json("/info", &body).await?)
    }

    /// 호가창 (`l2Book`). coin은 HIP-3 prefix 포함(`"xyz:SMSN"`).
    pub async fn l2_book(&self, coin: &str) -> Result<L2Book> {
        let body = json!({ "type": "l2Book", "coin": coin });
        parse_json(self.client.post_json("/info", &body).await?)
    }

    /// 펀딩비 이력. `[start, end)` epoch ms. end=None이면 현재까지.
    pub async fn funding_history(
        &self,
        coin: &str,
        start_time: i64,
        end_time: Option<i64>,
    ) -> Result<Vec<FundingHistory>> {
        let mut body = json!({ "type": "fundingHistory", "coin": coin, "startTime": start_time });
        if let Some(end) = end_time {
            body["endTime"] = json!(end);
        }
        parse_json(self.client.post_json("/info", &body).await?)
    }

    /// 캔들 조회 (`candleSnapshot`). `[start, end]` epoch ms.
    ///
    /// **주의:** KR 종목 오라클은 KRX 장중(09:00–15:30 KST)에만 갱신된다 — 장외
    /// 구간은 빈 배열일 수 있다.
    pub async fn candles(
        &self,
        coin: &str,
        interval: CandleInterval,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<Candle>> {
        let body = json!({
            "type": "candleSnapshot",
            "req": {
                "coin": coin,
                "interval": interval.code(),
                "startTime": start_time,
                "endTime": end_time,
            }
        });
        parse_json(self.client.post_json("/info", &body).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interval_codes() {
        assert_eq!(CandleInterval::Min1.code(), "1m");
        assert_eq!(CandleInterval::Hour1.code(), "1h");
        assert_eq!(CandleInterval::Day1.code(), "1d");
    }

    #[test]
    fn resolve_asset_id_derives_from_universe_index() {
        // universe 인덱스 2 → asset 110002 (xyz base 110000 + idx). 재정렬 안전.
        let meta: Meta = serde_json::from_value(serde_json::json!({
            "universe": [
                { "szDecimals": 3, "name": "xyz:SKHX", "maxLeverage": 10 },
                { "szDecimals": 3, "name": "xyz:HYUNDAI", "maxLeverage": 10 },
                { "szDecimals": 3, "name": "xyz:SMSN", "maxLeverage": 10 }
            ]
        }))
        .unwrap();
        assert_eq!(resolve_asset_id(&meta, "xyz", "xyz:SMSN").unwrap(), 110_002);
        assert_eq!(resolve_asset_id(&meta, "xyz", "xyz:SKHX").unwrap(), 110_000);
        // 미존재 coin → Decode 에러 (엉뚱한 id 산출 금지).
        assert!(resolve_asset_id(&meta, "xyz", "xyz:NOPE").is_err());
        // 미지원 dex base → 에러 (추측 금지).
        assert!(resolve_asset_id(&meta, "other", "xyz:SMSN").is_err());
    }

    #[test]
    fn meta_universe_parses() {
        // 실제 `meta` (dex="xyz") universe[34] 응답.
        let v = serde_json::json!({
            "universe": [{
                "szDecimals": 3,
                "name": "xyz:SMSN",
                "maxLeverage": 10,
                "marginTableId": 10,
                "onlyIsolated": true,
                "marginMode": "noCross"
            }]
        });
        let m: Meta = serde_json::from_value(v).unwrap();
        assert_eq!(m.universe[0].name, "xyz:SMSN");
        assert_eq!(m.universe[0].sz_decimals, 3);
        assert_eq!(m.universe[0].max_leverage, 10);
        assert!(m.universe[0].only_isolated);
    }

    #[test]
    fn meta_and_asset_ctxs_tuple_parses() {
        // 실제 `metaAndAssetCtxs` [meta, ctxs] 2원소 배열 형태.
        let v = serde_json::json!([
            { "universe": [{ "szDecimals": 3, "name": "xyz:SMSN", "maxLeverage": 10 }] },
            [{
                "funding": "0.0000056726",
                "openInterest": "54345.544",
                "prevDayPx": "229.61",
                "dayNtlVlm": "16365785.66",
                "premium": "0.0000555418",
                "oraclePx": "243.06",
                "markPx": "243.06",
                "midPx": "243.09",
                "impactPxs": ["242.277", "243.87"],
                "dayBaseVlm": "68469.88"
            }]
        ]);
        let (meta, ctxs): (Meta, Vec<AssetCtx>) = serde_json::from_value(v).unwrap();
        assert_eq!(meta.universe[0].name, "xyz:SMSN");
        assert_eq!(ctxs[0].mark_px, "243.06");
        assert_eq!(ctxs[0].oracle_px, "243.06");
        assert_eq!(ctxs[0].funding, "0.0000056726");
        assert_eq!(ctxs[0].mid_px.as_deref(), Some("243.09"));
    }

    #[test]
    fn funding_history_parses() {
        let v = serde_json::json!({
            "coin": "xyz:SMSN",
            "fundingRate": "0.00000625",
            "premium": "0.0001358705",
            "time": 1771549200190i64
        });
        let f: FundingHistory = serde_json::from_value(v).unwrap();
        assert_eq!(f.coin, "xyz:SMSN");
        assert_eq!(f.funding_rate, "0.00000625");
        assert_eq!(f.time, 1771549200190);
    }

    #[test]
    fn l2_book_parses_bids_asks() {
        // 실제 `l2Book` 응답: levels = [bids, asks].
        let v = serde_json::json!({
            "coin": "xyz:SMSN",
            "time": 1780409662070i64,
            "levels": [
                [{ "px": "243.0", "sz": "0.664", "n": 1 }, { "px": "242.99", "sz": "1.46", "n": 1 }],
                [{ "px": "243.1", "sz": "0.5", "n": 1 }]
            ]
        });
        let b: L2Book = serde_json::from_value(v).unwrap();
        assert_eq!(b.coin, "xyz:SMSN");
        assert_eq!(b.bids()[0].px, "243.0");
        assert_eq!(b.bids()[0].sz, "0.664");
        assert_eq!(b.asks()[0].px, "243.1");
        assert_eq!(b.asks()[0].n, 1);
    }

    #[test]
    fn candle_short_keys_map_to_named() {
        // 실제 `candleSnapshot` 봉 형태 (이종 약어 키).
        let v = serde_json::json!({
            "t": 1780322400000i64,
            "T": 1780325999999i64,
            "s": "xyz:GOLD",
            "i": "1h",
            "o": "4451.1",
            "c": "4458.5",
            "h": "4466.2",
            "l": "4450.0",
            "v": "388.4767",
            "n": 691
        });
        let c: Candle = serde_json::from_value(v).unwrap();
        assert_eq!(c.open_time, 1780322400000);
        assert_eq!(c.close_time, 1780325999999);
        assert_eq!(c.open, "4451.1");
        assert_eq!(c.close, "4458.5");
        assert_eq!(c.trades, 691);
    }
}
