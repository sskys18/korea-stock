# kis-adapter

한국투자증권(KIS) OpenAPI를 Rust에서 타입 안전하게 쓰는 비동기 어댑터.

국내주식·해외주식·국내선물옵션 거래/조회 + 실시간 WebSocket 시세를 단일 크레이트로 제공한다.

## 특징

- **4개 도메인 31개 타입 TR** — 요청·응답이 모두 타입 struct
- **실시간 WebSocket** — 체결가·호가·체결통보, AES-256-CBC 복호화 자동
- **실전/모의투자** — `KIS_ENV`로 런타임 분기, TR ID 자동 매핑
- **토큰 자동 관리** — 발급·파일 캐싱·만료 갱신
- **레이트리밋 + 재시도** — 토큰버킷, `EGW00201`(초당 거래건수 초과) 지수 백오프
- **연속조회** — `KisResponse<T>` envelope로 커서 보존, `*_all` 헬퍼
- **`raw_call` 탈출구** — 미구현 TR도 직접 호출

## 설치

```toml
[dependencies]
kis-adapter = { path = "." }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

## 빠른 시작

```rust
use kis_adapter::{KisClient, KisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KisClient::new(KisConfig::from_env()?)?;
    let price = client.domestic_stock().current_price("005930").await?;
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
| 실시간 WS | `client.realtime()` | 국내체결가·국내호가·체결통보·해외체결가 (4) |

```rust
use kis_adapter::overseas_stock::OverseasExchange;

let aapl = client
    .overseas_stock()
    .current_price(OverseasExchange::Nasd, "AAPL")
    .await?;
println!("{}", aapl.last);
```

### 실시간 WebSocket

```rust
use kis_adapter::{KisClient, KisConfig, RealtimeEvent, SubscriptionKind};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KisClient::new(KisConfig::from_env()?)?;
    let mut rt = client.realtime().await?;
    let mut events = rt.take_events().unwrap();

    let _handle = rt
        .subscribe(SubscriptionKind::DomesticTrade, "005930")
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

## 프로젝트 구조

```
src/
├── config.rs ─ error.rs ─ trid.rs ─ ratelimit.rs ─ auth.rs ─ client.rs   # core
├── domestic_stock/   overseas_stock/   futureoption/                     # REST 도메인
└── realtime/         # WebSocket — approval·subscribe·decode·crypto·run
docs/
├── specs/    # 설계 문서
├── plans/    # 구현 계획 (Plan 1~3)
└── kis-api/  # TR 명세 레퍼런스 (SSOT — 도메인별 응답 필드표)
examples/     # domestic_quote, domestic_order, overseas_quote, futureoption_quote, realtime_feed
```

## 테스트

```
cargo test                                    # 단위 19건
cargo test --test integration -- --ignored    # 통합 — 자격증명 필요
```

통합 테스트는 `KIS_*` 환경변수가 있어야 실행되며 모두 **조회 전용**(주문 없음)이다.

## 검증 상태

- 와이어 검증 완료: 4개 도메인 현재가, 국내 잔고, 실시간 WS 구독.
- 주문 TR(매수/매도/정정/취소)은 실거래가 발생하므로 자동 테스트하지 않는다.
- 그 외 조회 TR(기간시세·체결내역 등)의 응답 struct는 컴파일·`#[serde(default)]`
  내성만 보장 — 실사용 시 `rt_cd`/필드 오류가 나면 `docs/kis-api/*.md`와 대조한다.

## 문서

- 설계: [`docs/specs/`](docs/specs/)
- 구현 계획: [`docs/plans/`](docs/plans/)
- TR 명세 (SSOT): [`docs/kis-api/`](docs/kis-api/)

## 라이선스

MIT
