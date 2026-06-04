//! 거래·계좌 도메인 — 주문/취소는 `POST /exchange`(EIP-712 서명), 포지션/잔고는
//! `POST /info clearinghouseState`(서명 불필요, 주소 기반 공개 조회).
//!
//! **실거래 경고:** `place`/`cancel`은 실제 자금을 움직인다. 개발·검증은 반드시
//! [`crate::global::hyperliquid::HyperliquidConfig::testnet`]에서 한다(단, HIP-3 `xyz` dex가
//! 테스트넷에 존재하는지 먼저 확인).
//!
//! ## wire 필드 순서는 서명의 일부
//! 주문/취소 action struct의 **필드 선언 순서**가 msgpack 바이트를, 따라서 서명을
//! 결정한다. Python SDK 순서를 그대로 따른다:
//! - OrderWire: `a, b, p, s, r, t, (c)`
//! - OrderAction: `type, orders, grouping, (builder)`
//! - CancelAction: `type, cancels[{a, o}]`
//!
//! 비음수 정수(`a`, `o`)는 반드시 `u64` — `i64`는 signed msgpack 마커를 내보내
//! Python(unsigned)과 어긋나 서명이 무효가 된다. (client.rs 주석 참조.)

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::global::hyperliquid::client::{parse_json, HyperliquidClient};
use crate::global::hyperliquid::error::{HyperliquidError, Result};

/// 체결 조건 (LIMIT tif).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tif {
    /// 취소 전까지 유효.
    Gtc,
    /// 즉시 체결·잔량 취소.
    Ioc,
    /// Add-Liquidity-Only(post-only). 메이커만, 즉시 체결 시 취소.
    Alo,
}

impl Tif {
    fn as_str(self) -> &'static str {
        match self {
            Tif::Gtc => "Gtc",
            Tif::Ioc => "Ioc",
            Tif::Alo => "Alo",
        }
    }
}

// ── wire 직렬화 타입 (서명 대상). 필드 순서 고정. ──────────────────────────

/// limit order의 `t` 필드 wire: `{"limit":{"tif":"Gtc"}}`.
#[derive(Debug, Clone, Serialize)]
struct LimitWire {
    tif: &'static str,
}

#[derive(Debug, Clone, Serialize)]
struct OrderTypeWire {
    limit: LimitWire,
}

/// 단일 주문 wire. 필드 순서 `a,b,p,s,r,t,(c)` — Python `order_request_to_order_wire`.
#[derive(Debug, Clone, Serialize)]
struct OrderWire {
    /// asset 정수 id (HIP-3는 110000+universe_index). **u64 필수.**
    a: u64,
    /// is_buy.
    b: bool,
    /// limit price wire 문자열.
    p: String,
    /// size wire 문자열.
    s: String,
    /// reduce_only.
    r: bool,
    /// order type.
    t: OrderTypeWire,
    /// cloid (선택). None이면 직렬화에서 생략 → 서명 해시에도 미포함.
    #[serde(skip_serializing_if = "Option::is_none")]
    c: Option<String>,
}

/// 주문 action. 필드 순서 `type,orders,grouping` — Python `order_wires_to_order_action`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct OrderAction {
    #[serde(rename = "type")]
    kind: &'static str,
    orders: Vec<OrderWire>,
    grouping: &'static str,
}

/// 취소 1건 wire `{a, o}`. **a/o 모두 u64.**
#[derive(Debug, Clone, Serialize)]
struct CancelWire {
    a: u64,
    o: u64,
}

/// 취소 action `{type:"cancel", cancels:[{a,o}]}`.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct CancelAction {
    #[serde(rename = "type")]
    kind: &'static str,
    cancels: Vec<CancelWire>,
}

// ── 호출자용 요청 타입 ─────────────────────────────────────────────────────

