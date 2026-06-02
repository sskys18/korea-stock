# NXT(넥스트레이드)·통합시세 도입 설계

작성 2026-06-02. 근거: 공식 `koreainvestment/open-trading-api` 샘플 대조 + `docs/kis-api/domestic-stock.md`.
WS 필드 순서 원천: `docs/kis-api/nxt-ws-columns.txt` (공식 chk_*.py `COLUMN_MAPPING` verbatim).

## 배경

NXT = 2025-03-04 출범 국내 첫 대체거래소(ATS). KIS API는 3계층으로 노출:
- **시세 REST**: 동일 tr_id, `FID_COND_MRKT_DIV_CODE` = `J`(KRX)/`NX`(NXT)/`UN`(통합) 분기.
- **주문/계좌 REST**: 동일 tr_id, `EXCG_ID_DVSN_CD` = `KRX`/`NXT`/`SOR`/`ALL` 분기. SOR=최선집행.
- **실시간 WS**: 거래소별 **별도 tr_id** — `H0{ST,NX,UN}{CNT,ASP,ANC,MKO,MBC,PGM}0`.

## 공유 타입

```
Market { Krx, Nxt, Unified }          // 시세 REST + WS 공용
  fid_code()  -> "J" / "NX" / "UN"    // FID_COND_MRKT_DIV_CODE
  ws_infix()  -> "ST" / "NX" / "UN"   // WS tr_id 인픽스
Exchange { Krx, Nxt, Sor, All }       // 주문/계좌 EXCG_ID_DVSN_CD
  code() -> "KRX" / "NXT" / "SOR" / "ALL"
```

## Layer 1 — 시세 REST `market: Market` 파라미터

per-TR 지원 맵 (공식 샘플 대조, **doc만으론 불완전 → 샘플이 권위**):

| 메서드 | 엔드포인트 | market |
|---|---|---|
| current_price | inquire-price | J/NX/UN ✓ |
| asking_price | inquire-asking-price-exp-ccn | J/NX/UN ✓ |
| period_price | inquire-daily-itemchartprice | J/NX/UN ✓ |
| minute_chart | inquire-time-itemchartprice | J/NX/UN ✓ |
| investor_trend_daily(+_all) | investor-trade-by-stock-daily | J/NX/UN ✓ |
| program_trade_today | comp-program-trade-today | J/NX/UN ✓ |
| program_trade_daily | comp-program-trade-daily | J/NX/UN ✓ |
| **short_sale_daily** | daily-short-sale | **J만** — param 추가 안 함 |
| **investor_trend_estimate** | investor-trend-estimate | **param 없음** — 변경 없음 |

설계: 기존 `"J"` 하드코딩 제거, `market: Market` 필수 인자 추가(명시적 — 트레이딩 API라 암묵 KRX 디폴트는 footgun).

## Layer 2 — 주문/계좌 `Exchange`

- `OrderReq.exchange`·`ReviseCancelReq.exchange`: `String` → `Exchange`. `new()` 기본 `Krx`.
- `account.rs`: `EXCG_ID_DVSN_CD` 하드코딩 `"KRX"` (daily_conclusions) 파라미터화. 잔고/조회 TR이 KRX/NXT/SOR/ALL 수용 → `Exchange` 인자.

## Layer 3 — 실시간 WS 전 스트림 × 3시장

WS 필드수 매트릭스 (ST=KRX / NX=NXT / UN=통합):

| 스트림 | tr_id suffix | ST | NX | UN | decode |
|---|---|---|---|---|---|
| 체결가 CNT | CNT | 46 | 46 | 46 | `StockTrade` 재사용 (3 tr_id 동일 레이아웃) |
| 호가 ASP | ASP | 59 | 65 | 65 | `StockAsking` +6 tail (idx59–64), `f.len()>=65` 가드 |
| 예상체결 ANC | ANC | 45 | 46 | 46 | `StockTrade` 재사용, idx45 `vi_stnd_prc` 가드 (KRX 없음) |
| 장운영 MKO | MKO | 11 | 11 | **10** | 신규 `MarketOperation`; ⚠️ UN은 idx0 `mksc_shrn_iscd` 없음 |
| 회원사 MBC | MBC | 78 | 78 | 78 | 신규 `MemberTrade` |
| 프로그램 PGM | PGM | 11 | 11 | 11 | 신규 `ProgramTrade` |

