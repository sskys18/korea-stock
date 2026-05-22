# Plan 1 — KIS 어댑터: Core 인프라 + 국내주식

- 작성일: 2026-05-22
- 스펙: `docs/specs/2026-05-22-kis-adapter-design.md`
- TR 명세 SSOT: `docs/kis-api/domestic-stock.md`
- 상태: 실행 대기

## 목표

`kis-adapter` 크레이트의 토대를 세운다 — 설정·에러·인증·레이트리밋·HTTP 클라이언트 +
국내주식 12개 TR. 이 Plan이 끝나면 어댑터 전체 스택(토큰 발급 → 인증 헤더 →
레이트리밋 → 응답 envelope)이 국내주식 도메인으로 end-to-end 검증된다.
Plan 2(해외주식·선물옵션), Plan 3(실시간 WS)은 여기서 확정된 패턴을 복제한다.

## 범위

- 포함: 워크스페이스 아닌 단일 lib 크레이트, core 6모듈, `domestic_stock` 4파일,
  CLI 예제 2개, 모의환경 통합테스트.
- 제외: 해외주식, 선물옵션, 실시간 WebSocket (Plan 2/3).

## 전제

- Rust stable (1.75+), `cargo` 설치.
- 모의투자 자격증명 환경변수 (통합테스트 실행 시에만 필요):
  `KIS_APP_KEY`, `KIS_APP_SECRET`, `KIS_ACCOUNT_NO`, `KIS_ACCOUNT_PRODUCT`, `KIS_ENV=mock`.
- 저장소 루트 `/Users/sskys/Mine/korea-stock`가 곧 크레이트 (중첩 디렉토리 없음).
- 실행은 `main`이 아닌 작업 브랜치에서 — 실행 시작 전 `git checkout -b feat/kis-plan-1`.

## 파일 맵

| 파일 | 책임 | 생성 태스크 |
|------|------|------------|
| `Cargo.toml` | 패키지·의존성 | T1 |
| `.gitignore` | `.kis/`, `target/`, `*.env` 제외 | T1 |
| `src/lib.rs` | 모듈 선언·공개 재노출 | T1, T8 |
| `src/error.rs` | `KisError`, `Result` | T2 |
| `src/config.rs` | `Environment`, `KisConfig` | T3 |
| `src/trid.rs` | `TrId` 실전/모의 분기 | T4 |
| `src/ratelimit.rs` | `RateLimiter` 토큰버킷 | T5 |
| `src/auth.rs` | `Auth` 토큰 발급·캐싱·hashkey | T6 |
| `src/client.rs` | `KisClient`, `KisResponse<T>`, `raw_call` | T7 |
| `src/domestic_stock/mod.rs` | `DomesticStock` 액세서·공통 헬퍼 | T8 |
| `src/domestic_stock/quote.rs` | TR 9·10·11·12 시세 | T9 |
| `src/domestic_stock/order.rs` | TR 1·2·3·4 주문 | T10 |
| `src/domestic_stock/account.rs` | TR 5·6·7·8 계좌·체결 | T11 |
| `examples/domestic_quote.rs` | 현재가 조회 CLI | T12 |
| `examples/domestic_order.rs` | 잔고+매수가능 CLI | T13 |
| `tests/integration.rs` | 모의환경 실호출 스모크 | T14 |
| `README.md` | 사용법 | T15 |

---

## T1 — 크레이트 스캐폴드

`Cargo.toml` 작성:

```toml
[package]
name = "kis-adapter"
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
description = "한국투자증권(KIS) OpenAPI Rust 어댑터"
license = "MIT"

[dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time", "fs"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
chrono = { version = "0.4", features = ["serde", "clock"] }
tracing = "0.1"
dirs = "5"

[dev-dependencies]
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time", "fs"] }
tracing-subscriber = "0.3"

[[example]]
name = "domestic_quote"

[[example]]
name = "domestic_order"
```

`.gitignore` 작성:

```
/target
.kis/
*.env
Cargo.lock
```

> Cargo.lock 제외: 라이브러리 크레이트 관례.

`src/lib.rs` 초기 스켈레톤:

```rust
//! 한국투자증권(KIS) OpenAPI Rust 어댑터.

mod auth;
mod client;
mod config;
mod error;
mod ratelimit;
mod trid;

pub mod domestic_stock;

pub use client::{KisClient, KisResponse, RawRequest};
pub use config::{Environment, KisConfig};
pub use error::{KisError, Result};
```

검증: `cargo build` — 이 시점엔 하위 모듈이 없어 실패. T2~T8 완료 후 통과.
대신 `cargo init` 충돌 방지 위해 이 태스크는 파일만 생성하고 빌드는 T8 끝에서.

커밋: `chore: 크레이트 스캐폴드 + 의존성`

---

## T2 — `src/error.rs`

```rust
use thiserror::Error;

/// KIS 어댑터 전역 에러.
#[derive(Error, Debug)]
pub enum KisError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("auth: {0}")]
    Auth(String),

    /// KIS 응답 `rt_cd != "0"`.
    #[error("api error rt_cd={rt_cd} msg_cd={msg_cd}: {msg}")]
    Api {
        rt_cd: String,
        msg_cd: String,
        msg: String,
    },

    #[error("rate limited")]
    RateLimit,

    #[error("websocket: {0}")]
    Ws(String),

    #[error("decode: {0}")]
    Decode(String),

    /// 모의투자 환경에서 미지원 TR 호출.
    #[error("unsupported in mock environment: {tr_id}")]
    UnsupportedInMock { tr_id: String },

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, KisError>;
```

검증: 단독 컴파일 불가(크레이트 일부). T8에서 확인.
커밋: `feat: KisError 에러 타입`

---

## T3 — `src/config.rs`

