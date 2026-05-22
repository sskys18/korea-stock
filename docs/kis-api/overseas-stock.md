# KIS OpenAPI 해외주식 TR 레퍼런스

> 한국투자증권(KIS) OpenAPI 해외주식 도메인 주요 TR 8종 명세.

**수집일:** 2026-05-22

**출처 (우선순위순):**
1. GitHub `koreainvestment/open-trading-api` — Python 공식 샘플 코드
   - `examples_user/overseas_stock/overseas_stock_functions.py`
   - `examples_llm/overseas_stock/<기능>/<기능>.py` 및 `chk_<기능>.py` (COLUMN_MAPPING)
   - raw: `https://raw.githubusercontent.com/koreainvestment/open-trading-api/main/...`
2. KIS 개발자포털 — `https://apiportal.koreainvestment.com`

**검증 원칙:** TR ID / path / 요청·응답 필드는 위 공식 Python 샘플 코드에서 직접 추출. 샘플 코드에 없는 항목은 `[미확인]`으로 표기하고 추측하지 않음.

**일반 주의:**
- `hashkey`: KIS 주문 계열 POST 호출은 통상 hashkey 헤더를 사용하나, 공식 Python 샘플(`kis_auth`)에서 TR별 hashkey 강제 여부를 코드 레벨에서 명시하지 않음 → 본 문서에서는 `[미확인]` 처리. POST 주문 TR은 hashkey 사용을 권장.
- 연속조회: 응답 헤더 `tr_cont` 값이 `M`/`F`이면 다음 페이지 존재. 요청 헤더 `tr_cont`에 `N`을 넣고 `CTX_AREA_FK200`/`CTX_AREA_NK200`을 직전 응답값으로 채워 재호출.

---

## 거래소 / 국가 코드

### OVRS_EXCG_CD (주문/계좌 TR용 해외거래소코드)

| 코드 | 거래소 |
|------|--------|
| NASD | 미국 나스닥 |
| NYSE | 미국 뉴욕 |
| AMEX | 미국 아멕스 |
| SEHK | 홍콩 |
| SHAA | 중국 상해 |
| SZAA | 중국 심천 |
| TKSE | 일본 도쿄 |
| HASE | 베트남 하노이 |
| VNSE | 베트남 호치민 |

### EXCD (시세 TR용 거래소코드)

| 코드 | 거래소 |
|------|--------|
| NAS | 미국 나스닥 |
| NYS | 미국 뉴욕 |
| AMS | 미국 아멕스 |
| HKS | 홍콩 |
| SHS | 중국 상해 |
| SZS | 중국 심천 |
| TSE | 일본 도쿄 |
| HSX | 베트남 호치민 |
| HNX | 베트남 하노이 |

### 통화코드 (TR_CRCY_CD / CRCY_CD)

USD(미국 달러), HKD(홍콩 달러), CNY(중국 위안), JPY(일본 엔), VND(베트남 동)

### 국가코드 (NATN_CD)

000(전체), 840(미국), 344(홍콩), 156(중국), 392(일본), 704(베트남)

---

## 1. 해외주식 주문 - 매수

- **API명:** 해외주식 주문(매수) / Overseas Stock Order (Buy)
- **HTTP:** `POST /uapi/overseas-stock/v1/trading/order`
- **hashkey:** `[미확인]` (POST 주문, 사용 권장)
- **모의투자 지원:** 지원 (실전 tr_id 첫 글자 `T`→`V` 치환)
- **연속조회:** 미지원 (단건 주문)

### tr_id (매수, 거래소별)

| 거래소(OVRS_EXCG_CD) | 실전 | 모의 |
|----------------------|------|------|
| 미국 NASD/NYSE/AMEX | TTTT1002U | VTTT1002U |
| 홍콩 SEHK | TTTS1002U | VTTS1002U |
| 중국 상해 SHAA | TTTS0202U | VTTS0202U |
| 중국 심천 SZAA | TTTS0305U | VTTS0305U |
| 일본 TKSE | TTTS0308U | VTTS0308U |
| 베트남 HASE/VNSE | TTTS0311U | VTTS0311U |

