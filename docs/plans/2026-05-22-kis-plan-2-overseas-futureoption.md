# Plan 2 — KIS 어댑터: 해외주식 + 국내선물옵션

- 작성일: 2026-05-22
- 스펙: `docs/specs/2026-05-22-kis-adapter-design.md`
- TR 명세 SSOT: `docs/kis-api/overseas-stock.md`, `docs/kis-api/futureoption.md`
- 선행: Plan 1 (Core 인프라 + 국내주식 12 TR) — 구현·커밋 완료 (`28fc1de`)
- 상태: 실행 대기

## 목표

Plan 1이 확정한 패턴(`ApiCall`/`RawResponse`/`KisClient::call`/`KisResponse<T>`/`TrId`)을
**그대로 복제**해 해외주식 8개 TR + 국내선물옵션 7개 TR을 추가한다.
이 Plan이 끝나면 어댑터 REST 도메인 커버리지가 설계 §5의 31개 TR 중 27개(국내 12 +
해외 8 + 선물옵션 7)에 도달한다. Plan 3(실시간 WebSocket)만 남는다.

신규 코드는 core 인프라(`config`/`error`/`auth`/`ratelimit`/`client`/`trid`)를
수정하지 않는다 — **단 하나의 예외가 T0**: 연속조회 cursor가 해외·선물옵션에서
`CTX_AREA_*200` 체계라 `client.rs`의 envelope 추출 로직을 한 번 패치해야 한다.

## 범위

- 포함: `src/overseas_stock/{mod,quote,order,account}.rs` 4파일,
  `src/futureoption/{mod,quote,order,account}.rs` 4파일,
  `client.rs` envelope cursor 패치(T0), `lib.rs` `pub mod` 2개 + `KisClient`
  액세서 2개, CLI 예제 2개, 통합테스트 2건 추가, README 갱신.
- 제외: 실시간 WebSocket (Plan 3), 해외 주간거래 정정취소(`daytime-order-rvsecncl`),
  해외 지수/환율 기간시세(`inquire-daily-chartprice`), 선물옵션 야간 모의(KIS 미지원).

## 전제

- Plan 1이 `feat/kis-plan-1` 브랜치에 머지/커밋된 상태. Plan 2 실행 전
  `git checkout -b feat/kis-plan-2` (현재 HEAD에서 분기).
- Rust stable (1.75+), `cargo` 설치.
- 모의투자 자격증명 환경변수 (통합테스트 `--ignored` 실행 시에만):
  `KIS_APP_KEY` `KIS_APP_SECRET` `KIS_ACCOUNT_NO` `KIS_ACCOUNT_PRODUCT` `KIS_ENV=mock`.
- 선물옵션 계좌상품코드는 주식과 다를 수 있음(예 `03`) — `KisConfig.account_product`로 주입,
  도메인 코드는 환경값을 그대로 사용한다.

## Plan 1 패턴 복제 — 불변 규칙

아래는 Plan 1 구현 코드(`src/client.rs`, `src/domestic_stock/*`)에서 확정된 패턴.
Plan 2 전 태스크가 **정확히 그대로** 따른다.

- 각 TR = 요청 파라미터(struct 또는 함수 인자) + 타입 응답 struct + `impl` 메서드.
- 응답 struct 필드는 전부 `String` (KIS는 숫자도 문자열 반환). 필드명은
  kis-api 문서 응답표의 영문 필드명을 **그대로(verbatim)** 사용, 한글 의미를 줄 주석으로 병기.
- 단건 조회 메서드 → `Result<T>`. 목록/연속조회 메서드 → `Result<KisResponse<Vec<T>>>`
  (또는 `KisResponse<(Vec<Item>, Summary)>`), `*_all` 헬퍼가 envelope를 소비해 끝까지 수집.
- 도메인 `impl` 블록은 `ApiCall { method, path, tr_id, tr_cont, params, is_post, needs_hashkey }`
  7개 필드를 **전부** 채운다. **모든 `ApiCall` 리터럴에 `needs_hashkey: false`** —
  해외·선물옵션 주문 POST 포함. KIS는 hashkey 비강제(overseas-stock.md §일반주의,
  futureoption.md §수집한계 확인), `KisConfig.use_hashkey`로 전역 제어.
- `tr_id`는 `const TR_*: TrId = TrId::both/same/real_only(..)`로 모듈 상단 선언 후
  `TR_*.resolve(env)?`로 해석. 환경은 `self.client.config().environment`에서 획득.
- `with_account()` 헬퍼로 `CANO`/`ACNT_PRDT_CD` 자동 주입 (주문·계좌 TR).
- 시세계 GET은 `TrId::same(..)`, 모의 미지원 TR은 `TrId::real_only(..)`.
- `resp.field("output")` / `resp.field("output1")` 등으로 body 키를 역직렬화,
  연속조회는 `resp.envelope(data)`로 감싼다.

## 파일 맵

| 파일 | 책임 | 생성/수정 태스크 |
|------|------|------------------|
| `src/client.rs` | `RawResponse::envelope` cursor 200-suffix fallback | T0 (수정) |
| `src/overseas_stock/mod.rs` | `OverseasStock` 액세서·`OverseasExchange` enum·계좌 주입 | T1 |
| `src/lib.rs` | `pub mod overseas_stock; pub mod futureoption;` + 재노출 | T1, T6 (수정) |
| `src/overseas_stock/quote.rs` | TR 7·8 시세 (현재가/기간별) | T2 |
| `src/overseas_stock/order.rs` | TR 1·2·3 주문 (매수/매도/정정취소) | T3 |
| `src/overseas_stock/account.rs` | TR 4·5·6 계좌 (잔고/미체결/체결내역) | T4 |
| `src/futureoption/mod.rs` | `FutureOption` 액세서·`Session` enum·계좌 주입 | T6 |
| `src/futureoption/quote.rs` | TR 6·7 시세 (현재가/호가) | T7 |
| `src/futureoption/order.rs` | TR 1·2 주문 (주문/정정취소) | T8 |
| `src/futureoption/account.rs` | TR 3·4·5 계좌 (잔고/체결내역/매수가능) | T9 |
| `examples/overseas_quote.rs` | 해외 현재가+기간시세 CLI | T11 |
| `examples/futureoption_quote.rs` | 선물옵션 현재가+호가 CLI | T12 |
| `tests/integration.rs` | 해외·선물옵션 스모크 2건 추가 | T13 (수정) |
| `README.md` | Plan 2 범위 반영 | T14 (수정) |
| `Cargo.toml` | `[[example]]` 2개 추가 | T11, T12 |

태스크: T0 (core 패치) → T1~T5 (해외주식) → T6~T10 (선물옵션) → T11~T14 (예제·테스트·문서). 총 15.

---

## T0 — `src/client.rs`: 연속조회 cursor 200-suffix 지원 (선행 블로커)

**문제**: Plan 1의 `RawResponse::envelope`는 body에서 `ctx_area_fk100`/`ctx_area_nk100`만
추출한다(`src/client.rs` 81~82행). 그러나 해외주식 잔고/미체결/체결내역과 선물옵션
잔고/체결내역은 연속조회 cursor가 **`CTX_AREA_FK200`/`CTX_AREA_NK200`** 체계다
(overseas-stock.md §4·5·6, futureoption.md §3·4). 응답 body 키는 소문자
`ctx_area_fk200`/`ctx_area_nk200`. 패치 없이는 해외·선물옵션 연속조회가 조용히
깨진다(`ctx_area_*`가 항상 `None` → `*_all` 헬퍼가 1페이지에서 멈춤).

**수정**: `envelope`의 cursor 추출을 100→200 fallback 체인으로 교체. 국내(100)/해외·선물(200)
모두 한 함수로 흡수한다. `src/client.rs`의 다음 블록(75~87행)을 교체:

```rust
    /// `data`를 envelope로 감싼다. ctx_area는 body에서 추출.
    /// 국내주식은 `*100` 체계, 해외·선물옵션은 `*200` 체계 — 둘 다 수용(100 우선, 없으면 200).
    pub fn envelope<T>(&self, data: T) -> KisResponse<T> {
        let s = |k: &str| self.body.get(k).and_then(|v| v.as_str()).map(String::from);
        KisResponse {
            data,
            tr_cont: self.tr_cont.clone(),
            ctx_area_fk: s("ctx_area_fk100").or_else(|| s("ctx_area_fk200")),
            ctx_area_nk: s("ctx_area_nk100").or_else(|| s("ctx_area_nk200")),
            rt_cd: s("rt_cd").unwrap_or_default(),
            msg_cd: s("msg_cd").unwrap_or_default(),
            msg: s("msg1").unwrap_or_default(),
        }
    }
```

> `KisResponse<T>`의 필드명(`ctx_area_fk`/`ctx_area_nk`)은 그대로 유지 — 100/200은
> 내부 구현 디테일이고 호출자는 추상 cursor만 본다. 도메인 코드가 다음 페이지 요청 시
> 어느 키(`CTX_AREA_FK200` 등)에 다시 넣을지는 각 TR이 안다.

검증: 이 시점엔 신규 모듈이 없어 단독 빌드만 — `cargo build` 에러 0 (core만 변경, 회귀 없음).
`cargo test` — Plan 1 단위 테스트 6건 그대로 통과.

커밋: `fix: envelope cursor 200-suffix fallback (해외·선물옵션 연속조회)`

---

## T1 — `src/overseas_stock/mod.rs` + `lib.rs` 배선

`OverseasStock` 액세서, `OverseasExchange` 거래소 enum(2체계 코드 단일 출처),
`with_account` 헬퍼. `lib.rs`에 모듈 선언 + 재노출.

> **거래소 코드 2체계**: overseas-stock.md §거래소코드 — 주문/계좌 TR은 `OVRS_EXCG_CD`
> (NASD/NYSE/AMEX/SEHK/SHAA/SZAA/TKSE/HASE/VNSE), 시세 TR은 `EXCD`
> (NAS/NYS/AMS/HKS/SHS/SZS/TSE/HSX/HNX). 두 체계를 `OverseasExchange` enum 하나가
> 책임지게 한다 — 호출자는 enum만 고르고, 메서드가 알맞은 코드 문자열을 뽑는다.

`src/overseas_stock/mod.rs`:

