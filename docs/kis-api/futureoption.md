# KIS OpenAPI — 국내선물옵션 TR 레퍼런스

> 한국투자증권(KIS) OpenAPI 국내선물옵션(domestic-futureoption) 도메인의 핵심 TR 7개 명세.

## 출처

| 구분 | URL |
|---|---|
| 1차 (TR ID·path·요청필드) | `koreainvestment/open-trading-api` GitHub — `examples_llm/domestic_futureoption/` Python 샘플 코드 |
| 1차 (요청필드 메타데이터) | `MCP/Kis Trading MCP/configs/domestic_futureoption.json` (동일 레포) |
| 1차 (응답필드명·의미) | 동일 레포 각 예제 디렉터리의 `chk_<name>.py` 내 `COLUMN_MAPPING` dict |
| 인증 동작 (hashkey) | 동일 레포 `examples_llm/kis_auth.py` |
| 2차 | KIS 개발자포털 `https://apiportal.koreainvestment.com` (SPA·JS 렌더링으로 응답필드 테이블 수집 불가) |

- 수집일: **2026-05-22**
- 레포 raw 경로 예: `https://raw.githubusercontent.com/koreainvestment/open-trading-api/main/examples_llm/domestic_futureoption/<dir>/<file>.py`

## 수집 한계 (반드시 참조)

- 본 7개 TR의 **요청 필드는 공식 샘플 코드에서 100% 확인**됨.
- **응답 필드 이름·의미는 각 예제 디렉터리의 `chk_<name>.py` 파일 내 `COLUMN_MAPPING` dict에서 7개 TR 전부 확보**됨 (영문 필드명 + 한글 의미). 응답 구조(output 키 구성·배열/객체 여부)는 메인 `<name>.py` 샘플 코드에서 확인.
- 단, `chk_inquire_price.py`의 `COLUMN_MAPPING`은 `output1`/`output2`/`output3`를 **하나의 dict로 병합**하여 정의했다. 따라서 시세 TR(6번)의 응답 필드는 output별 분리가 불가능하며, **전체 필드 통합 목록**으로 기록한다.
- **hashkey**: `kis_auth.py` 주석에 `현재는 hash key 필수 사항 아님, 생략 가능, API 호출 과정에서 변조 우려 시 사용`이라 명시. 샘플의 POST 호출(`_url_fetch(..., postFlag=True)`)에서도 `set_order_hash_key` 호출은 활성화되어 있지 않다(주석 처리). 즉 **hashkey는 어느 TR에서도 API 필수값이 아니며**, POST 주문 TR에서 변조 방지 목적으로 선택 사용 가능.
- **연속조회**: `inquire_balance`, `inquire_ccnl` 두 TR만 `CTX_AREA_FK200`/`CTX_AREA_NK200` 기반 연속조회를 지원(샘플 코드 내 재귀 페이지네이션 구현 확인). 나머지 5개는 미지원.

---

## 1. 선물옵션 주문

| 항목 | 값 |
|---|---|
| API명 (한글) | 선물옵션 주문 |
| API명 (영문/식별자) | order — `[v1_국내선물-001]` |
| HTTP method + path | `POST /uapi/domestic-futureoption/v1/trading/order` |
| tr_id 실전 (주간) | `TTTO1101U` |
| tr_id 실전 (야간) | `STTN1101U` |
| tr_id 모의 (주간) | `VTTO1101U` |
| tr_id 모의 (야간) | 미지원 (샘플 코드가 `demo`+`night` 조합에서 ValueError 발생) |
| hashkey 필요 여부 | 불필요 (선택 사용 가능 — POST API, 변조 방지용) |
| 모의투자 지원 | 지원 (주간만) |
| 연속조회 지원 | 미지원 |

