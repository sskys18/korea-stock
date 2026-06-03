//! 거래·계좌 도메인 — 주문/취소·포지션·잔고.
//!
//! **상태: data-only.** Lighter 주문 서명은 zk 친화 해시(poseidon)+schnorr 류 커스텀
//! 스킴이며, 공식 `lighter-python`·비공식 `lighter-rust` 모두 네이티브 Go
//! 바이너리(`lighter-go`)를 FFI로 호출해 서명한다. crates.io에 순수 Rust 구현이 없고
//! 이 세션에서 알고리즘을 공식 문서로 재현 검증할 수 없어, **서명을 날조하지 않는다**
//! (HARD RULE 3).
//!
//! 따라서 이 모듈은:
//! - 타입드 주문/취소 요청·응답 구조체와 호출 경로를 **완비**한다.
//! - 서명이 필요 없는 [`Trade::next_nonce`](`GET /api/v1/nextNonce`)는 **동작**한다.
//! - 실제 서명 제출([`Trade::place`]/[`Trade::cancel`])은
//!   [`LighterError::SignerUnavailable`]로 명확히 거부한다.
//!
//! 서명 구현 시 추가해야 할 것: `lighter-go`의 poseidon 해시 + schnorr 서명을 순수
//! Rust로 포팅하거나 검증된 FFI로 래핑 → `tx_info`(서명된 트랜잭션 JSON) 생성 →
//! `POST /api/v1/sendTx`(form: `tx_type`, `tx_info`, `price_protection`) 또는
//! `POST /api/v1/sendTxBatch`(form: `tx_types`, `tx_infos`).

use serde::Deserialize;

use crate::lighter::client::{ApiCall, LighterClient};
use crate::lighter::error::{LighterError, Result};
use crate::lighter::sign;

/// 주문 방향.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// 매수 (is_ask=false).
    Buy,
    /// 매도 (is_ask=true).
    Sell,
}

impl Side {
    /// Lighter 트랜잭션 필드 `is_ask`로 변환 (매도=true).
    pub fn is_ask(self) -> bool {
        matches!(self, Side::Sell)
    }
}

/// 주문 유형. Lighter 트랜잭션 `order_type` 코드(constants.go)에 대응.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    Limit,
    Market,
}

impl OrderType {
    /// Lighter `order_type` 코드. LimitOrder=0, MarketOrder=1 (constants.go).
    pub fn code(self) -> u8 {
        match self {
            OrderType::Limit => 0,
            OrderType::Market => 1,
        }
    }
}

/// 체결 조건. Lighter 트랜잭션 `time_in_force` 코드(constants.go)에 대응.
///
/// **주의:** Lighter는 IOC=0, GTT=1, PostOnly=2 **세 가지만** 지원한다. FillOrKill에
/// 대응하는 코드가 없으므로 이 enum에 포함하지 않는다(예전 정의에서 제거).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// 즉시 체결·잔량 취소 (코드 0).
    ImmediateOrCancel,
    /// 취소 전까지 유효 (코드 1).
    GoodTillTime,
    /// 메이커 전용 — 즉시 체결되면 거부 (코드 2).
    PostOnly,
}

impl TimeInForce {
    /// Lighter `time_in_force` 코드.
    pub fn code(self) -> u8 {
        match self {
            TimeInForce::ImmediateOrCancel => 0,
            TimeInForce::GoodTillTime => 1,
            TimeInForce::PostOnly => 2,
        }
    }
}

/// 주문 요청. 수량·가격은 정밀도 보존 위해 String. 서명 단계에서 Lighter는 이를
/// `supported_*_decimals` 기준 정수 스케일로 바꾸므로, 호출부는 사람이 읽는 십진
/// 문자열을 그대로 넣는다(스케일 변환은 서명 구현이 담당).
#[derive(Debug, Clone)]
pub struct OrderRequest {
    /// 마켓 식별자. 심볼→id는 [`crate::lighter::market::Market::market_id`].
    pub market_id: u32,
    pub side: Side,
    pub order_type: OrderType,
    /// 주문 수량 (base 토큰, 십진 문자열).
    pub base_amount: String,
    /// 지정가 (십진 문자열). LIMIT 필수, MARKET이면 None.
    pub price: Option<String>,
    /// 체결 조건. LIMIT 필수.
    pub time_in_force: Option<TimeInForce>,
    /// 포지션 축소 전용 여부.
    pub reduce_only: bool,
    /// 클라이언트 주문 인덱스 (멱등 추적; Lighter `client_order_index`).
    pub client_order_index: Option<u64>,
    /// `sendTx`의 `price_protection` 폼 필드 (lighter-python 기본 true). 시장가에서
    /// 마크가 대비 과도한 슬리피지를 서버가 거부하게 한다. 서명 메시지가 아닌 **전송
    /// 봉투** 파라미터다.
    pub price_protection: bool,
}