```rust
use std::path::PathBuf;

use crate::error::{KisError, Result};

/// 실전투자 / 모의투자 환경.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Real,
    Mock,
}

impl Environment {
    /// REST API 베이스 URL.
    pub fn rest_base(&self) -> &'static str {
        match self {
            Environment::Real => "https://openapi.koreainvestment.com:9443",
            Environment::Mock => "https://openapivts.koreainvestment.com:29443",
        }
    }

    /// 실시간 WebSocket 베이스 URL (Plan 3에서 사용).
    pub fn ws_base(&self) -> &'static str {
        match self {
            Environment::Real => "ws://ops.koreainvestment.com:21000",
            Environment::Mock => "ws://ops.koreainvestment.com:31000",
        }
    }

    /// REST 레이트리밋 (요청/초). 실전 20, 모의 2.
    pub fn rate_limit(&self) -> u32 {
        match self {
            Environment::Real => 20,
            Environment::Mock => 2,
        }
    }
}

/// 어댑터 설정. app key/secret/계좌번호는 호출자가 주입.
#[derive(Debug, Clone)]
pub struct KisConfig {
    pub app_key: String,
    pub app_secret: String,
    /// 종합계좌번호 앞 8자리 (CANO).
    pub account_no: String,
    /// 계좌상품코드 뒤 2자리 (ACNT_PRDT_CD), 예: "01".
    pub account_product: String,
    pub environment: Environment,
    /// 토큰 캐시 파일 경로. None이면 메모리 캐시만.
    pub token_cache_path: Option<PathBuf>,
    /// POST 주문 호출 시 hashkey 발급·첨부 여부. 기본 false (KIS 비강제).
    pub use_hashkey: bool,
}

impl KisConfig {
    /// 환경변수에서 설정 로드.
    /// `KIS_APP_KEY` `KIS_APP_SECRET` `KIS_ACCOUNT_NO` `KIS_ACCOUNT_PRODUCT`
    /// `KIS_ENV`(real|mock, 기본 mock). 토큰 캐시는 기본 경로 사용.
    pub fn from_env() -> Result<Self> {
        fn var(name: &str) -> Result<String> {
            std::env::var(name)
                .map_err(|_| KisError::Auth(format!("missing env var {name}")))
        }
        let environment = match std::env::var("KIS_ENV").as_deref() {
            Ok("real") => Environment::Real,
            Ok("mock") | Err(_) => Environment::Mock,
            Ok(other) => {
                return Err(KisError::Auth(format!("invalid KIS_ENV: {other}")))
            }
        };
        Ok(Self {
            app_key: var("KIS_APP_KEY")?,
            app_secret: var("KIS_APP_SECRET")?,
            account_no: var("KIS_ACCOUNT_NO")?,
            account_product: var("KIS_ACCOUNT_PRODUCT")?,
            environment,
            token_cache_path: Self::default_token_cache_path(),
            use_hashkey: false,
        })
    }

    /// 기본 토큰 캐시 경로: `~/.kis/token.json`.
    pub fn default_token_cache_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".kis").join("token.json"))
    }
}
```

검증: T8에서 확인.
커밋: `feat: Environment + KisConfig 설정`

---

## T4 — `src/trid.rs`

```rust
use crate::config::Environment;
use crate::error::{KisError, Result};

/// 실전/모의 tr_id 쌍. 시세계 GET은 실전·모의 동일(`same`),
/// 일부 TR은 모의 미지원(`real_only`).
#[derive(Debug, Clone, Copy)]
pub struct TrId {
    pub real: &'static str,
    /// None = 모의투자 미지원.
    pub mock: Option<&'static str>,
}

impl TrId {
    /// 실전/모의 tr_id가 다른 경우.
    pub const fn both(real: &'static str, mock: &'static str) -> Self {
        Self { real, mock: Some(mock) }
    }

    /// 실전/모의 tr_id가 동일한 경우 (시세계 GET API).
    pub const fn same(id: &'static str) -> Self {
        Self { real: id, mock: Some(id) }
    }

    /// 모의투자 미지원 TR.
    pub const fn real_only(real: &'static str) -> Self {
        Self { real, mock: None }
    }

    /// 환경에 맞는 tr_id 반환. 모의 미지원이면 `UnsupportedInMock`.
    pub fn resolve(&self, env: Environment) -> Result<&'static str> {
        match env {
            Environment::Real => Ok(self.real),
            Environment::Mock => self.mock.ok_or_else(|| {
                KisError::UnsupportedInMock { tr_id: self.real.to_string() }
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_resolves_per_env() {
        let tr = TrId::both("TTTC0012U", "VTTC0012U");
        assert_eq!(tr.resolve(Environment::Real).unwrap(), "TTTC0012U");
        assert_eq!(tr.resolve(Environment::Mock).unwrap(), "VTTC0012U");
    }

    #[test]
    fn same_resolves_identical() {
        let tr = TrId::same("FHKST01010100");
        assert_eq!(tr.resolve(Environment::Real).unwrap(), "FHKST01010100");
        assert_eq!(tr.resolve(Environment::Mock).unwrap(), "FHKST01010100");
    }

    #[test]
    fn real_only_errors_in_mock() {
        let tr = TrId::real_only("TTTS3018R");
        assert!(tr.resolve(Environment::Real).is_ok());
        assert!(matches!(
            tr.resolve(Environment::Mock),
            Err(KisError::UnsupportedInMock { .. })
        ));
    }
}
```

검증: T8 빌드 후 `cargo test trid`.
커밋: `feat: TrId 실전/모의 분기`

---

## T5 — `src/ratelimit.rs`

```rust
use std::time::Duration;

use tokio::sync::Mutex;
use tokio::time::{sleep, Instant};

/// 토큰버킷 레이트리미터. 모든 REST 호출 전 `acquire().await`.
pub struct RateLimiter {
    inner: Mutex<Bucket>,
}

struct Bucket {
    capacity: f64,
    tokens: f64,
    refill_per_sec: f64,
    last: Instant,
}

impl RateLimiter {
    /// `per_sec` = 초당 허용 요청 수. 버킷 용량도 동일.
    pub fn new(per_sec: u32) -> Self {
        let cap = per_sec.max(1) as f64;
        Self {
            inner: Mutex::new(Bucket {
                capacity: cap,
                tokens: cap,
                refill_per_sec: cap,
                last: Instant::now(),
            }),
        }
    }

    /// 토큰 1개 확보까지 대기.
    pub async fn acquire(&self) {
        loop {
            let wait = {
                let mut b = self.inner.lock().await;
                let now = Instant::now();
                let elapsed = now.duration_since(b.last).as_secs_f64();
                b.tokens = (b.tokens + elapsed * b.refill_per_sec).min(b.capacity);
                b.last = now;
                if b.tokens >= 1.0 {
                    b.tokens -= 1.0;
                    return;
                }
                let deficit = 1.0 - b.tokens;
                Duration::from_secs_f64(deficit / b.refill_per_sec)
            };
            sleep(wait).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn burst_then_throttle() {
        // 2 req/s — 첫 2건 즉시, 3번째는 ~0.5s 대기.
        let rl = RateLimiter::new(2);
        let start = Instant::now();
        rl.acquire().await;
        rl.acquire().await;
        assert!(start.elapsed() < Duration::from_millis(100), "버스트 즉시 통과");
        rl.acquire().await;
        assert!(start.elapsed() >= Duration::from_millis(400), "3번째 throttle");
    }
}
```

검증: T8 빌드 후 `cargo test ratelimit`.
커밋: `feat: RateLimiter 토큰버킷`

---

## T6 — `src/auth.rs`

토큰 발급(`POST /oauth2/tokenP`), 파일·메모리 캐싱, atomic write,
hashkey 발급(`POST /uapi/hashkey`).