### 요청 필드

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| CANO | String | Y | 종합계좌번호 (계좌 8-2 체계 앞 8자리) |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 (뒤 2자리) |
| OVRS_EXCG_CD | String | Y | 해외거래소코드 (NASD/NYSE/AMEX/SEHK/SHAA/SZAA/TKSE/HASE/VNSE) |
| PDNO | String | Y | 상품번호 (종목코드, 예 `AAPL`) |
| ORD_QTY | String | Y | 주문수량 |
| OVRS_ORD_UNPR | String | Y | 해외주문단가 (시장가 등은 `0`) |
| ORD_DVSN | String | Y | 주문구분 (00:지정가, 31:MOO, 32:LOO, 33:MOC, 34:LOC) |
| CTAC_TLNO | String | N | 연락전화번호 |
| MGCO_APTM_ODNO | String | N | 운용사지정주문번호 |
| SLL_TYPE | String | N | 매도유형 (매수 시 공란) |
| ORD_SVR_DVSN_CD | String | Y | 주문서버구분코드 (기본 `0`) |

### 응답 필드 (output)

| 필드 | 의미 |
|------|------|
| KRX_FWDG_ORD_ORGNO | 한국거래소전송주문조직번호 |
| ODNO | 주문번호 |
| ORD_TMD | 주문시각 |

---

## 2. 해외주식 주문 - 매도

- **API명:** 해외주식 주문(매도) / Overseas Stock Order (Sell)
- **HTTP:** `POST /uapi/overseas-stock/v1/trading/order`
- **hashkey:** `[미확인]` (POST 주문, 사용 권장)
- **모의투자 지원:** 지원
- **연속조회:** 미지원

> 매수와 동일한 path/함수(`order`)를 공유하며, `ord_dv` 인자가 `sell`일 때 tr_id만 달라진다.

### tr_id (매도, 거래소별)

| 거래소(OVRS_EXCG_CD) | 실전 | 모의 |
|----------------------|------|------|
| 미국 NASD/NYSE/AMEX | TTTT1006U | VTTT1006U |
| 홍콩 SEHK | TTTS1001U | VTTS1001U |
| 중국 상해 SHAA | TTTS1005U | VTTS1005U |
| 중국 심천 SZAA | TTTS0304U | VTTS0304U |
| 일본 TKSE | TTTS0307U | VTTS0307U |
| 베트남 HASE/VNSE | TTTS0310U | VTTS0310U |

### 요청 필드

매수와 동일 (위 1번 표 참조). `SLL_TYPE`은 매도 시 사용될 수 있음(공식 샘플 기본값 공란) — 세부값 `[미확인]`.

### 응답 필드 (output)

매수와 동일: `KRX_FWDG_ORD_ORGNO`, `ODNO`, `ORD_TMD`.

---

## 3. 해외주식 정정취소주문

- **API명:** 해외주식 정정취소주문 / Overseas Stock Order Revise·Cancel
- **HTTP:** `POST /uapi/overseas-stock/v1/trading/order-rvsecncl`
- **hashkey:** `[미확인]` (POST 주문, 사용 권장)
- **모의투자 지원:** 지원
- **연속조회:** 미지원

### tr_id

| 구분 | 실전 | 모의 |
|------|------|------|
| 정정취소 (공통) | TTTT1004U | VTTT1004U |

> 공식 LLM 샘플 `order_rvsecncl.py`는 거래소 무관 단일 tr_id `TTTT1004U`/`VTTT1004U`를 사용. 미국 주간거래 정정취소는 별도 TR(`daytime-order-rvsecncl`, `TTTS6038U`)이며 본 항목 범위 밖.

### 요청 필드

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| CANO | String | Y | 종합계좌번호 앞 8자리 |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 뒤 2자리 |
| OVRS_EXCG_CD | String | Y | 해외거래소코드 |
| PDNO | String | Y | 상품번호(종목코드) |
| ORGN_ODNO | String | Y | 원주문번호 (정정·취소 대상 주문번호) |
| RVSE_CNCL_DVSN_CD | String | Y | 정정취소구분코드 (01:정정, 02:취소) |
| ORD_QTY | String | Y | 주문수량 |
| OVRS_ORD_UNPR | String | Y | 해외주문단가 (취소 시 `0`) |
| MGCO_APTM_ODNO | String | N | 운용사지정주문번호 |
| ORD_SVR_DVSN_CD | String | Y | 주문서버구분코드 (기본 `0`) |

### 응답 필드 (output)

| 필드 | 의미 |
|------|------|
| KRX_FWDG_ORD_ORGNO | 한국거래소전송주문조직번호 |
| ODNO | 주문번호 |
| ORD_TMD | 주문시각 |

---

## 4. 해외주식 잔고

- **API명:** 해외주식 잔고 / Overseas Stock Balance — `[v1_해외주식-006]`
- **HTTP:** `GET /uapi/overseas-stock/v1/trading/inquire-balance`
- **hashkey:** 불필요 (GET 조회)
- **모의투자 지원:** 지원
- **연속조회:** 지원 (`CTX_AREA_FK200`/`CTX_AREA_NK200`, 헤더 `tr_cont` M/F)