impl OrderRequest {
    /// 시장가 주문.
    ///
    /// **Lighter 시장가도 `price`가 필수다** — 최악 허용 체결가(슬리피지 상한)로 쓰인다
    /// (`create_order.go` Validate: `MinOrderPrice=1`, 시장가는 IOC + expiry/trigger nil
    /// 요구). 매수면 상한, 매도면 하한 가격을 십진 문자열로 넘긴다.
    pub fn market(
        market_id: u32,
        side: Side,
        base_amount: impl Into<String>,
        worst_price: impl Into<String>,
    ) -> Self {
        Self {
            market_id,
            side,
            order_type: OrderType::Market,
            base_amount: base_amount.into(),
            price: Some(worst_price.into()),
            time_in_force: Some(TimeInForce::ImmediateOrCancel),
            reduce_only: false,
            client_order_index: None,
            price_protection: true,
        }
    }

    /// 지정가 주문 (기본 GoodTillTime).
    pub fn limit(
        market_id: u32,
        side: Side,
        base_amount: impl Into<String>,
        price: impl Into<String>,
    ) -> Self {
        Self {
            market_id,
            side,
            order_type: OrderType::Limit,
            base_amount: base_amount.into(),
            price: Some(price.into()),
            time_in_force: Some(TimeInForce::GoodTillTime),
            reduce_only: false,
            client_order_index: None,
            price_protection: true,
        }
    }

    /// 포지션 축소 전용으로 표시 (builder).
    pub fn reduce_only(mut self, v: bool) -> Self {
        self.reduce_only = v;
        self
    }

    /// 클라이언트 주문 인덱스 부여 (builder).
    pub fn client_order_index(mut self, i: u64) -> Self {
        self.client_order_index = Some(i);
        self
    }

    /// `price_protection` 폼 플래그 설정 (builder, 기본 true).
    pub fn price_protection(mut self, v: bool) -> Self {
        self.price_protection = v;
        self
    }
}

/// nextNonce 응답 (`GET /api/v1/nextNonce`). 서명 불필요 — **동작**한다.
#[derive(Debug, Clone, Deserialize)]
pub struct NextNonce {
    /// 다음 트랜잭션에 쓸 nonce.
    pub nonce: u64,
}

/// 활성 주문 1건 (`GET /api/v1/accountActiveOrders` → orders[]).
///
/// **주의:** 이 엔드포인트는 `auth` 토큰(서명 파생)을 요구하므로 본 어댑터에서는
/// 호출할 수 없다. 구조체는 서명 구현 후를 위해 정의해 둔다.
#[derive(Debug, Clone, Deserialize)]
pub struct ActiveOrder {
    pub order_id: String,
    pub market_id: u32,
    pub is_ask: bool,
    pub price: String,
    pub remaining_base_amount: String,
    pub initial_base_amount: String,
    #[serde(default)]
    pub status: String,
}

/// 거래·계좌 도메인 액세서. `client.trade()`로 획득.
pub struct Trade<'a> {
    client: &'a LighterClient,
}

impl<'a> Trade<'a> {
    pub(crate) fn new(client: &'a LighterClient) -> Self {
        Self { client }
    }

    /// 다음 nonce 조회 (`GET /api/v1/nextNonce`). 서명 불필요 — **동작**한다.
    /// 서명 구현 시 트랜잭션 빌드 전에 호출한다.
    pub async fn next_nonce(&self) -> Result<NextNonce> {
        let cfg = self.client.config();
        let account_index = cfg.account_index.ok_or_else(|| {
            LighterError::Auth("nextNonce requires account_index (config.account_index)".into())
        })?;
        self.client
            .call(ApiCall::get(
                "/api/v1/nextNonce",
                vec![
                    ("account_index".into(), account_index.to_string()),
                    ("api_key_index".into(), cfg.api_key_index.to_string()),
                ],
            ))
            .await?
            .parse()
    }

