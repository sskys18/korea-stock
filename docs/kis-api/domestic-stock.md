# KIS OpenAPI 국내주식 도메인 TR 레퍼런스

> 한국투자증권(KIS) OpenAPI 국내주식 도메인 주요 TR 12개 명세.

## 출처

| 우선순위 | 소스 | URL |
|---|---|---|
| 1 | GitHub `koreainvestment/open-trading-api` — `examples_llm/domestic_stock/<dir>/<name>.py` (요청 명세·tr_id·path) 및 `chk_<name>.py` (`COLUMN_MAPPING` = 응답 필드·한글명) | https://github.com/koreainvestment/open-trading-api/tree/main/examples_llm/domestic_stock |
| 2 | KIS 개발자포털 API 문서 (JS 렌더링으로 직접 수집 제한적) | https://apiportal.koreainvestment.com/apiservice |

- **수집일**: 2026-05-22
- **수집 방식**: GitHub raw 파일 직접 다운로드. 모든 요청 필드/tr_id/path/응답 필드명은 위 Python 샘플에서 그대로 추출(추측 없음).
- **공통 베이스 URL**: 실전 `https://openapi.koreainvestment.com:9443` / 모의 `https://openapivts.koreainvestment.com:29443`
- **표기 규칙**: 공식 소스에서 확인 못한 항목은 `[미확인]`. 추론 항목은 `[추론]`.

## 주의 사항 (수집 중 확인된 사실)

- **order-cash tr_id 변경**: 본 문서의 매수/매도 tr_id는 GitHub 샘플(작성일 20250112) 기준 신규 ID `TTTC0011U`(매도)/`TTTC0012U`(매수)이다. 과거 문서에서 쓰이던 레거시 ID `TTTC0801U`/`TTTC0802U`가 아니다. 우선순위 1 소스를 따른다.
- **hashkey**: Python 샘플의 `kis_auth` 모듈 내부에서 처리되어 함수 시그니처에 노출되지 않는다. POST(주문계) API는 KIS 관례상 hashkey 사용이 권장/필요하다 `[Medium]`. GET(조회계) API는 hashkey 불필요 `[High]`.
- **연속조회**: 요청 파라미터에 `CTX_AREA_FK100`/`CTX_AREA_NK100`이 있고 응답 헤더 `tr_cont`가 `M`/`F`이면 다음 페이지 존재. 샘플 코드에서 직접 확인.
- **`chk_inquire_daily_ccld.py`의 COLUMN_MAPPING 말미 버그**: `tot_ccld_amt`/`prsm_tlex_smtl`/`pchs_avg_pric` 한글명이 샘플에서 잘못 매핑되어 있다(예: `prsm_tlex_smtl`을 '총체결금액'으로 표기). 본 문서는 표준 의미로 교정 표기하고 해당 행에 주석을 달았다.

---

## 1. 주식주문(현금) 매수

| 항목 | 내용 |
|---|---|
| API명 | 주식주문(현금) 매수 / Order Cash (Buy) |
| HTTP | `POST /uapi/domestic-stock/v1/trading/order-cash` |
| tr_id | 실전 `TTTC0012U` / 모의 `VTTC0012U` |
| hashkey | 필요 `[Medium]` (POST 주문 API) |
| 모의투자 | 지원 (`VTTC0012U`) |
| 연속조회 | 미지원 |

### 요청 필드 (Body, 키 대문자)

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| CANO | String | Y | 종합계좌번호 (계좌 8-2 체계 앞 8자리) |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 (뒤 2자리) |
| PDNO | String | Y | 상품번호 = 종목코드 6자리 (ETN은 7자리) |
| ORD_DVSN | String | Y | 주문구분 (00:지정가, 01:시장가 등) |
| ORD_QTY | String | Y | 주문수량 |
| ORD_UNPR | String | Y | 주문단가 (시장가 등 단가 없는 주문은 "0") |
| EXCG_ID_DVSN_CD | String | Y | 거래소ID구분코드 (KRX/NXT/SOR) |
| SLL_TYPE | String | N | 매도유형 (매수 시 공란) |
| CNDT_PRIC | String | N | 조건가격 (스탑지정가호가 주문 시) |

### 응답 필드 (`output`, object)

| 이름 | 의미 |
|---|---|
| KRX_FWDG_ORD_ORGNO | 한국거래소전송주문조직번호 (정정/취소 시 사용) |
| ODNO | 주문번호 (정정/취소 시 사용) |
| ORD_TMD | 주문시각 |

---

## 2. 주식주문(현금) 매도

| 항목 | 내용 |
|---|---|
| API명 | 주식주문(현금) 매도 / Order Cash (Sell) |
| HTTP | `POST /uapi/domestic-stock/v1/trading/order-cash` |
| tr_id | 실전 `TTTC0011U` / 모의 `VTTC0011U` |
| hashkey | 필요 `[Medium]` (POST 주문 API) |
| 모의투자 | 지원 (`VTTC0011U`) |
| 연속조회 | 미지원 |

### 요청 필드 (Body, 키 대문자)

매수와 동일. `SLL_TYPE`만 매도 시 의미 있음:

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| CANO | String | Y | 종합계좌번호 |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 |
| PDNO | String | Y | 상품번호(종목코드) |
| ORD_DVSN | String | Y | 주문구분 |
| ORD_QTY | String | Y | 주문수량 |
| ORD_UNPR | String | Y | 주문단가 |
| EXCG_ID_DVSN_CD | String | Y | 거래소ID구분코드 (KRX/NXT/SOR) |
| SLL_TYPE | String | N | 매도유형 (01:일반매도, 02:임의매매, 05:대차매도) |
| CNDT_PRIC | String | N | 조건가격 (스탑지정가호가 주문 시) |

