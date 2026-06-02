# KR Alpha Signals — 투자자 플로우·외부 소스 설계

- 작성일: 2026-06-02
- 상태: 설계 승인 → 구현 진행
- 선행 문서: `docs/specs/2026-05-22-kis-adapter-design.md` (어댑터 본체)

## 1. 목적

KR 주식 알파 신호(외국인/기관 순매수, 프로그램매매, 외국인 보유율, 체결강도, 공매도 잔고,
전자공시/재무, EOD 투자자 history)를 타입 안전한 Rust API로 노출. 일부는 KIS REST/WS로
직접 닿고, 나머지(공매도·DART·백테스트 history)는 외부 소스를 순수 Rust로 포팅.

## 2. 스코프 결정

| 항목 | 결정 |
|------|------|
| 범위 | **전부** — KIS-native + 외부(KRX/DART) + 백테스트 EOD history |
| 출력 형태 | 타입드 struct + `#[serde(default)]` 내성 (기존 `quote.rs` 패턴) |
| 스냅샷 신호 | `current_price()`의 `frgn_ntby_qty`·`pgtr_ntby_qty`·`hts_frgn_ehrt` **그대로 유지**, timeseries TR만 신규 |
| 실시간 체결강도 | 포함 — `H0STCNT0`(`DomesticTrade`) 디코드에 체결강도 필드 노출 |
| 외부 소스 방식 | **순수 Rust 포트(A)** — KRX MDC·OpenDART REST를 reqwest로 직접. Python 미사용 |
| 외부 게이팅 | Cargo `feature = "external"` |

비스코프: Python 사이드카, 백테스트 엔진(데이터 수집만), 스냅샷 신호 재노출.

## 3. 페이징

| Phase | 산출물 | 클라이언트 |
|---|---|---|
| **P0** | 모든 TR ID/path/응답 필드를 `koreainvestment/open-trading-api` 샘플 대조 검증 → `docs/kis-api/domestic-stock.md` 갱신 | — |
| **P1** | KIS-native flow TR(타입드) + 실시간 체결강도 노출 | `KisClient` |
| **P2** | 외부: KRX 공매도 잔고, OpenDART 공시·재무 | `KrxClient`·`DartClient` (feature `external`) |
| **P3** | EOD 투자자 history(백테스트용) | `external::history` |

## 4. P0 — TR 검증 (게이트)

프롬프트의 TR-ID 표는 **미검증**이며 일부 오류 확인됨:
`HHDFS76240000`은 해외 잔고 TR(`docs/kis-api/overseas-stock.md:425`)이지 "보유율 추이"가 아님.
P1 struct의 필드명은 P0 검증 전 **확정 금지**(추측 필드명 금지).

검증 절차: `github.com/koreainvestment/open-trading-api` `examples_llm/domestic-stock` 샘플에서
path·tr_id·요청/응답 필드명을 그대로 추출(추측 없음). 결과를 `docs/kis-api/domestic-stock.md`에
TR 13~ 로 추가. 검증 불가 항목은 `[미확인]` 명시 후 해당 메서드 보류.

**검증 완료 (codex spec-review가 공식 샘플 대조, 2026-06-02):**

| 신호 | TR | path | output | 신뢰도 |
|---|---|---|---|---|
| 종목별 투자자 **일별** 매매동향 | `FHPTJ04160001` | /quotations/investor-trade-by-stock-daily | output | [High] |
| 프로그램매매 당일(시간) | `FHPPG04600101` | /quotations/comp-program-trade-today | output | [High] |
| 프로그램매매 일별 | `FHPPG04600001` | /quotations/comp-program-trade-daily | output | [High] |
| 종목 외국인·기관 추정 집계(단건) | `HHPTJ04160200` | /quotations/investor-trend-estimate | **output2** | [High] |
| 외국인 보유율 추이 | **P0-resolve** (프롬프트 `HHDFS76240000`은 해외잔고 — 오류) | — | — | [Low] |

프롬프트 원안 오류 정정: `FHKST01010900`은 *주식현재가 투자자*(intraday, 현재가 화면)이지
일별 종목별 매매동향이 아님. `FHPTJ04400000`은 랭킹형 foreign-institution-total.
실제 일별/추정 TR은 위 표 기준.

## 5. P1 — KIS-native flow (`src/domestic_stock/flow.rs`)

`DomesticStock` 신규 메서드. 기존 `quote.rs`와 동일 패턴: `ApiCall` + `resp.field("outputN")`,
`TrId::same(...)` 상수, 타입드 struct + `#[serde(default)]`. **필드 struct는 P0 확정 후 정의.**

| 메서드 | TR | path | 반환 |
|---|---|---|---|
| `investor_trend_daily(code)` | FHPTJ04160001 | investor-trade-by-stock-daily | `Vec<InvestorTrendDay>` |
| `program_trade_today(code)` | FHPPG04600101 | comp-program-trade-today | `Vec<ProgramTrade>` |
| `program_trade_daily(code)` | FHPPG04600001 | comp-program-trade-daily | `Vec<ProgramTrade>` |
| `investor_trend_estimate(code)` | HHPTJ04160200 | investor-trend-estimate (output2) | `InvestorTrendEstimate` |
| `foreign_holding_trend(code)` | P0-resolve | — | `Vec<ForeignHoldingDay>` (보류 가능) |

`mod.rs`에 `mod flow; pub use flow::*;` 추가. `current_price()` 미변경.

### 5.1 실시간 체결강도

