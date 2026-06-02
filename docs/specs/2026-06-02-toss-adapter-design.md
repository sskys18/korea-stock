# Toss Adapter — 토스증권 Open API Rust 어댑터 설계

- 작성일: 2026-06-02
- 상태: 구현 완료. 기존 KIS 어댑터와 동일 크레이트 내 형제 모듈(`src/toss/`)로 추가.
- 권위 스펙: `docs/toss-api/openapi.json` (토스증권 Open API v1.0.3). 요약 `docs/toss-api/endpoints.md`.

## 1. 목적

토스증권 Open API를 Rust에서 타입 안전하게 사용하는 어댑터. 기존 KIS 어댑터(`KisClient`)와
동일 크레이트 내에서 공존하며, KIS의 파일 레이아웃·문서 주석 밀도·액세서 패턴을 그대로 미러링한다.
국내(KRX)·미국 주식의 시세·종목정보·시장정보·계좌·자산·주문·주문정보 20개 엔드포인트를 커버한다.

## 2. 스코프

| 항목 | 결정 |
|------|------|
| 도메인 | 시세(Market Data), 종목정보(Stock Info), 시장정보(Market Info), 계좌(Account), 자산(Asset), 주문(Order), 주문정보(Order Info) |
| 실행 모델 | async (tokio) — KIS와 동일 |
| 환경 | **단일** (`servers`에 URL 1개). KIS의 실전/모의 `Environment` 분기 없음 |
| 산출물 | 기존 `kis-adapter` 크레이트에 `pub mod toss;` 추가 |
| API 커버리지 접근 | KIS와 동일 하이브리드 — 20개 전부 완전 타입 struct + `raw_call` 탈출구 |

비스코프: 실시간 WebSocket(토스 Open API 미제공), 채권/파생.

## 3. KIS 대비 핵심 차이 (4대 축)

토스 어댑터가 KIS 패턴을 미러링하되 **반드시 달라야 하는** 지점은 인증·envelope·에러·레이트리밋 4축이다.

### 3.1 인증 차이 (auth.rs)

| 항목 | KIS | Toss |
|------|-----|------|
| 엔드포인트 | `POST /oauth2/tokenP` | `POST /oauth2/token` |
| 요청 포맷 | JSON `{grant_type, appkey, appsecret}` | **`application/x-www-form-urlencoded`** `{grant_type=client_credentials, client_id, client_secret}` |
| 응답 | `{access_token, expires_in}` (커스텀) | OAuth2 표준 `{access_token, token_type:"Bearer", expires_in}` |
| 발급 실패 | HTTP 에러 텍스트 | OAuth2 표준 `{error, error_description}` (BFF envelope 아님) |
| 인증 헤더 | `appkey`+`appsecret`+`Bearer` | **`Authorization: Bearer {access_token}` 만** (appkey/secret 헤더 없음) |
| 토큰 무효화 | 명시적 revoke | **재발급 시 이전 토큰 즉시 무효화** (client당 유효 토큰 1개) |

- form-urlencoded는 reqwest `.form(&[...])`로 처리 — **신규 의존성 불필요**.
- 캐싱 전략은 KIS를 그대로 재사용: 메모리 → 파일 → 신규 발급, `tokio::sync::Mutex` single-flight,
  scope 격리(`{base}|{client_id}`), 임시파일 → `chmod 0600` → atomic rename.
- **single-flight가 KIS보다 더 중요하다**: "client당 유효 토큰 1개, 재발급 시 이전 토큰 즉시 무효화"
  이므로 동시 발급이 발생하면 한쪽이 즉시 무효화된 토큰을 들고 호출 → 401. Mutex로 직렬화하여 1회만 발급.
- 만료 판정은 KIS와 동일하게 만료 1시간 전이면 stale 처리(`is_fresh`).

### 3.2 Envelope 차이 (client.rs)

