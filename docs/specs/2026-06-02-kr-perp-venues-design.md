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
  불가한 서명은 **날조하지 않고**(자금 손실 위험) 둘 중 하나로 처리한다: (a) 기본
  차단(`status=data-only`, 명시적 에러), 또는 (b) 암호 코어는 공식 벡터로 검증됐고
  남은 와이어 봉투만 미검증인 경우, 사용자가 캡처로 확인 후 명시적으로 여는 게이트
  (`allow_unverified_signing`, 기본 false) 뒤에 둔다(Lighter).

## Venue 능력 매트릭스

| Venue | 종류 | 시세(라이브검증) | 거래(코드) | 거래 실행검증 | 서명 |
|---|---|---|---|---|---|
| **Binance** (`binance`) | CEX | ✅ | 구현완료 주문/취소/포지션/잔고/레버리지 | ❌ **미실행** | HMAC-SHA256, 공식벡터 검증 |
| **Hyperliquid** (`hyperliquid`) | DEX (HIP-3/Trade.xyz) | ✅ | 구현완료 주문(IOC)/취소 | ❌ **미실행** | EIP-712+secp256k1, 서명 primitive 공식 SDK 벡터로 독립검증 |
| **MEXC** (`mexc`) | CEX | ✅ 심볼 라이브확정 | 조회만; 신규주문 서버측 차단 | n/a | HMAC-SHA256(헤더), 검증 |
| **Lighter** (`lighter`) | DEX (zk/Optimism) | ✅ +realtime WS | 구현완료 주문/취소(게이트) | ❌ **미실행** | Poseidon2+Schnorr/ECgFp5 순수Rust; **암호코어 upstream벡터 검증**, tx봉투 미검증 |

**정직한 라벨링(중요):** "거래 구현완료"는 코드+오프라인테스트(파싱·서명벡터)를
의미할 뿐, **어떤 venue도 testnet/live 엔드포인트로 실제 주문을 보낸 적 없다.**
거래코드의 진짜 검증은 사용자 키로 testnet 왕복이며 이 세션에서 불가. 실자금 투입
전 반드시 testnet 확인.

- 오프라인 전수 테스트 141 passed, 0 ignored(신규 모듈), `cargo build`/`clippy --all-targets` clean.
- 시세는 4개 venue 모두 라이브 응답으로 struct 검증. MEXC 심볼·Lighter 활성마켓은
  라이브 API로 문자열·state 확정(2026-06-04).
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
- **심볼 라이브 확정[High] (2026-06-04):** 실제 `symbol`은 `STOCK` 접미사 →
  `SAMSUNGSTOCK_USDT`·`SKHYNIXSTOCK_USDT`·`HYUNDAISTOCK_USDT`, 모두 `state=0`(체결가능).
  ticker로 교차확인(삼성 last≈247, 펀딩 라이브). 직전 추정치 `SAMSUNG_USDT`는
  `displayNameEn`(표시명)이었고 실제 거래 심볼 아님 → 수정. HYUNDAI 포함(직전 누락).
  (잔존: market/client/trade의 `#[cfg(test)]` fixture에 옛 `SAMSUNG_USDT` 문자열이
  inert 값으로 남음 — 공개 상수 참조 아님, 동작 무관.)

### Lighter (`src/lighter/`) — 암호검증/봉투 미검증(게이트)
- 시세 fully functional & 라이브 검증: 거래가능 마켓은 **USD 마진** 변형
  `SAMSUNGUSD`(id162)·`SKHYNIXUSD`(161)·`HYUNDAIUSD`(160) = active. bare
  `SAMSUNG/SKHYNIX/HYUNDAI`+`KRCOMP`는 현재 inactive. **realtime WS**(`realtime`)
  추가 — kimp(`../kimp/src/source/lighter.rs`)의 검증된 orderbook 어댑터 패턴 이식,
  `order_book/{id}` 구독·델타 적용, kimp fixture 형태로 단위테스트.
- 엔드포인트: `/api/v1/orderBookDetails|orderBookOrders|candles|funding-rates|
  fundings`. 캔들 경로는 `/candles`(문서의 `/candlesticks`는 404). 일부 정상
  응답이 HTTP 200 + 에러 envelope → status+body code 동시 검사.
- **서명 — 순수 Rust 포팅(`sign.rs`):** Goldilocks·GFp5·Poseidon2·ECgFp5·Schnorr를
  `lighter-go`+`poseidon_crypto`에서 수기 이식.
  - **암호 코어 검증됨[High]:** `poseidon_crypto/.../schnorr_test.go`
    `TestComparativeSchnorrSignAndVerify` 결정적 벡터 3케이스를 `#[cfg(test)]`에 박아
    통과 + **upstream Go 소스 limb과 바이트 일치 독립 대조**. Schnorr sign+verify가
    필드·확장체·해시·점곱을 독립 경로로 전이 검증.
  - **tx_info 봉투·스케일링·chain_id 미검증:** 불투명 native `.so` 산출물, 공식 픽스처
    없음. `place/cancel`은 [`LighterConfig::allow_unverified_signing`] 게이트(기본
    false) 뒤. 켜기 전 `scripts/lighter_capture_vector.md`로 공식 SDK 출력과 대조 필수.
- 향후: capture 스크립트로 봉투 확정 → 게이트 해제 → testnet 왕복.

## 통합 지점
- `Cargo.toml`: `hmac/sha2/hex`(CEX), `k256/sha3/rmp-serde`(Hyperliquid),
  `getrandom`(Lighter 서명 nonce).
- `src/lib.rs`: `pub mod {binance,hyperliquid,lighter,mexc};` + 편의 재수출.

## 다음 작업(우선순위)
1. Lighter `scripts/lighter_capture_vector.md`로 tx_info 봉투·chain_id 확정 →
   `allow_unverified_signing` 해제 → testnet 왕복. (암호 코어는 검증 완료.)
2. 4개 venue testnet 실주문 왕복 — 거래코드 실행검증(현재 전부 미실행).
3. MEXC 주문차단 해제 시 status→full 재평가.
4. Hyperliquid WS 실시간(`subscribe`) 추가(asset id 재도출 헬퍼는 완료).
5. 구체 4구현에서 공통 형태 추출되면 그때 trait 검토.