### 요청 필드

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `env_dv` | str | Y | 실전모의구분 (`real`:실전 / `demo`:모의) — tr_id 선택용, 호출 파라미터 아님 |
| `ord_dv` | str | Y | 주야간구분 (`day`:주간 / `night`:야간) — tr_id 선택용, 호출 파라미터 아님 |
| `ORD_PRCS_DVSN_CD` | str | Y | 주문처리구분코드 (`02`:주문전송) |
| `CANO` | str | Y | 종합계좌번호 (계좌번호 8-2 체계의 앞 8자리) |
| `ACNT_PRDT_CD` | str | Y | 계좌상품코드 (계좌번호 8-2 체계의 뒤 2자리) |
| `SLL_BUY_DVSN_CD` | str | Y | 매도매수구분코드 (`01`:매도 / `02`:매수) |
| `SHTN_PDNO` | str | Y | 단축상품번호 (선물 6자리 예 `101W09`, 옵션 9자리 예 `201S03370`) |
| `ORD_QTY` | str | Y | 주문수량 |
| `UNIT_PRICE` | str | Y | 주문가격1 (시장가·최유리지정가는 `0`) |
| `NMPR_TYPE_CD` | str | Y | 호가유형코드 (`01`:지정가 / `02`:시장가 / `03`:조건부 / `04`:최유리) |
| `KRX_NMPR_CNDT_CD` | str | Y | 한국거래소호가조건코드 (`0`:없음 / `3`:IOC / `4`:FOK) |
| `ORD_DVSN_CD` | str | Y | 주문구분코드 (`01`지정가 `02`시장가 `03`조건부 `04`최유리 `10`지정가IOC `11`지정가FOK `12`시장가IOC `13`시장가FOK `14`최유리IOC `15`최유리FOK) |
| `CTAC_TLNO` | str | N | 연락전화번호 (기본값 `""`) |
| `FUOP_ITEM_DVSN_CD` | str | N | 선물옵션종목구분코드 (기본값 `""` / 공란) |

> POST API이므로 BODY key는 모두 대문자 (`CANO`, `ACNT_PRDT_CD` 등).

### 응답 필드

구조: `output` — 단일 객체 (출처: `chk_order.py` `COLUMN_MAPPING`)

| 필드명 | 의미 |
|---|---|
| `KRX_FWDG_ORD_ORGNO` | 한국거래소전송주문조직번호 |
| `ODNO` | 주문번호 |
| `ORD_TMD` | 주문시각 |

---

## 2. 선물옵션 정정취소주문

| 항목 | 값 |
|---|---|
| API명 (한글) | 선물옵션 정정취소주문 |
| API명 (영문/식별자) | order_rvsecncl — `[v1_국내선물-002]` |
| HTTP method + path | `POST /uapi/domestic-futureoption/v1/trading/order-rvsecncl` |
| tr_id 실전 (주간) | `TTTO1103U` |
| tr_id 실전 (야간) | `TTTN1103U` |
| tr_id 모의 (주간) | `VTTO1103U` |
| tr_id 모의 (야간) | 미지원 (`demo`+`night` 조합 ValueError) |
| hashkey 필요 여부 | 불필요 (선택 사용 가능 — POST API) |
| 모의투자 지원 | 지원 (주간만) |
| 연속조회 지원 | 미지원 |

### 요청 필드

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `env_dv` | str | Y | 실전모의구분 (`real`/`demo`) — tr_id 선택용 |
| `day_dv` | str | Y | 주야간구분 (`day`/`night`) — tr_id 선택용 |
| `ORD_PRCS_DVSN_CD` | str | Y | 주문처리구분코드 (`02`) |
| `CANO` | str | Y | 종합계좌번호 |
| `ACNT_PRDT_CD` | str | Y | 계좌상품코드 |
| `RVSE_CNCL_DVSN_CD` | str | Y | 정정취소구분코드 (`01`:정정 / `02`:취소) |
| `ORGN_ODNO` | str | Y | 원주문번호 |
| `ORD_QTY` | str | Y | 주문수량 (`0`:전량, 그 외 수량) |
| `UNIT_PRICE` | str | Y | 주문가격1 (`0`:시장가/최유리, 그 외 가격) |
| `NMPR_TYPE_CD` | str | Y | 호가유형코드 (`01`지정가 `02`시장가 `03`조건부 `04`최유리) |
| `KRX_NMPR_CNDT_CD` | str | Y | 한국거래소호가조건코드 (`0`:취소/없음 `3`:IOC `4`:FOK) |
| `RMN_QTY_YN` | str | Y | 잔여수량여부 (`Y`:전량 / `N`:일부) |
| `ORD_DVSN_CD` | str | Y | 주문구분코드 |
| `FUOP_ITEM_DVSN_CD` | str | N | 선물옵션종목구분코드 (기본값 `""`) |

