# korea-stock

한국투자증권(KIS) + 토스증권(Toss) + 암호화폐 거래소·DEX의 한국주식 무기한선물을
Rust에서 타입 안전하게 쓰는 멀티 venue 비동기 어댑터.

KIS는 국내주식·해외주식·국내선물옵션 거래/조회 + 실시간 WebSocket 시세를,
Toss는 국내·미국 주식 시세·종목정보·시장정보·계좌·자산·주문(20개 엔드포인트)을,
그리고 `binance`·`hyperliquid`·`lighter`·`mexc`는 2026년 상장된 한국 대형주
(삼성전자·SK하이닉스·현대차) 무기한선물(perp) 시세·거래를 단일 크레이트의
형제 모듈로 제공한다(공유 트레이트 없음). 자세한 능력 매트릭스는
[`docs/specs/2026-06-02-kr-perp-venues-design.md`](docs/specs/2026-06-02-kr-perp-venues-design.md).

## 특징

- **4개 도메인 31개 타입 TR** — 요청·응답이 모두 타입 struct
- **실시간 WebSocket** — 체결가·호가·예상체결·장운영·회원사·프로그램매매·체결통보, AES-256-CBC 복호화 자동
- **NXT·통합시세** — KRX/NXT(넥스트레이드)/통합 거래소 선택(시세·주문·실시간)
- **실전/모의투자** — `KIS_ENV`로 런타임 분기, TR ID 자동 매핑
- **토큰 자동 관리** — 발급·파일 캐싱·만료 갱신
- **레이트리밋 + 재시도** — 토큰버킷, `EGW00201`(초당 거래건수 초과) 지수 백오프
- **연속조회** — `KisResponse<T>` envelope로 커서 보존, `*_all` 헬퍼
- **`raw_call` 탈출구** — 미구현 TR도 직접 호출

## 설치

```toml
[dependencies]
korea-stock = { path = "." }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

크레이트 식별자는 `korea_stock` (하이픈→언더스코어).

## 빠른 시작 (KIS)

```rust
use korea_stock::{KisClient, KisConfig, Market};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KisClient::new(KisConfig::from_env()?)?;
    // Market::Krx / Nxt / Unified(통합시세) 선택
    let price = client
        .domestic_stock()
        .current_price("005930", Market::Krx)
        .await?;
    println!("삼성전자 현재가: {}", price.stck_prpr);
    Ok(())
}
```

### 환경변수

| 변수 | 설명 |
|------|------|
| `KIS_APP_KEY` | 앱 키 |
| `KIS_APP_SECRET` | 앱 시크릿 |
| `KIS_ACCOUNT_NO` | 계좌번호 앞 8자리 |
| `KIS_ACCOUNT_PRODUCT` | 계좌상품코드 뒤 2자리 (예: `01`) |
| `KIS_ENV` | `real` 또는 `mock` (기본 `mock`) |

`KisConfig`를 직접 구성하면 환경변수 없이도 사용 가능하다.

## 도메인

| 도메인 | 액세서 | TR |
|--------|--------|-----|
| 국내주식 | `client.domestic_stock()` | 시세·호가·기간/분봉, 매수/매도/정정/취소, 잔고·매수가능·일별체결·정정취소가능 (12) |
| 해외주식 | `client.overseas_stock()` | 현재가·기간시세, 매수/매도/정정취소, 잔고·미체결·체결내역 (8) |
| 국내선물옵션 | `client.futureoption()` | 현재가·호가, 주문/정정취소, 잔고·체결내역·주문가능 (7) |
| 실시간 WS | `client.realtime()` | 국내 6종(체결가·호가·예상체결·장운영·회원사·프로그램) × KRX/NXT/통합, 체결통보, 해외체결가 |

```rust
use korea_stock::kis::overseas_stock::OverseasExchange;

let aapl = client
    .overseas_stock()
    .current_price(OverseasExchange::Nasd, "AAPL")
    .await?;
