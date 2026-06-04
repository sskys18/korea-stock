# korea-stock

한국투자증권(KIS) + 토스증권(Toss) + 암호화폐 거래소·DEX의 한국주식 무기한선물을
Rust에서 타입 안전하게 쓰는 멀티 venue 비동기 어댑터.

venue는 두 그룹. **`domestic`** — 국내 증권사(`kis`·`toss`). **`global`** — 한국
대형주 perp을 상장한 글로벌 거래소·DEX 16곳. 공유 트레이트 없음(서명·주문모델 상이),
종목 식별자 [`KrStock`] enum만 공유.

## 구성

| 그룹 | 모듈 | 범위 | 상세 |
|---|---|---|---|
| `domestic` | `kis` | 국내·해외주식·국내선물옵션 거래/조회 + 실시간 WebSocket 시세 (4개 도메인 31타입 TR) | [KIS](#kis) |
| `domestic` | `toss` | 국내·미국 주식 시세·종목·시장·계좌·자산·주문 (20 엔드포인트) | [Toss](#toss) |
| `global` | 16 venue | KR 대형주 perp 시세·거래 (CEX 12 / DEX 4) | [venues](docs/venues.md) |

## 설치

```toml
[dependencies]
korea-stock = { path = "." }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
```

크레이트 식별자는 `korea_stock` (하이픈→언더스코어).

## 빠른 시작

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

```rust
// 글로벌 perp — 시세는 키 불필요
use korea_stock::global::binance::{BinanceClient, BinanceConfig, SAMSUNG};
let client = BinanceClient::new(BinanceConfig::public())?;
let idx = client.market().premium_index(SAMSUNG).await?;     // 마크가·펀딩
```

## KIS

- **4개 도메인 31타입 TR** — 요청·응답 모두 타입 struct.
- **실시간 WebSocket** — 체결가·호가·예상체결·장운영·회원사·프로그램매매·체결통보, AES-256-CBC 자동 복호화.
- **NXT·통합시세** — KRX/NXT(넥스트레이드)/통합 거래소 선택(시세·주문·실시간).
- **실전/모의투자** — `KIS_ENV` 런타임 분기, TR ID 자동 매핑.
- **토큰 자동 관리·레이트리밋·연속조회** — 발급·캐싱·갱신, 토큰버킷 + 지수 백오프, `*_all` 커서 헬퍼.
- **`raw_call` 탈출구** — 미구현 TR 직접 호출.

| 환경변수 | 설명 |
|---|---|
| `KIS_APP_KEY` / `KIS_APP_SECRET` | 앱 키 / 시크릿 |
| `KIS_ACCOUNT_NO` | 계좌번호 앞 8자리 |
| `KIS_ACCOUNT_PRODUCT` | 계좌상품코드 뒤 2자리 (예: `01`) |
| `KIS_ENV` | `real` 또는 `mock` (기본 `mock`) |

도메인 TR 명세(SSOT): [`docs/kis-api/`](docs/kis-api/) · 실시간 WS: [`docs/kis-api/realtime.md`](docs/kis-api/realtime.md) · 설계: [`docs/specs/2026-05-22-kis-adapter-design.md`](docs/specs/2026-05-22-kis-adapter-design.md).

## Toss

`korea_stock::domestic::toss` 모듈. KIS와 함께 `domestic` 그룹 소속, 공통 레이트리미터 공유.

- **20 엔드포인트 타입 구현** — 시세·종목정보·시장정보·계좌·자산·주문·주문정보.
- **OAuth2 Client Credentials** — 토큰 자동 발급·캐싱·갱신.
- **HTTP status 기반 envelope** + **계좌 스코프 액세서**(`X-Tossinvest-Account` 구조적 보유) + **429 반응형 재시도**.

| 환경변수 | 설명 |
|---|---|
| `TOSS_CLIENT_ID` / `TOSS_CLIENT_SECRET` | 클라이언트 ID / 시크릿 |
| `TOSS_BASE_URL` | (선택) 기본 `https://openapi.tossinvest.com` |
| `TOSS_RATE_LIMIT` | (선택) 클라이언트측 글로벌 캡 (req/s) |

엔드포인트 요약: [`docs/toss-api/endpoints.md`](docs/toss-api/endpoints.md) · 설계: [`docs/specs/2026-06-02-toss-adapter-design.md`](docs/specs/2026-06-02-toss-adapter-design.md).

## 글로벌 perp (16 venue)

CEX 12(binance·bybit·bitget·kucoin·gateio·bingx·mexc·htx·phemex·bitunix·toobit·weex)
+ DEX 4(hyperliquid·lighter·aster·pacifica). 시세 전부 라이브 검증, 거래는 오프라인
서명벡터만 검증(**실주문 이력 없음, 실자금 전 testnet 필수**).

구현·서명 매트릭스·종목 커버리지·환경변수·게이트: **[`docs/venues.md`](docs/venues.md)**.

## 프로젝트 구조

```
src/
├── lib.rs ─ ratelimit.rs ─ kr_stock.rs    # thin 루트 + 공유 레이트리미터 + KrStock 어휘
├── domestic/                               # 국내 증권사
│   ├── kis/        # config·error·trid·auth·client + domestic_stock/overseas_stock/futureoption/realtime
│   └── toss/       # config·error·auth·client + market_data·stock_info·market_info·account·order·order_info
└── global/         # KR-주식 perp (venue별 6파일: config·error·client·market·trade·mod)
    ├── binance·bybit·bitget·kucoin·gateio·bingx·mexc·htx·phemex·bitunix·toobit·weex/  # CEX 12 (HMAC 계열)
    ├── hyperliquid/   # Trade.xyz HIP-3 — EIP-712+secp256k1
    ├── lighter/       # zkLighter — Poseidon2/Schnorr + realtime(WS)
    ├── aster/         # BNB Chain — Binance-호환 fapi HMAC
    └── pacifica/      # Solana — Ed25519 (게이트)
docs/    # specs(설계) · plans(구현계획) · research(perp 센서스) · kis-api(TR SSOT) · toss-api(스펙) · venues.md
examples/  # kis_*·toss_* · <venue>_kr_quote · kr_perp_live_check
```

## 테스트

```
cargo test                                    # 단위 349건 (16개 perp venue 포함)
cargo test --test integration -- --ignored    # 통합 24건 — 자격증명 필요
```

통합 테스트는 `KIS_*` 환경변수 필요. 대부분 **조회 전용**(주문 없음), 예외로
`live_order_unfilled_cycle`은 `KIS_LIVE_ORDER_TEST=1` 가드 + 장중 한정 실주문
사이클(중간 실패 시 원주문 best-effort 취소). 검증 상태 상세: [`docs/specs/`](docs/specs/).

## 문서

| 디렉터리 | 내용 |
|---|---|
| [`docs/specs/`](docs/specs/) | 설계 문서 (KIS·Toss·perp·NXT·alpha-signals) |
| [`docs/plans/`](docs/plans/) | 구현 계획 (KIS Plan 1~3) |
| [`docs/research/`](docs/research/) | perp venue 센서스 + 어댑터 빌드 계약서 |
| [`docs/kis-api/`](docs/kis-api/) | KIS TR 명세 레퍼런스 (SSOT) |
| [`docs/toss-api/`](docs/toss-api/) | Toss OpenAPI 스펙 + 엔드포인트 요약 |
| [`docs/venues.md`](docs/venues.md) | 글로벌 16 venue 구현·서명·커버리지 |

## 라이선스

MIT