> 이미 체결된 건은 정정·취소 불가.

### 응답 필드

구조: `output` — 객체 (출처: `chk_order_rvsecncl.py` `COLUMN_MAPPING`)

| 필드명 | 의미 |
|---|---|
| `ACNT_NAME` | 계좌명 |
| `TRAD_DVSN_NAME` | 매매구분명 |
| `ITEM_NAME` | 종목명 |
| `ORD_TMD` | 주문시각 |
| `ORD_GNO_BRNO` | 주문채번지점번호 |
| `ORGN_ODNO` | 원주문번호 |
| `ODNO` | 주문번호 |

---

## 3. 선물옵션 잔고현황

| 항목 | 값 |
|---|---|
| API명 (한글) | 선물옵션 잔고현황 |
| API명 (영문/식별자) | inquire_balance — `[v1_국내선물-004]` |
| HTTP method + path | `GET /uapi/domestic-futureoption/v1/trading/inquire-balance` |
| tr_id 실전 | `CTFO6118R` |
| tr_id 모의 | `VTFO6118R` |
| hashkey 필요 여부 | 불필요 (GET) |
| 모의투자 지원 | 지원 |
| 연속조회 지원 | **지원** — 1회 호출 최대 20건, 이후 `CTX_AREA_FK200`/`CTX_AREA_NK200`로 연속조회 (`tr_cont` 값 `M`/`F`면 다음 페이지) |

### 요청 필드

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `env_dv` | str | Y | 실전모의구분 (`real`/`demo`) — tr_id 선택용 |
| `CANO` | str | Y | 종합계좌번호 |
| `ACNT_PRDT_CD` | str | Y | 계좌상품코드 (예 `03`) |
| `MGNA_DVSN` | str | Y | 증거금 구분 (`01`:게시 / `02`:유지) |
| `EXCC_STAT_CD` | str | Y | 정산상태코드 (`1`:정산 / `2`:본정산) |
| `CTX_AREA_FK200` | str | N | 연속조회검색조건200 (최초 호출 시 `""`) |
| `CTX_AREA_NK200` | str | N | 연속조회키200 (최초 호출 시 `""`) |

> HTTP 헤더 `tr_cont`: 연속거래여부. 최초 `""`, 연속조회 시 `N`.

### 응답 필드

구조: `output1` — **배열** (잔고 종목 목록) / `output2` — **단일 객체** (계좌 요약). 응답 body에 `ctx_area_fk200`·`ctx_area_nk200` 포함 (다음 연속조회 키).

출처: `chk_inquire_balance.py` `COLUMN_MAPPING` — output1·output2 공용 단일 dict로 정의되어 있어 아래는 **통합 목록**이다. 앞쪽 종목 단위 필드는 `output1`, 뒤쪽 계좌 요약 필드는 `output2`에 해당하나 chk 코드상 명확히 분리되어 있지 않다.