```rust
//! 해외주식 도메인 — 주문·계좌·시세 TR.

mod account;
mod order;
mod quote;

pub use account::*;
pub use order::*;
pub use quote::*;

use crate::client::KisClient;

/// 해외 거래소. 주문/계좌 TR은 `ovrs_excg_cd()`, 시세 TR은 `excd()` 코드 사용.
/// 미국 3거래소(NASD/NYSE/AMEX)는 주문 tr_id가 동일하므로 한 그룹으로 분기된다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverseasExchange {
    /// 미국 나스닥.
    Nasd,
    /// 미국 뉴욕.
    Nyse,
    /// 미국 아멕스.
    Amex,
    /// 홍콩.
    Sehk,
    /// 중국 상해.
    Shaa,
    /// 중국 심천.
    Szaa,
    /// 일본 도쿄.
    Tkse,
    /// 베트남 하노이.
    Hase,
    /// 베트남 호치민.
    Vnse,
}

impl OverseasExchange {
    /// 주문/계좌 TR용 해외거래소코드 (`OVRS_EXCG_CD`).
    pub fn ovrs_excg_cd(self) -> &'static str {
        match self {
            OverseasExchange::Nasd => "NASD",
            OverseasExchange::Nyse => "NYSE",
            OverseasExchange::Amex => "AMEX",
            OverseasExchange::Sehk => "SEHK",
            OverseasExchange::Shaa => "SHAA",
            OverseasExchange::Szaa => "SZAA",
            OverseasExchange::Tkse => "TKSE",
            OverseasExchange::Hase => "HASE",
            OverseasExchange::Vnse => "VNSE",
        }
    }

    /// 시세 TR용 거래소코드 (`EXCD`).
    pub fn excd(self) -> &'static str {
        match self {
            OverseasExchange::Nasd => "NAS",
            OverseasExchange::Nyse => "NYS",
            OverseasExchange::Amex => "AMS",
            OverseasExchange::Sehk => "HKS",
            OverseasExchange::Shaa => "SHS",
            OverseasExchange::Szaa => "SZS",
            OverseasExchange::Tkse => "TSE",
            OverseasExchange::Hase => "HNX",
            OverseasExchange::Vnse => "HSX",
        }
    }

    /// 기본 거래통화코드 (`TR_CRCY_CD`). 잔고 TR 등에서 사용.
    pub fn currency(self) -> &'static str {
        match self {
            OverseasExchange::Nasd | OverseasExchange::Nyse | OverseasExchange::Amex => "USD",
            OverseasExchange::Sehk => "HKD",
            OverseasExchange::Shaa | OverseasExchange::Szaa => "CNY",
            OverseasExchange::Tkse => "JPY",
            OverseasExchange::Hase | OverseasExchange::Vnse => "VND",
        }
    }
}

/// 해외주식 도메인 액세서. `client.overseas_stock()`으로 획득.
pub struct OverseasStock<'a> {
    pub(crate) client: &'a KisClient,
}

impl<'a> OverseasStock<'a> {
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

> `EXCD` 매핑 검증: overseas-stock.md §EXCD 표 — 베트남은 호치민=HSX, 하노이=HNX이고
> `OVRS_EXCG_CD`는 하노이=HASE, 호치민=VNSE다. 즉 `Hase`→excg `HASE`/excd `HNX`,
> `Vnse`→excg `VNSE`/excd `HSX`. 위 코드는 이 교차를 반영했다 — 실행자는 표와 재대조.

`src/lib.rs` 수정 — `pub mod domestic_stock;` 줄 아래에 추가:

```rust
pub mod domestic_stock;
pub mod overseas_stock;
```

(`futureoption`는 T6에서 추가. T1에서는 `overseas_stock`만.)

`src/client.rs`의 `KisClient` impl에 액세서 추가 — `domestic_stock()` 메서드 바로 아래:

```rust
    /// 해외주식 도메인 액세서.
    pub fn overseas_stock(&self) -> crate::overseas_stock::OverseasStock<'_> {
        crate::overseas_stock::OverseasStock::new(self)
    }
```

이 시점에 `quote.rs`/`order.rs`/`account.rs`가 없어 빌드 실패 — **T1은 mod.rs·배선만,
빌드 검증은 T5에서.**

커밋: `feat: OverseasStock 액세서 + OverseasExchange 거래소 코드`

---

## T2 — `src/overseas_stock/quote.rs`

시세 2종: 현재가(TR7), 기간별시세(TR8). 모두 GET, hashkey 불필요,
tr_id 실전·모의 동일(`TrId::same`). 거래소는 `EXCD` 코드 사용.

> **응답 struct 필드 규칙**: 모든 필드 `String`. 필드명은 `docs/kis-api/overseas-stock.md`
> §7·§8 응답표의 영문 필드명을 **그대로** 전사. 아래 코드는 각 struct의 전 필드를
> 명시한다(해외 시세 응답표는 필드 수가 적어 verbatim 전량 기재 — `[미확인]` 행 없음).

```rust
use serde::Deserialize;

use crate::client::ApiCall;
use crate::overseas_stock::OverseasExchange;
use crate::overseas_stock::OverseasStock;
use crate::error::Result;
use crate::trid::TrId;

// ── TR 7: 해외주식 현재가 ────────────────────────────────────────────
const TR_PRICE: TrId = TrId::same("HHDFS00000300");

/// 해외주식 현재가 응답 (output). overseas-stock.md §7 응답표 verbatim.
#[derive(Debug, Clone, Deserialize)]
pub struct OverseasCurrentPrice {
    pub rsym: String, // 실시간조회종목코드
    pub zdiv: String, // 소수점자리수
    pub base: String, // 전일종가
    pub pvol: String, // 전일거래량
    pub last: String, // 현재가
    pub sign: String, // 대비기호
    pub diff: String, // 대비
    pub rate: String, // 등락율
    pub tvol: String, // 거래량
    pub tamt: String, // 거래대금
    pub ordy: String, // 매수가능여부
}

impl OverseasCurrentPrice {
    /// 현재가를 f64로 파싱.
    pub fn price(&self) -> Option<f64> {
        self.last.trim().parse().ok()
    }
}

// ── TR 8: 해외주식 기간별시세 ────────────────────────────────────────
const TR_PERIOD: TrId = TrId::same("HHDFS76240000");

/// 기간별시세 종목 요약 (output1). §8 output1 표 verbatim.
#[derive(Debug, Clone, Deserialize)]
pub struct OverseasPeriodSummary {
    pub rsym: String, // 실시간조회종목코드
    pub zdiv: String, // 소수점자리수
    pub nrec: String, // 전일종가
}

/// 기간별 봉 1건 (output2 배열 요소). §8 output2 표 verbatim.
#[derive(Debug, Clone, Deserialize)]
pub struct OverseasPeriodCandle {
    pub xymd: String, // 일자 (YYYYMMDD)
    pub clos: String, // 종가
    pub sign: String, // 대비기호
    pub diff: String, // 대비
    pub rate: String, // 등락율
    pub open: String, // 시가
    pub high: String, // 고가
    pub low: String,  // 저가
    pub tvol: String, // 거래량
    pub tamt: String, // 거래대금
    pub pbid: String, // 매수호가
    pub vbid: String, // 매수호가잔량
    pub pask: String, // 매도호가
    pub vask: String, // 매도호가잔량
}

/// 기간 분류. Daily=0 / Weekly=1 / Monthly=2 (overseas `GUBN`).
#[derive(Debug, Clone, Copy)]
pub enum OverseasPeriod {
    Daily,
    Weekly,
    Monthly,
}

impl OverseasPeriod {
    fn code(self) -> &'static str {
        match self {
            OverseasPeriod::Daily => "0",
            OverseasPeriod::Weekly => "1",
            OverseasPeriod::Monthly => "2",
        }
    }
}

impl OverseasStock<'_> {
    /// 해외주식 현재가 (TR 7).
    pub async fn current_price(
        &self,
        exchange: OverseasExchange,
        symbol: &str,
    ) -> Result<OverseasCurrentPrice> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/overseas-price/v1/quotations/price".into(),
                tr_id: TR_PRICE.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "AUTH": "",
                    "EXCD": exchange.excd(),
                    "SYMB": symbol,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }

    /// 해외주식 기간별시세 (TR 8). `base_date` YYYYMMDD(공란이면 최근일).
    /// (요약, 봉배열) 반환.
    pub async fn period_price(
        &self,
        exchange: OverseasExchange,
        symbol: &str,
        period: OverseasPeriod,
        base_date: &str,
        adjusted: bool, // true=수정주가 반영
    ) -> Result<(OverseasPeriodSummary, Vec<OverseasPeriodCandle>)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/overseas-price/v1/quotations/dailyprice".into(),
                tr_id: TR_PERIOD.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "AUTH": "",
                    "EXCD": exchange.excd(),
                    "SYMB": symbol,
                    "GUBN": period.code(),
                    "BYMD": base_date,
                    "MODP": if adjusted { "1" } else { "0" },
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        Ok((resp.field("output1")?, resp.field("output2")?))
    }
}
```

검증: T5에서 빌드.
커밋: `feat: 해외주식 시세 TR 2종 (현재가/기간별)`

---

## T3 — `src/overseas_stock/order.rs`

주문 3종: 매수(TR1), 매도(TR2), 정정취소(TR3). 모두 POST.
`OVRS_EXCG_CD` 코드 사용. **거래소별 tr_id 분기 — 매수 6쌍, 매도 6쌍**(overseas-stock.md
§1·§2 표). 정정취소는 거래소 무관 단일 쌍(`TTTT1004U`/`VTTT1004U`).

> **거래소별 tr_id 검증**: overseas-stock.md §1 매수 표 / §2 매도 표를 그대로 전사.
> 미국 NASD/NYSE/AMEX는 동일 tr_id(매수 `TTTT1002U`, 매도 `TTTT1006U`)이므로
> `match`에서 3거래소를 한 arm으로 묶는다. 모의 미지원 거래소·tr_id 없음 — 전부 `both`.

```rust
use serde::Deserialize;

use crate::client::ApiCall;
use crate::overseas_stock::OverseasExchange;
use crate::overseas_stock::OverseasStock;
use crate::error::Result;
use crate::trid::TrId;

// 정정취소 — 거래소 무관 단일 쌍.
const TR_RVSECNCL: TrId = TrId::both("TTTT1004U", "VTTT1004U");

/// 매수 거래소별 tr_id. overseas-stock.md §1 표 verbatim.
fn buy_tr(exchange: OverseasExchange) -> TrId {
    use OverseasExchange::*;
    match exchange {
        Nasd | Nyse | Amex => TrId::both("TTTT1002U", "VTTT1002U"),
        Sehk => TrId::both("TTTS1002U", "VTTS1002U"),
        Shaa => TrId::both("TTTS0202U", "VTTS0202U"),
        Szaa => TrId::both("TTTS0305U", "VTTS0305U"),
        Tkse => TrId::both("TTTS0308U", "VTTS0308U"),
        Hase | Vnse => TrId::both("TTTS0311U", "VTTS0311U"),
    }
}

/// 매도 거래소별 tr_id. overseas-stock.md §2 표 verbatim.
fn sell_tr(exchange: OverseasExchange) -> TrId {
    use OverseasExchange::*;
    match exchange {
        Nasd | Nyse | Amex => TrId::both("TTTT1006U", "VTTT1006U"),
        Sehk => TrId::both("TTTS1001U", "VTTS1001U"),
        Shaa => TrId::both("TTTS1005U", "VTTS1005U"),
        Szaa => TrId::both("TTTS0304U", "VTTS0304U"),
        Tkse => TrId::both("TTTS0307U", "VTTS0307U"),
        Hase | Vnse => TrId::both("TTTS0310U", "VTTS0310U"),
    }
}