/// 주문 요청. 가격·수량은 정밀도 보존 위해 String (심볼 `szDecimals`/틱에 맞춰
/// 호출부가 포맷; 불필요한 trailing zero는 wire에서 제거되어야 한다).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    /// asset 정수 id. KR 종목은 [`crate::global::hyperliquid::SAMSUNG_ASSET`] 등.
    pub asset: u64,
    pub is_buy: bool,
    /// 지정가 (wire 문자열). 시장가는 IOC + 멀리 떨어진 가드 가격으로 표현한다
    /// (Hyperliquid엔 별도 MARKET 타입이 없다).
    pub limit_px: String,
    /// 수량 (wire 문자열).
    pub sz: String,
    pub tif: Tif,
    /// 포지션 축소 전용.
    pub reduce_only: bool,
    /// 사용자 지정 cloid("0x"+32hex, 16바이트). 멱등 추적.
    pub cloid: Option<String>,
}

impl OrderRequest {
    /// 지정가 주문 (기본 GTC, reduce_only=false).
    pub fn limit(
        asset: u64,
        is_buy: bool,
        limit_px: impl Into<String>,
        sz: impl Into<String>,
        tif: Tif,
    ) -> Self {
        Self {
            asset,
            is_buy,
            limit_px: limit_px.into(),
            sz: sz.into(),
            tif,
            reduce_only: false,
            cloid: None,
        }
    }

    /// 포지션 축소 전용 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = v;
        self
    }

    /// cloid 부여 (builder).
    pub fn cloid(mut self, id: impl Into<String>) -> Self {
        self.cloid = Some(id.into());
        self
    }

    fn to_action(&self) -> OrderAction {
        OrderAction {
            kind: "order",
            orders: vec![OrderWire {
                a: self.asset,
                b: self.is_buy,
                p: self.limit_px.clone(),
                s: self.sz.clone(),
                r: self.reduce_only,
                t: OrderTypeWire {
                    limit: LimitWire {
                        tif: self.tif.as_str(),
                    },
                },
                c: self.cloid.clone(),
            }],
            grouping: "na",
        }
    }
}

// ── 응답 타입 (clearinghouseState 등 — /info, 서명 불필요) ──────────────────

/// 계좌 마진 요약 (`clearinghouseState.marginSummary`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarginSummary {
    /// 계좌 가치 (USD).
    pub account_value: String,
    /// 명목 포지션 합.
    pub total_ntl_pos: String,
    pub total_raw_usd: String,
    /// 사용 마진.
    pub total_margin_used: String,
}

/// 레버리지 (`position.leverage`).
#[derive(Debug, Clone, Deserialize)]
pub struct Leverage {
    /// "isolated" / "cross".
    #[serde(rename = "type")]
    pub kind: String,
    pub value: u32,
}

/// 포지션 상세 (`assetPositions[].position`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionDetail {
    /// coin 식별자 (`"xyz:SMSN"`).
    pub coin: String,
    /// 부호 있는 수량 (signed; 음수 short). "0"이면 무포지션.
    pub szi: String,
    pub entry_px: Option<String>,
    pub position_value: String,
    pub unrealized_pnl: String,
    pub return_on_equity: String,
    pub leverage: Leverage,
    pub liquidation_px: Option<String>,
    pub margin_used: String,
}

/// 포지션 1건 (`assetPositions[]`).
#[derive(Debug, Clone, Deserialize)]
pub struct AssetPosition {
    /// "oneWay" 등.
    #[serde(rename = "type")]
    pub kind: String,
    pub position: PositionDetail,
}

