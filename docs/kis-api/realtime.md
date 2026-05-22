# 한국투자증권(KIS) OpenAPI — 실시간 WebSocket 레퍼런스

> **수집일**: 2026-05-22
> **출처 (우선순위순)**:
> 1. GitHub `koreainvestment/open-trading-api` (raw 파일, branch `main`)
>    - `legacy/websocket/python/ws_domestic_stock.py`
>    - `legacy/websocket/python/ws_overseas_stock.py`
>    - `legacy/websocket/python/ws_domestic_overseas_all.py`
>    - `examples_llm/auth/auth_ws_token/auth_ws_token.py`
>    - `examples_user/domestic_stock/domestic_stock_functions_ws.py`
> 2. KIS 개발자포털 — `https://apiportal.koreainvestment.com/apiservice-apiservice?/oauth2/Approval=` (실시간-000 웹소켓 접속키 발급)
>
> 추측한 항목이 아닌, 위 공식 소스에서 직접 확인한 내용만 기재. 미확인 항목은 `[미확인]`으로 표기.

---

## A. 연결 / 인증

### A-1. WebSocket Endpoint URL

| 환경 | URL | 포트 |
|------|-----|------|
| 실전투자 | `ws://ops.koreainvestment.com:21000` | 21000 |
| 모의투자 | `ws://ops.koreainvestment.com:31000` | 31000 |

- 출처: `legacy/websocket/python/ws_domestic_stock.py` L148-149 (`url = 'ws://ops.koreainvestment.com:21000'` 실전, 주석 `ws://ops.koreainvestment.com:31000` 모의), 개발자포털.
- 프로토콜은 평문 `ws://` (TLS `wss://` 아님). [High]
- `websockets.connect(url, ping_interval=None)` — 클라이언트 측 자동 ping 비활성화 후 서버 PINGPONG 프레임을 직접 처리. (legacy 샘플 L154)

### A-2. approval_key 발급 — `POST /oauth2/Approval`

REST 호출. 발급 도메인은 WebSocket 도메인과 다름.

| 환경 | Base URL |
|------|----------|
| 실전투자 | `https://openapi.koreainvestment.com:9443` |
| 모의투자 | `https://openapivts.koreainvestment.com:29443` |

- Path: `/oauth2/Approval`
- 출처: `ws_domestic_stock.py` L36-43, `examples_llm/auth/auth_ws_token/auth_ws_token.py` L26.

**요청 헤더**

| 헤더 | 값 |
|------|-----|
| `content-type` / `Content-Type` | `application/json` |
| `Accept` | `text/plain` (auth_ws_token.py 기준) |
| `charset` | `UTF-8` (auth_ws_token.py 기준) |

**요청 Body (JSON)**

| 순번 | 필드명 | 값 / 의미 |
|------|--------|-----------|
| 1 | `grant_type` | 고정 `client_credentials` |
| 2 | `appkey` | 한국투자증권 발급 App Key |
| 3 | `secretkey` | 한국투자증권 발급 App Secret |

- 주의: 필드명이 `secretkey`임 (REST 토큰 발급의 `appsecret`과 다름). 출처: `ws_domestic_stock.py` L39-41, `auth_ws_token.py` L96-100.
- `auth_ws_token.py`에는 선택 필드 `token`(접근토큰)이 존재하나 통상 생략.

**응답 Body (JSON)**

| 순번 | 필드명 | 의미 |
|------|--------|------|
| 1 | `approval_key` | WebSocket 접속키. 구독 프레임 header에 사용. |

- 출처: `ws_domestic_stock.py` L46 (`res.json()["approval_key"]`).
- 기타 응답 필드(만료 등): `[미확인]` — 공식 샘플 코드는 `approval_key`만 추출.

### A-3. 구독 / 해지 프레임 (Client → Server)

`websocket.send()`로 전송하는 JSON 문자열. 일반 시세 TR과 체결통보 TR은 `tr_key` 값만 다름.