비균일 레이아웃(ASP·ANC·MKO)이 OOB 패닉 위험 → tr_id별 분기 + 길이 가드 필수.

### ASP 추가 6필드 (NX/UN idx59–64, 중간가호가)
`kmid_prc, kmid_total_rsqn, kmid_cls_code, nmid_prc, nmid_total_rsqn, nmid_cls_code`
KRX(59필드)는 없음 → `Option<...>` 또는 빈 String, 길이 가드 후 채움.

### MKO 필드 (ST/NX 11, UN 10 = idx0 종목코드 제외)
`[mksc_shrn_iscd,] trht_yn, tr_susp_reas_cntt, mkop_cls_code, antc_mkop_cls_code, mrkt_trtm_cls_code, divi_app_cls_code, iscd_stat_cls_code, vi_cls_code, ovtm_vi_cls_code, exch_cls_code`

### PGM 필드 (11)
`mksc_shrn_iscd, stck_cntg_hour, seln_cnqn, seln_tr_pbmn, shnu_cnqn, shnu_tr_pbmn, ntby_cnqn, ntby_tr_pbmn, seln_rsqn, shnu_rsqn, whol_ntby_qty`

### MBC 필드 (78) — `docs/kis-api/nxt-ws-columns.txt` H0NXMBC0 참조

### SubscriptionKind (Market 파라미터화)
```
DomesticTrade(Market)        // CNT
DomesticAsking(Market)       // ASP
ExpectedConclusion(Market)   // ANC
MarketOperation(Market)      // MKO
MemberTrade(Market)          // MBC
ProgramTrade(Market)         // PGM
OrderNotice                  // 불변 (H0STCNI0/9)
OverseasTrade                // 불변 (HDFSCNT0)
```
tr_id = `"H0" + market.ws_infix() + suffix + "0"`. OrderNotice/OverseasTrade는 예외 유지.

RealtimeEvent: 신규 variant `ExpectedConclusion`, `MarketOperation`, `MemberTrade`, `ProgramTrade` 추가.
기존 `DomesticTrade`/`DomesticAsking`는 tr_id로 시장 구분(이미 event에 tr_id 포함).

## 구현 상태 (2026-06-02)

도입 완료:
- `Market{Krx,Nxt,Unified}`·`Exchange{Krx,Nxt,Sor,All}` enum.
- 시세 REST 7개 메서드 `market` 파라미터(price/asking/period/minute + investor_daily(+_all) + program_today/daily).
- 주문 `OrderReq`/`ReviseCancelReq` `exchange: Exchange`. `daily_conclusions` `exchange` 파라미터.
- 잔고 `BalanceBasis{Default,AfterHours,Nxt}` → `balance`/`balance_all` `basis` 파라미터(`AFHR_FLPR_YN` N/Y/X). NXT 평가가격 기준.
- 실시간 WS 6스트림 × 3시장(`SubscriptionKind` Market 파라미터화) + decode 구조체(MKO/MBC/PGM 신규, ASP +6, CNT/ANC StockTrade 재사용) + RealtimeEvent 4종 신규.
- 단위 테스트 8건(tr_id 합성·ASP 가드·MKO UN 오프셋·MBC 78필드 등).

미도입(KIS 자체 미지원 — 변경 없음):
- short_sale_daily(`J`만)·investor_trend_estimate(market param 없음).

> 잔고 주의: NXT 보유 종목은 거래소 무관 동일 ISIN/계좌라 `inquire-balance`에 항상
> 나타난다. `AFHR_FLPR_YN`은 보유 목록을 거르지 않고 **평가가격 기준**만 바꾼다
> (`X`=NXT 체결가). `EXCG_ID_DVSN_CD` 미수용 TR.

## 검증 한계 ⚠️

- WS 신규 decode의 필드 순서 원천은 공식 Python `COLUMN_MAPPING` **[Medium]** — 실제 H0NX*/H0UN* 와이어 프레임 미검증.
- 특히 ASP 6 tail·MBC 78필드·MKO UN 오프셋은 라이브 프레임으로 재확인 전까지 **wire-unverified**.
- NXT가 모의투자(VTS)에서 동작하는지 불확실 — 실전 환경 검증 필요. 그 전까지 컴파일 + 길이 가드 내성만 보장.