| 필드명 | 의미 |
|---|---|
| `cano` | 종합계좌번호 |
| `acnt_prdt_cd` | 계좌상품코드 |
| `pdno` | 상품번호 |
| `prdt_type_cd` | 상품유형코드 |
| `shtn_pdno` | 단축상품번호 |
| `prdt_name` | 상품명 |
| `sll_buy_dvsn_name` | 매도매수구분명 |
| `cblc_qty` | 잔고수량 |
| `excc_unpr` | 정산단가 |
| `ccld_avg_unpr1` | 체결평균단가1 |
| `idx_clpr` | 지수종가 |
| `pchs_amt` | 매입금액 |
| `evlu_amt` | 평가금액 |
| `evlu_pfls_amt` | 평가손익금액 |
| `trad_pfls_amt` | 매매손익금액 |
| `lqd_psbl_qty` | 청산가능수량 |
| `dnca_cash` | 예수금현금 |
| `frcr_dncl_amt` | 외화예수금액 |
| `dnca_sbst` | 예수금대용 |
| `tot_dncl_amt` | 총예수금액 |
| `tot_ccld_amt` | 총체결금액 |
| `cash_mgna` | 현금증거금 |
| `sbst_mgna` | 대용증거금 |
| `mgna_tota` | 증거금총액 |
| `opt_dfpa` | 옵션차금 |
| `thdt_dfpa` | 당일차금 |
| `rnwl_dfpa` | 갱신차금 |
| `fee` | 수수료 |
| `nxdy_dnca` | 익일예수금 |
| `nxdy_dncl_amt` | 익일예수금액 |
| `prsm_dpast` | 추정예탁자산 |
| `prsm_dpast_amt` | 추정예탁자산금액 |
| `pprt_ord_psbl_cash` | 적정주문가능현금 |
| `add_mgna_cash` | 추가증거금현금 |
| `add_mgna_tota` | 추가증거금총액 |
| `futr_trad_pfls_amt` | 선물매매손익금액 |
| `opt_trad_pfls_amt` | 옵션매매손익금액 |
| `futr_evlu_pfls_amt` | 선물평가손익금액 |
| `opt_evlu_pfls_amt` | 옵션평가손익금액 |
| `trad_pfls_amt_smtl` | 매매손익금액합계 |
| `evlu_pfls_amt_smtl` | 평가손익금액합계 |
| `wdrw_psbl_tot_amt` | 인출가능총금액 |
| `ord_psbl_cash` | 주문가능현금 |
| `ord_psbl_sbst` | 주문가능대용 |
| `ord_psbl_tota` | 주문가능총액 |
| `pchs_amt_smtl` | 매입금액합계 |
| `evlu_amt_smtl` | 평가금액합계 |

---

## 4. 선물옵션 주문체결내역조회

| 항목 | 값 |
|---|---|
| API명 (한글) | 선물옵션 주문체결내역조회 |
| API명 (영문/식별자) | inquire_ccnl — `[v1_국내선물-003]` |
| HTTP method + path | `GET /uapi/domestic-futureoption/v1/trading/inquire-ccnl` |
| tr_id 실전 | `TTTO5201R` |
| tr_id 모의 | `VTTO5201R` |
| hashkey 필요 여부 | 불필요 (GET) |
| 모의투자 지원 | 지원 |
| 연속조회 지원 | **지원** — 1회 호출 최대 100건, 이후 `CTX_AREA_FK200`/`CTX_AREA_NK200`로 연속조회 (`tr_cont` `M`/`F`면 다음 페이지) |

### 요청 필드

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `env_dv` | str | Y | 실전모의구분 (`real`/`demo`) — tr_id 선택용 |
| `CANO` | str | Y | 종합계좌번호 |
| `ACNT_PRDT_CD` | str | Y | 계좌상품코드 (예 `03`) |
| `STRT_ORD_DT` | str | Y | 시작주문일자 (YYYYMMDD) |
| `END_ORD_DT` | str | Y | 종료주문일자 (YYYYMMDD) |
| `SLL_BUY_DVSN_CD` | str | Y | 매도매수구분코드 (`00`:전체 / `01`:매도 / `02`:매수) |
| `CCLD_NCCS_DVSN` | str | Y | 체결미체결구분 (`00`:전체 / `01`:체결 / `02`:미체결) |
| `SORT_SQN` | str | Y | 정렬순서 (`AS`:정순 / `DS`:역순) |
| `PDNO` | str | N | 상품번호 (기본값 `""`) |
| `STRT_ODNO` | str | N | 시작주문번호 (기본값 `""`) |
| `MKET_ID_CD` | str | N | 시장ID코드 (기본값 `""`) |
| `CTX_AREA_FK200` | str | N | 연속조회검색조건200 |
| `CTX_AREA_NK200` | str | N | 연속조회키200 |

> 샘플 함수 시그니처상 `PDNO`/`STRT_ODNO`/`MKET_ID_CD`는 `required:true`로 표기되나 기본값 `""`가 있어 실질 선택 입력이다. 위 표는 기본값 존재 기준으로 `N` 처리.

### 응답 필드

구조: `output1` — **배열** (주문체결내역 목록) / `output2` — **단일 객체** (요약). body에 `ctx_area_fk200`·`ctx_area_nk200` 포함.