    /// 주문 제출. **UNVERIFIED 서명** — `config.allow_unverified_signing`가 true여야 동작.
    ///
    /// 흐름: 마켓 메타(소수자리) 조회 → 십진 문자열을 정수 스케일로 변환 →
    /// nextNonce(미지정 시) → `expired_at` 계산 → 메시지 해시(poseidon2) → schnorr 서명
    /// (자기검증) → `tx_info` JSON 조립 → `POST /api/v1/sendTx`.
    ///
    /// 게이트가 닫혀 있으면(기본) [`LighterError::SignerUnavailable`]로 거부한다. 서명
    /// 스킴은 공식 픽스처로 종단 검증되지 않았다 — `scripts/lighter_capture_vector.md`
    /// 참고. 반환값은 서버 응답 raw JSON(예: `{code, tx_hash}`).
    pub async fn place(&self, req: &OrderRequest) -> Result<serde_json::Value> {
        let cfg = self.guard_signing()?;
        let sk = sign::parse_private_key(&cfg.api_key_private_key)
            .map_err(LighterError::Auth)?;
        let account_index = cfg.account_index.unwrap() as i64;

        // 마켓 소수자리.
        let detail = self
            .client
            .market()
            .order_book_details(req.market_id)
            .await?;

        let price_str = req.price.as_deref().ok_or_else(|| {
            LighterError::Auth("order requires price (market orders use worst-acceptable price)".into())
        })?;
        let price_scaled = scale_decimal(price_str, detail.supported_price_decimals)?;
        let price: u32 = u32::try_from(price_scaled)
            .map_err(|_| LighterError::Auth(format!("price out of u32 range: {price_scaled}")))?;
        let base_scaled = scale_decimal(&req.base_amount, detail.supported_size_decimals)?;
        let base_amount: i64 = i64::try_from(base_scaled)
            .map_err(|_| LighterError::Auth(format!("base_amount out of i64 range: {base_scaled}")))?;

        let tif = req
            .time_in_force
            .ok_or_else(|| LighterError::Auth("order requires time_in_force".into()))?;

        // order_expiry: GTT 한정가는 28일 후(ms), 그 외(IOC/시장가)는 nil(0).
        // 주의: -1(파이썬 DEFAULT_28_DAY) 관례의 해시 전 확장 시점은 캡처로 확정 필요.
        let now_ms = unix_millis();
        let order_expiry: i64 = match (req.order_type, tif) {
            (OrderType::Limit, TimeInForce::GoodTillTime)
            | (OrderType::Limit, TimeInForce::PostOnly) => now_ms + 28 * 24 * 60 * 60 * 1000,
            _ => 0, // NilOrderExpiry
        };
        // expired_at: tx 데드라인(now + ~10분). 해시·tx_info에서 동일 값 사용.
        let expired_at = now_ms + (10 * 60 - 1) * 1000;

        let nonce = self.resolve_nonce(req.client_order_index).await?;

        let msg = sign::CreateOrderMsg {
            account_index,
            api_key_index: cfg.api_key_index,
            market_index: i16::try_from(req.market_id)
                .map_err(|_| LighterError::Auth("market_id out of i16 range".into()))?,
            client_order_index: req.client_order_index.unwrap_or(0) as i64,
            base_amount,
            price,
            is_ask: req.side.is_ask() as u8,
            order_type: req.order_type.code(),
            time_in_force: tif.code(),
            reduce_only: req.reduce_only as u8,
            trigger_price: 0, // NilOrderTriggerPrice — 트리거 주문 미지원.
            order_expiry,
            nonce,
            expired_at,
        };

        let hash = msg.hash(cfg.chain_id);
        let k = random_scalar();
        let (sig, _signed_hash) =
            sign::sign_message(sk, &hash, k).map_err(LighterError::SignerUnavailable)?;

        let tx_info = build_create_order_tx_info(&msg, &sig);
        self.send_tx(sign::tx_type::CREATE_ORDER, &tx_info, Some(req.price_protection))
            .await
    }