/// 해외주문 응답 (output). 매수/매도/정정취소 공통. §1·§3 응답표 verbatim.
#[derive(Debug, Clone, Deserialize)]
pub struct OverseasOrderResult {
    /// 한국거래소전송주문조직번호 — 정정/취소 시 사용.
    #[serde(alias = "KRX_FWDG_ORD_ORGNO", alias = "krx_fwdg_ord_orgno")]
    pub krx_fwdg_ord_orgno: String,
    /// 주문번호 — 정정/취소 시 사용.
    #[serde(alias = "ODNO", alias = "odno")]
    pub odno: String,
    /// 주문시각.
    #[serde(alias = "ORD_TMD", alias = "ord_tmd")]
    pub ord_tmd: String,
}

/// 해외 주문구분 — KIS `ORD_DVSN` 코드. 자주 쓰는 1종 + 임의 코드 탈출구.
#[derive(Debug, Clone)]
pub enum OverseasOrderType {
    /// "00" 지정가.
    Limit,
    /// 그 외 ORD_DVSN 코드 (31:MOO, 32:LOO, 33:MOC, 34:LOC 등).
    Code(String),
}

impl OverseasOrderType {
    pub(crate) fn code(&self) -> &str {
        match self {
            OverseasOrderType::Limit => "00",
            OverseasOrderType::Code(c) => c,
        }
    }
}

/// 해외 매수/매도 주문 파라미터.
#[derive(Debug, Clone)]
pub struct OverseasOrderReq {
    pub exchange: OverseasExchange,
    /// 종목코드 (예 `AAPL`).
    pub symbol: String,
    pub order_type: OverseasOrderType,
    /// 주문수량.
    pub quantity: u64,
    /// 해외주문단가. 시장가류는 0.
    pub price: f64,
}

/// 해외 정정/취소 파라미터. 원주문의 OverseasOrderResult에서 식별자 획득.
#[derive(Debug, Clone)]
pub struct OverseasReviseCancelReq {
    pub exchange: OverseasExchange,
    pub symbol: String,
    /// 원주문번호 ORGN_ODNO.
    pub orig_order_no: String,
    pub order_type: OverseasOrderType,
    pub quantity: u64,
    /// 주문단가. 취소 시 0.
    pub price: f64,
}

impl OverseasStock<'_> {
    /// 해외주식 매수 (TR 1).
    pub async fn buy(&self, req: OverseasOrderReq) -> Result<OverseasOrderResult> {
        let tr = buy_tr(req.exchange);
        self.order(req, tr).await
    }

    /// 해외주식 매도 (TR 2).
    pub async fn sell(&self, req: OverseasOrderReq) -> Result<OverseasOrderResult> {
        let tr = sell_tr(req.exchange);
        self.order(req, tr).await
    }

    async fn order(&self, req: OverseasOrderReq, tr: TrId) -> Result<OverseasOrderResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "OVRS_EXCG_CD": req.exchange.ovrs_excg_cd(),
            "PDNO": req.symbol,
            "ORD_QTY": req.quantity.to_string(),
            "OVRS_ORD_UNPR": req.price.to_string(),
            "ORD_DVSN": req.order_type.code(),
            "ORD_SVR_DVSN_CD": "0",
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/overseas-stock/v1/trading/order".into(),
                tr_id: tr.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }

    /// 해외주식 주문 정정 (TR 3). `RVSE_CNCL_DVSN_CD=01`.
    pub async fn revise(&self, req: OverseasReviseCancelReq) -> Result<OverseasOrderResult> {
        self.order_rvsecncl(req, "01").await
    }

    /// 해외주식 주문 취소 (TR 3). `RVSE_CNCL_DVSN_CD=02`.
    pub async fn cancel(&self, req: OverseasReviseCancelReq) -> Result<OverseasOrderResult> {
        self.order_rvsecncl(req, "02").await
    }

    async fn order_rvsecncl(
        &self,
        req: OverseasReviseCancelReq,
        dvsn: &str,
    ) -> Result<OverseasOrderResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "OVRS_EXCG_CD": req.exchange.ovrs_excg_cd(),
            "PDNO": req.symbol,
            "ORGN_ODNO": req.orig_order_no,
            "RVSE_CNCL_DVSN_CD": dvsn,
            "ORD_QTY": req.quantity.to_string(),
            "OVRS_ORD_UNPR": req.price.to_string(),
            "ORD_SVR_DVSN_CD": "0",
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/overseas-stock/v1/trading/order-rvsecncl".into(),
                tr_id: TR_RVSECNCL.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }
}
```

검증: T5에서 빌드.
커밋: `feat: 해외주식 주문 TR 3종 (매수/매도/정정취소)`

---

## T4 — `src/overseas_stock/account.rs`

계좌 3종: 잔고(TR4), 미체결내역(TR5), 주문체결내역(TR6). 모두 GET, 전부 연속조회 지원
(cursor `CTX_AREA_FK200`/`NK200`) → envelope 반환 + `*_all` 헬퍼.

> **모의 미지원**: 미체결내역(TR5, `TTTS3018R`)은 모의투자 미지원(overseas-stock.md §5) →
> `TrId::real_only`. 잔고·체결내역은 모의 지원 → `TrId::both`.
>
> **잔고 output1/output2 귀속 미확인**: overseas-stock.md §4는 `chk_inquire_balance.py`의
> COLUMN_MAPPING이 output1·output2를 단일 dict로 병합했고 정확한 귀속이 불명확하다고
> 명시한다. **결정**: 문서가 제시한 추정 분리(종목 단위 → output1, 합계 단위 → output2)를
> 따라 `OverseasBalanceItem`(output1)·`OverseasBalanceSummary`(output2) 두 struct로
> 정의한다. 단 `#[serde(default)]`를 전 필드에 부여해 귀속 오류 시 누락 필드가
> 역직렬화 실패를 일으키지 않게 한다 — Plan 1의 `[미확인]` 처리 방식과 동일 취지
> (모의 실호출로 확정, 잘못된 키는 빈 String). `ovrs_item_name`은 문서상 존재하나
> 샘플 매핑 누락(`[미확인]`)이므로 `#[serde(default)]` 필드로 포함한다.

```rust
use serde::Deserialize;

use crate::client::{ApiCall, KisResponse};
use crate::overseas_stock::OverseasExchange;
use crate::overseas_stock::OverseasStock;
use crate::error::Result;
use crate::trid::TrId;

const TR_BALANCE: TrId = TrId::both("TTTS3012R", "VTTS3012R");
const TR_NCCS: TrId = TrId::real_only("TTTS3018R"); // 미체결 — 모의 미지원
const TR_CCNL: TrId = TrId::both("TTTS3035R", "VTTS3035R");

/// 보유종목 1건 (TR4 output1 추정). overseas-stock.md §4 output1 표 verbatim.
/// output1/2 귀속이 명세상 불명확 — 전 필드 `#[serde(default)]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OverseasBalanceItem {
    pub cano: String,                // 종합계좌번호
    pub acnt_prdt_cd: String,        // 계좌상품코드
    pub prdt_type_cd: String,        // 상품유형코드
    pub ovrs_pdno: String,           // 해외상품번호(종목코드)
    pub ovrs_item_name: String,      // 해외종목명 [미확인 — 샘플 매핑 누락]
    pub frcr_evlu_pfls_amt: String,  // 외화평가손익금액
    pub evlu_pfls_rt: String,        // 평가손익율
    pub pchs_avg_pric: String,       // 매입평균가격
    pub ovrs_cblc_qty: String,       // 해외잔고수량
    pub ord_psbl_qty: String,        // 주문가능수량
    pub frcr_pchs_amt1: String,      // 외화매입금액1
    pub ovrs_stck_evlu_amt: String,  // 해외주식평가금액
    pub now_pric2: String,           // 현재가격2
    pub tr_crcy_cd: String,          // 거래통화코드
    pub ovrs_excg_cd: String,        // 해외거래소코드
    pub loan_type_cd: String,        // 대출유형코드
    pub loan_dt: String,             // 대출일자
    pub expd_dt: String,             // 만기일자
}

/// 잔고 요약 (TR4 output2 추정). §4 output2 표 verbatim.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct OverseasBalanceSummary {
    pub frcr_buy_amt_smtl1: String,  // 외화매수금액합계1
    pub frcr_buy_amt_smtl2: String,  // 외화매수금액합계2
    pub ovrs_rlzt_pfls_amt: String,  // 해외실현손익금액
    pub ovrs_rlzt_pfls_amt2: String, // 해외실현손익금액2
    pub ovrs_tot_pfls: String,       // 해외총손익
    pub rlzt_erng_rt: String,        // 실현수익율
    pub tot_evlu_pfls_amt: String,   // 총평가손익금액
    pub tot_pftrt: String,           // 총수익률
}

/// 미체결 주문 1건 (TR5 output 배열 요소). §5 응답표 verbatim.
#[derive(Debug, Clone, Deserialize)]
pub struct OverseasUnfilledOrder {
    pub ord_dt: String,              // 주문일자
    pub ord_gno_brno: String,        // 주문채번지점번호
    pub odno: String,                // 주문번호
    pub orgn_odno: String,           // 원주문번호
    pub pdno: String,                // 상품번호
    pub sll_buy_dvsn_cd: String,     // 매도매수구분코드
    pub rvse_cncl_dvsn_cd: String,   // 정정취소구분코드
    pub rjct_rson: String,           // 거부사유
    pub ord_tmd: String,             // 주문시각
    pub tr_crcy_cd: String,          // 거래통화코드
    pub natn_cd: String,             // 국가코드
    pub ft_ord_qty: String,          // FT주문수량
    pub ft_ccld_qty: String,         // FT체결수량
    pub nccs_qty: String,            // 미체결수량
    pub ft_ord_unpr3: String,        // FT주문단가3
    pub ft_ccld_unpr3: String,       // FT체결단가3
    pub ft_ccld_amt3: String,        // FT체결금액3
    pub ovrs_excg_cd: String,        // 해외거래소코드
    pub loan_type_cd: String,        // 대출유형코드
    pub loan_dt: String,             // 대출일자
    pub usa_amk_exts_rqst_yn: String, // 미국애프터마켓연장신청여부
}