println!("{}", aapl.last);
```

### 실시간 WebSocket

```rust
use korea_stock::{KisClient, KisConfig, Market, RealtimeEvent, SubscriptionKind};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KisClient::new(KisConfig::from_env()?)?;
    let mut rt = client.realtime().await?;
    let mut events = rt.take_events().unwrap();

    let _handle = rt
        .subscribe(SubscriptionKind::DomesticTrade(Market::Krx), "005930")
        .await?;

    while let Some(ev) = events.recv().await {
        if let RealtimeEvent::DomesticTrade { tr_key, data, .. } = ev {
            println!("{tr_key} {}", data.stck_prpr);
        }
    }
    Ok(())
}
```

- 체결통보는 AES-256-CBC 복호화 자동 처리 (`tr_key`는 HTS ID).
- `SubscriptionHandle` drop 시 자동 해지. 동시 구독 한도 약 41건.
- 연결 끊김 시 자동 재연결·재구독 — `RealtimeEvent::Reconnecting`/`Reconnected` 통지.
- 채널 포화 시 새 이벤트 드롭 + `RealtimeEvent::Lagged(n)` 통지.

### 미구현 TR — `raw_call`

타입 TR로 노출되지 않은 TR은 `raw_call`로 직접 호출한다 (응답은 `serde_json::Value`).

### 주문 hashkey

기본 `KisConfig.use_hashkey = false`. KIS는 hashkey를 강제하지 않는다. 주문 호출에서
hashkey 관련 오류가 나면 `true`로 바꿔 재시도한다.

## 토스증권 (Toss)

`korea_stock::toss` 모듈. KIS와 동일 크레이트 내 형제 모듈로, 공통 레이트리미터를 공유한다.

- **20개 엔드포인트 타입 구현** — 시세(호가·현재가·체결·상하한가·캔들), 종목정보, 시장정보(환율·장운영),
  계좌, 자산(보유주식), 주문(생성·정정·취소·목록·상세), 주문정보(매수가능·판매가능·수수료)
- **OAuth2 Client Credentials** — `POST /oauth2/token` (form-urlencoded), 토큰 자동 발급·파일 캐싱·만료 갱신
- **HTTP status 기반 envelope** — `ApiResponse.result` 언래핑, `TossError`(OAuth2/BFF 에러 구분)
- **계좌 스코프 액세서** — `X-Tossinvest-Account` 헤더를 액세서가 구조적으로 보유해 누락 불가능
- **429 반응형 재시도** — `Retry-After` 기반, 그룹별 레이트리밋 대응
- **`raw_call` 탈출구** — 미래 엔드포인트 직접 호출

```rust
use korea_stock::{TossClient, TossConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = TossClient::new(TossConfig::from_env()?)?;

    // 시세 — 토큰만 필요 (계좌 헤더 불필요)
    let prices = client.market_data().prices(&["005930", "AAPL"]).await?;
    for p in &prices {
        println!("{} = {} {}", p.symbol, p.last_price, p.currency);
    }

    // 계좌 흐름 — accountSeq 획득 후 계좌 스코프 액세서로 호출
    let accounts = client.accounts().list().await?;
    let seq = accounts[0].account_seq;
    // order_info(seq)/asset(seq)/order(seq) 액세서가 X-Tossinvest-Account를 자동 주입
    let buying = client.order_info(seq).buying_power("KRW").await?;
    println!("매수가능: {} {}", buying.cash_buying_power, buying.currency);
    Ok(())
}
```

### Toss 환경변수

| 변수 | 설명 |
|------|------|
| `TOSS_CLIENT_ID` | 클라이언트 ID |
| `TOSS_CLIENT_SECRET` | 클라이언트 시크릿 |
| `TOSS_BASE_URL` | (선택) Base URL, 기본 `https://openapi.tossinvest.com` |
| `TOSS_RATE_LIMIT` | (선택) 클라이언트측 글로벌 캡 (req/s) |

설계 근거는 [`docs/specs/2026-06-02-toss-adapter-design.md`](docs/specs/2026-06-02-toss-adapter-design.md),
엔드포인트 요약은 [`docs/toss-api/endpoints.md`](docs/toss-api/endpoints.md) 참조.

## 암호화폐 거래소·DEX — KR 주식 무기한선물 (perp)

2026년 다수 CEX·perp DEX가 한국 대형주(삼성전자·SK하이닉스·현대차) 무기한선물을
상장했다. 각 venue는 KIS/Toss와 동일한 독립 형제 모듈이다. 시세는 모두 라이브
검증됐고, **거래 코드는 오프라인(파싱·서명벡터)만 검증 — 어떤 venue도 testnet/live로
실주문된 적 없다. 실자금 전 testnet 왕복 필수.**

| 모듈 | 종류 | 시세 | 거래 | 서명 |
|---|---|---|---|---|
| `binance` | CEX | ✅ | 주문/취소/포지션/잔고/레버리지 | HMAC-SHA256 (공식 doc 벡터 검증) |
| `hyperliquid` | DEX (HIP-3/Trade.xyz) | ✅ | 주문(IOC)/취소 | EIP-712+secp256k1 (서명 primitive upstream SDK 벡터 검증) |
| `mexc` | CEX | ✅ | 조회만 (신규주문 서버측 차단) | HMAC-SHA256 |
| `lighter` | DEX (zk) | ✅ +WS | 주문/취소 (게이트) | Poseidon2+Schnorr 순수Rust (암호코어 upstream 벡터 검증, tx봉투 미검증) |