| 항목 | KIS | Toss |
|------|-----|------|
| 성공 페이로드 | body 전체에 `output`/`output1`/`output2` 등 키 | `ApiResponse` → `{ "result": <payload> }`, 단일 `result` |
| 성공/실패 판정 | **HTTP는 항상 200**, body `rt_cd`로 분기 | **HTTP 상태 코드로 분기** (2xx 성공, 4xx/5xx 실패) |
| 메타 | `tr_cont`/`ctx_area_*` 연속조회 | 응답 헤더 `X-Request-Id`, 페이징은 엔드포인트별 body 필드(`nextBefore`/`nextCursor`) |

- KIS의 `rt_cd != "0"` 검사를 **그대로 베끼면 안 된다.** 토스는 HTTP status로 성공/실패를 가른다.
  `call_once`는 `resp.status()`로 분기: 2xx면 `result` 언래핑, 비2xx면 에러 envelope 파싱.
- `TossResponse<T>`는 KIS `KisResponse<T>`처럼 데이터+메타를 감싸지만 메타는 `request_id`(헤더)뿐이다.
  대부분의 도메인 메서드는 envelope 없이 `T`만 반환(KIS 단건 조회와 동일 컨벤션). 연속조회가 있는
  candles/orders는 페이징 필드가 응답 struct(`CandlePage`/`OrderPage`)에 포함되므로 별도 envelope 불필요.

### 3.3 에러 전략 결정 (error 타입)

**결정: KIS의 단일 `KisError`를 확장하지 않고, 병렬 타입 `toss::TossError` + `toss::Result<T>`를 신설한다.**

근거 (리뷰어 도전 예상 지점이므로 명시):
1. **필드 구조가 전혀 겹치지 않는다.** KIS API 에러는 `{rt_cd, msg_cd, msg}` (3-튜플 코드 체계).
   토스 BFF 에러는 `{requestId, code, message, data?}` (flat code + 해결힌트), OAuth2 에러는
   `{error(enum), error_description}`. `KisError::Api`에 토스 필드를 끼워넣으면 의미가 오염된다.
2. **`KisError`라는 이름이 KIS 전용이다.** 크레이트가 KIS 어댑터로 출발했고 enum 이름이 그 정체성을
   담는다. 토스 변형을 추가하면 KIS 사용자에게 무의미한 variant가 노출된다.
3. **제약 준수.** 과제 제약상 KIS 코드는 `lib.rs`/`error.rs` 외 손대지 않는다. 병렬 타입은 `error.rs`를
   건드리지 않고 `toss` 모듈에 자족적으로 위치한다.
4. **크레이트 전략과 모순 아님.** 크레이트는 이미 `KisError` 단일 enum을 KIS 도메인 전역에 쓴다.
   토스는 **별도 도메인**이므로 "도메인당 하나의 에러 enum" 원칙을 그대로 적용 — 토스 도메인의 단일
   enum이 `TossError`다. KIS의 단일-enum 정신을 위반하는 게 아니라 도메인 경계에 맞춰 적용한 것.

```rust
pub enum TossError {
    Http(reqwest::Error),               // 전송 계층
    Auth(String),                       // 토큰 발급/캐시
    OAuth2 { error: String, description: Option<String> },  // /oauth2/token 4xx/5xx
    Api { status: u16, request_id: Option<String>, code: String, message: String }, // BFF 4xx/5xx
    Decode(String),                     // envelope/필드 누락
    Io(std::io::Error),
    Json(serde_json::Error),
}
pub type Result<T> = std::result::Result<T, TossError>;
```

- `AccountHeaderMissing` 변형은 **두지 않는다.** 계좌 헤더는 액세서가 구조적으로 강제(§3.5)하므로 dead code.
- 에러 식별은 토스 권고대로 `code`(BFF) / `error`(OAuth2) 기반. unknown code 허용(문자열 그대로 보존).

### 3.4 레이트리밋 처리 (client.rs + ratelimit.rs 재사용)