### 응답 필드 (`output`, object)

| 이름 | 의미 |
|---|---|
| KRX_FWDG_ORD_ORGNO | 한국거래소전송주문조직번호 |
| ODNO | 주문번호 |
| ORD_TMD | 주문시각 |

---

## 3. 주식주문 정정

| 항목 | 내용 |
|---|---|
| API명 | 주식주문(정정취소) - 정정 / Order Revise-Cancel (Revise) |
| HTTP | `POST /uapi/domestic-stock/v1/trading/order-rvsecncl` |
| tr_id | 실전 `TTTC0013U` / 모의 `VTTC0013U` |
| hashkey | 필요 `[Medium]` (POST 주문 API) |
| 모의투자 | 지원 (`VTTC0013U`) |
| 연속조회 | 미지원 |
| 비고 | 정정/취소 동일 path·tr_id. `RVSE_CNCL_DVSN_CD`로 구분. 정정은 `01`. 호출 전 "주식정정취소가능주문조회"로 `psbl_qty` 확인 필수. |

### 요청 필드 (Body, 키 대문자)

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| CANO | String | Y | 종합계좌번호 |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 |
| KRX_FWDG_ORD_ORGNO | String | Y | 한국거래소전송주문조직번호 (원주문의 응답값) |
| ORGN_ODNO | String | Y | 원주문번호 (원주문의 ODNO) |
| ORD_DVSN | String | Y | 주문구분 |
| RVSE_CNCL_DVSN_CD | String | Y | 정정취소구분코드 — **정정 = `01`** |
| ORD_QTY | String | Y | 주문수량 (원주문수량 초과 불가) |
| ORD_UNPR | String | Y | 주문단가 |
| QTY_ALL_ORD_YN | String | Y | 잔량전부주문여부 (Y:전량, N:일부) |
| EXCG_ID_DVSN_CD | String | Y | 거래소ID구분코드 (KRX/NXT/SOR) |
| CNDT_PRIC | String | N | 조건가격 |

### 응답 필드 (`output`, object)

| 이름 | 의미 |
|---|---|
| krx_fwdg_ord_orgno | 한국거래소전송주문조직번호 |
| odno | 주문번호 |
| ord_tmd | 주문시각 |

---

## 4. 주식주문 취소

| 항목 | 내용 |
|---|---|
| API명 | 주식주문(정정취소) - 취소 / Order Revise-Cancel (Cancel) |
| HTTP | `POST /uapi/domestic-stock/v1/trading/order-rvsecncl` |
| tr_id | 실전 `TTTC0013U` / 모의 `VTTC0013U` |
| hashkey | 필요 `[Medium]` (POST 주문 API) |
| 모의투자 | 지원 (`VTTC0013U`) |
| 연속조회 | 미지원 |
| 비고 | 정정과 동일 API. 취소는 `RVSE_CNCL_DVSN_CD = 02`. 취소 시 `ORD_QTY`/`ORD_UNPR`도 전달하나 `QTY_ALL_ORD_YN=Y`면 잔량 전부 취소. |

### 요청 필드 (Body, 키 대문자)

정정(3번)과 동일. 차이점:

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| RVSE_CNCL_DVSN_CD | String | Y | 정정취소구분코드 — **취소 = `02`** |

(나머지 필드 CANO/ACNT_PRDT_CD/KRX_FWDG_ORD_ORGNO/ORGN_ODNO/ORD_DVSN/ORD_QTY/ORD_UNPR/QTY_ALL_ORD_YN/EXCG_ID_DVSN_CD/CNDT_PRIC는 3번과 동일)

### 응답 필드 (`output`, object)

| 이름 | 의미 |
|---|---|
| krx_fwdg_ord_orgno | 한국거래소전송주문조직번호 |
| odno | 주문번호 |
| ord_tmd | 주문시각 |

---

## 5. 주식정정취소가능주문조회

| 항목 | 내용 |
|---|---|
| API명 | 주식정정취소가능주문조회 / Inquire Possible Revise-Cancel |
| HTTP | `GET /uapi/domestic-stock/v1/trading/inquire-psbl-rvsecncl` |
| tr_id | 실전 `TTTC0084R` / 모의 `[미확인]` (샘플에 모의 분기 없음 — 단일 tr_id 사용) |
| hashkey | 불필요 (GET) |
| 모의투자 | `[미확인]` (샘플은 실전/모의 분기 없이 `TTTC0084R` 단일 사용) |
| 연속조회 | 지원 (`CTX_AREA_FK100`/`CTX_AREA_NK100`, `tr_cont` M/F). 1회 최대 50건. |

### 요청 필드 (Query)

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| CANO | String | Y | 종합계좌번호 |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 |
| INQR_DVSN_1 | String | Y | 조회구분1 (0:주문, 1:종목) |
| INQR_DVSN_2 | String | Y | 조회구분2 (0:전체, 1:매도, 2:매수) |
| CTX_AREA_FK100 | String | N | 연속조회검색조건100 (최초 공란) |
| CTX_AREA_NK100 | String | N | 연속조회키100 (최초 공란) |

### 응답 필드 (`output`, array)