`H0STCNT0`(`SubscriptionKind::DomesticTrade`)는 이미 구독+디코드됨. 체결강도 필드(`cttr`)도
`StockTrade`에 기노출(`decode.rs`). **추가 작업 불요** — codex spec-review 확인.

### 5.2 일별 공매도 (KIS-native, 재라우팅)

구현 중 발견: KIS REST가 공매도 *거래*를 직접 제공(`daily_short_sale`). 외부 불요.
`short_sale_daily(code, start, end)` TR 17 `FHPST04830000` /quotations/daily-short-sale,
(요약 output1, 일별 output2). 단 공매도 *잔고*(outstanding)는 KIS 미제공 → P2 KRX.

## 6. P2 — 외부 소스 (`src/external/`, feature `external`) — 재라우팅

**재라우팅(2026-06-02):** 원안은 공매도·재무를 외부로 가정했으나, KIS REST에 `daily_short_sale`·
`finance_*`·`inquire_investor_daily_by_market`·`frgnmem_*`가 존재함을 확인. KIS-native가
동일 데이터에서 우월(동일 인증/레이트리밋, 검증 가능)하므로 외부는 **KIS 진짜 부재 항목만**:

- `KrxClient::short_balance(start, end, isin)` — 공매도 **잔고**(KIS 부재).
  - KRX MDC `getJsonData.cmd` POST, bld `dbms/MDC/STAT/srt/MDCSTAT30502`, 응답 `OutBlock_1`.
  - **인증(2024~ 변경):** KRX가 MDC를 회원 로그인 세션 뒤로 이동(미인증 시 `LOGOUT` 400).
    무료 KRX 계정으로 폼 로그인(MDCCOMS001D1.cmd, CD001=정상/CD011=중복→skipDup) 후 쿠키 세션 조회.
    OTP·crypto 아님 — pykrx `comm/auth.py` 역공학. `reqwest` cookie_store로 순수 Rust 구현.
    `KrxClient::from_env`(`KRX_ID`/`KRX_PW`). bld·컬럼은 pykrx 소스/cassette 기준.
- `KrxClient::foreign_holding(start, end, isin)` — 외국인 보유량 추이(프롬프트 원안 미해결분).
  - bld `dbms/MDC/STAT/standard/MDCSTAT03702`, 응답 `output`.
- `DartClient::disclosures(corp_code, bgn_de, end_de)` — 전자공시(KIS 부재).
  - OpenDART `list.json`, 자체 `crtfc_key`. 필드는 공식 명세.

`ExternalConfig { dart_api_key: Option<String> }`. `KisError::External(String)` variant 추가.
**추가 의존성 없음** — reqwest `.form()`/`.query()` 재사용(원안의 `urlencoding` 불요).

## 7. P3 — EOD history — 재라우팅(대부분 KIS-native로 흡수)

원안의 별도 `history.rs`(pykrx-equiv) 대부분 불요:
- **종목별 EOD 투자자 history** = `investor_trend_daily_all`(P1)이 헤더 연속조회로 수집.
- **시장별 EOD 투자자** = KIS `inquire_investor_daily_by_market`(후속 래핑 가능).
- **외국인 보유 추이** = `KrxClient::foreign_holding`(P2).

KRX 장기간 백테스트(2년 초과)만 분할 호출 필요 — 호출자 책임으로 문서화.

## 8. 에러·인증 경계

- KIS-native: 기존 `KisClient` 토큰/레이트리밋 재사용.
- 외부: 자체 인증. `KisError::External { msg }`로 통합 표면. 네트워크/크레덴셜 실패 격리.

## 9. 테스트

- 타입드 struct: 컴파일 + `#[serde(default)]` 내성(누락 필드 무해).
- 와이어 테스트: 기존 `tests/integration.rs` 패턴 — env 크레덴셜 게이트.
- 외부 와이어: `DART_API_KEY`/네트워크 게이트. 미설정 시 skip.
- 기존 repo 관례 유지: 미와이어검증 struct는 "컴파일·serde 내성만 보장" 명시.

## 10. 리스크

- KRX MDC는 비공식 — 엔드포인트/bld 변경 시 깨질 수 있음. `feature = "external"` 게이트로 본체 격리.
- 외부(KRX/DART) 와이어 검증은 크레덴셜/네트워크 의존 → 컴파일·serde 내성까지만 CI 보장.
- KIS 공매도는 *거래*만, *잔고*는 KRX 의존 — 두 신호 성격 구분 필요.

## 11. 구현 현황 (as-built, 2026-06-02)

- **P1 완료**: `flow.rs` — investor_trend_daily(+_all 연속), investor_trend_estimate,
  program_trade_today/daily, short_sale_daily. TR 13~17 문서화. 단위테스트(serde 내성) 통과.
- **P2 완료**: `external/` (feature gated) — KrxClient(short_balance, foreign_holding) +
  KRX 폼 로그인 세션 트랜스포트(cookie_store, LOGOUT 시 재로그인), DartClient(disclosures).
  컴파일·serde·clippy 통과. 라이브 와이어는 `--ignored` 테스트(KRX_ID/KRX_PW·DART_API_KEY)로 검증.
- **realtime 체결강도**: 기존 코드에 이미 노출 — 무변경.
- 검증: `cargo test`(28 통과) + `cargo clippy`(external on/off 모두 clean).
- 후속(미구현): inquire_investor_daily_by_market 등 추가 KIS-native 래핑, DART financials,
  외부 와이어 실측 검증.