토스 레이트리밋은 **그룹별(MARKET_DATA, MARKET_DATA_CHART, ORDER, ASSET, ...)** 이고
응답 헤더로만 통보된다(호출 전 알 수 없음). 따라서 **반응형(reactive)** 이 권위 메커니즘이다.

- 429 수신 시 `Retry-After` 헤더(초) → 없으면 `X-RateLimit-Reset` → 없으면 지수 백오프 순으로
  대기 후 재시도. KIS의 `EGW00201` 백오프 루프 구조를 그대로 가져오되 트리거를 429 + Retry-After로 교체.
  `MAX_RETRIES`(4)로 상한.
- `src/ratelimit.rs`의 `RateLimiter`(토큰버킷)는 **부분 적합**하다: 토스 그룹별 한도를 정적으로 알 수 없으니
  버킷에 그룹별 req/s를 박지 않는다. 대신 **선택적 클라이언트측 글로벌 캡**으로만 재사용 —
  `TossConfig.rate_limit: Option<u32>`가 `Some(n)`이면 그 값으로 버킷 생성, `None`이면 캡 없음(429 루프에만 의존).
  권위 throttle은 어디까지나 반응형 429 루프.

## 4. 계좌 헤더 설계 (account.rs / order.rs / order_info.rs)

`X-Tossinvest-Account: {accountSeq}`는 asset/order/order-history/order-info 엔드포인트에 필수,
`GET /api/v1/accounts`에는 **불필요**. accountSeq는 `GET /api/v1/accounts` 응답의 `accountSeq`에서 획득.

**설계: 계좌 스코프 액세서가 `account_seq`를 생성 시점에 보유 → 헤더 자동 주입, 누락 불가능.**

```rust
client.accounts()            // seq 불필요 — 계좌 목록 조회
client.asset(seq)            // X-Tossinvest-Account 자동 주입
client.order(seq)            // 〃
client.order_info(seq)       // 〃
```

- `accounts()`는 seq-free 액세서, `asset(seq)`/`order(seq)`/`order_info(seq)`는 seq를 구조적으로 carry.
- `ApiCall`에 `account_seq: Option<i64>` 필드를 두고 `Some`일 때만 `call_once`가 헤더를 붙인다.
- seq를 잊을 수 있는 경로가 없으므로 런타임 가드/에러 불필요. 계좌 미지정 호출은 컴파일 단계에서 불가능.

## 5. 아키텍처

### 5.1 모듈 구조

```
src/toss/
├── mod.rs           # TossClient 액세서, 공개 재노출, Currency/MarketCountry 등 공통 enum-as-String 정책
├── config.rs        # TossConfig (client_id, client_secret, base_url, token_cache_path, rate_limit)
├── error.rs         # TossError, Result
├── auth.rs          # OAuth2 client_credentials 토큰 발급/캐싱 (form-urlencoded)
├── client.rs        # TossClient, TossResponse<T>, ApiCall/RawRequest, 2xx/result 언래핑, 429 재시도
├── market_data.rs   # orderbook, prices, trades, price-limits, candles
├── stock_info.rs    # stocks, stocks/{symbol}/warnings
├── market_info.rs   # exchange-rate, market-calendar/KR, market-calendar/US
├── account.rs       # accounts (seq-free), holdings (Asset 액세서, seq 필요)
├── order.rs         # orders 생성/정정/취소/목록/상세
└── order_info.rs    # buying-power, sellable-quantity, commissions
```

`src/ratelimit.rs`(크레이트 루트, 브로커 공유)·`src/kis/error.rs`는 **재사용 판단 결과**:
ratelimit은 선택적 캡으로만 재사용(§3.4), error는 재사용하지 않고 병렬 신설(§3.3).

### 5.2 핵심 타입