```json
{
  "header": {
    "approval_key": "<approval_key>",
    "custtype": "P",
    "tr_type": "1",
    "content-type": "utf-8"
  },
  "body": {
    "input": {
      "tr_id": "H0STCNT0",
      "tr_key": "005930"
    }
  }
}
```

| 위치 | 필드 | 값 / 의미 |
|------|------|-----------|
| `header.approval_key` | 접속키 | A-2에서 발급 |
| `header.custtype` | 고객 타입 | `P` 개인 / `B` 법인 |
| `header.tr_type` | 거래 타입 | `1` 등록(구독) / `2` 등록해제(해지) |
| `header.content-type` | 고정 | `utf-8` |
| `body.input.tr_id` | 실시간 TR ID | 예: `H0STCNT0`, `H0STASP0`, `H0STCNI0`, `HDFSCNT0` |
| `body.input.tr_key` | 구독 키 | 시세 TR: 종목코드(예: `005930`). 체결통보 TR: **HTS ID** |

- 출처: `ws_domestic_stock.py` L198-201. 구독/해지는 동일 구조에서 `tr_type`만 `1`↔`2`로 토글.
- 체결통보(`H0STCNI0/9`)는 `tr_key`에 종목코드가 아닌 HTS ID를 넣음 (L199).

### A-4. PINGPONG 프레임

- 서버는 주기적으로 텍스트 프레임을 보내며, 첫 글자가 `0`/`1`이 아니면 JSON으로 파싱 → `header.tr_id == "PINGPONG"`이면 핑퐁 프레임.
- 처리: 받은 데이터를 그대로 `websocket.pong(data)`로 되돌려 보냄. (`ws_domestic_stock.py` L256-259)
- `ping_interval=None`으로 클라이언트 자동 핑은 끄고, 서버 PINGPONG에만 응답하는 방식.

### A-5. 수신 프레임 판별 규칙

`websocket.recv()`로 받은 문자열 `data`의 첫 글자로 분기:

| `data[0]` | 의미 | 파싱 |
|-----------|------|------|
| `0` | 암호화되지 않은 실시간 시세 데이터 | `|`로 split |
| `1` | 암호화된 실시간 데이터 (체결통보) | `|`로 split 후 AES 복호화 |
| 그 외 | JSON 제어 메시지 (구독 응답, PINGPONG, 에러) | `json.loads` |

**실시간 데이터 프레임의 `|` 구분 구조** (출처 `ws_domestic_stock.py` L218-228):

| 순번 | 위치 | 의미 |
|------|------|------|
| 1 | `[0]` | 암호화 유무 (`0` 평문 / `1` 암호화) |
| 2 | `[1]` | TR ID (예: `H0STCNT0`) |
| 3 | `[2]` | 데이터 건수 (`data_cnt`) — 한 프레임에 다건 포함 가능 |
| 4 | `[3]` | 실데이터 본문 — `^`(캐럿)로 필드 구분, 다건이면 필드가 연속 반복 |

> **핵심**: 프레임 레벨 구분자는 `|`(파이프), 실데이터 필드 구분자는 `^`(캐럿). 아래 B/C 섹션의 "필드 순서"는 모두 `^`로 split된 본문 기준.

**JSON 제어 메시지 구조** (구독 응답):

- `header.tr_id`, `header.tr_key`
- `body.rt_cd` — `0` 정상 / `1` 에러
- `body.msg1` — 메시지
- `body.output.key`, `body.output.iv` — 체결통보 TR 구독 시에만 존재 (C 섹션 참조)

---

## B. 실시간 TR 4종 — 응답 필드 순서

> 모든 필드 순서는 실데이터 본문을 `^`로 split한 인덱스 기준. 한 프레임에 다건(`data_cnt`)이 올 수 있으며, 그 경우 아래 필드 묶음이 연속 반복됨.

### B-1. `H0STCNT0` — 국내주식 실시간체결가

출처: `ws_domestic_stock.py` L103 (`menulist`). 총 46개 필드.

