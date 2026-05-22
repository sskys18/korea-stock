# KIS Adapter — 한국투자증권 OpenAPI Rust 어댑터 설계

- 작성일: 2026-05-22
- 상태: 승인 대기

## 1. 목적

한국투자증권(KIS) OpenAPI를 Rust에서 타입 안전하게 사용하는 어댑터 라이브러리.
국내주식·해외주식·국내선물옵션 거래/조회 + 실시간 WebSocket 시세를 단일 크레이트로 제공.

## 2. 스코프

| 항목 | 결정 |
|------|------|
| 도메인 | 국내주식, 해외주식, 국내선물옵션, 실시간 WebSocket |
| 실행 모델 | async (tokio) |
| 환경 | 실전 + 모의투자 둘 다 (런타임 분기) |
| 산출물 | lib crate `kis-adapter` + `examples/` CLI 바이너리 |
| API 커버리지 접근 | **하이브리드(C)** — 공통 TR은 완전 타입 struct, 미구현 TR은 `raw_call` 탈출구 |
| AES 복호화 | 순수 Rust (`aes` + `cbc` crate), openssl 미사용 |

비스코프: 채권, ELW, 해외선물옵션, 업종/지수 전용 TR. 필요 시 `raw_call`로 대응.

## 3. 아키텍처

### 3.1 크레이트 구조

저장소 루트(`korea-stock/`)가 곧 크레이트 — 중첩 디렉토리 없음. 패키지명 `kis-adapter`.

```
korea-stock/
├── Cargo.toml          # [package] name = "kis-adapter"
├── src/
│   ├── lib.rs            # 공개 API 재노출, 모듈 선언
│   ├── config.rs         # KisConfig, Environment{Real,Mock}, base URL
│   ├── error.rs          # KisError (thiserror)
│   ├── auth.rs           # OAuth 토큰 발급/폐기/캐싱, hashkey 발급
│   ├── ratelimit.rs      # 토큰버킷 레이트리미터
│   ├── client.rs         # KisClient — reqwest, 공통 헤더, raw_call
│   ├── trid.rs           # TR ID 환경별 분기 매핑
│   ├── domestic_stock/
│   │   ├── mod.rs
│   │   ├── order.rs      # 매수/매도/정정/취소/정정취소가능조회
│   │   ├── account.rs    # 잔고조회, 매수가능조회, 일별주문체결조회
│   │   └── quote.rs      # 현재가, 호가/예상체결, 일자별, 분봉, 체결
│   ├── overseas_stock/
│   │   ├── mod.rs
│   │   ├── order.rs      # 매수/매도/정정취소
│   │   ├── account.rs    # 잔고, 미체결, 체결내역
│   │   └── quote.rs      # 현재가, 기간별시세
│   ├── futureoption/
│   │   ├── mod.rs
│   │   ├── order.rs      # 주문, 정정취소
│   │   ├── account.rs    # 잔고, 주문체결내역
│   │   └── quote.rs      # 현재가시세, 호가
│   └── realtime/
│       ├── mod.rs        # RealtimeClient
│       ├── approval.rs   # approval_key 발급
│       ├── subscribe.rs  # 구독/해지 프레임, 구독 상태 관리
│       ├── decode.rs     # 파이프(|) 구분 페이로드 파싱
│       └── crypto.rs     # AES256-CBC 복호화 (체결통보)
├── examples/
│   ├── domestic_quote.rs
│   ├── domestic_order.rs
│   ├── overseas_quote.rs
│   └── realtime_feed.rs
└── tests/
    └── integration.rs    # mock 환경 실호출, #[ignore] 게이트
```

### 3.2 핵심 타입