/// 주문체결 1건 (TR6 output 배열 요소). §6 응답표 31필드 verbatim.
#[derive(Debug, Clone, Deserialize)]
pub struct OverseasConclusion {
    pub ord_dt: String,                 // 주문일자
    pub ord_gno_brno: String,           // 주문채번지점번호
    pub odno: String,                   // 주문번호
    pub orgn_odno: String,              // 원주문번호
    pub sll_buy_dvsn_cd: String,        // 매도매수구분코드
    pub sll_buy_dvsn_cd_name: String,   // 매도매수구분코드명
    pub rvse_cncl_dvsn: String,         // 정정취소구분
    pub rvse_cncl_dvsn_name: String,    // 정정취소구분명
    pub pdno: String,                   // 상품번호
    pub prdt_name: String,              // 상품명
    pub ft_ord_qty: String,             // FT주문수량
    pub ft_ord_unpr3: String,           // FT주문단가3
    pub ft_ccld_qty: String,            // FT체결수량
    pub ft_ccld_unpr3: String,          // FT체결단가3
    pub ft_ccld_amt3: String,           // FT체결금액3
    pub nccs_qty: String,               // 미체결수량
    pub prcs_stat_name: String,         // 처리상태명
    pub rjct_rson: String,              // 거부사유
    pub rjct_rson_name: String,         // 거부사유명
    pub ord_tmd: String,                // 주문시각
    pub tr_mket_name: String,           // 거래시장명
    pub tr_crcy_cd: String,             // 거래통화코드
    pub tr_natn: String,                // 거래국가
    pub tr_natn_name: String,           // 거래국가명
    pub ovrs_excg_cd: String,           // 해외거래소코드
    pub dmst_ord_dt: String,            // 국내주문일자
    pub thco_ord_tmd: String,           // 당사주문시각
    pub loan_type_cd: String,           // 대출유형코드
    pub loan_dt: String,                // 대출일자
    pub mdia_dvsn_name: String,         // 매체구분명
    pub usa_amk_exts_rqst_yn: String,   // 미국애프터마켓연장신청여부
    pub splt_buy_attr_name: String,     // 분할매수/매도속성명
}

/// 매도/매수 구분. 체결내역 조회 필터 (`SLL_BUY_DVSN`).
#[derive(Debug, Clone, Copy)]
pub enum OverseasSellBuy {
    All,
    Sell,
    Buy,
}

impl OverseasSellBuy {
    fn code(self) -> &'static str {
        match self {
            OverseasSellBuy::All => "00",
            OverseasSellBuy::Sell => "01",
            OverseasSellBuy::Buy => "02",
        }
    }
}

/// 체결/미체결 구분. 체결내역 조회 필터 (`CCLD_NCCS_DVSN`).
#[derive(Debug, Clone, Copy)]
pub enum FilledFilter {
    All,
    Filled,
    Unfilled,
}

impl FilledFilter {
    fn code(self) -> &'static str {
        match self {
            FilledFilter::All => "00",
            FilledFilter::Filled => "01",
            FilledFilter::Unfilled => "02",
        }
    }
}

impl OverseasStock<'_> {
    /// 해외주식 잔고 (TR 4). 한 페이지. envelope의 `data`는 (보유종목, 요약).
    pub async fn balance(
        &self,
        exchange: OverseasExchange,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<(Vec<OverseasBalanceItem>, Vec<OverseasBalanceSummary>)>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "OVRS_EXCG_CD": exchange.ovrs_excg_cd(),
            "TR_CRCY_CD": exchange.currency(),
            "CTX_AREA_FK200": fk,
            "CTX_AREA_NK200": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/overseas-stock/v1/trading/inquire-balance".into(),
                tr_id: TR_BALANCE.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<OverseasBalanceItem> = resp.field("output1")?;
        let summary: Vec<OverseasBalanceSummary> = resp.field("output2")?;
        Ok(resp.envelope((items, summary)))
    }

    /// 해외주식 잔고 전체 페이지 수집.
    pub async fn balance_all(
        &self,
        exchange: OverseasExchange,
    ) -> Result<(Vec<OverseasBalanceItem>, Vec<OverseasBalanceSummary>)> {
        let mut items = Vec::new();
        let mut summary = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let page = self
                .balance(exchange, cursor.as_ref().map(|(f, n)| (f.as_str(), n.as_str())))
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

    /// 해외주식 미체결내역 (TR 5). 모의투자 미지원 → 모의 환경에서 `UnsupportedInMock`.
    pub async fn unfilled_orders(
        &self,
        exchange: OverseasExchange,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<Vec<OverseasUnfilledOrder>>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "OVRS_EXCG_CD": exchange.ovrs_excg_cd(),
            "SORT_SQN": "",
            "CTX_AREA_FK200": fk,
            "CTX_AREA_NK200": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/overseas-stock/v1/trading/inquire-nccs".into(),
                tr_id: TR_NCCS.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<OverseasUnfilledOrder> = resp.field("output")?;
        Ok(resp.envelope(items))
    }

    /// 해외주식 주문체결내역 (TR 6). 기간 조회. 한 페이지.
    #[allow(clippy::too_many_arguments)]
    pub async fn conclusions(
        &self,
        start: &str, // YYYYMMDD
        end: &str,
        sell_buy: OverseasSellBuy,
        filled: FilledFilter,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<Vec<OverseasConclusion>>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "PDNO": "%",
            "ORD_STRT_DT": start,
            "ORD_END_DT": end,
            "SLL_BUY_DVSN": sell_buy.code(),
            "CCLD_NCCS_DVSN": filled.code(),
            "OVRS_EXCG_CD": "%",
            "SORT_SQN": "DS",
            "ORD_DT": "",
            "ORD_GNO_BRNO": "",
            "ODNO": "",
            "CTX_AREA_FK200": fk,
            "CTX_AREA_NK200": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/overseas-stock/v1/trading/inquire-ccnl".into(),
                tr_id: TR_CCNL.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<OverseasConclusion> = resp.field("output")?;
        Ok(resp.envelope(items))
    }

    /// 해외주식 주문체결내역 전체 페이지 수집.
    pub async fn conclusions_all(
        &self,
        start: &str,
        end: &str,
        sell_buy: OverseasSellBuy,
        filled: FilledFilter,
    ) -> Result<Vec<OverseasConclusion>> {
        let mut all = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let page = self
                .conclusions(
                    start,
                    end,
                    sell_buy,
                    filled,
                    cursor.as_ref().map(|(f, n)| (f.as_str(), n.as_str())),
                )
                .await?;
            all.extend(page.data);
            if page.has_next() {
                match (page.ctx_area_fk, page.ctx_area_nk) {
                    (Some(f), Some(n)) => cursor = Some((f, n)),
                    _ => break,
                }
            } else {
                break;
            }
        }
        Ok(all)
    }
}
```

검증: T5에서 빌드.
커밋: `feat: 해외주식 계좌 TR 3종 (잔고/미체결/체결내역)`

---

## T5 — 해외주식 빌드·테스트 게이트

T1~T4 코드가 컴파일되는지 확인. 신규 단위 테스트 1개 추가 — `OverseasExchange`
코드 매핑(2체계 교차 검증). `src/overseas_stock/mod.rs` 하단에:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exchange_dual_code_system() {
        // OVRS_EXCG_CD vs EXCD — 베트남 교차 매핑 검증.
        assert_eq!(OverseasExchange::Nasd.ovrs_excg_cd(), "NASD");
        assert_eq!(OverseasExchange::Nasd.excd(), "NAS");
        assert_eq!(OverseasExchange::Hase.ovrs_excg_cd(), "HASE");
        assert_eq!(OverseasExchange::Hase.excd(), "HNX");
        assert_eq!(OverseasExchange::Vnse.ovrs_excg_cd(), "VNSE");
        assert_eq!(OverseasExchange::Vnse.excd(), "HSX");
        assert_eq!(OverseasExchange::Sehk.currency(), "HKD");
    }
}
```

검증:

```
cargo build
cargo clippy -- -D warnings
cargo test
```

기대: build 에러 0, clippy 경고 0, test — Plan 1 6건 + `exchange_dual_code_system` 1건 = 7 passed.

커밋: `feat: 해외주식 모듈 빌드 통과 + 거래소 코드 테스트`

---

## T6 — `src/futureoption/mod.rs` + `lib.rs` 배선

`FutureOption` 액세서, `Session`(주간/야간) enum, `with_account` 헬퍼.
`lib.rs`에 모듈 선언 + 재노출. `KisClient`에 액세서 추가.

> **주야간 × 실전모의 매트릭스**: futureoption.md §1·§2 — 주문/정정취소 tr_id가
> 주간/야간으로 갈린다. 야간 모의는 KIS 미지원(`demo`+`night` ValueError). 따라서:
> - 주간(`Day`): `TrId::both(주간실전, 주간모의)`
> - 야간(`Night`): `TrId::real_only(야간실전)` — 모의 호출 시 `UnsupportedInMock`
> 시세 TR(§6·§7)은 주야간 구분 없음, 실전·모의 동일 → `TrId::same`.

`src/futureoption/mod.rs`:

```rust
//! 국내선물옵션 도메인 — 주문·계좌·시세 TR.

mod account;
mod order;
mod quote;

pub use account::*;
pub use order::*;
pub use quote::*;

use crate::client::KisClient;

/// 선물옵션 거래 세션. 주문·정정취소 tr_id 선택용.
/// 야간 세션은 모의투자 미지원(KIS).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Session {
    /// 주간 거래.
    Day,
    /// 야간 거래 (모의 미지원).
    Night,
}

/// 선물옵션 도메인 액세서. `client.futureoption()`으로 획득.
pub struct FutureOption<'a> {
    pub(crate) client: &'a KisClient,
}

impl<'a> FutureOption<'a> {
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

`src/lib.rs` 수정 — `pub mod overseas_stock;` 줄 아래에 추가:

```rust
pub mod overseas_stock;
pub mod futureoption;
```

`src/client.rs`의 `KisClient` impl에 액세서 추가 — `overseas_stock()` 메서드 바로 아래:

```rust
    /// 국내선물옵션 도메인 액세서.
    pub fn futureoption(&self) -> crate::futureoption::FutureOption<'_> {
        crate::futureoption::FutureOption::new(self)
    }
```

이 시점에 하위 파일이 없어 빌드 실패 — **T6은 mod.rs·배선만, 빌드 검증은 T10에서.**

커밋: `feat: FutureOption 액세서 + Session enum`

---

## T7 — `src/futureoption/quote.rs`

시세 2종: 현재가시세(TR6), 호가(TR7). 모두 GET, hashkey 불필요,
tr_id 실전·모의 동일(`TrId::same`).

> **다중 output 병합 문제 (TR6·TR7)**: futureoption.md §6·§7 §수집한계 — `chk_*.py`의
> `COLUMN_MAPPING`이 output1/2/3를 **단일 dict로 병합** 정의했고, output별 필드 귀속이
> 코드상 분리 불가능하다. **결정**: 두 TR 모두 응답을 **통합 struct 하나**로 노출하되,
> 메서드는 body의 `output1`/`output2`/`output3`를 **각각 역직렬화 후 Rust에서 병합**한다.
> 통합 struct 전 필드에 `#[serde(default)]`를 부여 — 어느 output에서 오든 한 struct로
> 수용, 키가 없으면 빈 String. 추가로 메서드는 `KisResponse<T>`가 아닌 `(통합struct, raw)`
> 형태로, raw `serde_json::Value`(전체 body)를 함께 반환해 output별 원본을 호출자가
> 직접 검사할 수 있게 보존한다. 이 결정의 근거: 명세가 output 귀속을 확정하지 못하므로
> 잘못된 분리 struct를 강제하면 필드가 조용히 누락된다 — 통합 + raw 보존이 안전하다.