### tr_id

| 구분 | tr_id |
|------|-------|
| 실전 | TTTS3012R |
| 모의 | VTTS3012R |

### 요청 필드

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| CANO | String | Y | 종합계좌번호 앞 8자리 |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 뒤 2자리 |
| OVRS_EXCG_CD | String | Y | 해외거래소코드 (NASD/NYSE/AMEX/SEHK/SHAA/SZAA/TKSE/HASE/VNSE) |
| TR_CRCY_CD | String | Y | 거래통화코드 (USD/HKD/CNY/JPY/VND) |
| CTX_AREA_FK200 | String | N | 연속조회검색조건200 (최초 공란) |
| CTX_AREA_NK200 | String | N | 연속조회키200 (최초 공란) |

### 응답 필드 — output1 (보유종목 배열)

> 공식 `chk_inquire_balance.py`의 COLUMN_MAPPING은 output1·output2를 합친 단일 매핑이다. 종목 단위 필드를 output1(보유내역)으로, 합계 단위 필드를 output2(요약)로 정리한다. 세부 output 번호 귀속은 KIS 문서 기준이며 일부 `[미확인]`.

| 필드 | 의미 |
|------|------|
| cano | 종합계좌번호 |
| acnt_prdt_cd | 계좌상품코드 |
| prdt_type_cd | 상품유형코드 |
| ovrs_pdno | 해외상품번호(종목코드) |
| ovrs_item_name | 해외종목명 `[미확인]` (KIS 문서상 존재, 샘플 매핑 누락) |
| frcr_evlu_pfls_amt | 외화평가손익금액 |
| evlu_pfls_rt | 평가손익율 |
| pchs_avg_pric | 매입평균가격 |
| ovrs_cblc_qty | 해외잔고수량 |
| ord_psbl_qty | 주문가능수량 |
| frcr_pchs_amt1 | 외화매입금액1 |
| ovrs_stck_evlu_amt | 해외주식평가금액 |
| now_pric2 | 현재가격2 |
| tr_crcy_cd | 거래통화코드 |
| ovrs_excg_cd | 해외거래소코드 |
| loan_type_cd | 대출유형코드 |
| loan_dt | 대출일자 |
| expd_dt | 만기일자 |

### 응답 필드 — output2 (잔고 요약)

| 필드 | 의미 |
|------|------|
| frcr_buy_amt_smtl1 | 외화매수금액합계1 |
| frcr_buy_amt_smtl2 | 외화매수금액합계2 |
| ovrs_rlzt_pfls_amt | 해외실현손익금액 |
| ovrs_rlzt_pfls_amt2 | 해외실현손익금액2 |
| ovrs_tot_pfls | 해외총손익 |
| rlzt_erng_rt | 실현수익율 |
| tot_evlu_pfls_amt | 총평가손익금액 |
| tot_pftrt | 총수익률 |

---

## 5. 해외주식 미체결내역

- **API명:** 해외주식 미체결내역 / Overseas Stock Unfilled Orders — `[v1_해외주식-005]`
- **HTTP:** `GET /uapi/overseas-stock/v1/trading/inquire-nccs`
- **hashkey:** 불필요 (GET 조회)
- **모의투자 지원:** 미지원 (공식 샘플에 모의 tr_id 미정의)
- **연속조회:** 지원 (`CTX_AREA_FK200`/`CTX_AREA_NK200`, 헤더 `tr_cont` M/F)

### tr_id

| 구분 | tr_id |
|------|-------|
| 실전 | TTTS3018R |
| 모의 | 미지원 |

### 요청 필드

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| CANO | String | Y | 종합계좌번호 앞 8자리 |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 뒤 2자리 |
| OVRS_EXCG_CD | String | Y | 해외거래소코드 |
| SORT_SQN | String | N | 정렬순서 (DS:정순, 그외:역순; tr_id TTTS3018R 시 공란) |
| CTX_AREA_FK200 | String | N | 연속조회검색조건200 (최초 공란) |
| CTX_AREA_NK200 | String | N | 연속조회키200 (최초 공란) |

### 응답 필드 (output)