/// 계좌 상태 (`clearinghouseState`). 잔고+포지션을 한 번에 담는다.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClearinghouseState {
    pub margin_summary: MarginSummary,
    pub cross_margin_summary: MarginSummary,
    /// 출금 가능액 (USD).
    pub withdrawable: String,
    pub asset_positions: Vec<AssetPosition>,
    /// 스냅샷 시각 (epoch ms).
    pub time: i64,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a HyperliquidClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a HyperliquidClient) -> Self {
        Self { client }
    }

    /// 주문 제출 (`POST /exchange`, 서명). **실체결.** 응답 바디를 검사해
    /// `{"status":"err"}` 또는 개별 주문 error면 [`HyperliquidError::Exchange`].
    ///
    /// **주의:** `req.asset`은 호출자가 책임지는 정수 id다. 하드코딩 상수
    /// ([`crate::global::hyperliquid::SAMSUNG_ASSET`] 등)는 universe 재정렬로 틀어질 수 있다 —
    /// 운영에서는 coin 이름으로 안전하게 거는 [`Trade::place_by_coin`]를 권장한다.
    pub async fn place(&self, req: &OrderRequest) -> Result<serde_json::Value> {
        let action = req.to_action();
        let resp = self.client.post_signed(&action).await?;
        check_exchange_response(&resp)?;
        Ok(resp)
    }

    /// coin 이름(예 [`crate::global::hyperliquid::SAMSUNG`])으로 지정가 주문.
    ///
    /// asset id를 **주문 직전 라이브 meta에서 재도출**([`crate::global::hyperliquid::market::Market::asset_id`])
    /// 해 하드코딩 상수의 wrong-instrument 위험을 차단한다. 운영 권장 진입점이다.
    /// 시장가가 필요하면 `tif=Ioc`에 멀리 떨어진 가드 가격을 넘긴다.
    pub async fn place_by_coin(
        &self,
        dex: &str,
        coin: &str,
        is_buy: bool,
        limit_px: impl Into<String>,
        sz: impl Into<String>,
        tif: Tif,
    ) -> Result<serde_json::Value> {
        let asset = crate::global::hyperliquid::market::Market::new(self.client)
            .asset_id(dex, coin)
            .await?;
        let req = OrderRequest::limit(asset, is_buy, limit_px, sz, tif);
        self.place(&req).await
    }

    /// 주문 취소 (`POST /exchange`, 서명). oid는 `place` 응답의 정수 주문 id.
    pub async fn cancel(&self, asset: u64, oid: u64) -> Result<serde_json::Value> {
        let action = CancelAction {
            kind: "cancel",
            cancels: vec![CancelWire { a: asset, o: oid }],
        };
        let resp = self.client.post_signed(&action).await?;
        check_exchange_response(&resp)?;
        Ok(resp)
    }

    /// 계좌 상태(잔고+포지션) 조회 (`POST /info clearinghouseState`, **서명 불필요**).
    ///
    /// `user`는 자금 소속 주소("0x"+40hex; agent 위임 시 마스터 주소). HIP-3 빌더
    /// 종목 포지션은 `dex`(예 [`crate::global::hyperliquid::DEX`])를 줘야 보인다.
    pub async fn clearinghouse_state(
        &self,
        user: &str,
        dex: &str,
    ) -> Result<ClearinghouseState> {
        let body = json!({ "type": "clearinghouseState", "user": user, "dex": dex });
        parse_json(self.client.post_json("/info", &body).await?)
    }

    /// 포지션 목록만 추출 (clearinghouseState의 편의 래퍼).
    pub async fn positions(&self, user: &str, dex: &str) -> Result<Vec<AssetPosition>> {
        Ok(self.clearinghouse_state(user, dex).await?.asset_positions)
    }

    /// 잔고 요약만 추출 (clearinghouseState의 편의 래퍼).
    pub async fn balance(&self, user: &str, dex: &str) -> Result<MarginSummary> {
        Ok(self.clearinghouse_state(user, dex).await?.margin_summary)
    }
}