```rust
use serde::Deserialize;
use serde_json::Value;

use crate::client::ApiCall;
use crate::futureoption::FutureOption;
use crate::error::Result;
use crate::trid::TrId;

// ── TR 6: 선물옵션 현재가 시세 ────────────────────────────────────────
const TR_PRICE: TrId = TrId::same("FHMIF10000000");

/// 선물옵션 현재가 시세 — output1/2/3 통합. futureoption.md §6 통합 응답표 verbatim.
/// chk 코드가 output별 귀속을 분리하지 않으므로 전 필드 `#[serde(default)]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionPrice {
    pub hts_kor_isnm: String,        // HTS 한글 종목명
    pub futs_prpr: String,           // 선물 현재가
    pub futs_prdy_vrss: String,      // 선물 전일 대비
    pub prdy_vrss_sign: String,      // 전일 대비 부호
    pub futs_prdy_clpr: String,      // 선물 전일 종가
    pub futs_prdy_ctrt: String,      // 선물 전일 대비율
    pub acml_vol: String,            // 누적 거래량
    pub acml_tr_pbmn: String,        // 누적 거래 대금
    pub hts_otst_stpl_qty: String,   // HTS 미결제 약정 수량
    pub otst_stpl_qty_icdc: String,  // 미결제 약정 수량 증감
    pub futs_oprc: String,           // 선물 시가2
    pub futs_hgpr: String,           // 선물 최고가
    pub futs_lwpr: String,           // 선물 최저가
    pub futs_mxpr: String,           // 선물 상한가
    pub futs_llam: String,           // 선물 하한가
    pub basis: String,               // 베이시스
    pub futs_sdpr: String,            // 선물 기준가
    pub hts_thpr: String,             // HTS 이론가
    pub dprt: String,                 // 괴리율
    pub crbr_aply_mxpr: String,       // 서킷브레이커 적용 상한가
    pub crbr_aply_llam: String,       // 서킷브레이커 적용 하한가
    pub futs_last_tr_date: String,    // 선물 최종 거래 일자
    pub hts_rmnn_dynu: String,        // HTS 잔존 일수
    pub futs_lstn_medm_hgpr: String,  // 선물 상장 중 최고가
    pub futs_lstn_medm_lwpr: String,  // 선물 상장 중 최저가
    pub delta_val: String,            // 델타 값
    pub gama: String,                 // 감마
    pub theta: String,                // 세타
    pub vega: String,                 // 베가
    pub rho: String,                  // 로우
    pub hist_vltl: String,            // 역사적 변동성
    pub hts_ints_vltl: String,        // HTS 내재 변동성
    pub mrkt_basis: String,           // 시장 베이시스
    pub acpr: String,                 // 행사가
    pub bstp_cls_code: String,        // 업종 구분 코드
    pub bstp_nmix_prpr: String,       // 업종 지수 현재가
    pub bstp_nmix_prdy_vrss: String,  // 업종 지수 전일 대비
    pub bstp_nmix_prdy_ctrt: String,  // 업종 지수 전일 대비율
}

// ── TR 7: 선물옵션 시세호가 ──────────────────────────────────────────
const TR_ASKING: TrId = TrId::same("FHMIF10010000");

/// 선물옵션 호가 — output1/2 통합. futureoption.md §7 통합 응답표 verbatim.
/// 호가 5단계 배열 필드는 1~5 개별 필드로 전사. 전 필드 `#[serde(default)]`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionAskingPrice {
    pub hts_kor_isnm: String,    // HTS 한글 종목명
    pub futs_prpr: String,       // 선물 현재가
    pub prdy_vrss_sign: String,  // 전일 대비 부호
    pub futs_prdy_vrss: String,  // 선물 전일 대비
    pub futs_prdy_ctrt: String,  // 선물 전일 대비율
    pub acml_vol: String,        // 누적 거래량
    pub futs_prdy_clpr: String,  // 선물 전일 종가
    pub futs_shrn_iscd: String,  // 선물 단축 종목코드
    pub futs_askp1: String,      // 선물 매도호가1
    pub futs_askp2: String,      // 선물 매도호가2
    pub futs_askp3: String,      // 선물 매도호가3
    pub futs_askp4: String,      // 선물 매도호가4
    pub futs_askp5: String,      // 선물 매도호가5
    pub futs_bidp1: String,      // 선물 매수호가1
    pub futs_bidp2: String,      // 선물 매수호가2
    pub futs_bidp3: String,      // 선물 매수호가3
    pub futs_bidp4: String,      // 선물 매수호가4
    pub futs_bidp5: String,      // 선물 매수호가5
    pub askp_rsqn1: String,      // 매도호가 잔량1
    pub askp_rsqn2: String,      // 매도호가 잔량2
    pub askp_rsqn3: String,      // 매도호가 잔량3
    pub askp_rsqn4: String,      // 매도호가 잔량4
    pub askp_rsqn5: String,      // 매도호가 잔량5
    pub bidp_rsqn1: String,      // 매수호가 잔량1
    pub bidp_rsqn2: String,      // 매수호가 잔량2
    pub bidp_rsqn3: String,      // 매수호가 잔량3
    pub bidp_rsqn4: String,      // 매수호가 잔량4
    pub bidp_rsqn5: String,      // 매수호가 잔량5
    pub askp_csnu1: String,      // 매도호가 건수1
    pub askp_csnu2: String,      // 매도호가 건수2
    pub askp_csnu3: String,      // 매도호가 건수3
    pub askp_csnu4: String,      // 매도호가 건수4
    pub askp_csnu5: String,      // 매도호가 건수5
    pub bidp_csnu1: String,      // 매수호가 건수1
    pub bidp_csnu2: String,      // 매수호가 건수2
    pub bidp_csnu3: String,      // 매수호가 건수3
    pub bidp_csnu4: String,      // 매수호가 건수4
    pub bidp_csnu5: String,      // 매수호가 건수5
    pub total_askp_rsqn: String, // 총 매도호가 잔량
    pub total_bidp_rsqn: String, // 총 매수호가 잔량
    pub total_askp_csnu: String, // 총 매도호가 건수
    pub total_bidp_csnu: String, // 총 매수호가 건수
    pub aspr_acpt_hour: String,  // 호가 접수 시간
}

/// FID 시장 분류 코드. F=지수선물 / O=지수옵션 (현재가) / JF=주식선물 (호가).
#[derive(Debug, Clone, Copy)]
pub enum MarketDiv {
    /// 지수선물 (`F`).
    IndexFuture,
    /// 지수옵션 (`O`).
    IndexOption,
    /// 주식선물 (`JF`).
    StockFuture,
}

impl MarketDiv {
    fn code(self) -> &'static str {
        match self {
            MarketDiv::IndexFuture => "F",
            MarketDiv::IndexOption => "O",
            MarketDiv::StockFuture => "JF",
        }
    }
}