```rust
use std::path::PathBuf;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::config::KisConfig;
use crate::error::{KisError, Result};

#[derive(Serialize, Deserialize, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: DateTime<Utc>,
}

impl CachedToken {
    /// 만료 1시간 전이면 아직 유효.
    fn is_fresh(&self) -> bool {
        self.expires_at > Utc::now() + ChronoDuration::hours(1)
    }
}

/// 토큰 발급·캐싱·hashkey 발급 담당.
pub(crate) struct Auth {
    app_key: String,
    app_secret: String,
    rest_base: String,
    cache_path: Option<PathBuf>,
    http: reqwest::Client,
    state: Mutex<Option<CachedToken>>,
}

impl Auth {
    pub fn new(config: &KisConfig, http: reqwest::Client) -> Self {
        Self {
            app_key: config.app_key.clone(),
            app_secret: config.app_secret.clone(),
            rest_base: config.environment.rest_base().to_string(),
            cache_path: config.token_cache_path.clone(),
            http,
            state: Mutex::new(None),
        }
    }

    /// 유효한 access token 반환. 메모리 → 파일 캐시 → 신규 발급 순.
    /// `Mutex`로 동시 호출 시 1회만 발급.
    pub async fn token(&self) -> Result<String> {
        let mut state = self.state.lock().await;

        if let Some(t) = state.as_ref() {
            if t.is_fresh() {
                return Ok(t.access_token.clone());
            }
        }
        if state.is_none() {
            if let Some(t) = self.load_cache().await {
                if t.is_fresh() {
                    let token = t.access_token.clone();
                    *state = Some(t);
                    return Ok(token);
                }
            }
        }
        let fresh = self.issue().await?;
        self.save_cache(&fresh).await;
        let token = fresh.access_token.clone();
        *state = Some(fresh);
        Ok(token)
    }

    async fn issue(&self) -> Result<CachedToken> {
        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
            expires_in: i64,
        }
        let body = serde_json::json!({
            "grant_type": "client_credentials",
            "appkey": self.app_key,
            "appsecret": self.app_secret,
        });
        let resp = self
            .http
            .post(format!("{}/oauth2/tokenP", self.rest_base))
            .json(&body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(KisError::Auth(format!("token issue failed: {txt}")));
        }
        let tr: TokenResponse = resp.json().await?;
        Ok(CachedToken {
            access_token: tr.access_token,
            expires_at: Utc::now() + ChronoDuration::seconds(tr.expires_in),
        })
    }

    /// 주문 body의 hashkey 발급. `use_hashkey=true`일 때만 호출.
    pub async fn hashkey(&self, body: &serde_json::Value) -> Result<String> {
        #[derive(Deserialize)]
        struct HashResponse {
            #[serde(rename = "HASH")]
            hash: String,
        }
        let resp = self
            .http
            .post(format!("{}/uapi/hashkey", self.rest_base))
            .header("appkey", &self.app_key)
            .header("appsecret", &self.app_secret)
            .json(body)
            .send()
            .await?;
        if !resp.status().is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(KisError::Auth(format!("hashkey failed: {txt}")));
        }
        let h: HashResponse = resp.json().await?;
        Ok(h.hash)
    }

    async fn load_cache(&self) -> Option<CachedToken> {
        let path = self.cache_path.as_ref()?;
        let data = tokio::fs::read(path).await.ok()?;
        serde_json::from_slice(&data).ok()
    }

    /// 임시파일 → chmod 0600 → atomic rename. 실패 시 warn만 (치명적 아님).
    async fn save_cache(&self, token: &CachedToken) {
        let Some(path) = self.cache_path.as_ref() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        let tmp = path.with_extension("tmp");
        let json = match serde_json::to_vec_pretty(token) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!("token cache serialize failed: {e}");
                return;
            }
        };
        if let Err(e) = tokio::fs::write(&tmp, &json).await {
            tracing::warn!("token cache write failed: {e}");
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perm = std::fs::Permissions::from_mode(0o600);
            if let Err(e) = tokio::fs::set_permissions(&tmp, perm).await {
                tracing::warn!("token cache chmod failed: {e}");
            }
        }
        if let Err(e) = tokio::fs::rename(&tmp, path).await {
            tracing::warn!("token cache rename failed: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_token_within_ttl() {
        let t = CachedToken {
            access_token: "x".into(),
            expires_at: Utc::now() + ChronoDuration::hours(5),
        };
        assert!(t.is_fresh());
    }

    #[test]
    fn stale_token_near_expiry() {
        let t = CachedToken {
            access_token: "x".into(),
            expires_at: Utc::now() + ChronoDuration::minutes(30),
        };
        assert!(!t.is_fresh(), "만료 1시간 이내는 stale");
    }
}
```

검증: T8 빌드 후 `cargo test auth`.
커밋: `feat: Auth 토큰 발급·캐싱·hashkey`

---

## T7 — `src/client.rs`

`KisClient`(공개), `KisResponse<T>`(공개 envelope), 내부 `call`,
공개 `raw_call`. 도메인 모듈은 `pub(crate) call`을 통해 요청.