```rust
// config.rs
pub enum Environment { Real, Mock }

pub struct KisConfig {
    pub app_key: String,
    pub app_secret: String,
    pub account_no: String,      // 계좌 8자리
    pub account_product: String, // 상품코드 2자리 (예: "01")
    pub environment: Environment,
    pub token_cache_path: Option<PathBuf>, // 기본 ~/.kis/token.json
    pub use_hashkey: bool,       // POST 주문 hashkey 선택 사용
}

// client.rs
pub struct KisClient {
    config: KisConfig,
    http: reqwest::Client,
    auth: Auth,                  // 토큰 관리 (내부 Mutex)
    limiter: RateLimiter,
}

impl KisClient {
    pub fn new(config: KisConfig) -> Result<Self>;
    pub fn domestic_stock(&self) -> DomesticStock<'_>;
    pub fn overseas_stock(&self) -> OverseasStock<'_>;
    pub fn futureoption(&self) -> FutureOption<'_>;
    pub async fn realtime(&self) -> Result<RealtimeClient>;

    /// 탈출구 — 미구현 TR 직접 호출
    pub async fn raw_call(&self, req: RawRequest) -> Result<KisResponse<serde_json::Value>>;
}

/// 모든 도메인 응답 공통 envelope. 데이터 + 연속조회 메타 보존.
pub struct KisResponse<T> {
    pub data: T,
    pub tr_cont: Option<String>,    // 응답 헤더 — "F"/"M"=다음 페이지 있음
    pub ctx_area_fk: Option<String>, // body cursor (연속조회 키)
    pub ctx_area_nk: Option<String>,
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg: String,
}

pub struct RawRequest {
    pub method: Method,          // GET/POST
    pub path: String,            // "/uapi/..."
    pub tr_id: String,
    pub tr_cont: Option<String>, // 연속조회
    pub params: serde_json::Value, // GET=query, POST=body
    pub is_post: bool,
    pub needs_hashkey: bool,
}
```

### 3.3 인증 (auth.rs)

- `POST /oauth2/tokenP` → access token (유효 24h).
- 토큰 캐싱: `token_cache_path` JSON 파일에 `{access_token, expires_at}` 저장.
  로드 시 만료 1시간 전이면 재발급. **KIS 토큰 발급은 1분 1회 제한** → 캐시 우선, 발급 실패 시 백오프.
- 캐시 쓰기: 임시 파일 생성 → `chmod 0600` → atomic rename. 권한 설정 실패 시
  캐시를 비활성화(메모리 토큰만 사용)하고 `tracing::warn` — 치명적 오류 아님.
  Windows는 `0600` 무시(NTFS ACL 미적용), 경고만.
- 토큰 폐기: `POST /oauth2/revokeP` (선택, Drop 시 호출 안 함 — 명시적 호출만).
- hashkey: **KIS 어느 TR도 강제 아님** (공식 `kis_auth.py`가 "생략 가능" 명시). POST 주문 시
  변조방지용 선택 기능 — `KisConfig.use_hashkey: bool`(기본 false). true면 POST 호출 전
  `POST /uapi/hashkey`로 body 해시 발급 후 헤더 첨부.
- 내부 `Mutex<TokenState>`로 동시 호출 시 토큰 1회만 발급 (이중 발급 방지).

### 3.4 레이트리밋 (ratelimit.rs)

- 토큰버킷. 실전 20 req/s, 모의 2 req/s (환경에서 결정).
- 모든 REST 호출(`raw_call` 포함) 전에 `limiter.acquire().await`.
- 토큰 발급 호출은 별도 — 버킷 미적용, auth.rs 캐시 로직이 빈도 제어.

### 3.5 TR ID 분기 (trid.rs)

- 다수 TR이 실전/모의에서 접두사만 다름 (`T...U` ↔ `V...U`).
- `fn resolve(real: &str, mock: &str, env: Environment) -> &str` 또는
  상수 쌍 테이블. 도메인 함수는 환경 모름 — `client`가 주입.
- 정확한 tr_id는 `docs/kis-api/*.md`가 SSOT. 주의: order-cash는 신규 ID
  매수 `TTTC0012U`/`VTTC0012U`, 매도 `TTTC0011U`/`VTTC0011U` (레거시 `TTTC080xU` 폐기).
- **시세계 GET API는 실전/모의 tr_id 동일** (`FHK*`/`FHMIF*` 등) — resolve 시 동일값 쌍.
- 모의 미지원 TR(해외 미체결 `TTTS3018R`, 모의 야간선물 등): `KisError::UnsupportedInMock` 반환.

### 3.6 도메인 모듈 패턴

각 TR = 타입 요청 struct + 타입 응답 struct + 메서드.