```rust
pub struct TossConfig {
    pub client_id: String,
    pub client_secret: String,
    pub base_url: String,                  // 기본 https://openapi.tossinvest.com
    pub token_cache_path: Option<PathBuf>, // 기본 ~/.toss/token.json
    pub rate_limit: Option<u32>,           // 선택적 클라이언트측 글로벌 캡 (req/s)
}

pub struct TossClient { config, http, auth, limiter: Option<RateLimiter> }
impl TossClient {
    pub fn new(config: TossConfig) -> Result<Self>;
    pub fn market_data(&self) -> MarketData<'_>;
    pub fn stock_info(&self) -> StockInfoApi<'_>;
    pub fn market_info(&self) -> MarketInfoApi<'_>;
    pub fn accounts(&self) -> Accounts<'_>;        // seq-free
    pub fn asset(&self, account_seq: i64) -> Asset<'_>;
    pub fn order(&self, account_seq: i64) -> Order<'_>;
    pub fn order_info(&self, account_seq: i64) -> OrderInfo<'_>;
    pub async fn raw_call(&self, req: RawRequest) -> Result<serde_json::Value>;
}
```

### 5.3 enum-as-String 정책 (unknown 허용)

스펙 거의 모든 enum이 "클라이언트는 unknown enum 값을 허용하도록 구현해야 합니다"를 명시한다.
Rust bare enum + `#[derive(Deserialize)]`는 미지 값에 **하드 실패**한다. 대응:

- **응답 필드의 enum (Currency, MarketCountry, OrderStatus, accountType, securityType, market,
  warningType, orderType, timeInForce, side, rateChangeType 등)** → KIS 컨벤션대로 **`String`으로 보존.**
  미지 값에도 절대 실패하지 않음. 의미 해석은 호출자가 코드 비교로 수행.
- **요청 파라미터의 enum (side, orderType, interval, status, currency)** → 값을 우리가 통제하므로
  타입 Rust enum 사용 가능. 직렬화 시 정확한 코드 문자열로 변환.
- decimal/price/quantity/timestamp/date 필드 → 전부 **`String`** (KIS 컨벤션, 정밀도 손실 방지).

### 5.4 OrderCreateRequest (oneOf)

스펙은 `oneOf` [수량기반(quantity), 금액기반(orderAmount, US MARKET 전용)]. 두-변형 enum으로 모델링:

```rust
pub enum OrderCreate {
    Quantity { symbol, side, order_type, time_in_force?, quantity, price?, client_order_id?, confirm_high_value? },
    Amount   { symbol, side /*BUY*/, quantity_amount /*orderAmount, US MARKET 전용*/, client_order_id?, confirm_high_value? },
}
```

전부-Optional 단일 struct로 평탄화하면 "orderAmount ⇒ US MARKET" 불변식이 사라지므로 금지.

## 6. 엔드포인트 → 메서드 매핑 (20개)