/// body의 output1/2/3를 하나의 통합 struct로 병합 역직렬화하는 헬퍼.
/// 각 output 객체의 키를 합쳐 단일 객체로 만든 뒤 T로 역직렬화한다.
fn merge_outputs<T: serde::de::DeserializeOwned>(body: &Value) -> Result<T> {
    let mut merged = serde_json::Map::new();
    for key in ["output1", "output2", "output3"] {
        if let Some(Value::Object(o)) = body.get(key) {
            for (k, v) in o {
                merged.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(serde_json::from_value(Value::Object(merged))?)
}

impl FutureOption<'_> {
    /// 선물옵션 현재가 시세 (TR 6). output1/2/3 병합 통합 struct + raw body 반환.
    pub async fn current_price(
        &self,
        market: MarketDiv,
        item_code: &str, // 예 "101W09"
    ) -> Result<(FutureOptionPrice, Value)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-futureoption/v1/quotations/inquire-price".into(),
                tr_id: TR_PRICE.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.code(),
                    "FID_INPUT_ISCD": item_code,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let merged = merge_outputs(&resp.body)?;
        Ok((merged, resp.body))
    }

    /// 선물옵션 시세호가 (TR 7). output1/2 병합 통합 struct + raw body 반환.
    pub async fn asking_price(
        &self,
        market: MarketDiv,
        item_code: &str,
    ) -> Result<(FutureOptionAskingPrice, Value)> {
        let env = self.client.config().environment;
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-futureoption/v1/quotations/inquire-asking-price".into(),
                tr_id: TR_ASKING.resolve(env)?.into(),
                tr_cont: None,
                params: serde_json::json!({
                    "FID_COND_MRKT_DIV_CODE": market.code(),
                    "FID_INPUT_ISCD": item_code,
                }),
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let merged = merge_outputs(&resp.body)?;
        Ok((merged, resp.body))
    }
}
```

> `RawResponse::body`는 `pub` 필드(`src/client.rs` 61행) — `pub(crate)` struct 안이라
> 크레이트 내부에서 접근 가능. `merge_outputs`가 이를 직접 읽는다.

검증: T10에서 빌드.
커밋: `feat: 선물옵션 시세 TR 2종 (현재가/호가) — output 병합 통합 struct`

---

## T8 — `src/futureoption/order.rs`

주문 2종: 주문(TR1), 정정취소(TR2). 모두 POST. 주야간 세션별 tr_id 분기.

> **tr_id 검증**: futureoption.md §1 — 주문 주간실전 `TTTO1101U`/주간모의 `VTTO1101U`/
> 야간실전 `STTN1101U`/야간모의 미지원. §2 — 정정취소 주간실전 `TTTO1103U`/
> 주간모의 `VTTO1103U`/야간실전 `TTTN1103U`/야간모의 미지원. 야간은 `real_only`.
>
> **응답 키 대소문자**: futureoption.md §1·§2 응답표는 대문자 필드명
> (`KRX_FWDG_ORD_ORGNO` 등)으로 기재됐다. 그러나 KIS 응답 키 대소문자는 TR마다 다르고,
> Plan 1 `domestic_stock/order.rs::OrderResult`는 `#[serde(alias)]`로 양쪽을 수용했다.
> 아래 `FutureOptionOrderResult`·`FutureOptionReviseCancelResult` 두 struct는 그 패턴을
> 따라 **전 필드에 `#[serde(alias = "<대문자>", alias = "<소문자>")]`** 를 부여한다
> (코드에 이미 반영됨).

```rust
use serde::Deserialize;

use crate::client::ApiCall;
use crate::futureoption::{FutureOption, Session};
use crate::error::Result;
use crate::trid::TrId;

/// 주문 세션별 tr_id. futureoption.md §1 표 verbatim.
fn order_tr(session: Session) -> TrId {
    match session {
        Session::Day => TrId::both("TTTO1101U", "VTTO1101U"),
        Session::Night => TrId::real_only("STTN1101U"), // 야간 모의 미지원
    }
}

/// 정정취소 세션별 tr_id. futureoption.md §2 표 verbatim.
fn rvsecncl_tr(session: Session) -> TrId {
    match session {
        Session::Day => TrId::both("TTTO1103U", "VTTO1103U"),
        Session::Night => TrId::real_only("TTTN1103U"), // 야간 모의 미지원
    }
}

/// 선물옵션 주문 응답 (output). futureoption.md §1 응답표 verbatim.
#[derive(Debug, Clone, Deserialize)]
pub struct FutureOptionOrderResult {
    /// 한국거래소전송주문조직번호.
    #[serde(alias = "KRX_FWDG_ORD_ORGNO", alias = "krx_fwdg_ord_orgno")]
    pub krx_fwdg_ord_orgno: String,
    /// 주문번호.
    #[serde(alias = "ODNO", alias = "odno")]
    pub odno: String,
    /// 주문시각.
    #[serde(alias = "ORD_TMD", alias = "ord_tmd")]
    pub ord_tmd: String,
}

/// 선물옵션 정정취소 응답 (output). futureoption.md §2 응답표 verbatim.
#[derive(Debug, Clone, Deserialize)]
pub struct FutureOptionReviseCancelResult {
    #[serde(alias = "ACNT_NAME", alias = "acnt_name")]
    pub acnt_name: String,        // 계좌명
    #[serde(alias = "TRAD_DVSN_NAME", alias = "trad_dvsn_name")]
    pub trad_dvsn_name: String,   // 매매구분명
    #[serde(alias = "ITEM_NAME", alias = "item_name")]
    pub item_name: String,        // 종목명
    #[serde(alias = "ORD_TMD", alias = "ord_tmd")]
    pub ord_tmd: String,          // 주문시각
    #[serde(alias = "ORD_GNO_BRNO", alias = "ord_gno_brno")]
    pub ord_gno_brno: String,     // 주문채번지점번호
    #[serde(alias = "ORGN_ODNO", alias = "orgn_odno")]
    pub orgn_odno: String,        // 원주문번호
    #[serde(alias = "ODNO", alias = "odno")]
    pub odno: String,             // 주문번호
}

/// 매도/매수 구분 (`SLL_BUY_DVSN_CD`).
#[derive(Debug, Clone, Copy)]
pub enum SellBuy {
    /// `01` 매도.
    Sell,
    /// `02` 매수.
    Buy,
}

impl SellBuy {
    fn code(self) -> &'static str {
        match self {
            SellBuy::Sell => "01",
            SellBuy::Buy => "02",
        }
    }
}

/// 호가유형 — `NMPR_TYPE_CD` + 대응 `ORD_DVSN_CD`. 자주 쓰는 2종 + 코드 탈출구.
#[derive(Debug, Clone)]
pub enum FoOrderType {
    /// 지정가 (NMPR_TYPE_CD=01, ORD_DVSN_CD=01).
    Limit,
    /// 시장가 (NMPR_TYPE_CD=02, ORD_DVSN_CD=02).
    Market,
    /// 임의 코드 (nmpr_type_cd, ord_dvsn_cd).
    Code { nmpr_type_cd: String, ord_dvsn_cd: String },
}

impl FoOrderType {
    fn nmpr_type_cd(&self) -> &str {
        match self {
            FoOrderType::Limit => "01",
            FoOrderType::Market => "02",
            FoOrderType::Code { nmpr_type_cd, .. } => nmpr_type_cd,
        }
    }
    fn ord_dvsn_cd(&self) -> &str {
        match self {
            FoOrderType::Limit => "01",
            FoOrderType::Market => "02",
            FoOrderType::Code { ord_dvsn_cd, .. } => ord_dvsn_cd,
        }
    }
}

/// 선물옵션 주문 파라미터.
#[derive(Debug, Clone)]
pub struct FutureOptionOrderReq {
    pub session: Session,
    pub sell_buy: SellBuy,
    /// 단축상품번호 (선물 6자리 예 `101W09`, 옵션 9자리 예 `201S03370`).
    pub item_code: String,
    pub order_type: FoOrderType,
    pub quantity: u64,
    /// 주문가격. 시장가·최유리는 0.
    pub price: f64,
}

/// 선물옵션 정정/취소 파라미터.
#[derive(Debug, Clone)]
pub struct FutureOptionReviseCancelReq {
    pub session: Session,
    /// 원주문번호 ORGN_ODNO.
    pub orig_order_no: String,
    pub order_type: FoOrderType,
    /// 주문수량. `all=true`면 0(전량).
    pub quantity: u64,
    /// 주문가격. 취소·시장가는 0.
    pub price: f64,
    /// 잔량 전부 대상이면 true.
    pub all: bool,
}

impl FutureOption<'_> {
    /// 선물옵션 주문 (TR 1).
    pub async fn order(
        &self,
        req: FutureOptionOrderReq,
    ) -> Result<FutureOptionOrderResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "ORD_PRCS_DVSN_CD": "02",
            "SLL_BUY_DVSN_CD": req.sell_buy.code(),
            "SHTN_PDNO": req.item_code,
            "ORD_QTY": req.quantity.to_string(),
            "UNIT_PRICE": req.price.to_string(),
            "NMPR_TYPE_CD": req.order_type.nmpr_type_cd(),
            "KRX_NMPR_CNDT_CD": "0",
            "ORD_DVSN_CD": req.order_type.ord_dvsn_cd(),
            "CTAC_TLNO": "",
            "FUOP_ITEM_DVSN_CD": "",
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/domestic-futureoption/v1/trading/order".into(),
                tr_id: order_tr(req.session).resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }

    /// 선물옵션 주문 정정 (TR 2). `RVSE_CNCL_DVSN_CD=01`.
    pub async fn revise(
        &self,
        req: FutureOptionReviseCancelReq,
    ) -> Result<FutureOptionReviseCancelResult> {
        self.order_rvsecncl(req, "01").await
    }

    /// 선물옵션 주문 취소 (TR 2). `RVSE_CNCL_DVSN_CD=02`.
    pub async fn cancel(
        &self,
        req: FutureOptionReviseCancelReq,
    ) -> Result<FutureOptionReviseCancelResult> {
        self.order_rvsecncl(req, "02").await
    }

    async fn order_rvsecncl(
        &self,
        req: FutureOptionReviseCancelReq,
        dvsn: &str,
    ) -> Result<FutureOptionReviseCancelResult> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "ORD_PRCS_DVSN_CD": "02",
            "RVSE_CNCL_DVSN_CD": dvsn,
            "ORGN_ODNO": req.orig_order_no,
            "ORD_QTY": if req.all { "0".to_string() } else { req.quantity.to_string() },
            "UNIT_PRICE": req.price.to_string(),
            "NMPR_TYPE_CD": req.order_type.nmpr_type_cd(),
            "KRX_NMPR_CNDT_CD": "0",
            "RMN_QTY_YN": if req.all { "Y" } else { "N" },
            "ORD_DVSN_CD": req.order_type.ord_dvsn_cd(),
            "FUOP_ITEM_DVSN_CD": "",
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/uapi/domestic-futureoption/v1/trading/order-rvsecncl".into(),
                tr_id: rvsecncl_tr(req.session).resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: true,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }
}
```

> 위 코드 블록 안의 `>` 인용문(응답 키 대소문자 안내)은 실행자용 지시 — 실제 소스에는
> 주석으로 옮기거나 제거하고, `FutureOptionOrderResult`·`FutureOptionReviseCancelResult`
> 전 필드에 `#[serde(alias = "<대문자>", alias = "<소문자>")]`를 부여한다
> (Plan 1 `domestic_stock/order.rs::OrderResult` 패턴 그대로).

검증: T10에서 빌드.
커밋: `feat: 선물옵션 주문 TR 2종 (주문/정정취소)`

---

## T9 — `src/futureoption/account.rs`

계좌 3종: 잔고현황(TR3), 주문체결내역(TR4), 매수가능(TR5). 모두 GET.
TR3·TR4는 연속조회 지원(cursor `CTX_AREA_FK200`/`NK200`), TR5는 미지원.

> **다중 output 병합 (TR3·TR4)**: futureoption.md §3·§4 §수집한계 — 잔고/체결내역의
> `chk_*.py` COLUMN_MAPPING이 output1(배열)·output2(객체)를 단일 dict 병합 정의했다.
> **결정**: 응답 구조는 명세가 명시(`output1`=배열, `output2`=단일 객체)하므로 **배열은
> `output1`에서, 요약은 `output2`에서 각각 역직렬화**한다. 단 필드 귀속이 chk 코드상
> 불분명하므로 — 명세의 통합 목록에서 종목/주문 단위 필드를 `*Item`(output1)으로,
> 합계/요약 필드(`tot_*`, `*_smtl`, 예수금·증거금류)를 `*Summary`(output2)로 나누어
> 정의하되, **두 struct 전 필드에 `#[serde(default)]`** 를 부여한다. 귀속 오류 시
> 누락 필드는 빈 String이 되고 역직렬화는 실패하지 않는다(Plan 1 `[미확인]` 처리 취지).
> output별 분리가 chk 코드상 불명확하다는 점, 모의 실호출로 확정해야 한다는 점을
> 두 struct 상단 doc 주석에 명시한다.

```rust
use serde::Deserialize;

use crate::client::{ApiCall, KisResponse};
use crate::futureoption::FutureOption;
use crate::error::Result;
use crate::trid::TrId;

const TR_BALANCE: TrId = TrId::both("CTFO6118R", "VTFO6118R");
const TR_CCNL: TrId = TrId::both("TTTO5201R", "VTTO5201R");
const TR_PSBL_ORDER: TrId = TrId::both("TTTO5105R", "VTTO5105R");

/// 잔고 종목 1건 (TR3 output1 추정). futureoption.md §3 통합표 종목 단위 필드 verbatim.
/// output1/2 귀속이 chk 코드상 불명확 — 전 필드 `#[serde(default)]`. 모의 실호출로 확정.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionBalanceItem {
    pub cano: String,             // 종합계좌번호
    pub acnt_prdt_cd: String,     // 계좌상품코드
    pub pdno: String,             // 상품번호
    pub prdt_type_cd: String,     // 상품유형코드
    pub shtn_pdno: String,        // 단축상품번호
    pub prdt_name: String,        // 상품명
    pub sll_buy_dvsn_name: String, // 매도매수구분명
    pub cblc_qty: String,         // 잔고수량
    pub excc_unpr: String,        // 정산단가
    pub ccld_avg_unpr1: String,   // 체결평균단가1
    pub idx_clpr: String,         // 지수종가
    pub pchs_amt: String,         // 매입금액
    pub evlu_amt: String,         // 평가금액
    pub evlu_pfls_amt: String,    // 평가손익금액
    pub trad_pfls_amt: String,    // 매매손익금액
    pub lqd_psbl_qty: String,     // 청산가능수량
}