    /// 주문 취소. **UNVERIFIED 서명** — `config.allow_unverified_signing`가 true여야 동작.
    ///
    /// `order_index`는 취소 대상의 order index 또는 client order index (Lighter
    /// `cancel_order.go`의 `Index`). 게이트가 닫혀 있으면 SignerUnavailable.
    pub async fn cancel(&self, market_id: u32, order_index: i64) -> Result<serde_json::Value> {
        let cfg = self.guard_signing()?;
        let sk = sign::parse_private_key(&cfg.api_key_private_key)
            .map_err(LighterError::Auth)?;
        let account_index = cfg.account_index.unwrap() as i64;

        let now_ms = unix_millis();
        let expired_at = now_ms + (10 * 60 - 1) * 1000;
        let nonce = self.resolve_nonce(None).await?;

        let msg = sign::CancelOrderMsg {
            account_index,
            api_key_index: cfg.api_key_index,
            market_index: i16::try_from(market_id)
                .map_err(|_| LighterError::Auth("market_id out of i16 range".into()))?,
            index: order_index,
            nonce,
            expired_at,
        };
        let hash = msg.hash(cfg.chain_id);
        let k = random_scalar();
        let (sig, _signed_hash) =
            sign::sign_message(sk, &hash, k).map_err(LighterError::SignerUnavailable)?;

        let tx_info = build_cancel_order_tx_info(&msg, &sig);
        // 취소는 price_protection 무관 → 폼 필드 생략.
        self.send_tx(sign::tx_type::CANCEL_ORDER, &tx_info, None).await
    }

    /// `POST /api/v1/sendTx` (form: tx_type, tx_info, [price_protection]). 응답 raw JSON.
    ///
    /// `price_protection`은 create-order 전송 봉투 파라미터(lighter-python 기본 true)다.
    /// 서명 메시지에는 들어가지 않으며, 무관한 tx_type(취소 등)에는 `None`으로 생략한다.
    async fn send_tx(
        &self,
        tx_type: u8,
        tx_info: &str,
        price_protection: Option<bool>,
    ) -> Result<serde_json::Value> {
        let mut form = vec![
            ("tx_type".to_string(), tx_type.to_string()),
            ("tx_info".to_string(), tx_info.to_string()),
        ];
        if let Some(pp) = price_protection {
            form.push(("price_protection".into(), pp.to_string()));
        }
        let resp = self.client.call(ApiCall::post_form("/api/v1/sendTx", form)).await?;
        Ok(resp.body)
    }

    /// nonce 결정: client_order_index가 주어졌어도 nonce는 nextNonce에서 받는다
    /// (둘은 별개 — client_order_index는 멱등 추적용, nonce는 tx 직렬). nextNonce 호출.
    async fn resolve_nonce(&self, _client_order_index: Option<u64>) -> Result<i64> {
        let n = self.next_nonce().await?;
        i64::try_from(n.nonce)
            .map_err(|_| LighterError::Auth(format!("nonce out of i64 range: {}", n.nonce)))
    }

    /// 서명 게이트 + 자격증명 확인. 통과하면 config 참조 반환.
    fn guard_signing(&self) -> Result<&crate::lighter::config::LighterConfig> {
        self.ensure_signer()?;
        let cfg = self.client.config();
        if !cfg.allow_unverified_signing {
            return Err(LighterError::SignerUnavailable(
                "Lighter 서명은 UNVERIFIED(공식 픽스처로 종단 검증 안 됨) — \
                 config.allow_unverified_signing=true로 명시적으로 켜야 제출됨. \
                 검증 절차: scripts/lighter_capture_vector.md"
                    .into(),
            ));
        }
        Ok(cfg)
    }

    /// 활성 주문 조회. **차단됨** — `auth`(서명 파생) 토큰 미구현.
    pub async fn active_orders(&self) -> Result<Vec<ActiveOrder>> {
        self.ensure_signer()?;
        Err(LighterError::SignerUnavailable(
            "accountActiveOrders는 서명 파생 auth 토큰을 요구 — 미구현(data-only)".into(),
        ))
    }