```rust
// domestic_stock/quote.rs 예시
pub struct CurrentPriceReq { pub stock_code: String }

#[derive(Deserialize)]
pub struct CurrentPrice {
    pub stck_prpr: String,   // 현재가
    pub prdy_vrss: String,   // 전일대비
    // ... KIS output 필드명 유지, 문서 주석 병기
}

impl DomesticStock<'_> {
    // 단건 조회 — envelope 불필요, 데이터만
    pub async fn current_price(&self, req: CurrentPriceReq) -> Result<CurrentPrice>;
    // 목록/연속조회 — envelope 반환, cursor 보존
    pub async fn balance(&self, req: BalanceReq) -> Result<KisResponse<Vec<BalanceItem>>>;
    // 전체 페이지 수집 헬퍼
    pub async fn balance_all(&self, req: BalanceReq) -> Result<Vec<BalanceItem>>;
}
```

- KIS 응답은 숫자도 문자열 → struct는 원본 `String` 유지, 편의 메서드(`as_f64()`)는 보조 제공.
- `rt_cd != "0"`이면 `KisError::Api { rt_cd, msg_cd, msg }` 반환.
- **API 분리**: 단건 조회 메서드는 `Result<T>` 반환. 목록/연속조회 메서드는 `Result<KisResponse<Vec<T>>>` 반환 — 호출자가 `tr_cont`/`ctx_area_*`로 다음 페이지 요청 가능. `*_all` 헬퍼가 envelope를 소비해 끝까지 수집.

### 3.7 실시간 WebSocket (realtime/)

- `ws://ops.koreainvestment.com:21000` (실전) / `ws://ops.koreainvestment.com:31000` (모의).
- `approval_key`: `POST /oauth2/Approval`로 발급 (access token과 별개).
- `approval` 발급 요청 Body 필드명은 `secretkey` (REST 토큰 발급의 `appsecret`과 다름).
- 구독: JSON 프레임 `{header:{tr_type:"1"}, body:{input:{tr_id, tr_key}}}`. 해지 `tr_type:"2"`.
- 수신 데이터: **2단계 구분자** — 프레임 레벨 `|`(파이프), 실데이터 필드 `^`(캐럿).
  `decode.rs`가 `|`로 분해 후 데이터부를 `^`로 분해해 tr_id별 필드 순서 매핑.