| 이름 | 의미 |
|---|---|
| ord_gno_brno | 주문채번지점번호 |
| odno | 주문번호 |
| orgn_odno | 원주문번호 |
| ord_dvsn_name | 주문구분명 |
| pdno | 상품번호 |
| prdt_name | 상품명 |
| rvse_cncl_dvsn_name | 정정취소구분명 |
| ord_qty | 주문수량 |
| ord_unpr | 주문단가 |
| ord_tmd | 주문시각 |
| tot_ccld_qty | 총체결수량 |
| tot_ccld_amt | 총체결금액 |
| psbl_qty | 가능수량 (정정취소가능수량 — 정정/취소 주문 전 확인 대상) |
| sll_buy_dvsn_cd | 매도매수구분코드 |
| ord_dvsn_cd | 주문구분코드 |
| mgco_aptm_odno | 운용사지정주문번호 |
| excg_dvsn_cd | 거래소구분코드 |
| excg_id_dvsn_cd | 거래소ID구분코드 |
| excg_id_dvsn_name | 거래소ID구분명 |
| stpm_cndt_pric | 스톱지정가조건가격 |
| stpm_efct_occr_yn | 스톱지정가효력발생여부 |

응답 본문에는 `ctx_area_fk100`/`ctx_area_nk100`도 포함(연속조회용).

---

## 6. 주식잔고조회

| 항목 | 내용 |
|---|---|
| API명 | 주식잔고조회 / Inquire Balance |
| HTTP | `GET /uapi/domestic-stock/v1/trading/inquire-balance` |
| tr_id | 실전 `TTTC8434R` / 모의 `VTTC8434R` |
| hashkey | 불필요 (GET) |
| 모의투자 | 지원 (`VTTC8434R`) |
| 연속조회 | 지원 (`CTX_AREA_FK100`/`CTX_AREA_NK100`). 실전 1회 최대 50건 / 모의 20건. |

### 요청 필드 (Query)

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| CANO | String | Y | 종합계좌번호 |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 |
| AFHR_FLPR_YN | String | Y | 시간외단일가·거래소여부 (N:기본, Y:시간외단일가, X:NXT) |
| OFL_YN | String | N | 오프라인여부 (샘플은 공란 전송) |
| INQR_DVSN | String | Y | 조회구분 (01:대출일별, 02:종목별) |
| UNPR_DVSN | String | Y | 단가구분 (01) |
| FUND_STTL_ICLD_YN | String | Y | 펀드결제분포함여부 (N/Y) |
| FNCG_AMT_AUTO_RDPT_YN | String | Y | 융자금액자동상환여부 (N) |
| PRCS_DVSN | String | Y | 처리구분 (00:전일매매포함, 01:전일매매미포함) |
| CTX_AREA_FK100 | String | N | 연속조회검색조건100 |
| CTX_AREA_NK100 | String | N | 연속조회키100 |

### 응답 필드 `output1` (array — 보유종목별)

| 이름 | 의미 |
|---|---|
| pdno | 상품번호 |
| prdt_name | 상품명 |
| trad_dvsn_name | 매매구분명 |
| bfdy_buy_qty | 전일매수수량 |
| bfdy_sll_qty | 전일매도수량 |
| thdt_buyqty | 금일매수수량 |
| thdt_sll_qty | 금일매도수량 |
| hldg_qty | 보유수량 |
| ord_psbl_qty | 주문가능수량 |
| pchs_avg_pric | 매입평균가격 |
| pchs_amt | 매입금액 |
| prpr | 현재가 |
| evlu_amt | 평가금액 |
| evlu_pfls_amt | 평가손익금액 |
| evlu_pfls_rt | 평가손익율 |
| evlu_erng_rt | 평가수익율 |
| loan_dt | 대출일자 |
| loan_amt | 대출금액 |
| stln_slng_chgs | 대주매각대금 |
| expd_dt | 만기일자 |
| fltt_rt | 등락율 |
| bfdy_cprs_icdc | 전일대비증감 |
| item_mgna_rt_name | 종목증거금율명 |
| grta_rt_name | 보증금율명 |
| sbst_pric | 대용가격 |
| stck_loan_unpr | 주식대출단가 |

### 응답 필드 `output2` (array — 계좌 요약, 통상 1건)

| 이름 | 의미 |
|---|---|
| dnca_tot_amt | 예수금총금액 |
| nxdy_excc_amt | 익일정산금액 |
| prvs_rcdl_excc_amt | 가수도정산금액 |
| cma_evlu_amt | CMA평가금액 |
| bfdy_buy_amt | 전일매수금액 |
| thdt_buy_amt | 금일매수금액 |
| nxdy_auto_rdpt_amt | 익일자동상환금액 |
| bfdy_sll_amt | 전일매도금액 |
| thdt_sll_amt | 금일매도금액 |
| d2_auto_rdpt_amt | D+2자동상환금액 |
| bfdy_tlex_amt | 전일제비용금액 |
| thdt_tlex_amt | 금일제비용금액 |
| tot_loan_amt | 총대출금액 |
| scts_evlu_amt | 유가평가금액 |
| tot_evlu_amt | 총평가금액 |
| nass_amt | 순자산금액 |
| fncg_gld_auto_rdpt_yn | 융자금자동상환여부 |
| pchs_amt_smtl_amt | 매입금액합계금액 |
| evlu_amt_smtl_amt | 평가금액합계금액 |
| evlu_pfls_smtl_amt | 평가손익합계금액 |
| tot_stln_slng_chgs | 총대주매각대금 |
| bfdy_tot_asst_evlu_amt | 전일총자산평가금액 |
| asst_icdc_amt | 자산증감액 |
| asst_icdc_erng_rt | 자산증감수익율 |

---

## 7. 매수가능조회

| 항목 | 내용 |
|---|---|
| API명 | 매수가능조회 / Inquire Possible Order |
| HTTP | `GET /uapi/domestic-stock/v1/trading/inquire-psbl-order` |
| tr_id | 실전 `TTTC8908R` / 모의 `VTTC8908R` |
| hashkey | 불필요 (GET) |
| 모의투자 | 지원 (`VTTC8908R`) |
| 연속조회 | 미지원 (1회 최대 1건) |
| 비고 | 매수가능수량 확인 시 `ORD_DVSN=01`(시장가)로 호출해야 종목증거금율이 반영된 수량이 나옴. |