| 순번 | 필드명 | 의미 |
|------|--------|------|
| 1 | MKSC_SHRN_ISCD | 유가증권 단축 종목코드 |
| 2 | STCK_CNTG_HOUR | 주식 체결 시간 |
| 3 | STCK_PRPR | 주식 현재가 |
| 4 | PRDY_VRSS_SIGN | 전일 대비 부호 |
| 5 | PRDY_VRSS | 전일 대비 |
| 6 | PRDY_CTRT | 전일 대비율 |
| 7 | WGHN_AVRG_STCK_PRC | 가중 평균 주식 가격 |
| 8 | STCK_OPRC | 주식 시가 |
| 9 | STCK_HGPR | 주식 최고가 |
| 10 | STCK_LWPR | 주식 최저가 |
| 11 | ASKP1 | 매도호가1 |
| 12 | BIDP1 | 매수호가1 |
| 13 | CNTG_VOL | 체결 거래량 |
| 14 | ACML_VOL | 누적 거래량 |
| 15 | ACML_TR_PBMN | 누적 거래대금 |
| 16 | SELN_CNTG_CSNU | 매도 체결 건수 |
| 17 | SHNU_CNTG_CSNU | 매수 체결 건수 |
| 18 | NTBY_CNTG_CSNU | 순매수 체결 건수 |
| 19 | CTTR | 체결강도 |
| 20 | SELN_CNTG_SMTN | 총 매도 수량 |
| 21 | SHNU_CNTG_SMTN | 총 매수 수량 |
| 22 | CCLD_DVSN | 체결 구분 |
| 23 | SHNU_RATE | 매수비율 |
| 24 | PRDY_VOL_VRSS_ACML_VOL_RATE | 전일 거래량 대비 등락율 |
| 25 | OPRC_HOUR | 시가 시간 |
| 26 | OPRC_VRSS_PRPR_SIGN | 시가 대비 구분 |
| 27 | OPRC_VRSS_PRPR | 시가 대비 |
| 28 | HGPR_HOUR | 최고가 시간 |
| 29 | HGPR_VRSS_PRPR_SIGN | 고가 대비 구분 |
| 30 | HGPR_VRSS_PRPR | 고가 대비 |
| 31 | LWPR_HOUR | 최저가 시간 |
| 32 | LWPR_VRSS_PRPR_SIGN | 저가 대비 구분 |
| 33 | LWPR_VRSS_PRPR | 저가 대비 |
| 34 | BSOP_DATE | 영업 일자 |
| 35 | NEW_MKOP_CLS_CODE | 신 장운영 구분 코드 |
| 36 | TRHT_YN | 거래정지 여부 |
| 37 | ASKP_RSQN1 | 매도호가 잔량1 |
| 38 | BIDP_RSQN1 | 매수호가 잔량1 |
| 39 | TOTAL_ASKP_RSQN | 총 매도호가 잔량 |
| 40 | TOTAL_BIDP_RSQN | 총 매수호가 잔량 |
| 41 | VOL_TNRT | 거래량 회전율 |
| 42 | PRDY_SMNS_HOUR_ACML_VOL | 전일 동시간 누적 거래량 |
| 43 | PRDY_SMNS_HOUR_ACML_VOL_RATE | 전일 동시간 누적 거래량 비율 |
| 44 | HOUR_CLS_CODE | 시간 구분 코드 |
| 45 | MRKT_TRTM_CLS_CODE | 임의종료 구분 코드 |
| 46 | VI_STND_PRC | 정적 VI 발동기준가 |

> 영문 필드명은 KIS 표준 약어를 적용한 것이며, 공식 샘플의 한글 `menulist`(`유가증권단축종목코드|주식체결시간|...`)와 1:1 대응. 한글 라벨이 원본 진실 출처.

### B-2. `H0STASP0` — 국내주식 실시간호가

출처: `ws_domestic_stock.py` `stockhoka()` 함수 — `recvvalue` 인덱스 0~58 사용. 총 59개 필드.