/// `/exchange` 응답 바디 검사. HTTP 200이라도 바디 내부에 에러가 담길 수 있다:
/// - `{"status":"err","response":"<메시지>"}`
/// - `{"status":"ok","response":{"type":"order","data":{"statuses":[{"error":"..."}]}}}`
///
/// 위 두 경우를 [`HyperliquidError::Exchange`]로 변환한다. `call_once`에서 분리해
/// 단위 테스트가 검증 가능하도록 한다.
pub(crate) fn check_exchange_response(resp: &serde_json::Value) -> Result<()> {
    if resp.get("status").and_then(|v| v.as_str()) == Some("err") {
        let msg = resp
            .get("response")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown exchange error")
            .to_string();
        return Err(HyperliquidError::Exchange(msg));
    }
    // status=ok여도 개별 주문 status에 error가 섞일 수 있다.
    if let Some(statuses) = resp
        .get("response")
        .and_then(|r| r.get("data"))
        .and_then(|d| d.get("statuses"))
        .and_then(|s| s.as_array())
    {
        for st in statuses {
            if let Some(err) = st.get("error").and_then(|v| v.as_str()) {
                return Err(HyperliquidError::Exchange(err.to_string()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::global::hyperliquid::client::sign_l1_action;

    const TEST_KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";

    // ── 실제 wire 타입으로 주문 서명 벡터 검증 ──────────────────────────────
    // 케이스: ETH 지정가 GTC, asset=1, buy, sz=100, px=100, nonce=1.
    // 기대값은 **hyperliquid-python-sdk(설치본)의 sign_l1_action 실행 출력**으로
    // 직접 산출했다 — msgpack 바이트가 SDK와 동일(83a47479...)함을 교차검증한 뒤
    // 그 위에서 EIP-712+ECDSA 서명을 떴다. 이 테스트가 OrderWire/OrderAction의
    // 필드 순서·정수 인코딩이 SDK와 바이트 단위로 일치함을 보증한다(client.rs의
    // dummy 벡터로는 order wire 구조를 못 잡는다).
    //   action msgpack: 83a474797065a56f72646572a66f72646572739186a16101a162c3...
    #[test]
    fn order_action_signing_matches_sdk_vector() {
        let action = OrderRequest::limit(1, true, "100", "100", Tif::Gtc).to_action();

        // mainnet (source="a")
        let sig = sign_l1_action(TEST_KEY, &action, 1, None, true).unwrap();
        assert_eq!(
            sig.r,
            "0x51c760ee9fec28151da82cf78a1419cc728d68dbd5c526fdcf93ac1259456804"
        );
        assert_eq!(
            sig.s,
            "0x23d7ca9a525150b15e9e7a4be494dff00178262b5d5419fe959f442d8c9456a9"
        );
        assert_eq!(sig.v, 27);

        // testnet (source="b")
        let sig_t = sign_l1_action(TEST_KEY, &action, 1, None, false).unwrap();
        assert_eq!(
            sig_t.r,
            "0x9ba6a978d67c22e3f311877011b4c0ca0e442386245150405c6a96e63a2c0433"
        );
        assert_eq!(
            sig_t.s,
            "0x6a263ac51985eea5971926c300b403677f75ba06ac8ef04bbe93b06b7bcee855"
        );
        assert_eq!(sig_t.v, 28);
    }

    // cancel은 공식 서명 벡터가 없어 wire **형태/필드 순서**를 구조적으로 검증한다.
    // a/o가 u64로 직렬화되는지(서명 안전 핵심)도 함께 확인.
    #[test]
    fn cancel_action_wire_shape() {
        let action = CancelAction {
            kind: "cancel",
            cancels: vec![CancelWire {
                a: 110034,
                o: 987654321,
            }],
        };
        let v = serde_json::to_value(&action).unwrap();
        assert_eq!(v["type"], "cancel");
        assert_eq!(v["cancels"][0]["a"], 110034);
        assert_eq!(v["cancels"][0]["o"], 987654321u64);
        // msgpack(named)로도 직렬화 가능해야 한다(서명 경로).
        assert!(rmp_serde::to_vec_named(&action).is_ok());
    }

    #[test]
    fn order_wire_omits_cloid_when_none() {
        let action = OrderRequest::limit(110034, true, "240.0", "1", Tif::Gtc).to_action();
        let v = serde_json::to_value(&action.orders[0]).unwrap();
        // cloid 미지정 시 `c` 키 자체가 없어야 한다(서명 해시 일관성).
        assert!(v.get("c").is_none());
        assert_eq!(v["a"], 110034);
        assert_eq!(v["t"]["limit"]["tif"], "Gtc");
    }

    // 전송 바디(serde 직렬화)가 **서명한 msgpack 선언순서**(a,b,p,s,r,t)를 보존하는지
    // 가드한다. `serde_json::to_string`은 serde 직렬화라 선언순서 유지 — 키를 정렬하는
    // `to_value`(BTreeMap)와 달리 r↔s가 뒤집히지 않는다. 필드 재배열 회귀를 잡는다.
    #[test]
    fn order_wire_json_preserves_signed_field_order() {
        let action = OrderRequest::limit(110034, true, "240.0", "1", Tif::Gtc).to_action();
        let wire = serde_json::to_string(&action.orders[0]).unwrap();
        assert_eq!(
            wire,
            r#"{"a":110034,"b":true,"p":"240.0","s":"1","r":false,"t":{"limit":{"tif":"Gtc"}}}"#
        );
    }

    #[test]
    fn order_wire_includes_cloid_when_set() {
        let action = OrderRequest::limit(110034, false, "240.0", "1", Tif::Alo)
            .reduce_only(true)
            .cloid("0x00000000000000000000000000000001")
            .to_action();
        let v = serde_json::to_value(&action.orders[0]).unwrap();
        assert_eq!(v["c"], "0x00000000000000000000000000000001");
        assert_eq!(v["r"], true);
        assert_eq!(v["t"]["limit"]["tif"], "Alo");
    }

    #[test]
    fn exchange_err_status_maps_to_error() {
        let resp = serde_json::json!({
            "status": "err",
            "response": "Insufficient margin to place order."
        });
        let err = check_exchange_response(&resp).unwrap_err();
        assert!(matches!(err, HyperliquidError::Exchange(ref m) if m.contains("Insufficient")));
    }

    #[test]
    fn exchange_per_order_error_maps_to_error() {
        let resp = serde_json::json!({
            "status": "ok",
            "response": {
                "type": "order",
                "data": { "statuses": [{ "error": "Order price cannot be more than 95% away from the reference price" }] }
            }
        });
        let err = check_exchange_response(&resp).unwrap_err();
        assert!(matches!(err, HyperliquidError::Exchange(_)));
    }

    #[test]
    fn exchange_ok_resting_order_passes() {
        let resp = serde_json::json!({
            "status": "ok",
            "response": {
                "type": "order",
                "data": { "statuses": [{ "resting": { "oid": 123456789u64 } }] }
            }
        });
        assert!(check_exchange_response(&resp).is_ok());
    }

    #[test]
    fn clearinghouse_state_parses() {
        // 실제 `clearinghouseState` 응답 형태(포지션 1건 포함).
        let v = serde_json::json!({
            "marginSummary": {
                "accountValue": "1000.0",
                "totalNtlPos": "243.06",
                "totalRawUsd": "757.0",
                "totalMarginUsed": "24.3"
            },
            "crossMarginSummary": {
                "accountValue": "1000.0",
                "totalNtlPos": "0.0",
                "totalRawUsd": "1000.0",
                "totalMarginUsed": "0.0"
            },
            "crossMaintenanceMarginUsed": "0.0",
            "withdrawable": "975.7",
            "assetPositions": [{
                "type": "oneWay",
                "position": {
                    "coin": "xyz:SMSN",
                    "szi": "1.0",
                    "entryPx": "240.0",
                    "positionValue": "243.06",
                    "unrealizedPnl": "3.06",
                    "returnOnEquity": "0.125",
                    "leverage": { "type": "isolated", "value": 10 },
                    "liquidationPx": "120.0",
                    "marginUsed": "24.3",
                    "maxLeverage": 10
                }
            }],
            "time": 1780409662070i64
        });
        let s: ClearinghouseState = serde_json::from_value(v).unwrap();
        assert_eq!(s.margin_summary.account_value, "1000.0");
        assert_eq!(s.withdrawable, "975.7");
        let pos = &s.asset_positions[0];
        assert_eq!(pos.position.coin, "xyz:SMSN");
        assert_eq!(pos.position.szi, "1.0");
        assert_eq!(pos.position.leverage.kind, "isolated");
        assert_eq!(pos.position.leverage.value, 10);
        assert_eq!(pos.position.unrealized_pnl, "3.06");
    }
}