| 필드 | 의미 |
|------|------|
| ord_dt | 주문일자 |
| ord_gno_brno | 주문채번지점번호 |
| odno | 주문번호 |
| orgn_odno | 원주문번호 |
| pdno | 상품번호 |
| sll_buy_dvsn_cd | 매도매수구분코드 |
| rvse_cncl_dvsn_cd | 정정취소구분코드 |
| rjct_rson | 거부사유 |
| ord_tmd | 주문시각 |
| tr_crcy_cd | 거래통화코드 |
| natn_cd | 국가코드 |
| ft_ord_qty | FT주문수량 |
| ft_ccld_qty | FT체결수량 |
| nccs_qty | 미체결수량 |
| ft_ord_unpr3 | FT주문단가3 |
| ft_ccld_unpr3 | FT체결단가3 |
| ft_ccld_amt3 | FT체결금액3 |
| ovrs_excg_cd | 해외거래소코드 |
| loan_type_cd | 대출유형코드 |
| loan_dt | 대출일자 |
| usa_amk_exts_rqst_yn | 미국애프터마켓연장신청여부 |

---

## 6. 해외주식 주문체결내역

- **API명:** 해외주식 주문체결내역 / Overseas Stock Order Conclusion History
- **HTTP:** `GET /uapi/overseas-stock/v1/trading/inquire-ccnl`
- **hashkey:** 불필요 (GET 조회)
- **모의투자 지원:** 지원
- **연속조회:** 지원 (`CTX_AREA_FK200`/`CTX_AREA_NK200`, 헤더 `tr_cont` M/F)

### tr_id

| 구분 | tr_id |
|------|-------|
| 실전 | TTTS3035R |
| 모의 | VTTS3035R |

### 요청 필드

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| CANO | String | Y | 종합계좌번호 앞 8자리 |
| ACNT_PRDT_CD | String | Y | 계좌상품코드 뒤 2자리 |
| PDNO | String | N | 상품번호 (`%`: 전종목) |
| ORD_STRT_DT | String | Y | 주문시작일자 (YYYYMMDD) |
| ORD_END_DT | String | Y | 주문종료일자 (YYYYMMDD) |
| SLL_BUY_DVSN | String | Y | 매도매수구분 (00:전체, 01:매도, 02:매수) |
| CCLD_NCCS_DVSN | String | Y | 체결미체결구분 (00:전체, 01:체결, 02:미체결) |
| OVRS_EXCG_CD | String | N | 해외거래소코드 (`%`: 전체) |
| SORT_SQN | String | N | 정렬순서 (DS:정순, AS:역순) |
| ORD_DT | String | N | 주문일자 (공란) |
| ORD_GNO_BRNO | String | N | 주문채번지점번호 (공란) |
| ODNO | String | N | 주문번호 (공란) |
| CTX_AREA_FK200 | String | N | 연속조회검색조건200 (최초 공란) |
| CTX_AREA_NK200 | String | N | 연속조회키200 (최초 공란) |

### 응답 필드 (output)

| 필드 | 의미 |
|------|------|
| ord_dt | 주문일자 |
| ord_gno_brno | 주문채번지점번호 |
| odno | 주문번호 |
| orgn_odno | 원주문번호 |
| sll_buy_dvsn_cd | 매도매수구분코드 |
| sll_buy_dvsn_cd_name | 매도매수구분코드명 |
| rvse_cncl_dvsn | 정정취소구분 |
| rvse_cncl_dvsn_name | 정정취소구분명 |
| pdno | 상품번호 |
| prdt_name | 상품명 |
| ft_ord_qty | FT주문수량 |
| ft_ord_unpr3 | FT주문단가3 |
| ft_ccld_qty | FT체결수량 |
| ft_ccld_unpr3 | FT체결단가3 |
| ft_ccld_amt3 | FT체결금액3 |
| nccs_qty | 미체결수량 |
| prcs_stat_name | 처리상태명 |
| rjct_rson | 거부사유 |
| rjct_rson_name | 거부사유명 |
| ord_tmd | 주문시각 |
| tr_mket_name | 거래시장명 |
| tr_crcy_cd | 거래통화코드 |
| tr_natn | 거래국가 |
| tr_natn_name | 거래국가명 |
| ovrs_excg_cd | 해외거래소코드 |
| dmst_ord_dt | 국내주문일자 |
| thco_ord_tmd | 당사주문시각 |
| loan_type_cd | 대출유형코드 |
| loan_dt | 대출일자 |
| mdia_dvsn_name | 매체구분명 |
| usa_amk_exts_rqst_yn | 미국애프터마켓연장신청여부 |
| splt_buy_attr_name | 분할매수/매도속성명 |

---

## 7. 해외주식 현재가

- **API명:** 해외주식 현재가(현재체결가) / Overseas Stock Current Price — `[v1_해외주식-009]`
- **HTTP:** `GET /uapi/overseas-price/v1/quotations/price`
- **hashkey:** 불필요 (GET 조회)
- **모의투자 지원:** 실전·모의 동일 tr_id 사용
- **연속조회:** 헤더 `tr_cont` 지원 (샘플상 단건 위주, 재귀 페이지네이션 지원)