```rust
use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::auth::Auth;
use crate::config::KisConfig;
use crate::error::{KisError, Result};
use crate::ratelimit::RateLimiter;

/// 모든 도메인 응답 공통 envelope. 데이터 + 연속조회 메타.
#[derive(Debug, Clone)]
pub struct KisResponse<T> {
    pub data: T,
    /// 응답 헤더 `tr_cont`. "F"/"M"이면 다음 페이지 존재.
    pub tr_cont: Option<String>,
    /// body cursor — 다음 페이지 요청 시 그대로 전달.
    pub ctx_area_fk: Option<String>,
    pub ctx_area_nk: Option<String>,
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg: String,
}

impl<T> KisResponse<T> {
    /// 다음 페이지가 있으면 true.
    pub fn has_next(&self) -> bool {
        matches!(self.tr_cont.as_deref(), Some("F") | Some("M"))
    }
}

/// 미구현 TR 직접 호출용 저수준 요청.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/uapi/..." 경로.
    pub path: String,
    pub tr_id: String,
    pub tr_cont: Option<String>,
    /// GET=query 파라미터, POST=body. JSON object.
    pub params: Value,
    /// POST일 때 true면 body, false면 GET query.
    pub is_post: bool,
    /// 이 요청에 hashkey 발급·첨부 (config.use_hashkey와 OR). 기본 false.
    pub needs_hashkey: bool,
}

/// 내부 도메인 호출 명세.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub tr_id: String,
    pub tr_cont: Option<String>,
    pub params: Value,
    pub is_post: bool,
    /// 요청별 hashkey 강제. config.use_hashkey와 OR로 적용.
    pub needs_hashkey: bool,
}

/// HTTP 응답 — 공통 envelope 검사 후 raw body 보존.
pub(crate) struct RawResponse {
    pub body: Value,
    pub tr_cont: Option<String>,
}

impl RawResponse {
    /// body의 지정 키를 타입 T로 역직렬화.
    pub fn field<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let v = self
            .body
            .get(key)
            .ok_or_else(|| KisError::Decode(format!("missing {key}")))?;
        Ok(serde_json::from_value(v.clone())?)
    }

    /// `data`를 envelope로 감싼다. ctx_area는 body에서 추출.
    pub fn envelope<T>(&self, data: T) -> KisResponse<T> {
        let s = |k: &str| {
            self.body
                .get(k)
                .and_then(|v| v.as_str())
                .map(String::from)
        };
        KisResponse {
            data,
            tr_cont: self.tr_cont.clone(),
            ctx_area_fk: s("ctx_area_fk100"),
            ctx_area_nk: s("ctx_area_nk100"),
            rt_cd: s("rt_cd").unwrap_or_default(),
            msg_cd: s("msg_cd").unwrap_or_default(),
            msg: s("msg1").unwrap_or_default(),
        }
    }
}

/// JSON 스칼라를 query 파라미터 문자열로. KIS는 모든 값을 문자열로 받음.
fn json_scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// KIS OpenAPI 클라이언트.
pub struct KisClient {
    config: KisConfig,
    http: reqwest::Client,
    auth: Auth,
    limiter: RateLimiter,
}

impl KisClient {
    /// 클라이언트 생성. 토큰은 첫 호출 시 lazy 발급.
    pub fn new(config: KisConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let auth = Auth::new(&config, http.clone());
        let limiter = RateLimiter::new(config.environment.rate_limit());
        Ok(Self { config, http, auth, limiter })
    }

    pub(crate) fn config(&self) -> &KisConfig {
        &self.config
    }

    /// 국내주식 도메인 액세서.
    pub fn domestic_stock(&self) -> crate::domestic_stock::DomesticStock<'_> {
        crate::domestic_stock::DomesticStock::new(self)
    }

    /// 미구현 TR 직접 호출. 응답은 raw JSON envelope.
    pub async fn raw_call(&self, req: RawRequest) -> Result<KisResponse<Value>> {
        let resp = self
            .call(ApiCall {
                method: req.method,
                path: req.path,
                tr_id: req.tr_id,
                tr_cont: req.tr_cont,
                params: req.params,
                is_post: req.is_post,
                needs_hashkey: req.needs_hashkey,
            })
            .await?;
        let data = resp.body.clone();
        Ok(resp.envelope(data))
    }

    /// 도메인 모듈 공용 호출. 레이트리밋 → 토큰 → 헤더 → 전송 → rt_cd 검사.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        self.limiter.acquire().await;
        let token = self.auth.token().await?;
        let url = format!("{}{}", self.config.environment.rest_base(), c.path);

        let mut req = self
            .http
            .request(c.method.clone(), &url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.config.app_key)
            .header("appsecret", &self.config.app_secret)
            .header("tr_id", &c.tr_id)
            .header("custtype", "P");

        if let Some(tc) = &c.tr_cont {
            req = req.header("tr_cont", tc);
        }

        if c.is_post {
            if self.config.use_hashkey || c.needs_hashkey {
                let hash = self.auth.hashkey(&c.params).await?;
                req = req.header("hashkey", hash);
            }
            req = req.json(&c.params);
        } else {
            let obj = c.params.as_object().ok_or_else(|| {
                KisError::Decode("query params must be object".into())
            })?;
            let pairs: Vec<(String, String)> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_scalar_to_string(v)))
                .collect();
            req = req.query(&pairs);
        }

        let resp = req.send().await?;
        let tr_cont = resp
            .headers()
            .get("tr_cont")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);

        let status = resp.status();
        let body: Value = resp.json().await?;

        let rt_cd = body.get("rt_cd").and_then(|v| v.as_str());
        match rt_cd {
            Some("0") => Ok(RawResponse { body, tr_cont }),
            Some(code) => Err(KisError::Api {
                rt_cd: code.to_string(),
                msg_cd: body
                    .get("msg_cd")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                msg: body
                    .get("msg1")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            }),
            // rt_cd 없는 응답(예: 토큰 만료 HTTP 에러 body) → Auth/Decode
            None => Err(KisError::Decode(format!(
                "no rt_cd in response (http {status}): {body}"
            ))),
        }
    }
}
```

검증: T8에서 빌드.
커밋: `feat: KisClient + KisResponse envelope + raw_call`

---

## T8 — `src/domestic_stock/mod.rs` + 빌드 통과

`DomesticStock` 액세서, 계좌 필드 자동주입 헬퍼, 공통 query 빌더.
`lib.rs`에 `domestic_stock` 하위모듈 선언 추가.

> **T9~T11 공통 지침**: 모든 `ApiCall { .. }` 리터럴에 `needs_hashkey: false`
> 필드를 포함한다 (아래 T9·T10·T11 코드의 `ApiCall` 리터럴에는 생략돼 있음 —
> 실행자가 각 리터럴에 `needs_hashkey: false`를 추가). 국내주식 주문 POST도
> `false` — hashkey는 `KisConfig.use_hashkey`로 전역 제어, KIS 비강제(T6 주석 참조).

`src/domestic_stock/mod.rs`:

```rust
//! 국내주식 도메인 — 주문·계좌·시세 TR.

mod account;
mod order;
mod quote;

pub use account::*;
pub use order::*;
pub use quote::*;

use crate::client::KisClient;

/// 국내주식 도메인 액세서. `client.domestic_stock()`으로 획득.
pub struct DomesticStock<'a> {
    pub(crate) client: &'a KisClient,
}

impl<'a> DomesticStock<'a> {
    pub(crate) fn new(client: &'a KisClient) -> Self {
        Self { client }
    }

    /// 요청 object에 CANO/ACNT_PRDT_CD 주입. 주문·계좌 TR 공용.
    pub(crate) fn with_account(&self, mut params: serde_json::Value) -> serde_json::Value {
        let cfg = self.client.config();
        if let Some(obj) = params.as_object_mut() {
            obj.insert("CANO".into(), cfg.account_no.clone().into());
            obj.insert("ACNT_PRDT_CD".into(), cfg.account_product.clone().into());
        }
        params
    }
}
```

`src/lib.rs`의 `pub mod domestic_stock;`는 T1에서 이미 선언됨 — 확인만.

이 시점에 `quote.rs`/`order.rs`/`account.rs`가 없어 빌드 실패.
**T8은 mod.rs까지만 작성, 빌드 검증은 T11 끝에서.**

커밋: `feat: DomesticStock 액세서 + 계좌 주입 헬퍼`

---

## T9 — `src/domestic_stock/quote.rs`

시세 4종: 현재가(TR9), 호가/예상체결(TR10), 기간별시세(TR11), 당일분봉(TR12).
모두 GET, hashkey 불필요, tr_id 실전·모의 동일(`TrId::same`).

> **응답 struct 필드 규칙**: 모든 필드 `String`(KIS가 숫자도 문자열 반환).
> 필드명은 `docs/kis-api/domestic-stock.md`의 해당 TR 응답표를 **그대로** 사용.
> 아래 코드는 각 struct의 시작 필드 3개를 템플릿으로 보이고, 나머지는
> "§N 응답표 verbatim"으로 지시 — 실행자는 doc 표의 전 행을 `필드명: String`으로 전사.
> 편의 변환은 §자유 메서드에서 제공.