### 요청 필드 (Query)

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| CANO | String | Y | 종합계좌번호 |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 |
| PDNO | String | Y | 상품번호 (종목코드 6자리) |
| ORD_UNPR | String | Y | 주문단가 (1주당 가격) |
| ORD_DVSN | String | Y | 주문구분 (01:시장가 권장) |
| CMA_EVLU_AMT_ICLD_YN | String | Y | CMA평가금액포함여부 (Y/N) |
| OVRS_ICLD_YN | String | Y | 해외포함여부 (Y/N) |

### 응답 필드 (`output`, object)

| 이름 | 의미 |
|---|---|
| ord_psbl_cash | 주문가능현금 |
| ord_psbl_sbst | 주문가능대용 |
| ruse_psbl_amt | 재사용가능금액 |
| fund_rpch_chgs | 펀드환매대금 |
| psbl_qty_calc_unpr | 가능수량계산단가 |
| nrcvb_buy_amt | 미수없는매수금액 (미수 미사용 시 매수가능금액) |
| nrcvb_buy_qty | 미수없는매수수량 (미수 미사용 시 매수가능수량) |
| max_buy_amt | 최대매수금액 (미수 사용 시 매수가능금액) |
| max_buy_qty | 최대매수수량 (미수 사용 시 매수가능수량) |
| cma_evlu_amt | CMA평가금액 |
| ovrs_re_use_amt_wcrc | 해외재사용금액원화 |
| ord_psbl_frcr_amt_wcrc | 주문가능외화금액원화 |

---

## 8. 주식일별주문체결조회

| 항목 | 내용 |
|---|---|
| API명 | 주식일별주문체결조회 / Inquire Daily Conclusion |
| HTTP | `GET /uapi/domestic-stock/v1/trading/inquire-daily-ccld` |
| tr_id | 3개월 **이내** — 실전 `TTTC0081R` / 모의 `VTTC0081R`<br>3개월 **이전** — 실전 `CTSC9215R` / 모의 `VTSC9215R` |
| hashkey | 불필요 (GET) |
| 모의투자 | 지원 (`VTTC0081R` / `VTSC9215R`) |
| 연속조회 | 지원 (`CTX_AREA_FK100`/`CTX_AREA_NK100`). 실전 1회 최대 100건 / 모의 15건. |
| 비고 | 3개월 이전 조회는 장중 응답 지연 가능 — 15:30 이후·짧은 기간 조회 권장. |

### 요청 필드 (Query)

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| CANO | String | Y | 종합계좌번호 |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 |
| INQR_STRT_DT | String | Y | 조회시작일자 (YYYYMMDD) |
| INQR_END_DT | String | Y | 조회종료일자 (YYYYMMDD) |
| SLL_BUY_DVSN_CD | String | Y | 매도매수구분코드 (00:전체, 01:매도, 02:매수) |
| CCLD_DVSN | String | Y | 체결구분 (00:전체, 01:체결, 02:미체결) |
| INQR_DVSN | String | Y | 조회구분 (00:역순, 01:정순) |
| INQR_DVSN_3 | String | Y | 조회구분3 (00:전체, 01:현금, 02:신용, 03:담보, 04:대주, 05:대여, 06:자기융자, 07:유통융자) |
| PDNO | String | N | 상품번호(종목코드) |
| ORD_GNO_BRNO | String | N | 주문채번지점번호 |
| ODNO | String | N | 주문번호 |
| INQR_DVSN_1 | String | N | 조회구분1 (공란:전체, 1:ELW, 2:프리보드) |
| EXCG_ID_DVSN_CD | String | N | 거래소ID구분코드 (KRX/NXT/SOR/ALL, 기본 KRX) |
| CTX_AREA_FK100 | String | N | 연속조회검색조건100 |
| CTX_AREA_NK100 | String | N | 연속조회키100 |

### 응답 필드 `output1` (array — 주문체결 내역)

| 이름 | 의미 |
|---|---|
| ord_dt | 주문일자 |
| ord_gno_brno | 주문채번지점번호 |
| odno | 주문번호 |
| orgn_odno | 원주문번호 |
| ord_dvsn_name | 주문구분명 |
| sll_buy_dvsn_cd | 매도매수구분코드 |
| sll_buy_dvsn_cd_name | 매도매수구분코드명 |
| pdno | 상품번호 |
| prdt_name | 상품명 |
| ord_qty | 주문수량 |
| ord_unpr | 주문단가 |
| ord_tmd | 주문시각 |
| tot_ccld_qty | 총체결수량 |
| avg_prvs | 평균가 |
| cncl_yn | 취소여부 |
| tot_ccld_amt | 총체결금액 |
| loan_dt | 대출일자 |
| ordr_empno | 주문자사번 |
| ord_dvsn_cd | 주문구분코드 |
| cnc_cfrm_qty | 취소확인수량 |
| rmn_qty | 잔여수량 |
| rjct_qty | 거부수량 |
| ccld_cndt_name | 체결조건명 |
| inqr_ip_addr | 조회IP주소 |
| cpbc_ordp_ord_rcit_dvsn_cd | 전산주문표주문접수구분코드 |
| cpbc_ordp_infm_mthd_dvsn_cd | 전산주문표통보방법구분코드 |
| infm_tmd | 통보시각 |
| ctac_tlno | 연락전화번호 |
| prdt_type_cd | 상품유형코드 |
| excg_dvsn_cd | 거래소구분코드 |
| cpbc_ordp_mtrl_dvsn_cd | 전산주문표자료구분코드 |
| ord_orgno | 주문조직번호 |
| rsvn_ord_end_dt | 예약주문종료일자 |
| excg_id_dvsn_cd | 거래소ID구분코드 |
| stpm_cndt_pric | 스톱지정가조건가격 |
| stpm_efct_occr_dtmd | 스톱지정가효력발생상세시각 |