- 체결통보(H0STCNI0 실전/H0STCNI9 모의): 구독 응답 `body.output.{key,iv}`에 AES key/iv 포함
  → 이후 데이터 **AES-256-CBC 복호화**(PKCS#7 패딩, Base64 입력 → UTF-8) 후 파싱.
  `tr_key`는 종목코드 아닌 HTS ID 사용.
- API: `subscribe(kind, key) -> Result<SubscriptionHandle>`. `SubscriptionHandle`은
  `unsubscribe().await` 제공 + `Drop` 시 자동 해지 프레임 전송(best-effort).
  이벤트는 생성 시 받은 단일 `mpsc::Receiver<RealtimeEvent>`로 노출 — 이벤트에 `tr_id`+`tr_key` 포함되어 호출자가 분기.
- 이벤트 채널 용량 고정(기본 1024). 가득 차면 송신측이 블록되지 않도록 새 이벤트를 드롭(drop-newest)하고 `RealtimeEvent::Lagged(n)`으로 유실 건수를 통지.
- 구독 상태는 `RealtimeClient` 내부 `Mutex<HashMap<(tr_id,key), SubState>>`로 관리. 재연결 중 subscribe/unsubscribe는 이 락으로 직렬화.
- 자동 재연결: 연결 끊김 시 지수 백오프 재연결 + 등록된 구독 전체 재전송. PINGPONG 프레임 응답 처리.
- 동시 구독 한도 약 41건 — 초과 시 `subscribe`가 `KisError::Ws` 반환.
- 지원 tr_id: H0STCNT0(국내체결가), H0STASP0(국내호가), H0STCNI0/9(체결통보), HDFSCNT0(해외체결가).

### 3.8 에러 (error.rs)

```rust
#[derive(thiserror::Error, Debug)]
pub enum KisError {
    #[error("http: {0}")] Http(#[from] reqwest::Error),
    #[error("auth: {0}")] Auth(String),
    #[error("api error rt_cd={rt_cd} msg_cd={msg_cd}: {msg}")]
    Api { rt_cd: String, msg_cd: String, msg: String },
    #[error("rate limited")] RateLimit,
    #[error("websocket: {0}")] Ws(String),
    #[error("decode: {0}")] Decode(String),
    #[error("unsupported in mock environment: {tr_id}")] UnsupportedInMock { tr_id: String },
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("json: {0}")] Json(#[from] serde_json::Error),
}
pub type Result<T> = std::result::Result<T, KisError>;
```

## 4. 의존성

```toml
tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time", "fs"] }
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
tokio-tungstenite = { version = "0.24", features = ["rustls-tls-webpki-roots"] }
futures-util = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
chrono = { version = "0.4", features = ["serde"] }
aes = "0.8"
cbc = { version = "0.1", features = ["alloc"] }
base64 = "0.22"
tracing = "0.1"
dirs = "5"          # 토큰 캐시 경로
```

## 5. TR 커버리지 (완전 타입 구현 대상)

### 국내주식 (~12)
주식주문(현금) 매수/매도, 주문정정, 주문취소, 정정취소가능주문조회,
주식잔고조회, 매수가능조회, 주식일별주문체결조회,
주식현재가시세, 현재가호가/예상체결, 국내주식기간별시세, 주식당일분봉.
※ 정정·취소는 동일 API `order-rvsecncl`(`TTTC0013U`) — `RVSE_CNCL_DVSN_CD`(01=정정/02=취소)로 분기.
공개 메서드는 `revise`/`cancel` 2개로 노출하되 내부 1개 호출 공유.
※ 일별주문체결조회 tr_id 2종: 3개월 이내 `TTTC0081R`, 이전 `CTSC9215R` — 조회기간으로 자동 선택.

### 해외주식 (~8)
해외주식주문 매수/매도, 주문정정취소, 해외주식잔고, 해외주식미체결내역,
해외주식주문체결내역, 해외주식현재가, 해외주식기간별시세.

### 국내선물옵션 (~7)
선물옵션주문, 정정취소, 선물옵션잔고현황, 선물옵션주문체결내역조회,
선물옵션 매수가능조회, 선물옵션현재가시세, 선물옵션호가.

### 실시간 WebSocket (~4)
H0STCNT0 국내주식체결가, H0STASP0 국내주식호가,
H0STCNI0/H0STCNI9 체결통보, HDFSCNT0 해외주식체결가.

**타입 구현 커버리지는 위 ~31개 TR로 한정.** 미구현 TR은 `raw_call`로 수동 호출 가능
(저수준 HTTP 탈출구 — 타입 안전성 없음, `serde_json::Value` 반환).
타입 커버리지와 raw 호출 가능성은 별개 범주.

**구현 완료 기준**: (1) 위 31개 TR 타입 메서드 동작, (2) `raw_call` 임의 TR 호출 동작,
(3) WebSocket 4개 tr_id 구독/수신/복호화 동작.

## 6. 검증 전략

- **단위 테스트**: 직렬화/역직렬화, TR ID 환경 분기, 레이트리미터 토큰버킷, AES 복호화(고정 AES-CBC fixture; KIS 공식 KAT 부재 시 자체 fixture).
- **통합 테스트** (`tests/integration.rs`, `#[ignore]`): 모의투자 환경 실호출 — 토큰 발급, 현재가 조회, 잔고 조회, WebSocket 구독 1건. 자격증명은 env var(`KIS_APP_KEY` 등)로 주입, 없으면 skip.
- `cargo build`, `cargo clippy -- -D warnings`, `cargo test`.
- examples 수동 실행 확인.

## 7. 보안

- app key/secret/계좌번호: env var 또는 호출자 주입만. 소스/예제에 하드코딩 금지.
- 토큰 캐시 파일 권한 `0600`.
- `.gitignore`에 `.kis/`, `*.env` 추가.
- 로그(`tracing`)에 토큰/시크릿 마스킹.

## 8. 미해결/위험

- KIS 모의투자는 일부 시세/해외 TR 미지원 → `UnsupportedInMock`으로 명시 차단, 표는 trid.rs에 유지.
- 실시간 WebSocket 동시 구독 한도(약 41건) → 초과 시 `KisError::Ws` 반환.
- **TR 명세 SSOT**: `docs/kis-api/{domestic-stock,overseas-stock,futureoption,realtime}.md`.
  공식 GitHub 샘플(`koreainvestment/open-trading-api`)의 `chk_*.py` `COLUMN_MAPPING`에서 필드명 확정.
  구현 시 struct 필드는 이 문서들을 그대로 따른다.
- 잔여 `[미확인]` 필드(해외 잔고 output 귀속, 일부 모의 지원 여부)는 구현 중 모의 환경 실호출로 확정.