```rust
use serde::Deserialize;

use crate::client::ApiCall;
use crate::domestic_stock::DomesticStock;
use crate::error::Result;
use crate::trid::TrId;

// ── TR 9: 주식현재가 시세 ────────────────────────────────────────────
const TR_PRICE: TrId = TrId::same("FHKST01010100");

/// 주식현재가 시세 응답. 필드 전체는 docs/kis-api/domestic-stock.md §9.
#[derive(Debug, Clone, Deserialize)]
pub struct CurrentPrice {
    pub iscd_stat_cls_code: String, // 종목 상태 구분 코드
    pub stck_prpr: String,          // 주식 현재가
    pub prdy_vrss: String,          // 전일 대비
    // ── 나머지 필드: docs/kis-api/domestic-stock.md §9 응답표(`output`)의
    //    전 행을 `필드명: String`으로 전사. 총 ~80개 필드. ──
}

impl CurrentPrice {
    /// 현재가를 f64로 파싱.
    pub fn price(&self) -> Option<f64> {
        self.stck_prpr.trim().parse().ok()
    }
}

// ── TR 10: 주식현재가 호가/예상체결 ──────────────────────────────────
const TR_ASKING: TrId = TrId::same("FHKST01010200");

/// 호가 정보 (output1). 필드 전체는 §10 output1 표.
#[derive(Debug, Clone, Deserialize)]
pub struct AskingPrice {
    pub aspr_acpt_hour: String, // 호가 접수 시간
    pub askp1: String,          // 매도호가1
    pub bidp1: String,          // 매수호가1
    // ── 나머지: §10 output1 표 verbatim. askp1~10/bidp1~10/잔량/증감 등. ──
}

/// 예상체결 정보 (output2). 필드 전체는 §10 output2 표.
#[derive(Debug, Clone, Deserialize)]
pub struct ExpectedConclusion {
    pub antc_cnpr: String,       // 예상 체결가
    pub antc_cntg_vrss: String,  // 예상 체결 대비
    pub antc_vol: String,        // 예상 거래량
    // ── 나머지: §10 output2 표 verbatim. ──
}

// ── TR 11: 국내주식기간별시세 ────────────────────────────────────────
const TR_PERIOD: TrId = TrId::same("FHKST03010100");

/// 기간별시세 종목 요약 (output1). 필드 전체는 §11 output1 표.
#[derive(Debug, Clone, Deserialize)]
pub struct PeriodSummary {
    pub stck_prpr: String,       // 주식 현재가
    pub hts_kor_isnm: String,    // HTS 한글 종목명
    pub acml_vol: String,        // 누적 거래량
    // ── 나머지: §11 output1 표 verbatim. ──
}

/// 기간별 봉 1건 (output2 배열 요소). 필드 전체는 §11 output2 표.
#[derive(Debug, Clone, Deserialize)]
pub struct PeriodCandle {
    pub stck_bsop_date: String,  // 주식 영업 일자
    pub stck_clpr: String,       // 주식 종가
    pub stck_oprc: String,       // 주식 시가
    // ── 나머지: §11 output2 표 verbatim. stck_hgpr/lwpr/acml_vol 등. ──
}

/// 기간 분류. D=일/W=주/M=월/Y=년.
#[derive(Debug, Clone, Copy)]
pub enum Period { Daily, Weekly, Monthly, Yearly }

impl Period {
    fn code(self) -> &'static str {
        match self {
            Period::Daily => "D",
            Period::Weekly => "W",
            Period::Monthly => "M",
            Period::Yearly => "Y",
        }
    }
}

// ── TR 12: 주식당일분봉조회 ──────────────────────────────────────────
const TR_MINUTE: TrId = TrId::same("FHKST03010200");

/// 분봉 종목 요약 (output1). 필드 전체는 §12 output1 표.
#[derive(Debug, Clone, Deserialize)]
pub struct MinuteSummary {
    pub stck_prpr: String,       // 주식 현재가
    pub hts_kor_isnm: String,    // HTS 한글 종목명
    pub acml_vol: String,        // 누적 거래량
    // ── 나머지: §12 output1 표 verbatim. ──
}

/// 분봉 1건 (output2 배열 요소). 필드 전체는 §12 output2 표.
#[derive(Debug, Clone, Deserialize)]
pub struct MinuteCandle {
    pub stck_bsop_date: String,  // 주식 영업일자
    pub stck_cntg_hour: String,  // 주식 체결시간
    pub stck_prpr: String,       // 주식 현재가
    // ── 나머지: §12 output2 표 verbatim. stck_oprc/hgpr/lwpr/cntg_vol 등. ──
}

impl DomesticStock<'_> {
    /// 주식현재가 시세 (TR 9). `market`: J=KRX, NX=NXT, UN=통합.
    pub async fn current_price(&self, stock_code: &str) -> Result<CurrentPrice> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/inquire-price".into(),
                tr_id: TR_PRICE.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": "J",
                    "FID_INPUT_ISCD": stock_code,
                }),
                is_post: false,
            })
            .await?;
        resp.field("output")
    }

    /// 주식현재가 호가/예상체결 (TR 10). (호가, 예상체결) 튜플 반환.
    pub async fn asking_price(
        &self,
        stock_code: &str,
    ) -> Result<(AskingPrice, ExpectedConclusion)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/quotations/inquire-asking-price-exp-ccn"
                    .into(),
                tr_id: TR_ASKING.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": "J",
                    "FID_INPUT_ISCD": stock_code,
                }),
                is_post: false,
            })
            .await?;
        Ok((resp.field("output1")?, resp.field("output2")?))
    }

    /// 국내주식기간별시세 (TR 11). 최대 100건. (요약, 봉배열) 반환.
    pub async fn period_price(
        &self,
        stock_code: &str,
        start: &str, // YYYYMMDD
        end: &str,   // YYYYMMDD
        period: Period,
        adjusted: bool, // true=수정주가
    ) -> Result<(PeriodSummary, Vec<PeriodCandle>)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path:
                    "/uapi/domestic-stock/v1/quotations/inquire-daily-itemchartprice"
                        .into(),
                tr_id: TR_PERIOD.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": "J",
                    "FID_INPUT_ISCD": stock_code,
                    "FID_INPUT_DATE_1": start,
                    "FID_INPUT_DATE_2": end,
                    "FID_PERIOD_DIV_CODE": period.code(),
                    "FID_ORG_ADJ_PRC": if adjusted { "0" } else { "1" },
                }),
                is_post: false,
            })
            .await?;
        Ok((resp.field("output1")?, resp.field("output2")?))
    }

    /// 주식당일분봉조회 (TR 12). `time` HHMMSS, 최대 30건.
    pub async fn minute_chart(
        &self,
        stock_code: &str,
        time: &str,
        include_past: bool,
    ) -> Result<(MinuteSummary, Vec<MinuteCandle>)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path:
                    "/uapi/domestic-stock/v1/quotations/inquire-time-itemchartprice"
                        .into(),
                tr_id: TR_MINUTE.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": "J",
                    "FID_INPUT_ISCD": stock_code,
                    "FID_INPUT_HOUR_1": time,
                    "FID_PW_DATA_INCU_YN": if include_past { "Y" } else { "N" },
                    "FID_ETC_CLS_CODE": "",
                }),
                is_post: false,
            })
            .await?;
        Ok((resp.field("output1")?, resp.field("output2")?))
    }
}
```

검증: T11에서 빌드.
커밋: `feat: 국내주식 시세 TR 4종 (현재가/호가/기간/분봉)`

---

## T10 — `src/domestic_stock/order.rs`

주문 4종: 매수(TR1), 매도(TR2), 정정(TR3), 취소(TR4). 모두 POST.
정정·취소는 동일 API `order-rvsecncl` — 내부 1개 함수 공유, 공개 `revise`/`cancel` 2개.