### 응답 필드 `output2` (object — 합계)

| 이름 | 의미 |
|---|---|
| tot_ord_qty | 총주문수량 |
| tot_ccld_qty | 총체결수량 |
| tot_ccld_amt | 총체결금액 |
| pchs_avg_pric | 매입평균가격 |
| prsm_tlex_smtl | 추정제비용합계 |

> 주: `chk_inquire_daily_ccld.py`의 `COLUMN_MAPPING`은 `tot_ccld_amt`/`prsm_tlex_smtl`/`pchs_avg_pric` 한글명이 서로 어긋나게 적혀 있다(샘플 버그). 위 표는 필드명 의미 기준으로 교정한 값이다 `[Medium]`.

---

## 9. 주식현재가 시세

| 항목 | 내용 |
|---|---|
| API명 | 주식현재가 시세 / Inquire Price |
| HTTP | `GET /uapi/domestic-stock/v1/quotations/inquire-price` |
| tr_id | 실전 `FHKST01010100` / 모의 `FHKST01010100` (동일) |
| hashkey | 불필요 (GET) |
| 모의투자 | 지원 (실전과 동일 tr_id 사용) |
| 연속조회 | 미지원 |
| 비고 | 실시간 시세는 웹소켓 API 권장. ETN은 종목코드 6자리 앞에 `Q` 입력. |

### 요청 필드 (Query)

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| FID_COND_MRKT_DIV_CODE | String | Y | 조건 시장 분류 코드 (J:KRX, NX:NXT, UN:통합) |
| FID_INPUT_ISCD | String | Y | 입력 종목코드 (예: 005930) |

### 응답 필드 (`output`, object)

| 이름 | 의미 |
|---|---|
| iscd_stat_cls_code | 종목 상태 구분 코드 |
| marg_rate | 증거금 비율 |
| rprs_mrkt_kor_name | 대표 시장 한글명 |
| new_hgpr_lwpr_cls_code | 신 고가/저가 구분 코드 |
| bstp_kor_isnm | 업종 한글 종목명 |
| temp_stop_yn | 임시 정지 여부 |
| oprc_rang_cont_yn | 시가 범위 연장 여부 |
| clpr_rang_cont_yn | 종가 범위 연장 여부 |
| crdt_able_yn | 신용 가능 여부 |
| grmn_rate_cls_code | 보증금 비율 구분 코드 |
| elw_pblc_yn | ELW 발행 여부 |
| stck_prpr | 주식 현재가 |
| prdy_vrss | 전일 대비 |
| prdy_vrss_sign | 전일 대비 부호 |
| prdy_ctrt | 전일 대비율 |
| acml_tr_pbmn | 누적 거래 대금 |
| acml_vol | 누적 거래량 |
| prdy_vrss_vol_rate | 전일 대비 거래량 비율 |
| stck_oprc | 주식 시가 |
| stck_hgpr | 주식 최고가 |
| stck_lwpr | 주식 최저가 |
| stck_mxpr | 주식 상한가 |
| stck_llam | 주식 하한가 |
| stck_sdpr | 주식 기준가 |
| wghn_avrg_stck_prc | 가중 평균 주식 가격 |
| hts_frgn_ehrt | HTS 외국인 소진율 |
| frgn_ntby_qty | 외국인 순매수 수량 |
| pgtr_ntby_qty | 프로그램매매 순매수 수량 |
| pvt_scnd_dmrs_prc | 피벗 2차 저항 가격 |
| pvt_frst_dmrs_prc | 피벗 1차 저항 가격 |
| pvt_pont_val | 피벗 포인트 값 |
| pvt_frst_dmsp_prc | 피벗 1차 지지 가격 |
| pvt_scnd_dmsp_prc | 피벗 2차 지지 가격 |
| dmrs_val | 저항 값 |
| dmsp_val | 지지 값 |
| cpfn | 자본금 |
| rstc_wdth_prc | 제한 폭 가격 |
| stck_fcam | 주식 액면가 |
| stck_sspr | 주식 대용가 |
| aspr_unit | 호가단위 |
| hts_deal_qty_unit_val | HTS 매매 수량 단위 값 |
| lstn_stcn | 상장 주수 |
| hts_avls | HTS 시가총액 |
| per | PER |
| pbr | PBR |
| stac_month | 결산 월 |
| vol_tnrt | 거래량 회전율 |
| eps | EPS |
| bps | BPS |
| d250_hgpr | 250일 최고가 |
| d250_hgpr_date | 250일 최고가 일자 |
| d250_hgpr_vrss_prpr_rate | 250일 최고가 대비 현재가 비율 |
| d250_lwpr | 250일 최저가 |
| d250_lwpr_date | 250일 최저가 일자 |
| d250_lwpr_vrss_prpr_rate | 250일 최저가 대비 현재가 비율 |
| stck_dryy_hgpr | 주식 연중 최고가 |
| dryy_hgpr_vrss_prpr_rate | 연중 최고가 대비 현재가 비율 |
| dryy_hgpr_date | 연중 최고가 일자 |
| stck_dryy_lwpr | 주식 연중 최저가 |
| dryy_lwpr_vrss_prpr_rate | 연중 최저가 대비 현재가 비율 |
| dryy_lwpr_date | 연중 최저가 일자 |
| w52_hgpr | 52주 최고가 |
| w52_hgpr_vrss_prpr_ctrt | 52주 최고가 대비 현재가 대비 |
| w52_hgpr_date | 52주 최고가 일자 |
| w52_lwpr | 52주 최저가 |
| w52_lwpr_vrss_prpr_ctrt | 52주 최저가 대비 현재가 대비 |
| w52_lwpr_date | 52주 최저가 일자 |
| whol_loan_rmnd_rate | 전체 융자 잔고 비율 |
| ssts_yn | 공매도가능여부 |
| stck_shrn_iscd | 주식 단축 종목코드 |
| fcam_cnnm | 액면가 통화명 |
| cpfn_cnnm | 자본금 통화명 |
| apprch_rate | 접근도 |
| frgn_hldn_qty | 외국인 보유 수량 |
| vi_cls_code | VI 적용 구분 코드 |
| ovtm_vi_cls_code | 시간외단일가 VI 적용 구분 코드 |
| last_ssts_cntg_qty | 최종 공매도 체결 수량 |
| invt_caful_yn | 투자유의여부 |
| mrkt_warn_cls_code | 시장경고코드 |
| short_over_yn | 단기과열여부 |
| sltr_yn | 정리매매여부 |
| mang_issu_cls_code | 관리종목여부 |