    /// 자격증명(account_index + 개인키) 존재 확인. 없으면 [`LighterError::Auth`].
    /// 자격증명이 있어도 서명기 자체가 없으므로 제출 메서드는 SignerUnavailable로 끝난다.
    fn ensure_signer(&self) -> Result<()> {
        let cfg = self.client.config();
        if cfg.account_index.is_none() || cfg.api_key_private_key.is_empty() {
            return Err(LighterError::Auth(
                "trading requires account_index + api_key_private_key".into(),
            ));
        }
        Ok(())
    }
}

/// 십진 문자열을 `decimals` 자릿수 정수 스케일로 (정수/문자열 기반 — f64 절대 미사용).
///
/// 예: `scale_decimal("244.045", 3) = 244045`. 소수 자리가 `decimals`를 초과하면(정밀도
/// 손실) 에러. 부호·지수표기 미지원. Lighter는 `value * 10^supported_*_decimals`를
/// 기대한다(파이썬 SDK가 FFI 호출 전에 동일하게 정수화).
fn scale_decimal(s: &str, decimals: u32) -> Result<u128> {
    let s = s.trim();
    if s.is_empty() || s.starts_with('-') {
        return Err(LighterError::Auth(format!("invalid decimal: {s:?}")));
    }
    let (int_part, frac_part) = match s.split_once('.') {
        Some((i, f)) => (i, f),
        None => (s, ""),
    };
    if !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(LighterError::Auth(format!("non-numeric decimal: {s:?}")));
    }
    let d = decimals as usize;
    if frac_part.len() > d {
        return Err(LighterError::Auth(format!(
            "{s:?} exceeds {decimals} decimal places — refusing to round"
        )));
    }
    // int_part ‖ frac_part(우측 0 패딩) → 정수 문자열.
    let mut digits = String::with_capacity(int_part.len() + d);
    digits.push_str(int_part);
    digits.push_str(frac_part);
    for _ in 0..(d - frac_part.len()) {
        digits.push('0');
    }
    // 선행 0 제거 후 파싱(빈 문자열이면 0).
    let trimmed = digits.trim_start_matches('0');
    let parse_src = if trimmed.is_empty() { "0" } else { trimmed };
    parse_src
        .parse::<u128>()
        .map_err(|e| LighterError::Auth(format!("scale overflow {s:?}: {e}")))
}

/// 현재 UNIX 시각(ms).
fn unix_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// OS CSPRNG로 40바이트 추출 → ECgFp5 스칼라(mod n)로. Schnorr nonce `k`.
///
/// `getrandom`(OS 엔트로피)을 직접 쓴다 — 약한 난수로 서명 시 개인키가 노출될 수 있으므로
/// 실패하면 패닉한다. 40바이트(320비트)는 군 위수 n(≈2^319)을 살짝 넘으므로
/// `Scalar::from_le_bytes`가 mod n으로 환원한다(편향은 무시 가능한 ~2^-319 수준).
fn random_scalar() -> crate::lighter::sign::scalar::Scalar {
    let mut buf = [0u8; 40];
    getrandom::getrandom(&mut buf).expect("OS CSPRNG failed — refusing to sign with weak nonce");
    crate::lighter::sign::scalar::Scalar::from_le_bytes(&buf)
}

/// create_order `tx_info` JSON 조립. **UNVERIFIED 와이어 봉투.**
///
/// Go `json.Marshal(L2CreateOrderTxInfo)`를 모사한다: 키는 Go 필드명(PascalCase), 임베디드
/// `*OrderInfo` 평탄화, `Sig`는 base64 문자열, `SignedHash`는 제외. 서버가 기대하는 정확한
/// 키 집합·attributes 직렬화는 라이브 미검증 — `scripts/lighter_capture_vector.md`로
/// 공식 SDK 출력과 1:1 대조해야 한다.
fn build_create_order_tx_info(m: &sign::CreateOrderMsg, sig: &[u8; 80]) -> String {
    let v = serde_json::json!({
        "AccountIndex": m.account_index,
        "ApiKeyIndex": m.api_key_index,
        "MarketIndex": m.market_index,
        "ClientOrderIndex": m.client_order_index,
        "BaseAmount": m.base_amount,
        "Price": m.price,
        "IsAsk": m.is_ask,
        "Type": m.order_type,
        "TimeInForce": m.time_in_force,
        "ReduceOnly": m.reduce_only,
        "TriggerPrice": m.trigger_price,
        "OrderExpiry": m.order_expiry,
        "ExpiredAt": m.expired_at,
        "Nonce": m.nonce,
        "Sig": base64_std(sig),
    });
    v.to_string()
}