```rust
use serde::Deserialize;

use crate::client::ApiCall;
use crate::domestic_stock::DomesticStock;
use crate::error::Result;
use crate::trid::TrId;

const TR_BUY: TrId = TrId::both("TTTC0012U", "VTTC0012U");
const TR_SELL: TrId = TrId::both("TTTC0011U", "VTTC0011U");
const TR_RVSECNCL: TrId = TrId::both("TTTC0013U", "VTTC0013U");

/// 주문 응답 (output). 매수/매도/정정/취소 공통.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct OrderResult {
    /// 한국거래소전송주문조직번호 — 정정/취소 시 사용.
    pub krx_fwdg_ord_orgno: String,
    /// 주문번호 — 정정/취소 시 사용.
    pub odno: String,
    /// 주문시각.
    pub ord_tmd: String,
}
```

> 주: 매수/매도 응답 키는 대문자(`KRX_FWDG_ORD_ORGNO`), 정정/취소 응답 키는
> 소문자(`krx_fwdg_ord_orgno`) — doc §1~4 확인. `#[serde(alias)]`로 양쪽 수용:

```rust
// OrderResult 재정의 — 대/소문자 키 모두 수용
#[derive(Debug, Clone, Deserialize)]
pub struct OrderResult {
    #[serde(alias = "KRX_FWDG_ORD_ORGNO", alias = "krx_fwdg_ord_orgno")]
    pub krx_fwdg_ord_orgno: String,
    #[serde(alias = "ODNO", alias = "odno")]
    pub odno: String,
    #[serde(alias = "ORD_TMD", alias = "ord_tmd")]
    pub ord_tmd: String,
}

/// 주문 구분 — KIS `ORD_DVSN` 코드. 자주 쓰는 2종 + 임의 코드 탈출구.
#[derive(Debug, Clone)]
pub enum OrderType {
    /// "00" 지정가.
    Limit,
    /// "01" 시장가.
    Market,
    /// 그 외 ORD_DVSN 코드 (예: "02" 조건부지정가, "03" 최유리지정가 등).
    Code(String),
}

impl OrderType {
    /// query/body에 넣을 ORD_DVSN 코드 문자열.
    pub(crate) fn code(&self) -> &str {
        match self {
            OrderType::Limit => "00",
            OrderType::Market => "01",
            OrderType::Code(c) => c,
        }
    }
}

/// 매수/매도 주문 파라미터.
#[derive(Debug, Clone)]
pub struct OrderReq {
    /// 종목코드 6자리.
    pub stock_code: String,
    pub order_type: OrderType,
    /// 주문수량.
    pub quantity: u64,
    /// 주문단가. 시장가는 0.
    pub price: u64,
    /// 거래소ID구분코드. 기본 "KRX".
    pub exchange: String,
}

impl OrderReq {
    /// KRX 거래소 기본 주문 파라미터.
    pub fn new(stock_code: impl Into<String>, order_type: OrderType, quantity: u64, price: u64) -> Self {
        Self {
            stock_code: stock_code.into(),
            order_type,
            quantity,
            price,
            exchange: "KRX".into(),
        }
    }
}

/// 정정/취소 파라미터. 원주문의 OrderResult에서 식별자 획득.
#[derive(Debug, Clone)]
pub struct ReviseCancelReq {
    /// 원주문 KRX_FWDG_ORD_ORGNO.
    pub krx_fwdg_ord_orgno: String,
    /// 원주문번호 ODNO.
    pub orig_order_no: String,
    pub order_type: OrderType,
    /// 주문수량. `all=true`면 무시 가능하나 KIS는 값 요구 → 잔량 전달 권장.
    pub quantity: u64,
    /// 주문단가. 취소 시에도 전달.
    pub price: u64,
    /// 잔량 전부 대상이면 true.
    pub all: bool,
    pub exchange: String,
}

impl DomesticStock<'_> {
    /// 현금 매수 (TR 1).
    pub async fn buy(&self, req: OrderReq) -> Result<OrderResult> {
        self.order_cash(req, TR_BUY).await
    }

    /// 현금 매도 (TR 2).
    pub async fn sell(&self, req: OrderReq) -> Result<OrderResult> {
        self.order_cash(req, TR_SELL).await
    }

    async fn order_cash(&self, req: OrderReq, tr: TrId) -> Result<OrderResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "PDNO": req.stock_code,
            "ORD_DVSN": req.order_type.code(),
            "ORD_QTY": req.quantity.to_string(),
            "ORD_UNPR": req.price.to_string(),
            "EXCG_ID_DVSN_CD": req.exchange,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/domestic-stock/v1/trading/order-cash".into(),
                tr_id: tr.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
            })
            .await?;
        resp.field("output")
    }

    /// 주문 정정 (TR 3). `RVSE_CNCL_DVSN_CD=01`.
    pub async fn revise(&self, req: ReviseCancelReq) -> Result<OrderResult> {
        self.order_rvsecncl(req, "01").await
    }

    /// 주문 취소 (TR 4). `RVSE_CNCL_DVSN_CD=02`.
    pub async fn cancel(&self, req: ReviseCancelReq) -> Result<OrderResult> {
        self.order_rvsecncl(req, "02").await
    }

    async fn order_rvsecncl(&self, req: ReviseCancelReq, dvsn: &str) -> Result<OrderResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "KRX_FWDG_ORD_ORGNO": req.krx_fwdg_ord_orgno,
            "ORGN_ODNO": req.orig_order_no,
            "ORD_DVSN": req.order_type.code(),
            "RVSE_CNCL_DVSN_CD": dvsn,
            "ORD_QTY": req.quantity.to_string(),
            "ORD_UNPR": req.price.to_string(),
            "QTY_ALL_ORD_YN": if req.all { "Y" } else { "N" },
            "EXCG_ID_DVSN_CD": req.exchange,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/domestic-stock/v1/trading/order-rvsecncl".into(),
                tr_id: TR_RVSECNCL.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
            })
            .await?;
        resp.field("output")
    }
}
```

검증: T11에서 빌드.
커밋: `feat: 국내주식 주문 TR 4종 (매수/매도/정정/취소)`

---

## T11 — `src/domestic_stock/account.rs` + 빌드·테스트 통과

계좌·체결 4종: 정정취소가능조회(TR5), 잔고(TR6), 매수가능(TR7), 일별주문체결(TR8).
모두 GET. TR5·6·8은 연속조회 지원 → envelope 반환 + `*_all` 헬퍼.