출처: `chk_inquire_ccnl.py` `COLUMN_MAPPING` — output1·output2 공용 단일 dict로 정의되어 있어 아래는 **통합 목록**이다. 앞쪽 주문/체결 단위 필드는 `output1`, 뒤쪽 합계 필드(`tot_*_smtl`, `fee_smtl`)는 `output2`에 해당하나 chk 코드상 명확히 분리되어 있지 않다.

| 필드명 | 의미 |
|---|---|
| `ord_gno_brno` | 주문채번지점번호 |
| `cano` | 종합계좌번호 |
| `csac_name` | 종합계좌명 |
| `acnt_prdt_cd` | 계좌상품코드 |
| `ord_dt` | 주문일자 |
| `odno` | 주문번호 |
| `orgn_odno` | 원주문번호 |
| `sll_buy_dvsn_cd` | 매도매수구분코드 |
| `trad_dvsn_name` | 매매구분명 |
| `nmpr_type_cd` | 호가유형코드 |
| `nmpr_type_name` | 호가유형명 |
| `pdno` | 상품번호 |
| `prdt_name` | 상품명 |
| `prdt_type_cd` | 상품유형코드 |
| `ord_qty` | 주문수량 |
| `ord_idx` | 주문지수 |
| `qty` | 잔량 |
| `ord_tmd` | 주문시각 |
| `tot_ccld_qty` | 총체결수량 |
| `avg_idx` | 평균지수 |
| `tot_ccld_amt` | 총체결금액 |
| `rjct_qty` | 거부수량 |
| `ingr_trad_rjct_rson_cd` | 장내매매거부사유코드 |
| `ingr_trad_rjct_rson_name` | 장내매매거부사유명 |
| `ord_stfno` | 주문직원번호 |
| `sprd_item_yn` | 스프레드종목여부 |
| `ord_ip_addr` | 주문IP주소 |
| `tot_ord_qty` | 총주문수량 |
| `tot_ccld_amt_smtl` | 총체결금액합계 |
| `tot_ccld_qty_smtl` | 총체결수량합계 |
| `fee_smtl` | 수수료합계 |
| `ctac_tlno` | 연락전화번호 |

---

## 5. 선물옵션 매수가능조회 (주문가능)

| 항목 | 값 |
|---|---|
| API명 (한글) | 선물옵션 주문가능 (매수가능조회) |
| API명 (영문/식별자) | inquire_psbl_order — `[v1_국내선물-005]` |
| HTTP method + path | `GET /uapi/domestic-futureoption/v1/trading/inquire-psbl-order` |
| tr_id 실전 | `TTTO5105R` |
| tr_id 모의 | `VTTO5105R` |
| hashkey 필요 여부 | 불필요 (GET) |
| 모의투자 지원 | 지원 |
| 연속조회 지원 | 미지원 |

### 요청 필드

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `env_dv` | str | Y | 실전모의구분 (`real`/`demo`) — tr_id 선택용 |
| `CANO` | str | Y | 종합계좌번호 |
| `ACNT_PRDT_CD` | str | Y | 계좌상품코드 (예 `03`) |
| `PDNO` | str | Y | 상품번호 (선물 6자리, 옵션 9자리) |
| `SLL_BUY_DVSN_CD` | str | Y | 매도매수구분코드 (`01`:매도 / `02`:매수) |
| `UNIT_PRICE` | str | Y | 주문가격1 |
| `ORD_DVSN_CD` | str | Y | 주문구분코드 |

### 응답 필드

구조: `output` — 단일 객체 (주문가능 내역·수량). 출처: `chk_inquire_psbl_order.py` `COLUMN_MAPPING`

| 필드명 | 의미 |
|---|---|
| `tot_psbl_qty` | 총가능수량 |
| `lqd_psbl_qty1` | 청산가능수량1 |
| `ord_psbl_qty` | 주문가능수량 |
| `bass_idx` | 기준지수 |

---

## 6. 선물옵션 현재가 시세