---

## 10. 주식현재가 호가/예상체결

| 항목 | 내용 |
|---|---|
| API명 | 주식현재가 호가/예상체결 / Inquire Asking Price & Expected Conclusion |
| HTTP | `GET /uapi/domestic-stock/v1/quotations/inquire-asking-price-exp-ccn` |
| tr_id | 실전 `FHKST01010200` / 모의 `FHKST01010200` (동일) |
| hashkey | 불필요 (GET) |
| 모의투자 | 지원 (실전과 동일 tr_id 사용) |
| 연속조회 | 미지원 |
| 비고 | 실시간 데이터는 웹소켓 API 권장. |

### 요청 필드 (Query)

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| FID_COND_MRKT_DIV_CODE | String | Y | 조건 시장 분류 코드 (J:KRX, NX:NXT, UN:통합) |
| FID_INPUT_ISCD | String | Y | 입력 종목코드 (6자리) |

### 응답 필드 `output1` (object — 호가 정보)

| 이름 | 의미 |
|---|---|
| aspr_acpt_hour | 호가 접수 시간 |
| askp1 ~ askp10 | 매도호가 1~10 |
| bidp1 ~ bidp10 | 매수호가 1~10 |
| askp_rsqn1 ~ askp_rsqn10 | 매도호가 잔량 1~10 |
| bidp_rsqn1 ~ bidp_rsqn10 | 매수호가 잔량 1~10 |
| askp_rsqn_icdc1 ~ askp_rsqn_icdc10 | 매도호가 잔량 증감 1~10 |
| bidp_rsqn_icdc1 ~ bidp_rsqn_icdc10 | 매수호가 잔량 증감 1~10 |
| total_askp_rsqn | 총 매도호가 잔량 |
| total_bidp_rsqn | 총 매수호가 잔량 |
| total_askp_rsqn_icdc | 총 매도호가 잔량 증감 |
| total_bidp_rsqn_icdc | 총 매수호가 잔량 증감 |
| ovtm_total_askp_icdc | 시간외 총 매도호가 증감 |
| ovtm_total_bidp_icdc | 시간외 총 매수호가 증감 |
| ovtm_total_askp_rsqn | 시간외 총 매도호가 잔량 |
| ovtm_total_bidp_rsqn | 시간외 총 매수호가 잔량 |
| ntby_aspr_rsqn | 순매수 호가 잔량 |
| new_mkop_cls_code | 신 장운영 구분 코드 |
| antc_mkop_cls_code | 예상 장운영 구분 코드 |

### 응답 필드 `output2` (object — 예상체결 정보)

| 이름 | 의미 |
|---|---|
| antc_cnpr | 예상 체결가 |
| antc_cntg_vrss_sign | 예상 체결 대비 부호 |
| antc_cntg_vrss | 예상 체결 대비 |
| antc_cntg_prdy_ctrt | 예상 체결 전일 대비율 |
| antc_vol | 예상 거래량 |
| stck_prpr | 주식 현재가 |
| stck_oprc | 주식 시가 |
| stck_hgpr | 주식 최고가 |
| stck_lwpr | 주식 최저가 |
| stck_sdpr | 주식 기준가 |
| stck_shrn_iscd | 주식 단축 종목코드 |
| vi_cls_code | VI 적용 구분 코드 |

> 주: 샘플 `COLUMN_MAPPING`은 output1·output2를 하나의 dict로 합쳐 두었다. 위 분류는 필드 성격(`antc_*`=예상체결, `askp/bidp*`=호가)에 따라 정리한 것 `[Medium]`. 일부 시세 필드(stck_prpr 등)는 양쪽 output에 모두 노출될 수 있음.

---

## 11. 국내주식기간별시세(일/주/월/년)

| 항목 | 내용 |
|---|---|
| API명 | 국내주식기간별시세(일/주/월/년) / Inquire Daily Item Chart Price |
| HTTP | `GET /uapi/domestic-stock/v1/quotations/inquire-daily-itemchartprice` |
| tr_id | 실전 `FHKST03010100` / 모의 `FHKST03010100` (동일) |
| hashkey | 불필요 (GET) |
| 모의투자 | 지원 (실전과 동일 tr_id 사용) |
| 연속조회 | 미지원 (1회 최대 100건) |

