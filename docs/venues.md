# 글로벌 perp venue 레퍼런스 (`global`)

한국 대형주(삼성전자·SK하이닉스·현대차, 일부 KOSPI200 지수) 무기한선물(perp)을
상장한 글로벌 거래소·DEX **16곳**의 구현·서명·커버리지 현황. 각 venue는 `global`
그룹의 독립 모듈(공유 트레이트 없음 — 종목 어휘 [`KrStock`]만 공유, `KrStock`→자기
심볼 매핑).

- 시세: 16곳 모두 **라이브 검증**(2026-06-04, 키 불필요 공개 API).
- 거래: **오프라인(파싱·서명벡터)만 검증 — 어떤 venue도 testnet/live 실주문 이력 없음.
  실자금 전 testnet 왕복 필수.**
- 시장 조사(상장 종목·계약 사양) 출처: [`docs/research/kr-stock-perp-venues-2026-06.md`](research/kr-stock-perp-venues-2026-06.md).
- 설계 근거(4 venue 상세): [`docs/specs/2026-06-02-kr-perp-venues-design.md`](specs/2026-06-02-kr-perp-venues-design.md).

## 구현·서명 매트릭스

| 모듈 | 종류 | 시세 | 거래 | 서명 |
|---|---|---|---|---|
| `binance` | CEX | ✅ | 주문/취소/포지션/잔고/레버리지 | HMAC-SHA256 (공식 doc 벡터 검증) |
| `bybit` | CEX (v5 linear) | ✅ | 주문/취소/포지션/잔고 | HMAC-SHA256 (openssl 벡터 검증) |
| `bitget` | CEX (USDT-M) | ✅ | 주문/취소/포지션 | HMAC-SHA256 Base64 + passphrase (openssl 벡터 검증) |
| `kucoin` | CEX (futures) | ✅ | 주문/취소/포지션 | HMAC-SHA256 Base64 + passphrase v2 (openssl 벡터 검증) |
| `gateio` | CEX (USDT perp) | ✅ | 주문/취소 | HMAC-SHA512 (공식 Python gen_sign 벡터 검증) |
| `bingx` | CEX (perp swap) | ✅ | 주문/취소/포지션/레버리지 | HMAC-SHA256 (공식 doc 벡터 검증) |
| `mexc` | CEX | ✅ | 조회만 (신규주문 서버측 차단) | HMAC-SHA256 |
| `htx` | CEX (USDT-M swap) | ✅ | 주문/취소/포지션 | HMAC-SHA256 Base64, GET-스타일 prehash (openssl 교차검증) |
| `phemex` | CEX (Perp v2) | ✅ | 주문/취소/포지션 | HMAC-SHA256, base64url 시크릿 (공식 doc 벡터 검증) |
| `bitunix` | CEX | ✅(PREVIEW) | 주문/취소/포지션 | 더블 SHA256 (공식 ref 벡터 검증) |
| `toobit` | CEX (swap) | ✅ | 주문/취소 | HMAC-SHA256 (공식 doc 벡터 검증) |
| `weex` | CEX | ✅ | 주문/취소/포지션 | HMAC-SHA256 Base64 |
| `hyperliquid` | DEX (HIP-3/Trade.xyz) | ✅ | 주문(IOC)/취소 | EIP-712+secp256k1 (서명 primitive upstream SDK 벡터 검증) |
| `lighter` | DEX (zk) | ✅ +WS | 주문/취소 (게이트) | Poseidon2+Schnorr 순수Rust (암호코어 upstream 벡터 검증, tx봉투 미검증) |
| `aster` | DEX (BNB Chain) | ✅ | 주문/취소/포지션 | HMAC-SHA256 (Binance-호환 fapi, 2 doc 벡터 검증; on-chain EIP-712 미구현) |
| `pacifica` | DEX (Solana) | ✅ | 주문/취소 (게이트) | Ed25519+base58 (공식 python-sdk 골든벡터 검증, 봉투 검증) |

**16 venue** (CEX 12 / DEX 4).

## 종목 커버리지 차이

- **KOSPI200 지수**: `hyperliquid`(`xyz:KR200`)·`bingx`(`NCSIKOSPI2USD-USDT`)·`lighter`(`KRCOMP`)만 상장. 그 외 `symbol(KrStock::Kospi200)`은 `None`.
- **현대차 미상장**: `bingx`·`aster`·`pacifica`는 삼성·SK하이닉스만 → `symbol(KrStock::HyundaiMotor)`은 `None`.
- `bitunix` KR 심볼은 현재 `symbolStatus=PREVIEW`(상장 전, mark만 응답).
- `pacifica` 거래는 `allow_unverified_signing`(기본 false) 게이트 뒤(`lighter`와 동일).

## 사용

```rust
use korea_stock::global::binance::{BinanceClient, BinanceConfig, SAMSUNG};
let client = BinanceClient::new(BinanceConfig::public())?;   // 시세는 키 불필요
let idx = client.market().premium_index(SAMSUNG).await?;     // 마크가·펀딩

// 종목 어휘로 venue 비종속 조회:
use korea_stock::KrStock;
let sym = korea_stock::global::bybit::symbol(KrStock::SamsungElec).unwrap(); // "SAMSUNGUSDT"
```

## 환경변수·게이트

- 각 venue: `<VENUE>_API_KEY`/`<VENUE>_API_SECRET` (예 `BYBIT_API_KEY`, `HTX_API_SECRET`).
  DEX는 `HYPERLIQUID_*`(비밀키)·`LIGHTER_*`.
- Lighter 거래는 `LighterConfig::allow_unverified_signing`(기본 false) 게이트 뒤 —
  tx_info 봉투 미검증이므로 `scripts/lighter_capture_vector.md`로 공식 SDK 출력 대조 후 해제.
- Hyperliquid 운영 주문은 `trade().place_by_coin(DEX, coin, ...)` 사용 — asset id를
  라이브 meta에서 재도출해 wrong-instrument 사고 방지.