```rust
use serde::Deserialize;

use crate::client::{ApiCall, KisResponse};
use crate::domestic_stock::DomesticStock;
use crate::error::Result;
use crate::trid::TrId;

const TR_PSBL_RVSECNCL: TrId = TrId::real_only("TTTC0084R"); // 모의 미확인 → real_only
const TR_BALANCE: TrId = TrId::both("TTTC8434R", "VTTC8434R");
const TR_PSBL_ORDER: TrId = TrId::both("TTTC8908R", "VTTC8908R");
const TR_DAILY_CCLD: TrId = TrId::both("TTTC0081R", "VTTC0081R"); // 3개월 이내

/// 정정취소가능주문 1건 (TR5 output 배열 요소). 필드 전체는 §5 응답표.
#[derive(Debug, Clone, Deserialize)]
pub struct RevisableOrder {
    pub odno: String,            // 주문번호
    pub orgn_odno: String,       // 원주문번호
    pub psbl_qty: String,        // 정정취소가능수량
    // ── 나머지: docs/kis-api/domestic-stock.md §5 응답표 verbatim. ──
}

/// 보유종목 1건 (TR6 output1 요소). 필드 전체는 §6 output1 표.
#[derive(Debug, Clone, Deserialize)]
pub struct BalanceItem {
    pub pdno: String,            // 상품번호(종목코드)
    pub prdt_name: String,       // 상품명
    pub hldg_qty: String,        // 보유수량
    // ── 나머지: §6 output1 표 verbatim (~26 필드). ──
}

/// 계좌 요약 (TR6 output2). 필드 전체는 §6 output2 표.
#[derive(Debug, Clone, Deserialize)]
pub struct BalanceSummary {
    pub dnca_tot_amt: String,    // 예수금총금액
    pub tot_evlu_amt: String,    // 총평가금액
    pub nass_amt: String,        // 순자산금액
    // ── 나머지: §6 output2 표 verbatim. ──
}

/// 매수가능 정보 (TR7 output). 필드 전체는 §7 응답표.
#[derive(Debug, Clone, Deserialize)]
pub struct BuyableInfo {
    pub ord_psbl_cash: String,   // 주문가능현금
    pub nrcvb_buy_amt: String,   // 미수없는매수금액
    pub nrcvb_buy_qty: String,   // 미수없는매수수량
    // ── 나머지: §7 응답표 verbatim. ──
}

/// 주문체결 1건 (TR8 output1 요소). 필드 전체는 §8 output1 표.
#[derive(Debug, Clone, Deserialize)]
pub struct DailyConclusion {
    pub ord_dt: String,          // 주문일자
    pub odno: String,            // 주문번호
    pub pdno: String,            // 상품번호
    // ── 나머지: §8 output1 표 verbatim (~37 필드). ──
}

/// 주문체결 합계 (TR8 output2). 필드 전체는 §8 output2 표.
#[derive(Debug, Clone, Deserialize)]
pub struct DailyConclusionSummary {
    pub tot_ord_qty: String,     // 총주문수량
    pub tot_ccld_qty: String,    // 총체결수량
    pub tot_ccld_amt: String,    // 총체결금액
    pub pchs_avg_pric: String,   // 매입평균가격
    pub prsm_tlex_smtl: String,  // 추정제비용합계
}

/// 매도/매수 구분. 잔고·체결 조회 필터.
#[derive(Debug, Clone, Copy)]
pub enum SellBuy { All, Sell, Buy }

impl SellBuy {
    fn code(self) -> &'static str {
        match self {
            SellBuy::All => "00",
            SellBuy::Sell => "01",
            SellBuy::Buy => "02",
        }
    }
}

impl DomesticStock<'_> {
    /// 주식잔고조회 (TR 6). 한 페이지. envelope의 `data`는 (보유종목, 요약).
    pub async fn balance(
        &self,
        cursor: Option<(&str, &str)>, // (ctx_area_fk, ctx_area_nk)
    ) -> Result<KisResponse<(Vec<BalanceItem>, Vec<BalanceSummary>)>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "AFHR_FLPR_YN": "N",
            "OFL_YN": "",
            "INQR_DVSN": "02",
            "UNPR_DVSN": "01",
            "FUND_STTL_ICLD_YN": "N",
            "FNCG_AMT_AUTO_RDPT_YN": "N",
            "PRCS_DVSN": "00",
            "CTX_AREA_FK100": fk,
            "CTX_AREA_NK100": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/trading/inquire-balance".into(),
                tr_id: TR_BALANCE.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
            })
            .await?;
        let items: Vec<BalanceItem> = resp.field("output1")?;
        let summary: Vec<BalanceSummary> = resp.field("output2")?;
        Ok(resp.envelope((items, summary)))
    }

    /// 잔고 전체 페이지 수집.
    pub async fn balance_all(&self) -> Result<(Vec<BalanceItem>, Vec<BalanceSummary>)> {
        let mut items = Vec::new();
        let mut summary = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let page = self
                .balance(cursor.as_ref().map(|(f, n)| (f.as_str(), n.as_str())))
                .await?;
            let (mut it, mut su) = page.data;
            items.append(&mut it);
            summary.append(&mut su);
            if page.has_next() {
                match (page.ctx_area_fk, page.ctx_area_nk) {
                    (Some(f), Some(n)) => cursor = Some((f, n)),
                    _ => break,
                }
            } else {
                break;
            }
        }
        Ok((items, summary))
    }

    /// 매수가능조회 (TR 7). `order_type`은 시장가 권장(증거금율 반영).
    pub async fn buyable(
        &self,
        stock_code: &str,
        price: u64,
        order_type: super::OrderType,
    ) -> Result<BuyableInfo> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "PDNO": stock_code,
            "ORD_UNPR": price.to_string(),
            "ORD_DVSN": order_type.code_pub(),
            "CMA_EVLU_AMT_ICLD_YN": "N",
            "OVRS_ICLD_YN": "N",
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/trading/inquire-psbl-order".into(),
                tr_id: TR_PSBL_ORDER.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: false,
            })
            .await?;
        resp.field("output")
    }

    /// 주식일별주문체결조회 (TR 8). 3개월 이내. 한 페이지.
    /// envelope의 `data`는 (체결내역, 합계).
    pub async fn daily_conclusions(
        &self,
        start: &str, // YYYYMMDD
        end: &str,
        sell_buy: SellBuy,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<(Vec<DailyConclusion>, DailyConclusionSummary)>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "INQR_STRT_DT": start,
            "INQR_END_DT": end,
            "SLL_BUY_DVSN_CD": sell_buy.code(),
            "CCLD_DVSN": "00",
            "INQR_DVSN": "00",
            "INQR_DVSN_3": "00",
            "PDNO": "",
            "ORD_GNO_BRNO": "",
            "ODNO": "",
            "INQR_DVSN_1": "",
            "EXCG_ID_DVSN_CD": "KRX",
            "CTX_AREA_FK100": fk,
            "CTX_AREA_NK100": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/trading/inquire-daily-ccld".into(),
                tr_id: TR_DAILY_CCLD.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
            })
            .await?;
        let items: Vec<DailyConclusion> = resp.field("output1")?;
        let summary: DailyConclusionSummary = resp.field("output2")?;
        Ok(resp.envelope((items, summary)))
    }

    /// 정정취소가능주문조회 (TR 5). `inqr_dvsn_2`: All/Sell/Buy 필터.
    pub async fn revisable_orders(
        &self,
        sell_buy: SellBuy,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<Vec<RevisableOrder>>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "INQR_DVSN_1": "0",
            "INQR_DVSN_2": sell_buy.code_one_digit(),
            "CTX_AREA_FK100": fk,
            "CTX_AREA_NK100": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl".into(),
                tr_id: TR_PSBL_RVSECNCL.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
            })
            .await?;
        let items: Vec<RevisableOrder> = resp.field("output")?;
        Ok(resp.envelope(items))
    }
}
```