| 항목 | 값 |
|---|---|
| API명 (한글) | 선물옵션 시세 |
| API명 (영문/식별자) | inquire_price — `[v1_국내선물-006]` |
| HTTP method + path | `GET /uapi/domestic-futureoption/v1/quotations/inquire-price` |
| tr_id 실전 | `FHMIF10000000` |
| tr_id 모의 | `FHMIF10000000` (실전과 동일 — 시세 TR은 실전/모의 tr_id 구분 없음) |
| hashkey 필요 여부 | 불필요 (GET) |
| 모의투자 지원 | 지원 (단일 tr_id 공용) |
| 연속조회 지원 | 미지원 |

### 요청 필드

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `FID_COND_MRKT_DIV_CODE` | str | Y | FID 조건 시장 분류 코드 (`F`:지수선물 / `O`:지수옵션) |
| `FID_INPUT_ISCD` | str | Y | FID 입력 종목코드 (예 `101W09`) |
| `env_dv` | str | Y | 실전모의구분 (`real`/`demo`) — 본 TR은 tr_id가 동일하나 샘플상 필수 입력 |

### 응답 필드

구조: `output1` / `output2` / `output3` — 각각 단일 객체 (샘플은 셋 다 `pd.DataFrame(..., index=[0])`로 단일 행 변환).

출처: `chk_inquire_price.py` `COLUMN_MAPPING` — **output1/2/3를 하나의 dict로 병합** 정의했으므로 아래는 3개 output 전체 통합 목록이다 (output별 귀속은 chk 코드상 분리 불가). 통상 `output1`은 선물 시세·그릭스, `output3`은 업종 지수 관련(`bstp_*`) 필드를 포함한다.

| 필드명 | 의미 |
|---|---|
| `hts_kor_isnm` | HTS 한글 종목명 |
| `futs_prpr` | 선물 현재가 |
| `futs_prdy_vrss` | 선물 전일 대비 |
| `prdy_vrss_sign` | 전일 대비 부호 |
| `futs_prdy_clpr` | 선물 전일 종가 |
| `futs_prdy_ctrt` | 선물 전일 대비율 |
| `acml_vol` | 누적 거래량 |
| `acml_tr_pbmn` | 누적 거래 대금 |
| `hts_otst_stpl_qty` | HTS 미결제 약정 수량 |
| `otst_stpl_qty_icdc` | 미결제 약정 수량 증감 |
| `futs_oprc` | 선물 시가2 |
| `futs_hgpr` | 선물 최고가 |
| `futs_lwpr` | 선물 최저가 |
| `futs_mxpr` | 선물 상한가 |
| `futs_llam` | 선물 하한가 |
| `basis` | 베이시스 |
| `futs_sdpr` | 선물 기준가 |
| `hts_thpr` | HTS 이론가 |
| `dprt` | 괴리율 |
| `crbr_aply_mxpr` | 서킷브레이커 적용 상한가 |
| `crbr_aply_llam` | 서킷브레이커 적용 하한가 |
| `futs_last_tr_date` | 선물 최종 거래 일자 |
| `hts_rmnn_dynu` | HTS 잔존 일수 |
| `futs_lstn_medm_hgpr` | 선물 상장 중 최고가 |
| `futs_lstn_medm_lwpr` | 선물 상장 중 최저가 |
| `delta_val` | 델타 값 |
| `gama` | 감마 |
| `theta` | 세타 |
| `vega` | 베가 |
| `rho` | 로우 |
| `hist_vltl` | 역사적 변동성 |
| `hts_ints_vltl` | HTS 내재 변동성 |
| `mrkt_basis` | 시장 베이시스 |
| `acpr` | 행사가 |
| `bstp_cls_code` | 업종 구분 코드 |
| `bstp_nmix_prpr` | 업종 지수 현재가 |
| `bstp_nmix_prdy_vrss` | 업종 지수 전일 대비 |
| `bstp_nmix_prdy_ctrt` | 업종 지수 전일 대비율 |

---

## 7. 선물옵션 시세호가

| 항목 | 값 |
|---|---|
| API명 (한글) | 선물옵션 시세호가 |
| API명 (영문/식별자) | inquire_asking_price — `[v1_국내선물-007]` |
| HTTP method + path | `GET /uapi/domestic-futureoption/v1/quotations/inquire-asking-price` |
| tr_id 실전 | `FHMIF10010000` |
| tr_id 모의 | `FHMIF10010000` (실전과 동일) |
| hashkey 필요 여부 | 불필요 (GET) |
| 모의투자 지원 | 지원 (단일 tr_id 공용) |
| 연속조회 지원 | 미지원 |