| 순번 | idx | 필드 의미 |
|------|-----|-----------|
| 1 | 0 | 유가증권 단축 종목코드 |
| 2 | 1 | 영업시간 |
| 3 | 2 | 시간구분코드 |
| 4 | 3 | 매도호가01 |
| 5 | 4 | 매도호가02 |
| 6 | 5 | 매도호가03 |
| 7 | 6 | 매도호가04 |
| 8 | 7 | 매도호가05 |
| 9 | 8 | 매도호가06 |
| 10 | 9 | 매도호가07 |
| 11 | 10 | 매도호가08 |
| 12 | 11 | 매도호가09 |
| 13 | 12 | 매도호가10 |
| 14 | 13 | 매수호가01 |
| 15 | 14 | 매수호가02 |
| 16 | 15 | 매수호가03 |
| 17 | 16 | 매수호가04 |
| 18 | 17 | 매수호가05 |
| 19 | 18 | 매수호가06 |
| 20 | 19 | 매수호가07 |
| 21 | 20 | 매수호가08 |
| 22 | 21 | 매수호가09 |
| 23 | 22 | 매수호가10 |
| 24 | 23 | 매도호가 잔량01 |
| 25 | 24 | 매도호가 잔량02 |
| 26 | 25 | 매도호가 잔량03 |
| 27 | 26 | 매도호가 잔량04 |
| 28 | 27 | 매도호가 잔량05 |
| 29 | 28 | 매도호가 잔량06 |
| 30 | 29 | 매도호가 잔량07 |
| 31 | 30 | 매도호가 잔량08 |
| 32 | 31 | 매도호가 잔량09 |
| 33 | 32 | 매도호가 잔량10 |
| 34 | 33 | 매수호가 잔량01 |
| 35 | 34 | 매수호가 잔량02 |
| 36 | 35 | 매수호가 잔량03 |
| 37 | 36 | 매수호가 잔량04 |
| 38 | 37 | 매수호가 잔량05 |
| 39 | 38 | 매수호가 잔량06 |
| 40 | 39 | 매수호가 잔량07 |
| 41 | 40 | 매수호가 잔량08 |
| 42 | 41 | 매수호가 잔량09 |
| 43 | 42 | 매수호가 잔량10 |
| 44 | 43 | 총 매도호가 잔량 |
| 45 | 44 | 총 매수호가 잔량 |
| 46 | 45 | 시간외 총 매도호가 잔량 |
| 47 | 46 | 시간외 총 매수호가 잔량 |
| 48 | 47 | 예상 체결가 |
| 49 | 48 | 예상 체결량 |
| 50 | 49 | 예상 거래량 |
| 51 | 50 | 예상체결 대비 |
| 52 | 51 | 부호 |
| 53 | 52 | 예상체결 전일대비율 |
| 54 | 53 | 누적 거래량 |
| 55 | 54 | 총 매도호가 잔량 증감 |
| 56 | 55 | 총 매수호가 잔량 증감 |
| 57 | 56 | 시간외 총 매도호가 잔량 (증감) |
| 58 | 57 | 시간외 총 매수호가 잔량 (증감) |
| 59 | 58 | 주식매매 구분코드 |

> 주의: 샘플 코드의 `print` 라벨이 idx 56/57을 "시간외 총매도/매수호가 잔량"으로 또 표기하나(중복 라벨), 인덱스는 명확. idx 45/46은 시간외 총잔량, idx 56/57은 그 증감으로 해석됨. 정밀 정의는 개발자포털 호가 문서 교차확인 권장 — 샘플 라벨 자체가 모호 `[Medium]`.

### B-3. `H0STCNI0` / `H0STCNI9` — 실시간 체결통보 (실전 / 모의)

- `H0STCNI0`: 실전투자(고객) 체결통보
- `H0STCNI9`: 모의투자 체결통보
- 두 TR 모두 동일 구조. 수신 데이터는 **AES256 암호화** 되어 있어 복호화 필요(C 섹션).
- 복호화 후 본문은 `^`로 split. `pValue[13]`(체결여부) 값에 따라 필드 레이아웃 분기.

**(a) `pValue[13] == '2'` → 체결 통보** (출처 `ws_domestic_stock.py` L123, 26필드)