| # | 메서드 | HTTP | 경로 | seq | 요청 타입 | 응답 타입 | rate group |
|---|--------|------|------|-----|-----------|-----------|------------|
| 1 | `auth.token()` (내부) | POST | /oauth2/token | — | form | OAuth2TokenResponse | AUTH |
| 2 | `market_data().orderbook(symbol)` | GET | /api/v1/orderbook | — | symbol | `OrderbookResponse` | MARKET_DATA |
| 3 | `market_data().prices(&[symbols])` | GET | /api/v1/prices | — | symbols(≤200) | `Vec<PriceResponse>` | MARKET_DATA |
| 4 | `market_data().trades(symbol, count?)` | GET | /api/v1/trades | — | symbol,count(≤50) | `Vec<Trade>` | MARKET_DATA |
| 5 | `market_data().price_limits(symbol)` | GET | /api/v1/price-limits | — | symbol | `PriceLimitResponse` | MARKET_DATA |
| 6 | `market_data().candles(req)` | GET | /api/v1/candles | — | symbol,interval,count?,before?,adjusted? | `CandlePage` | MARKET_DATA_CHART |
| 7 | `stock_info().stocks(&[symbols])` | GET | /api/v1/stocks | — | symbols | `Vec<StockInfo>` | STOCK |
| 8 | `stock_info().warnings(symbol)` | GET | /api/v1/stocks/{symbol}/warnings | — | symbol(path) | `Vec<StockWarning>` | STOCK |
| 9 | `market_info().exchange_rate(req)` | GET | /api/v1/exchange-rate | — | base,quote,dateTime? | `ExchangeRateResponse` | MARKET_INFO |
| 10 | `market_info().calendar_kr(date?)` | GET | /api/v1/market-calendar/KR | — | date? | `KrMarketCalendarResponse` | MARKET_INFO |
| 11 | `market_info().calendar_us(date?)` | GET | /api/v1/market-calendar/US | — | date? | `UsMarketCalendarResponse` | MARKET_INFO |
| 12 | `accounts().list()` | GET | /api/v1/accounts | no | — | `Vec<Account>` | ACCOUNT |
| 13 | `asset(seq).holdings(symbol?)` | GET | /api/v1/holdings | yes | symbol? | `HoldingsOverview` | ASSET |
| 14 | `order(seq).create(OrderCreate)` | POST | /api/v1/orders | yes | OrderCreate(oneOf) | `OrderResponse` | ORDER |
| 15 | `order(seq).modify(order_id, OrderModify)` | POST | /api/v1/orders/{orderId}/modify | yes | OrderModify | `OrderOperationResponse` | ORDER |
| 16 | `order(seq).cancel(order_id)` | POST | /api/v1/orders/{orderId}/cancel | yes | — | `OrderOperationResponse` | ORDER |
| 17 | `order(seq).list(req)` | GET | /api/v1/orders | yes | status,symbol?,from?,to?,cursor?,limit? | `PaginatedOrderResponse` | ORDER_HISTORY |
| 18 | `order(seq).get(order_id)` | GET | /api/v1/orders/{orderId} | yes | orderId(path) | `Order` | ORDER_HISTORY |
| 19 | `order_info(seq).buying_power(currency)` | GET | /api/v1/buying-power | yes | currency | `BuyingPowerResponse` | ORDER_INFO |
| 20 | `order_info(seq).sellable_quantity(symbol)` | GET | /api/v1/sellable-quantity | yes | symbol | `SellableQuantityResponse` | ORDER_INFO |
| 21 | `order_info(seq).commissions()` | GET | /api/v1/commissions | yes | — | `Vec<Commission>` | ORDER_INFO |

표는 메서드 21행이지만 토큰 발급(#1)은 내부 자동 호출 — **공개 엔드포인트는 20개**(과제 명세 일치).
20개 전부 완전 타입 struct로 구현. `raw_call`은 미래 엔드포인트용 탈출구(타입 안전성 없음, `serde_json::Value`).

## 7. 검증 전략

- **단위 테스트** (KIS가 둔 위치 미러링):
  - 토큰 freshness (`is_fresh`, auth.rs) — KIS와 동일.
  - `result` envelope 언래핑 (성공 2xx).
  - BFF 에러 → `TossError::Api` 매핑 (4xx/5xx).
  - OAuth2 에러 → `TossError::OAuth2` 매핑.
  - OrderCreate oneOf 직렬화(수량/금액 변형이 올바른 body 생성).
- `cargo build`, `cargo clippy --all-targets`, `cargo test` clean.

## 8. 미해결/위험

- 토스 그룹별 레이트리밋 수치는 헤더로만 통보 → 정적 캡 불가, 반응형 429 루프가 권위(§3.4).
- `status=CLOSED` 주문 목록은 현재 서버가 `400 closed-not-supported` 반환 — 타입은 노출하되 사용 시 에러.
- 페이징: candles `nextBefore`, orders `nextCursor`/`hasNext`는 응답 struct에 그대로 보존 — 호출자가 다음 페이지 요청.
- 토스 Open API는 실시간 WebSocket 미제공 → KIS `realtime/` 같은 모듈 없음.