### 요청 필드

| 이름 | 타입 | 필수 | 설명 |
|---|---|---|---|
| `FID_COND_MRKT_DIV_CODE` | str | Y | FID 조건 시장 분류 코드 (`F`:지수선물 / `JF`:주식선물) |
| `FID_INPUT_ISCD` | str | Y | FID 입력 종목코드 (예 `101W09`) |
| `env_dv` | str | Y | 실전모의구분 (`real`/`demo`) — tr_id는 동일하나 샘플상 필수 입력 |

### 응답 필드

구조: `output1` / `output2` — 각각 단일 객체.

출처: `chk_inquire_asking_price.py` `COLUMN_MAPPING` — output1·output2 공용 단일 dict로 정의되어 있어 아래는 **통합 목록**이다. 통상 종목 시세 요약 필드는 `output1`, 호가 5단계(`futs_askp*`/`futs_bidp*`/`*_rsqn*`/`*_csnu*`)는 `output2`에 해당하나 chk 코드상 명확히 분리되어 있지 않다.

| 필드명 | 의미 |
|---|---|
| `hts_kor_isnm` | HTS 한글 종목명 |
| `futs_prpr` | 선물 현재가 |
| `prdy_vrss_sign` | 전일 대비 부호 |
| `futs_prdy_vrss` | 선물 전일 대비 |
| `futs_prdy_ctrt` | 선물 전일 대비율 |
| `acml_vol` | 누적 거래량 |
| `futs_prdy_clpr` | 선물 전일 종가 |
| `futs_shrn_iscd` | 선물 단축 종목코드 |
| `futs_askp1`~`futs_askp5` | 선물 매도호가1~5 |
| `futs_bidp1`~`futs_bidp5` | 선물 매수호가1~5 |
| `askp_rsqn1`~`askp_rsqn5` | 매도호가 잔량1~5 |
| `bidp_rsqn1`~`bidp_rsqn5` | 매수호가 잔량1~5 |
| `askp_csnu1`~`askp_csnu5` | 매도호가 건수1~5 |
| `bidp_csnu1`~`bidp_csnu5` | 매수호가 건수1~5 |
| `total_askp_rsqn` | 총 매도호가 잔량 |
| `total_bidp_rsqn` | 총 매수호가 잔량 |
| `total_askp_csnu` | 총 매도호가 건수 |
| `total_bidp_csnu` | 총 매수호가 건수 |
| `aspr_acpt_hour` | 호가 접수 시간 |

---

## 부록 — 수집 상태 요약

| TR | 헤더(path·tr_id·옵션) | 요청필드 | 응답구조 | 응답필드명 |
|---|---|---|---|---|
| 1 선물옵션 주문 | 완전 | 완전 | 완전 | 완전 (3필드) |
| 2 정정취소주문 | 완전 | 완전 | 완전 | 완전 (7필드) |
| 3 잔고현황 | 완전 | 완전 | 완전 | 완전 (46필드, output1/2 통합) |
| 4 주문체결내역조회 | 완전 | 완전 | 완전 | 완전 (32필드, output1/2 통합) |
| 5 주문가능 | 완전 | 완전 | 완전 | 완전 (4필드) |
| 6 현재가 시세 | 완전 | 완전 | 완전 | 완전 (38필드, output1/2/3 통합) |
| 7 시세호가 | 완전 | 완전 | 완전 | 완전 (호가 5단계 포함) |

응답 필드명·의미는 7개 TR 전부 각 예제 디렉터리의 `chk_<name>.py` 내 `COLUMN_MAPPING` dict에서 확보했다. 단 `COLUMN_MAPPING`이 다중 output을 단일 dict로 병합 정의한 TR(3·4·6·7)은 output별 필드 귀속이 코드상 분리되어 있지 않아 통합 목록으로 기록했다 — output별 정확한 분리가 필요하면 KIS 개발자포털 각 API 페이지의 응답 스펙 테이블을 별도 확인해야 한다.