/// 잔고 계좌 요약 (TR3 output2 추정). §3 통합표 계좌/예수금/증거금 필드 verbatim.
/// output1/2 귀속이 chk 코드상 불명확 — 전 필드 `#[serde(default)]`. 모의 실호출로 확정.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionBalanceSummary {
    pub dnca_cash: String,             // 예수금현금
    pub frcr_dncl_amt: String,         // 외화예수금액
    pub dnca_sbst: String,             // 예수금대용
    pub tot_dncl_amt: String,          // 총예수금액
    pub tot_ccld_amt: String,          // 총체결금액
    pub cash_mgna: String,             // 현금증거금
    pub sbst_mgna: String,             // 대용증거금
    pub mgna_tota: String,             // 증거금총액
    pub opt_dfpa: String,              // 옵션차금
    pub thdt_dfpa: String,             // 당일차금
    pub rnwl_dfpa: String,             // 갱신차금
    pub fee: String,                   // 수수료
    pub nxdy_dnca: String,             // 익일예수금
    pub nxdy_dncl_amt: String,         // 익일예수금액
    pub prsm_dpast: String,            // 추정예탁자산
    pub prsm_dpast_amt: String,        // 추정예탁자산금액
    pub pprt_ord_psbl_cash: String,    // 적정주문가능현금
    pub add_mgna_cash: String,         // 추가증거금현금
    pub add_mgna_tota: String,         // 추가증거금총액
    pub futr_trad_pfls_amt: String,    // 선물매매손익금액
    pub opt_trad_pfls_amt: String,     // 옵션매매손익금액
    pub futr_evlu_pfls_amt: String,    // 선물평가손익금액
    pub opt_evlu_pfls_amt: String,     // 옵션평가손익금액
    pub trad_pfls_amt_smtl: String,    // 매매손익금액합계
    pub evlu_pfls_amt_smtl: String,    // 평가손익금액합계
    pub wdrw_psbl_tot_amt: String,     // 인출가능총금액
    pub ord_psbl_cash: String,         // 주문가능현금
    pub ord_psbl_sbst: String,         // 주문가능대용
    pub ord_psbl_tota: String,         // 주문가능총액
    pub pchs_amt_smtl: String,         // 매입금액합계
    pub evlu_amt_smtl: String,         // 평가금액합계
}

/// 주문체결 1건 (TR4 output1 추정). §4 통합표 주문/체결 단위 필드 verbatim.
/// output1/2 귀속이 chk 코드상 불명확 — 전 필드 `#[serde(default)]`. 모의 실호출로 확정.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionConclusion {
    pub ord_gno_brno: String,             // 주문채번지점번호
    pub cano: String,                     // 종합계좌번호
    pub csac_name: String,                // 종합계좌명
    pub acnt_prdt_cd: String,             // 계좌상품코드
    pub ord_dt: String,                   // 주문일자
    pub odno: String,                     // 주문번호
    pub orgn_odno: String,                // 원주문번호
    pub sll_buy_dvsn_cd: String,          // 매도매수구분코드
    pub trad_dvsn_name: String,           // 매매구분명
    pub nmpr_type_cd: String,             // 호가유형코드
    pub nmpr_type_name: String,           // 호가유형명
    pub pdno: String,                     // 상품번호
    pub prdt_name: String,                // 상품명
    pub prdt_type_cd: String,             // 상품유형코드
    pub ord_qty: String,                  // 주문수량
    pub ord_idx: String,                  // 주문지수
    pub qty: String,                      // 잔량
    pub ord_tmd: String,                  // 주문시각
    pub tot_ccld_qty: String,             // 총체결수량
    pub avg_idx: String,                  // 평균지수
    pub tot_ccld_amt: String,             // 총체결금액
    pub rjct_qty: String,                 // 거부수량
    pub ingr_trad_rjct_rson_cd: String,   // 장내매매거부사유코드
    pub ingr_trad_rjct_rson_name: String, // 장내매매거부사유명
    pub ord_stfno: String,                // 주문직원번호
    pub sprd_item_yn: String,             // 스프레드종목여부
    pub ord_ip_addr: String,              // 주문IP주소
}

/// 주문체결 요약 (TR4 output2 추정). §4 통합표 합계 필드 verbatim.
/// output1/2 귀속이 chk 코드상 불명확 — 전 필드 `#[serde(default)]`. 모의 실호출로 확정.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct FutureOptionConclusionSummary {
    pub tot_ord_qty: String,        // 총주문수량
    pub tot_ccld_amt_smtl: String,  // 총체결금액합계
    pub tot_ccld_qty_smtl: String,  // 총체결수량합계
    pub fee_smtl: String,           // 수수료합계
    pub ctac_tlno: String,          // 연락전화번호
}

/// 매수가능 정보 (TR5 output). futureoption.md §5 응답표 verbatim.
#[derive(Debug, Clone, Deserialize)]
pub struct FutureOptionBuyable {
    pub tot_psbl_qty: String,   // 총가능수량
    pub lqd_psbl_qty1: String,  // 청산가능수량1
    pub ord_psbl_qty: String,   // 주문가능수량
    pub bass_idx: String,       // 기준지수
}

/// 매도/매수 구분. 체결내역 필터 (`SLL_BUY_DVSN_CD`).
#[derive(Debug, Clone, Copy)]
pub enum CcnlSellBuy {
    All,
    Sell,
    Buy,
}

impl CcnlSellBuy {
    fn code(self) -> &'static str {
        match self {
            CcnlSellBuy::All => "00",
            CcnlSellBuy::Sell => "01",
            CcnlSellBuy::Buy => "02",
        }
    }
}

/// 체결/미체결 구분. 체결내역 필터 (`CCLD_NCCS_DVSN`).
#[derive(Debug, Clone, Copy)]
pub enum CcnlFilter {
    All,
    Filled,
    Unfilled,
}

impl CcnlFilter {
    fn code(self) -> &'static str {
        match self {
            CcnlFilter::All => "00",
            CcnlFilter::Filled => "01",
            CcnlFilter::Unfilled => "02",
        }
    }
}

impl FutureOption<'_> {
    /// 선물옵션 잔고현황 (TR 3). 한 페이지. envelope `data`는 (잔고종목, 요약).
    /// `MGNA_DVSN`=01(게시)/02(유지), `EXCC_STAT_CD`=1(정산)/2(본정산).
    pub async fn balance(
        &self,
        margin_div: &str,   // "01" 또는 "02"
        excc_stat: &str,    // "1" 또는 "2"
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<(Vec<FutureOptionBalanceItem>, FutureOptionBalanceSummary)>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "MGNA_DVSN": margin_div,
            "EXCC_STAT_CD": excc_stat,
            "CTX_AREA_FK200": fk,
            "CTX_AREA_NK200": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-futureoption/v1/trading/inquire-balance".into(),
                tr_id: TR_BALANCE.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<FutureOptionBalanceItem> = resp.field("output1")?;
        let summary: FutureOptionBalanceSummary = resp.field("output2")?;
        Ok(resp.envelope((items, summary)))
    }

    /// 선물옵션 잔고현황 전체 페이지 수집.
    pub async fn balance_all(
        &self,
        margin_div: &str,
        excc_stat: &str,
    ) -> Result<(Vec<FutureOptionBalanceItem>, Vec<FutureOptionBalanceSummary>)> {
        let mut items = Vec::new();
        let mut summaries = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let page = self
                .balance(
                    margin_div,
                    excc_stat,
                    cursor.as_ref().map(|(f, n)| (f.as_str(), n.as_str())),
                )
                .await?;
            let (mut it, su) = page.data;
            items.append(&mut it);
            summaries.push(su);
            if page.has_next() {
                match (page.ctx_area_fk, page.ctx_area_nk) {
                    (Some(f), Some(n)) => cursor = Some((f, n)),
                    _ => break,
                }
            } else {
                break;
            }
        }
        Ok((items, summaries))
    }

    /// 선물옵션 주문체결내역조회 (TR 4). 한 페이지. envelope `data`는 (체결내역, 요약).
    #[allow(clippy::too_many_arguments)]
    pub async fn conclusions(
        &self,
        start: &str, // YYYYMMDD
        end: &str,
        sell_buy: CcnlSellBuy,
        filter: CcnlFilter,
        cursor: Option<(&str, &str)>,
    ) -> Result<KisResponse<(Vec<FutureOptionConclusion>, FutureOptionConclusionSummary)>> {
        let env = self.client.config().environment;
        let (fk, nk) = cursor.unwrap_or(("", ""));
        let params = self.with_account(serde_json::json!({
            "STRT_ORD_DT": start,
            "END_ORD_DT": end,
            "SLL_BUY_DVSN_CD": sell_buy.code(),
            "CCLD_NCCS_DVSN": filter.code(),
            "SORT_SQN": "DS",
            "PDNO": "",
            "STRT_ODNO": "",
            "MKET_ID_CD": "",
            "CTX_AREA_FK200": fk,
            "CTX_AREA_NK200": nk,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-futureoption/v1/trading/inquire-ccnl".into(),
                tr_id: TR_CCNL.resolve(env)?.into(),
                tr_cont: cursor.map(|_| "N".to_string()),
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        let items: Vec<FutureOptionConclusion> = resp.field("output1")?;
        let summary: FutureOptionConclusionSummary = resp.field("output2")?;
        Ok(resp.envelope((items, summary)))
    }

    /// 선물옵션 주문체결내역 전체 페이지 수집.
    pub async fn conclusions_all(
        &self,
        start: &str,
        end: &str,
        sell_buy: CcnlSellBuy,
        filter: CcnlFilter,
    ) -> Result<Vec<FutureOptionConclusion>> {
        let mut all = Vec::new();
        let mut cursor: Option<(String, String)> = None;
        loop {
            let page = self
                .conclusions(
                    start,
                    end,
                    sell_buy,
                    filter,
                    cursor.as_ref().map(|(f, n)| (f.as_str(), n.as_str())),
                )
                .await?;
            let (mut it, _su) = page.data;
            all.append(&mut it);
            if page.has_next() {
                match (page.ctx_area_fk, page.ctx_area_nk) {
                    (Some(f), Some(n)) => cursor = Some((f, n)),
                    _ => break,
                }
            } else {
                break;
            }
        }
        Ok(all)
    }

    /// 선물옵션 매수가능조회 (TR 5). 연속조회 미지원 — 단건.
    pub async fn buyable(
        &self,
        item_code: &str,
        sell_buy: SellBuy, // futureoption/order.rs의 SellBuy 재사용
        price: f64,
        ord_dvsn_cd: &str, // 주문구분코드
    ) -> Result<FutureOptionBuyable> {
        let env = self.client.config().environment;
        let params = self.with_account(serde_json::json!({
            "PDNO": item_code,
            "SLL_BUY_DVSN_CD": sell_buy_code(sell_buy),
            "UNIT_PRICE": price.to_string(),
            "ORD_DVSN_CD": ord_dvsn_cd,
        }));
        let resp = self
            .client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/uapi/domestic-futureoption/v1/trading/inquire-psbl-order".into(),
                tr_id: TR_PSBL_ORDER.resolve(env)?.into(),
                tr_cont: None,
                params,
                is_post: false,
                needs_hashkey: false,
            })
            .await?;
        resp.field("output")
    }
}
```

> **`SellBuy` 재사용 처리**: `buyable`의 `SLL_BUY_DVSN_CD`는 주문 TR과 동일 코드체계
> (01:매도/02:매수). `order.rs`의 `SellBuy::code()`는 `fn`(비공개)이다 — 두 방법 중 하나:
> (A) `order.rs`의 `SellBuy::code`를 `pub(crate) fn code`로 승격 후 `account.rs`에서
>     `sell_buy.code()` 직접 호출 (위 코드의 `sell_buy_code()` 헬퍼 제거).
> (B) `account.rs`에 `fn sell_buy_code(s: SellBuy) -> &'static str` 로컬 헬퍼 정의.
> **결정: (A)** — 단일 출처 원칙. 실행자는 `order.rs`에서 `impl SellBuy { fn code →
> pub(crate) fn code }`로 바꾸고 `account.rs`는 `use crate::futureoption::SellBuy;`
> 후 `sell_buy.code()` 호출. 위 코드의 `sell_buy_code(sell_buy)`를 `sell_buy.code()`로 교체.