| 순번 | idx | 필드 의미 |
|------|-----|-----------|
| 1 | 0 | 고객 ID |
| 2 | 1 | 계좌번호 |
| 3 | 2 | 주문번호 |
| 4 | 3 | 원주문번호 |
| 5 | 4 | 매도매수구분 |
| 6 | 5 | 정정구분 |
| 7 | 6 | 주문종류 |
| 8 | 7 | 주문조건 |
| 9 | 8 | 주식 단축 종목코드 |
| 10 | 9 | 체결수량 |
| 11 | 10 | 체결단가 |
| 12 | 11 | 주식 체결시간 |
| 13 | 12 | 거부여부 |
| 14 | 13 | 체결여부 (`1` 주문/`2` 체결) |
| 15 | 14 | 접수여부 |
| 16 | 15 | 지점번호 |
| 17 | 16 | 주문수량 |
| 18 | 17 | 계좌명 |
| 19 | 18 | 호가조건가격 |
| 20 | 19 | 주문거래소 구분 |
| 21 | 20 | 실시간체결창 표시여부 |
| 22 | 21 | 필러 |
| 23 | 22 | 신용구분 |
| 24 | 23 | 신용대출일자 |
| 25 | 24 | 체결종목명40 |
| 26 | 25 | 주문가격 |

**(b) `pValue[13] != '2'` → 주문·정정·취소·거부 접수 통보** (출처 L127, 26필드)

idx 0~24는 (a)와 동일 의미이나 일부 명칭 차이, idx 25만 다름:

| 순번 | idx | 필드 의미 |
|------|-----|-----------|
| 10 | 9 | 주문수량 (체결수량 자리) |
| 11 | 10 | 주문가격 (체결단가 자리) |
| 26 | 25 | 체결단가 |

> 즉 idx 9·10·25 세 필드가 통보 유형에 따라 의미가 바뀜. 나머지 23개 필드는 동일.

### B-4. `HDFSCNT0` — 해외주식 실시간체결가

출처: `ws_overseas_stock.py` `stockspurchase_overseas()` L113 `menulist`. 총 26개 필드. (참고: 해외주식 호가 TR은 `HDFSASP0`)

| 순번 | 필드명 | 의미 |
|------|--------|------|
| 1 | RSYM | 실시간 종목코드 |
| 2 | SYMB | 종목코드 |
| 3 | ZDIV | 소수점 자리수 |
| 4 | TYMD | 현지영업일자 |
| 5 | XYMD | 현지일자 |
| 6 | XHMS | 현지시간 |
| 7 | KYMD | 한국일자 |
| 8 | KHMS | 한국시간 |
| 9 | OPEN | 시가 |
| 10 | HIGH | 고가 |
| 11 | LOW | 저가 |
| 12 | LAST | 현재가 |
| 13 | SIGN | 대비구분 |
| 14 | DIFF | 전일대비 |
| 15 | RATE | 등락율 |
| 16 | PBID | 매수호가 |
| 17 | PASK | 매도호가 |
| 18 | VBID | 매수잔량 |
| 19 | VASK | 매도잔량 |
| 20 | EVOL | 체결량 |
| 21 | TVOL | 거래량 |
| 22 | TAMT | 거래대금 |
| 23 | BIVL | 매도체결량 |
| 24 | ASVL | 매수체결량 |
| 25 | STRN | 체결강도 |
| 26 | MTYP | 시장구분 |

> 한글 `menulist` 원본: `실시간종목코드|종목코드|수수점자리수|현지영업일자|현지일자|현지시간|한국일자|한국시간|시가|고가|저가|현재가|대비구분|전일대비|등락율|매수호가|매도호가|매수잔량|매도잔량|체결량|거래량|거래대금|매도체결량|매수체결량|체결강도|시장구분` ("수수점"은 샘플 원문 오타, "소수점"). 영문 약어는 KIS 해외시세 표준 매핑.

---

## C. 체결통보 AES256 복호화

체결통보 TR(`H0STCNI0`/`H0STCNI9`, 해외 `H0GSCNI0` 등)의 실시간 데이터는 암호화되어 전송됨. 일반 시세 TR(`H0STCNT0`/`H0STASP0`/`HDFSCNT0`)은 평문이라 복호화 불필요.