> **연결 수정 필요**: `OrderType::code`는 T10에서 이미 `pub(crate) fn code(&self)`로
> 정의됨. 위 `account.rs` 코드의 `order_type.code_pub()` → `order_type.code()`로 교체.
>
> `SellBuy`는 `code()`(2자리, TR8용)와 `code_one_digit()`(1자리, TR5 INQR_DVSN_2:
> 0=전체/1=매도/2=매수) 둘 다 필요 — `SellBuy`에 메서드 2개 정의:
> ```rust
> fn code_one_digit(self) -> &'static str {
>     match self { SellBuy::All => "0", SellBuy::Sell => "1", SellBuy::Buy => "2" }
> }
> ```

빌드·테스트 통과 검증:

```
cargo build
cargo clippy -- -D warnings
cargo test
```

기대 출력: `cargo build` 에러 0. `cargo test` — trid 3개, ratelimit 1개,
auth 2개 통과 (총 6 passed).

커밋: `feat: 국내주식 계좌·체결 TR 4종 + 빌드 통과`

---

## T12 — `examples/domestic_quote.rs`

```rust
//! 현재가 조회 예제. 실행: KIS_* 환경변수 설정 후
//! `cargo run --example domestic_quote -- 005930`

use kis_adapter::{KisClient, KisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let code = std::env::args().nth(1).unwrap_or_else(|| "005930".into());

    let client = KisClient::new(KisConfig::from_env()?)?;
    let ds = client.domestic_stock();

    let price = ds.current_price(&code).await?;
    println!("종목 {code} 현재가: {}", price.stck_prpr);
    println!("전일대비: {}", price.prdy_vrss);

    let (asking, expected) = ds.asking_price(&code).await?;
    println!("매도1호가: {} / 매수1호가: {}", asking.askp1, asking.bidp1);
    println!("예상체결가: {}", expected.antc_cnpr);

    Ok(())
}
```

검증: `cargo build --example domestic_quote` 에러 0.
(자격증명 있으면 `cargo run --example domestic_quote -- 005930` 수동 확인.)

커밋: `docs: 현재가 조회 예제`

---

## T13 — `examples/domestic_order.rs`

```rust
//! 잔고 + 매수가능 조회 예제 (주문은 실행 안 함 — 안전).
//! 실행: `cargo run --example domestic_order`

use kis_adapter::domestic_stock::OrderType;
use kis_adapter::{KisClient, KisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let client = KisClient::new(KisConfig::from_env()?)?;
    let ds = client.domestic_stock();

    let (items, summary) = ds.balance_all().await?;
    println!("보유종목 {}건", items.len());
    for it in &items {
        println!("  {} {} 보유 {}주", it.pdno, it.prdt_name, it.hldg_qty);
    }
    if let Some(s) = summary.first() {
        println!("총평가금액: {}", s.tot_evlu_amt);
    }

    let buyable = ds.buyable("005930", 70000, OrderType::Market).await?;
    println!("삼성전자 매수가능: {}주", buyable.nrcvb_buy_qty);

    Ok(())
}
```

> `OrderType`이 공개 경로에 노출돼야 함. `domestic_stock/mod.rs`의 `pub use order::*;`가
> `OrderType`을 재노출 — 확인. `kis_adapter::domestic_stock::OrderType` 경로 유효.

검증: `cargo build --example domestic_order` 에러 0.
커밋: `docs: 잔고·매수가능 조회 예제`

---

## T14 — `tests/integration.rs`

모의환경 실호출 스모크. 자격증명 없으면 skip (`#[ignore]`).

```rust
//! 모의투자 환경 통합 스모크 테스트.
//! 실행: KIS_* 환경변수 설정 후 `cargo test --test integration -- --ignored`

use kis_adapter::{KisClient, KisConfig};

fn client() -> Option<KisClient> {
    let config = KisConfig::from_env().ok()?;
    KisClient::new(config).ok()
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn token_and_current_price() {
    let client = client().expect("KIS_* env vars + valid config");
    let ds = client.domestic_stock();
    let price = ds
        .current_price("005930")
        .await
        .expect("current price call");
    assert!(!price.stck_prpr.is_empty(), "현재가 비어있지 않음");
    // 토큰 재사용 — 2번째 호출도 성공해야 함
    let again = ds.current_price("000660").await.expect("second call");
    assert!(!again.stck_prpr.is_empty());
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn balance_query() {
    let client = client().expect("KIS_* env vars");
    let ds = client.domestic_stock();
    // 잔고는 비어 있어도 호출 자체는 성공해야 함 (rt_cd=0)
    let (_items, summary) = ds.balance_all().await.expect("balance call");
    assert!(!summary.is_empty(), "계좌 요약 1건 이상");
}
```

검증: `cargo test --test integration` — ignored 2개 표시(미실행).
자격증명 있으면 `cargo test --test integration -- --ignored` 통과.

커밋: `test: 모의환경 통합 스모크 테스트`

---

## T15 — `README.md` + 최종 검증

`README.md` 작성:

```markdown
# kis-adapter

한국투자증권(KIS) OpenAPI Rust 어댑터.

## 현재 범위 (Plan 1)

- 국내주식 12개 TR — 시세·주문·계좌·체결
- 실전/모의투자 환경
- 토큰 자동 발급·캐싱, 레이트리밋, 연속조회

해외주식·선물옵션은 Plan 2, 실시간 WebSocket은 Plan 3에서 추가.

## 사용법

\`\`\`rust
use kis_adapter::{KisClient, KisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KisClient::new(KisConfig::from_env()?)?;
    let price = client.domestic_stock().current_price("005930").await?;
    println!("{}", price.stck_prpr);
    Ok(())
}
\`\`\`

## 환경변수

| 변수 | 설명 |
|------|------|
| `KIS_APP_KEY` | 앱 키 |
| `KIS_APP_SECRET` | 앱 시크릿 |
| `KIS_ACCOUNT_NO` | 계좌번호 8자리 |
| `KIS_ACCOUNT_PRODUCT` | 계좌상품코드 2자리 |
| `KIS_ENV` | `real` 또는 `mock` (기본 mock) |

## 라이선스

MIT
```

최종 검증 — 전체 명령 실행, 출력 확인:

```
cargo build
cargo build --examples
cargo clippy -- -D warnings
cargo test
```

기대: build 에러 0, clippy 경고 0, test 6 passed (+ ignored 2).

커밋: `docs: README + Plan 1 완료`

---

## Plan 1 완료 기준

- [ ] `cargo build` / `cargo build --examples` 에러 0
- [ ] `cargo clippy -- -D warnings` 경고 0
- [ ] `cargo test` — 단위 6건 통과, 통합 2건 ignored
- [ ] 자격증명 있으면 `cargo test --test integration -- --ignored` 통과
- [ ] 국내주식 12개 TR 메서드 전부 노출 (`current_price` `asking_price`
      `period_price` `minute_chart` `buy` `sell` `revise` `cancel`
      `balance`/`balance_all` `buyable` `daily_conclusions` `revisable_orders`)
- [ ] `raw_call` 동작 (raw envelope 반환)

## 다음

Plan 2 — `docs/kis-api/overseas-stock.md` + `futureoption.md` 기반으로
`overseas_stock/`·`futureoption/` 모듈을 동일 패턴으로 작성.
