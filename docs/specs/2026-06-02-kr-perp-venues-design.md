# KR-stock perp 어댑터 설계 (Binance · Hyperliquid · Lighter · MEXC)

2026-06-02. 한국 대형주(삼성전자·SK하이닉스·현대차)를 무기한선물/perp로 거래
가능한 암호화폐 거래소·DEX를 `korea-stock` 크레이트의 형제 모듈로 추가한다.

## 설계 원칙

- **공유 trait 없음.** 기존 `kis`/`toss`와 동일하게 각 venue는 독립 모듈이다.
  CEX(HMAC 쿼리서명)와 DEX(EIP-712/zk 서명)는 인증·주문모델·에러판정이 전혀
  달라 단일 추상화가 누수된다. 구체 구현 2개 이상에서 공통 형태가 드러나기 전엔
  trait를 만들지 않는다(YAGNI).
- **파일 레이아웃은 `toss`/`binance` 미러:** `mod/config/error/client/market/trade`.
  가격·수량은 정밀도 보존 위해 전부 `String`. 응답 struct마다 spec-example
  단위 테스트.
- **서명 무결성 > 기능 범위.** 공식 문서로 검증 가능한 서명만 구현한다. 검증
  불가한 서명은 **날조하지 않고**(자금 손실 위험) 타입+호출경로만 두고 명시적
  에러를 반환한다(`status=data-only`).

## Venue 능력 매트릭스

| Venue | 종류 | 시세(라이브검증) | 거래(코드) | 거래 실행검증 | 서명 |
|---|---|---|---|---|---|
| **Binance** (`binance`) | CEX | ✅ | 구현완료 주문/취소/포지션/잔고/레버리지 | ❌ **미실행** | HMAC-SHA256, 공식벡터 검증 |
| **Hyperliquid** (`hyperliquid`) | DEX (HIP-3/Trade.xyz) | ✅ | 구현완료 주문(IOC)/취소 | ❌ **미실행** | EIP-712+secp256k1, 서명 primitive 공식 SDK 벡터로 독립검증 |
| **MEXC** (`mexc`) | CEX | ✅ | 조회만; 신규주문 서버측 차단 | n/a | HMAC-SHA256(헤더), 검증 |
| **Lighter** (`lighter`) | DEX (zk/Optimism) | ✅ | ❌ 서명 미구현 | n/a | poseidon+schnorr (Go FFI) |

**정직한 라벨링(중요):** "거래 구현완료"는 코드+오프라인테스트(파싱·서명벡터)를
의미할 뿐, **어떤 venue도 testnet/live 엔드포인트로 실제 주문을 보낸 적 없다.**
거래코드의 진짜 검증은 사용자 키로 testnet 왕복이며 이 세션에서 불가. 실자금 투입
전 반드시 testnet 확인.

- 오프라인 전수 테스트 126 passed, 0 ignored(신규 모듈), `cargo build`/`clippy --all-targets` clean.
- 시세는 4개 venue 모두 라이브 응답으로 struct 검증.
- HL 서명 primitive: `sign_l1_action` dummy 벡터(mainnet/testnet r·s·v 6값)가
  upstream `hyperliquid-python-sdk/tests/signing_test.py`와 **바이트 일치 확인**
  (자작 벡터 아님, 공식 fixture 대조). 단 **order wire 바이트 레이아웃**(필드순
  a,b,p,s,r,t)은 공식 published 벡터가 없어 subagent 재현에만 의존 → live 미검증.

## Venue별 핵심

### Binance USDM Futures (`src/binance/`) — full
- 심볼(라이브 검증): `SAMSUNGUSDT` `SKHYNIXUSDT` `HYUNDAIUSDT`,
  status=TRADING, contractType=`TRADIFI_PERPETUAL`, px5/qty2, 20x, 8h 펀딩.
- 인증: `X-MBX-APIKEY` 헤더 + 쿼리스트링 HMAC-SHA256(`signature`). Binance 공식
  문서 테스트벡터로 서명기 검증(`hmac_matches_binance_doc_vector`).
- 시세 키 불필요. 거래는 `.testnet()` 강제 권장(example가 그렇게 함).

### Hyperliquid (`src/hyperliquid/`) — full
- HIP-3 빌더 Trade.xyz dex 네임스페이스 `"xyz"`. coin 식별자
  `xyz:SMSN`(삼성)·`xyz:SKHX`(하이닉스)·`xyz:HYUNDAI`.