### 요청 필드 (Query)

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| FID_COND_MRKT_DIV_CODE | String | Y | 조건 시장 분류 코드 (J:KRX, NX:NXT, UN:통합) |
| FID_INPUT_ISCD | String | Y | 입력 종목코드 (예: 005930) |
| FID_INPUT_DATE_1 | String | Y | 조회 시작일자 (YYYYMMDD) |
| FID_INPUT_DATE_2 | String | Y | 조회 종료일자 (YYYYMMDD, 최대 100건) |
| FID_PERIOD_DIV_CODE | String | Y | 기간분류코드 (D:일봉, W:주봉, M:월봉, Y:년봉) |
| FID_ORG_ADJ_PRC | String | Y | 수정주가 여부 (0:수정주가, 1:원주가) |

### 응답 필드 `output1` (object — 종목 요약)

| 이름 | 의미 |
|---|---|
| prdy_vrss | 전일 대비 |
| prdy_vrss_sign | 전일 대비 부호 |
| prdy_ctrt | 전일 대비율 |
| stck_prdy_clpr | 주식 전일 종가 |
| acml_vol | 누적 거래량 |
| acml_tr_pbmn | 누적 거래 대금 |
| hts_kor_isnm | HTS 한글 종목명 |
| stck_prpr | 주식 현재가 |
| stck_shrn_iscd | 주식 단축 종목코드 |
| prdy_vol | 전일 거래량 |
| stck_mxpr | 주식 상한가 |
| stck_llam | 주식 하한가 |
| stck_oprc | 주식 시가 |
| stck_hgpr | 주식 최고가 |
| stck_lwpr | 주식 최저가 |
| stck_prdy_oprc | 주식 전일 시가 |
| stck_prdy_hgpr | 주식 전일 최고가 |
| stck_prdy_lwpr | 주식 전일 최저가 |
| askp | 매도호가 |
| bidp | 매수호가 |
| prdy_vrss_vol | 전일 대비 거래량 |
| vol_tnrt | 거래량 회전율 |
| stck_fcam | 주식 액면가 |
| lstn_stcn | 상장 주수 |
| cpfn | 자본금 |
| hts_avls | HTS 시가총액 |
| per | PER |
| eps | EPS |
| pbr | PBR |
| itewhol_loan_rmnd_ratem | 전체 융자 잔고 비율 |

### 응답 필드 `output2` (array — 기간별 봉 데이터)

| 이름 | 의미 |
|---|---|
| stck_bsop_date | 주식 영업 일자 |
| stck_clpr | 주식 종가 |
| stck_oprc | 주식 시가 |
| stck_hgpr | 주식 최고가 |
| stck_lwpr | 주식 최저가 |
| acml_vol | 누적 거래량 |
| acml_tr_pbmn | 누적 거래 대금 |
| flng_cls_code | 락 구분 코드 |
| prtt_rate | 분할 비율 |
| mod_yn | 변경 여부 |
| prdy_vrss_sign | 전일 대비 부호 |
| prdy_vrss | 전일 대비 |
| revl_issu_reas | 재평가사유코드 |

---

## 12. 주식당일분봉조회

| 항목 | 내용 |
|---|---|
| API명 | 주식당일분봉조회 / Inquire Time Item Chart Price |
| HTTP | `GET /uapi/domestic-stock/v1/quotations/inquire-time-itemchartprice` |
| tr_id | 실전 `FHKST03010200` / 모의 `FHKST03010200` (동일) |
| hashkey | 불필요 (GET) |
| 모의투자 | 지원 (실전과 동일 tr_id 사용) |
| 연속조회 | 미지원 (1회 최대 30건) |
| 비고 | 당일 분봉만 제공(전일자 미제공). `FID_INPUT_HOUR_1`에 미래 시각 입력 시 현재가로 조회. |

### 요청 필드 (Query)

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| FID_COND_MRKT_DIV_CODE | String | Y | 조건 시장 분류 코드 (J:KRX, NX:NXT, UN:통합) |
| FID_INPUT_ISCD | String | Y | 입력 종목코드 (6자리) |
| FID_INPUT_HOUR_1 | String | Y | 입력 시간1 (HHMMSS) |
| FID_PW_DATA_INCU_YN | String | Y | 과거 데이터 포함 여부 (Y/N) |
| FID_ETC_CLS_CODE | String | Y | 기타 구분 코드 (샘플 기본값 공란) |

### 응답 필드 `output1` (object — 종목 요약)

| 이름 | 의미 |
|---|---|
| prdy_vrss | 전일 대비 |
| prdy_vrss_sign | 전일 대비 부호 |
| prdy_ctrt | 전일 대비율 |
| stck_prdy_clpr | 주식 전일 종가 |
| acml_vol | 누적 거래량 |
| acml_tr_pbmn | 누적 거래대금 |
| hts_kor_isnm | HTS 한글 종목명 |
| stck_prpr | 주식 현재가 |

### 응답 필드 `output2` (array — 분봉 데이터)

| 이름 | 의미 |
|---|---|
| stck_bsop_date | 주식 영업일자 |
| stck_cntg_hour | 주식 체결시간 |
| stck_prpr | 주식 현재가 |
| stck_oprc | 주식 시가 |
| stck_hgpr | 주식 최고가 |
| stck_lwpr | 주식 최저가 |
| cntg_vol | 체결 거래량 |
| acml_tr_pbmn | 누적 거래대금 |

> 주: `output2`의 첫 배열 `cntg_vol`은 당일 첫 체결 발생 전까지 이전 분봉 체결량이 표시되며, 첫 체결 시 갱신됨.

---

## 13. 종목별 투자자매매동향(일별) — 알파 플로우