/// cancel_order `tx_info` JSON 조립. **UNVERIFIED 와이어 봉투** (위와 동일 주의).
fn build_cancel_order_tx_info(m: &sign::CancelOrderMsg, sig: &[u8; 80]) -> String {
    let v = serde_json::json!({
        "AccountIndex": m.account_index,
        "ApiKeyIndex": m.api_key_index,
        "MarketIndex": m.market_index,
        "Index": m.index,
        "ExpiredAt": m.expired_at,
        "Nonce": m.nonce,
        "Sig": base64_std(sig),
    });
    v.to_string()
}

/// Go `json.Marshal`의 `[]byte` → 표준 base64(패딩 포함) 인코딩과 일치.
fn base64_std(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[(n >> 6 & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn side_is_ask_mapping() {
        assert!(!Side::Buy.is_ask());
        assert!(Side::Sell.is_ask());
    }

    #[test]
    fn market_order_requires_price() {
        let req = OrderRequest::market(162, Side::Buy, "3", "250.0");
        assert_eq!(req.market_id, 162);
        assert_eq!(req.order_type, OrderType::Market);
        assert_eq!(req.time_in_force, Some(TimeInForce::ImmediateOrCancel));
        assert_eq!(req.price.as_deref(), Some("250.0"));
    }

    #[test]
    fn scale_decimal_basic() {
        assert_eq!(scale_decimal("244.045", 3).unwrap(), 244045);
        assert_eq!(scale_decimal("3", 4).unwrap(), 30000);
        assert_eq!(scale_decimal("0.1", 4).unwrap(), 1000);
        assert_eq!(scale_decimal("120.50", 2).unwrap(), 12050);
        // 과정밀 거부.
        assert!(scale_decimal("1.23456", 3).is_err());
        // 음수·비숫자 거부.
        assert!(scale_decimal("-1.0", 3).is_err());
        assert!(scale_decimal("1.2e3", 3).is_err());
    }

    #[test]
    fn base64_matches_go_std() {
        // Go base64.StdEncoding 참조값.
        assert_eq!(base64_std(b""), "");
        assert_eq!(base64_std(b"f"), "Zg==");
        assert_eq!(base64_std(b"fo"), "Zm8=");
        assert_eq!(base64_std(b"foo"), "Zm9v");
        assert_eq!(base64_std(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn order_type_tif_codes() {
        assert_eq!(OrderType::Limit.code(), 0);
        assert_eq!(OrderType::Market.code(), 1);
        assert_eq!(TimeInForce::ImmediateOrCancel.code(), 0);
        assert_eq!(TimeInForce::GoodTillTime.code(), 1);
        assert_eq!(TimeInForce::PostOnly.code(), 2);
    }

    #[test]
    fn limit_order_defaults_gtt_and_builders() {
        let req = OrderRequest::limit(161, Side::Sell, "2", "120.50")
            .reduce_only(true)
            .client_order_index(7);
        assert_eq!(req.order_type, OrderType::Limit);
        assert_eq!(req.price.as_deref(), Some("120.50"));
        assert_eq!(req.time_in_force, Some(TimeInForce::GoodTillTime));
        assert!(req.reduce_only);
        assert_eq!(req.client_order_index, Some(7));
    }

    #[test]
    fn next_nonce_parses() {
        // 라이브: {"code":200,"nonce":1}.
        let v = serde_json::json!({ "code": 200, "nonce": 1 });
        let n: NextNonce = serde_json::from_value(v).unwrap();
        assert_eq!(n.nonce, 1);
    }

    #[test]
    fn active_order_parses() {
        let v = serde_json::json!({
            "order_id": "45880421206297236",
            "market_id": 162,
            "is_ask": true,
            "price": "244.045",
            "remaining_base_amount": "22.531",
            "initial_base_amount": "22.531",
            "status": "open"
        });
        let o: ActiveOrder = serde_json::from_value(v).unwrap();
        assert_eq!(o.market_id, 162);
        assert!(o.is_ask);
        assert_eq!(o.price, "244.045");
    }
}