- 모든 호출 **POST + JSON 바디**(GET+query 아님). 시세 `/info`, 거래 `/exchange`.
- 서명: `rmp_serde::to_vec_named(action)` → nonce(8B BE)+vault+expires →
  keccak256(connectionId) → EIP-712 Agent(source "a"/"b", chainId 1337,
  verifyingContract 0x0) → k256 recoverable → {r,s(minimal-hex),v=recid+27}.
  python SDK `sign_l1_action`와 바이트 동일. 단위테스트 2개(order/dummy 벡터)가
  설치된 SDK 출력과 일치 — **거래 전 반드시 통과 확인**(통과함).
- **⚠️ 운영 주의:**
  - 거래 경로 정수 asset id(SMSN=110034/SKHX=110022/HYUNDAI=110045)는 universe
    순서로 바뀔 수 있다. **해소:** `trade().place_by_coin(DEX, coin, ...)`가 주문
    직전 `market().asset_id`로 라이브 재도출한다(운영 권장 진입점). 하드코딩
    상수를 쓰는 `place(OrderRequest)`는 power-user용. 순수 `resolve_asset_id`
    단위 테스트로 인덱스 매핑 검증.
  - msgpack 비음수 정수(`a`,`o`,`num`) 필드는 반드시 `u64`(타입으로 강제). i64면
    서명 불일치.
  - MARKET 주문 타입 없음 → 먼 가드가격 IOC limit으로 표현.
  - 오라클은 KRX 장중(09:00–15:30 KST)에만 갱신 → 장외 candleSnapshot 빈값 가능.

### MEXC Contract (`src/mexc/`) — data-only(주문 차단)
- 시세 fully functional: ticker/depth/kline(병렬배열)/funding_rate, 시각 epoch 초.
- 서명(HMAC-SHA256: `accessKey+reqTime+paramString`) **검증됨** — place/cancel은
  실제 서명호출로 구현(스텁 아님). 단 MEXC가 2022-07-25부터 contract 신규주문
  API(`/private/order/submit|cancel`)를 대부분 계정에 차단 → 호출 시 서버
  maintenance 에러 반환. 조회(positions/assets/open_orders)는 정상.
- 에러판정은 HTTP status 아닌 바디 `{success,code}`(KIS식). `interpret_envelope` 단위테스트.
- **⚠️ 심볼 미확정[Medium]:** `SAMSUNG_USDT` 등 언더스코어 관례 사용했으나
  세션 내 라이브 확인 실패. 운영 전 `market().contracts()`로 정확 문자열+state==0 검증.

### Lighter (`src/lighter/`) — data-only(서명 미구현)
- 시세 fully functional & 라이브 검증(2026-06): 거래가능 마켓은 **USD 마진** 변형
  `SAMSUNGUSD`(id162)·`SKHYNIXUSD`(161)·`HYUNDAIUSD`(160) = active. bare
  `SAMSUNG/SKHYNIX/HYUNDAI`+`KRCOMP`(Korean Composite)는 현재 inactive.
- 엔드포인트: `/api/v1/orderBookDetails|orderBookOrders|candles|funding-rates|
  fundings`. 캔들 경로는 `/candles`(문서의 `/candlesticks`는 404). 일부 정상
  응답이 HTTP 200 + 에러 envelope → status+body code 동시 검사.
- **❌ 서명 미구현:** 주문서명(sign_create_order)은 native `lighter-go` 바이너리의
  poseidon+schnorr 스킴. python SDK/`lighter-rust` 모두 Go 바이너리 FFI. 순수
  Rust 구현 없고 문서로 재현 불가 → 날조 금지 원칙에 따라 `place/cancel`은
  `LighterError::SignerUnavailable` 반환. `next_nonce`(GET, 무서명)는 동작.
- 향후: lighter-go FFI 바인딩 또는 poseidon/schnorr 순수 Rust 포팅 필요.

## 통합 지점
- `Cargo.toml`: `hmac/sha2/hex`(CEX), `k256/sha3/rmp-serde`(Hyperliquid).
- `src/lib.rs`: `pub mod {binance,hyperliquid,lighter,mexc};` + 편의 재수출.

## 다음 작업(우선순위)
1. Lighter 서명(FFI 또는 순수 Rust) — 유일한 거래 불가 DEX 해소.
2. MEXC 심볼 라이브 확정, 주문차단 해제 시 status→full 재평가.
3. Hyperliquid asset id 운영 재도출 헬퍼 + WS 실시간(`subscribe`) 추가.
4. 구체 4구현에서 공통 형태 추출되면 그때 trait 검토.