### C-1. AES key / iv 수신 경로

- 체결통보 TR을 **구독(`tr_type=1`)** 하면 서버가 JSON 제어 메시지로 구독 응답을 보냄.
- 그 응답의 `body.output` 객체 안에 복호화용 키가 들어 있음:
  - `body.output.key` → AES256 secret key
  - `body.output.iv` → AES256 Initialize Vector
- 출처: `ws_domestic_stock.py` L251-254:
  ```python
  if trid == "H0STCNI0" or trid == "H0STCNI9":
      aes_key = jsonObject["body"]["output"]["key"]
      aes_iv  = jsonObject["body"]["output"]["iv"]
  ```
- 이후 도착하는 암호화 실시간 프레임(`data[0] == '1'`)을 이 key/iv로 복호화.

### C-2. 복호화 알고리즘

출처: `ws_domestic_stock.py` L22-30 `aes_cbc_base64_dec()`.

```python
from Crypto.Cipher import AES
from Crypto.Util.Padding import unpad
from base64 import b64decode

def aes_cbc_base64_dec(key, iv, cipher_text):
    cipher = AES.new(key.encode('utf-8'), AES.MODE_CBC, iv.encode('utf-8'))
    return bytes.decode(unpad(cipher.decrypt(b64decode(cipher_text)), AES.block_size))
```

| 항목 | 값 |
|------|-----|
| 알고리즘 | AES-256 |
| 운용 모드 | CBC (`AES.MODE_CBC`) |
| 키 길이 | 32바이트 (`key_bytes = 32`); `key`/`iv` 문자열을 UTF-8 인코딩하여 사용 |
| 입력 인코딩 | 암호문은 Base64 인코딩 → `b64decode` 후 복호화 |
| 패딩 | PKCS#7 (블록 패딩). `unpad(..., AES.block_size)` — `AES.block_size`=16바이트 |
| 출력 | 복호화 바이트열을 UTF-8 디코딩하여 문자열 |

처리 순서: `Base64 디코드 → AES-256-CBC 복호화 → PKCS#7 언패딩 → UTF-8 디코드`.

### C-3. 복호화 후 필드 순서

- 복호화된 문자열을 `^`(캐럿)로 split → B-3의 필드 배열과 동일.
- `H0STCNI0`/`H0STCNI9`: B-3 표 참조 (idx 13 `체결여부`로 체결/접수 레이아웃 분기).
- 해외 체결통보(`H0GSCNI0`)는 `ws_overseas_stock.py` `stocksigningnotice_overseas()` 기준 별도 레이아웃 (idx 12 `체결여부`로 분기, 체결통보 25필드: `고객ID|계좌번호|주문번호|원주문번호|매도매수구분|정정구분|주문종류2|단축종목코드|체결수량|체결단가|체결시간|거부여부|체결여부|접수여부|지점번호|주문수량|계좌명|체결종목명|해외종목구분|담보유형코드|담보대출일자|분할매수매도시작시간|분할매수매도종료시간|시간분할타입유형|체결단가12`). 본 문서 B 섹션 대상은 아니나 참고용 기재.

---

## 부록 — TR ID 요약

| TR ID | 설명 | 데이터 | tr_key |
|-------|------|--------|--------|
| `H0STASP0` | 국내주식 실시간호가 | 평문 | 종목코드 |
| `H0STCNT0` | 국내주식 실시간체결가 | 평문 | 종목코드 |
| `H0STCNI0` | 국내주식 실시간체결통보 (실전) | AES256 암호화 | HTS ID |
| `H0STCNI9` | 국내주식 실시간체결통보 (모의) | AES256 암호화 | HTS ID |
| `HDFSASP0` | 해외주식 실시간호가 | 평문 | 실시간종목코드 |
| `HDFSCNT0` | 해외주식 실시간체결가 | 평문 | 실시간종목코드 |
| `H0GSCNI0` | 해외주식 실시간체결통보 | AES256 암호화 | HTS ID |