### tr_id

| 구분 | tr_id |
|------|-------|
| 실전 | HHDFS00000300 |
| 모의 | HHDFS00000300 (동일) |

### 요청 필드

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| AUTH | String | Y | 사용자권한정보 (공란 가능) |
| EXCD | String | Y | 거래소코드 (NAS/NYS/AMS/HKS/SHS/SZS/TSE/HSX/HNX) |
| SYMB | String | Y | 종목코드 (예 `AAPL`) |

### 응답 필드 (output)

| 필드 | 의미 |
|------|------|
| rsym | 실시간조회종목코드 |
| zdiv | 소수점자리수 |
| base | 전일종가 |
| pvol | 전일거래량 |
| last | 현재가 |
| sign | 대비기호 |
| diff | 대비 |
| rate | 등락율 |
| tvol | 거래량 |
| tamt | 거래대금 |
| ordy | 매수가능여부 |

---

## 8. 해외주식 기간별시세

- **API명:** 해외주식 기간별시세 / Overseas Stock Daily Price (Period)
- **HTTP:** `GET /uapi/overseas-price/v1/quotations/dailyprice`
- **hashkey:** 불필요 (GET 조회)
- **모의투자 지원:** 실전·모의 동일 tr_id 사용
- **연속조회:** 헤더 `tr_cont` 지원 (재귀 페이지네이션, output2 누적)

### tr_id

| 구분 | tr_id |
|------|-------|
| 실전 | HHDFS76240000 |
| 모의 | HHDFS76240000 (동일) |

### 요청 필드

| 필드 | 타입 | 필수 | 설명 |
|------|------|------|------|
| AUTH | String | Y | 사용자권한정보 (공란 가능) |
| EXCD | String | Y | 거래소코드 (NAS/NYS/AMS/HKS/SHS/SZS/TSE/HSX/HNX) |
| SYMB | String | Y | 종목코드 (예 `TSLA`) |
| GUBN | String | Y | 일/주/월 구분 (0:일, 1:주, 2:월) |
| BYMD | String | N | 조회기준일자 (YYYYMMDD, 공란 시 최근일) |
| MODP | String | Y | 수정주가반영여부 (0:미반영, 1:반영) |

### 응답 필드 — output1 (요약/종목정보)

| 필드 | 의미 |
|------|------|
| rsym | 실시간조회종목코드 |
| zdiv | 소수점자리수 |
| nrec | 전일종가 |

### 응답 필드 — output2 (기간별 시세 배열)

| 필드 | 의미 |
|------|------|
| xymd | 일자 (YYYYMMDD) |
| clos | 종가 |
| sign | 대비기호 |
| diff | 대비 |
| rate | 등락율 |
| open | 시가 |
| high | 고가 |
| low | 저가 |
| tvol | 거래량 |
| tamt | 거래대금 |
| pbid | 매수호가 |
| vbid | 매수호가잔량 |
| pask | 매도호가 |
| vask | 매도호가잔량 |

> 참고: 해외 **지수/환율** 기간시세는 별도 TR `inquire-daily-chartprice`(`FHKST03030100`, path `/uapi/overseas-price/v1/quotations/inquire-daily-chartprice`)를 사용. 본 항목(개별 종목 기간시세)은 `dailyprice` TR.

---

## 수집 결과 요약

| TR | 상태 | 비고 |
|----|------|------|
| 1. 해외주식 매수 | 완전수집 | tr_id 6쌍, 요청/응답 필드 확보 |
| 2. 해외주식 매도 | 완전수집 | tr_id 6쌍, 요청/응답 필드 확보 |
| 3. 정정취소주문 | 완전수집 | tr_id 1쌍, 요청/응답 필드 확보 |
| 4. 해외주식 잔고 | 부분수집 | output1·output2 분리가 샘플상 불명확, `ovrs_item_name` 미확인 |
| 5. 미체결내역 | 완전수집 | 모의 미지원(공식 샘플 기준) |
| 6. 주문체결내역 | 완전수집 | tr_id 1쌍, 31개 응답 필드 확보 |
| 7. 해외주식 현재가 | 완전수집 | 11개 응답 필드 확보 |
| 8. 기간별시세 | 완전수집 | output1/output2 분리 확보 |

**완전수집 7 / 부분수집 1 / 실패 0**

미확인 항목: 전 주문 TR의 hashkey 강제 여부(`[미확인]`), 잔고 TR의 output1/output2 정확한 귀속 및 `ovrs_item_name`. hashkey는 KIS 관례상 POST 주문 TR에서 사용 권장.