| 항목 | 내용 |
|---|---|
| API명 | 종목별 투자자매매동향(일별) / Investor Trade By Stock Daily |
| HTTP | `GET /uapi/domestic-stock/v1/quotations/investor-trade-by-stock-daily` |
| tr_id | `FHPTJ04160001` (실전·모의 동일 가정) |
| 연속조회 | 지원 — 헤더 `tr_cont` `M`/`F`면 다음 페이지(ctx_area 없는 헤더 전용). 어댑터 `investor_trend_daily_all`이 수집. |
| 출처 | open-trading-api `examples_llm/domestic_stock/investor_trade_by_stock_daily` |

### 요청 필드 (Query)

| 이름 | 필수 | 설명 |
|---|---|---|
| FID_COND_MRKT_DIV_CODE | Y | J:KRX, NX:NXT, UN:통합 |
| FID_INPUT_ISCD | Y | 종목코드 6자리 |
| FID_INPUT_DATE_1 | Y | 기준일 YYYYMMDD |
| FID_ORG_ADJ_PRC | Y | 공란 |
| FID_ETC_CLS_CODE | Y | 공란 |

### 응답 필드 (`output1` 요약 / `output2` 일별 배열)

핵심: `stck_bsop_date`, `stck_clpr`, `frgn_ntby_qty`(외국인 순매수량), `prsn_ntby_qty`(개인),
`orgn_ntby_qty`(기관계), 각 `*_ntby_tr_pbmn`(순매수 대금), 세분류
`scrt`/`ivtr`/`pe_fund`/`bank`/`insu`/`fund`/`etc_corp` 순매수. 전체 매핑은 샘플 `chk_*.py`
COLUMN_MAPPING(110+ 필드) 참조. 어댑터는 알파 핵심 필드만 타입화(`#[serde(default)]`).

## 14. 종목별 외국인·기관 추정 가집계

| 항목 | 내용 |
|---|---|
| API명 | 종목별 외국인기관 추정가집계 / Investor Trend Estimate |
| HTTP | `GET /uapi/domestic-stock/v1/quotations/investor-trend-estimate` |
| tr_id | `HHPTJ04160200` |
| 요청 | `MKSC_SHRN_ISCD`(종목코드) 단일 |
| 응답 | **`output2`** 배열: `bsop_hour_gb`(입력구분), `frgn_fake_ntby_qty`(외국인 가집계), `orgn_fake_ntby_qty`(기관 가집계), `sum_fake_ntby_qty`(합산 가집계) |
| 비고 | 장중 확정 전 추정치. 실시간 플로우 신호. |

## 15. 프로그램매매 종합현황(시간)

| 항목 | 내용 |
|---|---|
| HTTP | `GET /uapi/domestic-stock/v1/quotations/comp-program-trade-today` |
| tr_id | `FHPPG04600101` |
| 요청 | `FID_COND_MRKT_DIV_CODE`(J), `FID_MRKT_CLS_CODE`(K:코스피/Q:코스닥, 필수), `FID_SCTN_CLS_CODE`, `FID_INPUT_ISCD`, `FID_COND_MRKT_DIV_CODE1`, `FID_INPUT_HOUR_1` (뒤 4개 공란 허용) |
| 응답 | `output` 배열: `whol_smtn_ntby_qty`(전체합계 프로그램 순매수량), `whol_smtn_ntby_tr_pbmn`(순매수 대금), `whol_ntby_vol_icdc`(순매수 증감) 등 15필드 |
| 비고 | 최근 30분, 다음조회 불가. **시장 단위**(종목코드 선택). |

## 16. 프로그램매매 종합현황(일별)

| 항목 | 내용 |
|---|---|
| HTTP | `GET /uapi/domestic-stock/v1/quotations/comp-program-trade-daily` |
| tr_id | `FHPPG04600001` |
| 요청 | `FID_COND_MRKT_DIV_CODE`(J), `FID_MRKT_CLS_CODE`(K/Q), `FID_INPUT_DATE_1`, `FID_INPUT_DATE_2` |
| 응답 | `output` 배열: 차익(`arbt_*`)·비차익(`nabt_*`) 합계 순매수 수량/대금 — `arbt_smtm_ntby_qty`, `nabt_smtn_ntby_qty`, `whol_entm_ntby_qty` 등 70필드(비율 포함). 어댑터는 차익/비차익 순매수 핵심만 타입화. |

> TR 13~16은 codex spec-review가 공식 GitHub 샘플 대조로 검증(2026-06-02). 프롬프트 원안의
> `FHKST01010900`(주식현재가 투자자)·`FHPTJ04400000`(foreign-institution-total 랭킹) 오인을 정정.
> 응답 struct는 컴파일·`#[serde(default)]` 내성만 보장 — 실사용 시 런타임 와이어 검증 필요.

---

## 수집 결과 요약

- **완전수집: 11개** — TR 1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12 (path·tr_id 실전/모의 쌍·요청 필드·응답 필드 모두 우선순위 1 소스에서 확인).
- **알파 플로우 추가: 4개** — TR 13~16 (path·tr_id·요청 필드·output 슬롯 샘플 확인. 응답 struct는 핵심 필드만 타입화, 와이어 미검증).
- **부분수집: 1개** — TR 5 (주식정정취소가능주문조회): 요청/응답 필드는 완전 확인. 단 모의투자 tr_id가 GitHub 샘플에 분기 없이 `TTTC0084R` 단일이라 모의 지원 여부 `[미확인]`.
- **실패: 0개**.

기타 `[미확인]`/`[Medium]` 표기 항목: hashkey 필요 여부(POST API)는 `kis_auth` 모듈 내부 처리로 함수 시그니처 미노출 → `[Medium]` 추론. `inquire_daily_ccld`/`inquire_asking_price_exp_ccn` output 구분은 샘플이 단일 `COLUMN_MAPPING`으로 병합해 둬 필드 성격 기준으로 분류 `[Medium]`.