검증: T10에서 빌드.
커밋: `feat: 선물옵션 계좌 TR 3종 (잔고/체결내역/매수가능)`

---

## T10 — 선물옵션 빌드·테스트 게이트

T6~T9 코드 컴파일 확인. 신규 단위 테스트 1개 — `merge_outputs` 병합 동작 검증.
`src/futureoption/quote.rs` 하단에:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_outputs_combines_keys() {
        let body = serde_json::json!({
            "rt_cd": "0",
            "output1": { "futs_prpr": "350.05", "acml_vol": "12000" },
            "output3": { "bstp_nmix_prpr": "2500.10" }
        });
        let merged: FutureOptionPrice = merge_outputs(&body).unwrap();
        assert_eq!(merged.futs_prpr, "350.05");
        assert_eq!(merged.acml_vol, "12000");
        assert_eq!(merged.bstp_nmix_prpr, "2500.10");
        // 누락 키는 #[serde(default)]로 빈 String.
        assert_eq!(merged.delta_val, "");
    }
}
```

검증:

```
cargo build
cargo clippy -- -D warnings
cargo test
```

기대: build 에러 0, clippy 경고 0, test — Plan 1 6건 + 해외 1건(T5) +
`merge_outputs_combines_keys` 1건 = 8 passed.

커밋: `feat: 선물옵션 모듈 빌드 통과 + output 병합 테스트`

---

## T11 — `examples/overseas_quote.rs`

```rust
//! 해외주식 현재가 + 기간시세 예제.
//! 실행: KIS_* 환경변수 설정 후
//! `cargo run --example overseas_quote -- AAPL`

use korea_stock::kis::overseas_stock::{OverseasExchange, OverseasPeriod};
use korea_stock::{KisClient, KisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let symbol = std::env::args().nth(1).unwrap_or_else(|| "AAPL".into());

    let client = KisClient::new(KisConfig::from_env()?)?;
    let os = client.overseas_stock();

    let price = os
        .current_price(OverseasExchange::Nasd, &symbol)
        .await?;
    println!("{symbol} 현재가: {} ({})", price.last, price.rate);

    let (summary, candles) = os
        .period_price(OverseasExchange::Nasd, &symbol, OverseasPeriod::Daily, "", true)
        .await?;
    println!("종목 {} — 일봉 {}건", summary.rsym, candles.len());
    for c in candles.iter().take(5) {
        println!("  {} 종가 {} 거래량 {}", c.xymd, c.clos, c.tvol);
    }

    Ok(())
}
```

`Cargo.toml`에 추가:

```toml
[[example]]
name = "overseas_quote"
```

검증: `cargo build --example overseas_quote` 에러 0.
커밋: `docs: 해외주식 현재가·기간시세 예제`

---

## T12 — `examples/futureoption_quote.rs`

```rust
//! 선물옵션 현재가 + 호가 예제.
//! 실행: `cargo run --example futureoption_quote -- 101W09`

use korea_stock::kis::futureoption::MarketDiv;
use korea_stock::{KisClient, KisConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();
    let code = std::env::args().nth(1).unwrap_or_else(|| "101W09".into());

    let client = KisClient::new(KisConfig::from_env()?)?;
    let fo = client.futureoption();

    let (price, _raw) = fo.current_price(MarketDiv::IndexFuture, &code).await?;
    println!("{} {} 현재가: {} ({})", code, price.hts_kor_isnm, price.futs_prpr, price.futs_prdy_ctrt);
    println!("미결제약정: {}  델타: {}", price.hts_otst_stpl_qty, price.delta_val);

    let (asking, _raw) = fo.asking_price(MarketDiv::IndexFuture, &code).await?;
    println!("매도1: {} / 매수1: {}", asking.futs_askp1, asking.futs_bidp1);

    Ok(())
}
```

`Cargo.toml`에 추가:

```toml
[[example]]
name = "futureoption_quote"
```

검증: `cargo build --example futureoption_quote` 에러 0.
커밋: `docs: 선물옵션 현재가·호가 예제`

---

## T13 — `tests/integration.rs` 해외·선물옵션 스모크 추가

기존 `tests/integration.rs`에 테스트 2건 **추가**(기존 국내 테스트는 유지).

```rust
// ── Plan 2: 해외주식·선물옵션 스모크 ──────────────────────────────────

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn overseas_current_price() {
    let client = client().expect("KIS_* env vars");
    let os = client.overseas_stock();
    let price = os
        .current_price(korea_stock::kis::overseas_stock::OverseasExchange::Nasd, "AAPL")
        .await
        .expect("overseas current price call");
    assert!(!price.last.is_empty(), "현재가 비어있지 않음");
}

#[tokio::test]
#[ignore = "requires KIS_* credentials"]
async fn futureoption_current_price() {
    let client = client().expect("KIS_* env vars");
    let fo = client.futureoption();
    // 시세 TR은 실전·모의 동일 tr_id — 모의 환경에서도 호출 가능.
    // 종목코드는 만기에 따라 달라짐 — 호출 성공(rt_cd=0) 여부만 확인.
    let (price, raw) = fo
        .current_price(korea_stock::kis::futureoption::MarketDiv::IndexFuture, "101W09")
        .await
        .expect("futureoption current price call");
    // 통합 struct 또는 raw 둘 중 하나에 데이터가 있어야 함.
    assert!(raw.get("output1").is_some() || !price.futs_prpr.is_empty());
}
```

> `client()` 헬퍼는 Plan 1의 `tests/integration.rs`에 이미 정의됨 — 재사용.

검증: `cargo test --test integration` — ignored 4개 표시(국내 2 + 신규 2, 미실행).
자격증명 있으면 `cargo test --test integration -- --ignored` 통과.

커밋: `test: 해외·선물옵션 통합 스모크 테스트`

---

## T14 — `README.md` 갱신 + 최종 검증

`README.md`의 "현재 범위" 섹션을 Plan 2 반영으로 교체:

```markdown
## 현재 범위 (Plan 1 + 2)

- 국내주식 12개 TR — 시세·주문·계좌·체결
- 해외주식 8개 TR — 현재가·기간시세·매수/매도/정정취소·잔고·미체결·체결내역
- 국내선물옵션 7개 TR — 현재가·호가·주문/정정취소·잔고·체결내역·매수가능
- 실전/모의투자 환경, 토큰 자동 발급·캐싱, 레이트리밋, 연속조회

실시간 WebSocket은 Plan 3에서 추가.
```

사용 예에 해외주식 한 줄 추가(선택):

```rust
use korea_stock::kis::overseas_stock::OverseasExchange;
let p = client.overseas_stock().current_price(OverseasExchange::Nasd, "AAPL").await?;
```

최종 검증 — 전체 명령 실행, 출력 확인:

```
cargo build
cargo build --examples
cargo clippy -- -D warnings
cargo test
```

기대: build 에러 0, clippy 경고 0, test 8 passed (Plan 1 6 + 해외 1 + 선물옵션 1),
ignored 4 (통합테스트).

커밋: `docs: README + Plan 2 완료`

---

## Plan 2 완료 기준

- [ ] `cargo build` / `cargo build --examples` 에러 0
- [ ] `cargo clippy -- -D warnings` 경고 0
- [ ] `cargo test` — 단위 8건 통과, 통합 4건 ignored
- [ ] 자격증명 있으면 `cargo test --test integration -- --ignored` 통과
- [ ] `client.overseas_stock()` / `client.futureoption()` 액세서 노출
- [ ] 해외주식 8개 TR 메서드 노출 (`current_price` `period_price` `buy` `sell`
      `revise` `cancel` `balance`/`balance_all` `unfilled_orders`
      `conclusions`/`conclusions_all`)
- [ ] 선물옵션 7개 TR 메서드 노출 (`current_price` `asking_price` `order`
      `revise` `cancel` `balance`/`balance_all` `conclusions`/`conclusions_all`
      `buyable`)
- [ ] 해외 거래소 2체계 코드(`OVRS_EXCG_CD`/`EXCD`)가 `OverseasExchange` 단일 출처로 분기
- [ ] 해외 매수 6쌍·매도 6쌍 tr_id가 거래소별로 정확히 분기
- [ ] 선물옵션 야간 세션이 모의 환경에서 `UnsupportedInMock` 반환
- [ ] 해외 미체결(`TTTS3018R`)이 모의 환경에서 `UnsupportedInMock` 반환
- [ ] `RawResponse::envelope`가 `CTX_AREA_*200` cursor를 추출 (T0 패치)
- [ ] 모든 `ApiCall` 리터럴에 `needs_hashkey: false` 포함

## 명세 한계 — 구현 결정 요약

| 항목 | 명세 한계 | Plan 2 결정 |
|------|-----------|-------------|
| 해외 잔고 output1/2 귀속 | `chk_inquire_balance.py`가 단일 dict 병합, 귀속 불명확 | `OverseasBalanceItem`(output1)·`OverseasBalanceSummary`(output2) 분리 정의, 전 필드 `#[serde(default)]` — 모의 실호출로 확정 |
| 선물옵션 시세(TR6) output1/2/3 | `chk_inquire_price.py`가 3 output 단일 dict 병합 | 통합 struct `FutureOptionPrice` 하나로 노출, `merge_outputs`로 output1/2/3 키 병합, raw body 동반 반환 |
| 선물옵션 호가(TR7) output1/2 | `chk_inquire_asking_price.py` 단일 dict 병합 | 통합 struct `FutureOptionAskingPrice`, `merge_outputs` 병합, raw body 동반 반환 |
| 선물옵션 잔고(TR3)·체결(TR4) output1/2 | 명세가 구조는 명시(output1=배열/output2=객체), chk는 단일 dict | 명시된 구조대로 `output1`→`*Item` 배열, `output2`→`*Summary` 객체 분리, 전 필드 `#[serde(default)]`로 귀속 오류 흡수 |
| 전 주문 TR hashkey 강제 여부 | `[미확인]` (kis_auth.py상 비강제) | `needs_hashkey: false` 고정, `KisConfig.use_hashkey`로 전역 선택 — Plan 1과 동일 |
| 해외 `ovrs_item_name` | 문서상 존재, 샘플 매핑 누락 | `OverseasBalanceItem`에 `#[serde(default)]` 필드로 포함 |

## 다음

Plan 3 — `docs/kis-api/realtime.md` 기반 `realtime/` 모듈 (WebSocket 4 tr_id,
approval_key, AES-256-CBC 복호화).