```rust
use korea_stock::binance::{BinanceClient, BinanceConfig, SAMSUNG};
let client = BinanceClient::new(BinanceConfig::public())?;   // 시세는 키 불필요
let idx = client.market().premium_index(SAMSUNG).await?;     // 마크가·펀딩
```

- 환경변수: `BINANCE_API_KEY/SECRET`, `HYPERLIQUID_*`(비밀키), `MEXC_*`, `LIGHTER_*`.
- Lighter 거래는 `LighterConfig::allow_unverified_signing`(기본 false) 게이트 뒤 —
  tx_info 봉투가 미검증이므로 `scripts/lighter_capture_vector.md`로 공식 SDK 출력과
  대조 후 해제할 것.
- Hyperliquid 운영 주문은 `trade().place_by_coin(DEX, coin, ...)` 사용 — asset id를
  라이브 meta에서 재도출해 wrong-instrument 사고를 막는다.

## 프로젝트 구조

```
src/
├── lib.rs ─ ratelimit.rs                  # thin 루트 + 브로커 공유 레이트리미터
├── kis/                                    # 한국투자증권
│   ├── config.rs ─ error.rs ─ trid.rs ─ auth.rs ─ client.rs   # core
│   ├── domestic_stock/   overseas_stock/   futureoption/        # REST 도메인
│   └── realtime/         # WebSocket — approval·subscribe·decode·crypto·run
├── toss/                                   # 토스증권
│   ├── config.rs ─ error.rs ─ auth.rs ─ client.rs              # core
│   └── market_data·stock_info·market_info·account·order·order_info.rs
├── binance/      # Binance USDM Futures — config·error·client(HMAC)·market·trade
├── hyperliquid/  # Trade.xyz HIP-3 — client(EIP-712)·market·trade
├── lighter/      # zkLighter — client·market·trade·realtime(WS)·sign(Poseidon2/Schnorr)
└── mexc/                                   # MEXC Contract — client(HMAC)·market·trade
docs/
├── specs/    # 설계 문서
├── plans/    # 구현 계획 (KIS Plan 1~3)
├── kis-api/  # KIS TR 명세 레퍼런스 (SSOT)
└── toss-api/ # Toss OpenAPI 스펙 + 엔드포인트 요약
examples/
├── kis_*    # kis_domestic_quote, kis_domestic_order, kis_overseas_quote, kis_futureoption_quote, kis_realtime_feed
├── toss_*   # toss_quote, toss_order
└── binance_kr_*  # binance_kr_quote (시세), binance_kr_order (테스트넷 주문)
```

## 테스트

```
cargo test                                    # 단위 141건 (4개 perp venue 포함)
cargo test --test integration -- --ignored    # 통합 24건 — 자격증명 필요
```

통합 테스트는 `KIS_*` 환경변수가 있어야 실행된다. 대부분 **조회 전용**(주문 없음)이며,
예외로 `live_order_unfilled_cycle`은 `KIS_LIVE_ORDER_TEST=1` 가드 + 장중에만 실행되는
실주문(미체결 매수→정정→취소) 사이클이다. 중간 실패 시 원주문을 best-effort 취소한다.

## 검증 상태

실전 API 통합테스트로 응답 struct를 와이어 검증했다. NXT/통합 신규 경로(시세·실시간)는
공식 샘플 필드맵 기준 — 라이브 와이어 미검증(`docs/specs/2026-06-02-nxt-integration-design.md` 참조).

- **검증 완료** — 국내주식 조회 8(현재가·호가·기간·분봉·매수가능·잔고·일별체결·정정취소가능),
  해외주식 조회 5(현재가·기간·잔고·미체결·체결내역), 선물옵션 시세 2(현재가·호가),
  실시간 WS 구독.
- **미검증** — 주문 TR(매수/매도/정정/취소)은 실거래가 발생하므로 자동 테스트하지 않는다.
  선물옵션 잔고·체결·주문가능은 선물옵션 거래계좌가 있어야 응답을 검증할 수 있다.
- 미검증 TR의 응답 struct는 컴파일·`#[serde(default)]` 내성만 보장 — 실사용 시
  `rt_cd`/필드 오류가 나면 `docs/kis-api/*.md`와 대조한다.

## 문서

- 설계: [`docs/specs/`](docs/specs/)
- 구현 계획: [`docs/plans/`](docs/plans/)
- TR 명세 (SSOT): [`docs/kis-api/`](docs/kis-api/)

## 라이선스

MIT
