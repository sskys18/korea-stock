# Lighter 서명 검증 벡터 캡처 절차

`src/lighter/sign.rs`의 순수 Rust 서명 포팅은 **UNVERIFIED**다. 곡선·해시·Schnorr 코어는
공식 `poseidon_crypto` 결정적 테스트 벡터를 `#[cfg(test)]`에 박아 `cargo test`로 검증
가능하지만, 다음 세 층은 **공식 픽스처가 없어** 라이브 SDK 출력과 직접 대조해야 한다:

1. **메시지 해시 필드 레이아웃** (`CreateOrderMsg::hash` / `CancelOrderMsg::hash`).
2. **`tx_info` JSON 와이어 봉투** (`build_create_order_tx_info` / `build_cancel_order_tx_info`).
3. **`chain_id`(메인넷 실제 값)·`expired_at`/`order_expiry` 처리·십진 스케일링**.

이 문서는 공식 `lighter-python`(네이티브 `lighter-go` 바이너리를 FFI로 호출)을 **고정 입력**으로
돌려 위 세 층의 기대값을 캡처하는 절차다. 캡처한 값을 Rust 테스트로 박으면 종단 검증이 된다.

---

## 0. 사전 준비

```bash
python3 -m venv /tmp/lighter-venv
/tmp/lighter-venv/bin/pip install lighter-sdk   # 또는: pip install git+https://github.com/elliottech/lighter-python
```

> `lighter-python`은 `lighter/signers/lighter-signer-<os>-<arch>.{so,dylib}`를 동봉한 FFI
> 래퍼다. 순수 Python 서명 경로는 없다 — 서명은 네이티브 바이너리가 수행하고 `tx_info`
> 문자열을 그대로 반환한다. 따라서 이 캡처가 곧 **정답(ground truth)**이다.

## 1. 고정 입력 (Rust 테스트와 1:1로 맞출 것)

| 항목 | 값 |
|---|---|
| `api_key_private_key` | 임의 40바이트 hex (예: `0x` + 80 hex chars). **테스트 전용 키.** |
| `account_index` | `1` |
| `api_key_index` | `0` |
| `chain_id` | **메인넷 실제 값** — 아래 2단계에서 확인 |
| `nonce` | `42` (고정) |
| `expired_at` | 고정 ms 값 (예: `1_900_000_000_000`) — 시간 의존 제거 |

## 2. 메인넷 chain_id 확인 (CRITICAL)

`lighter-go` 테스트는 `chainId=304`(테스트 체인)를 쓴다. 메인넷은 다를 수 있고, 틀리면
서명은 암호학적으로 유효하나 **서버가 거부**한다.

```python
# lighter-python SignerClient 생성 시 넘기는 chain_id를 확인하거나,
# 공개 설정에서 읽는다. 캡처한 값을 LighterConfig::chain_id(...)로 설정.
from lighter import SignerClient
# 문서/설정의 LIGHTER_CHAIN_ID 또는 client 내부 기본값을 출력해 기록.
```

확인한 메인넷 값을 `src/lighter/config.rs`의 호출부(`LighterConfig::new(...).chain_id(N)`)와
아래 Rust 테스트의 `chain_id`에 반영한다.

## 3. create_order 캡처 — IOC(시장가)와 GTT(한정가) 둘 다

```python
import json
from lighter import SignerClient

PRIV = "0x" + "11"*40          # 고정 테스트 키 (40바이트)
client = SignerClient(
    url="https://mainnet.zklighter.elliot.ai",
    private_key=PRIV,
    account_index=1,
    api_key_index=0,
)

# (A) 시장가/IOC: price=worst-acceptable, order_expiry nil, trigger nil
txType, txInfo, txHash, err = client.sign_create_order(
    market_index=0,
    client_order_index=0,
    base_amount=1000,           # 이미 스케일된 정수
    price=50000,                # 이미 스케일된 정수 (worst price)
    is_ask=0,
    order_type=1,               # MarketOrder
    time_in_force=0,            # IOC
    reduce_only=0,
    trigger_price=0,
    order_expiry=-1,            # DEFAULT_28_DAY 관례 — 해시 전 확장 여부를 txHash로 확인
    nonce=42,
    api_key_index=0,
)
print("MARKET pubkey:", client.signer.... )  # 가능하면 파생 공개키도
print("MARKET txType:", txType)
print("MARKET txHash:", txHash)              # ← CreateOrderMsg::hash 결과(LE 40B hex)와 대조
print("MARKET txInfo:", txInfo)              # ← build_create_order_tx_info 결과와 대조

# (B) 한정가/GTT: order_expiry 비-nil 필수
txType, txInfo, txHash, err = client.sign_create_order(
    market_index=0, client_order_index=7,
    base_amount=2000, price=51000, is_ask=1,
    order_type=0,               # LimitOrder
    time_in_force=1,            # GTT
    reduce_only=0, trigger_price=0,
    order_expiry=1_900_000_000_000,   # 고정 ms
    nonce=42, api_key_index=0,
)
print("LIMIT txHash:", txHash)
print("LIMIT txInfo:", txInfo)
```

> **주의:** `sign_create_order`가 `expired_at`(tx 데드라인)을 내부에서 `now+10분`으로
> 잡는다면 캡처값이 매번 달라진다. 가능하면 `expired_at`을 명시 인자로 고정하거나,
> 반환된 `txInfo`에서 `ExpiredAt` 키를 읽어 Rust 테스트에 동일 값을 넣는다.

## 4. cancel_order 캡처

```python
txType, txInfo, txHash, err = client.sign_cancel_order(
    market_index=0,
    order_index=12345,
    nonce=42, api_key_index=0,
)
print("CANCEL txHash:", txHash)
print("CANCEL txInfo:", txInfo)
```

## 5. 대조 항목 (Rust ↔ Python)

| 검증 대상 | Rust | Python 캡처 |
|---|---|---|
| **공개키** | `sign::schnorr::pk_from_sk(parse_private_key(PRIV)).to_le_bytes()` → hex | 등록/파생 pubkey |
| **메시지 해시** | `CreateOrderMsg::hash(chain_id)` (40B LE hex) | `txHash` |
| **tx_info JSON** | `build_create_order_tx_info(&msg, &sig)` | `txInfo` (키·base64 Sig 포함) |
| **서명 유효성** | 서버 `POST /sendTx`가 수락 | — |

해시(공개키 무관·결정적)가 먼저 일치해야 한다. 그다음 `txInfo`의 키 집합·타입·`Sig`
base64·attributes(빈 맵일 때 `null`/생략/`{}` 중 무엇인지)를 글자 단위로 맞춘다.

## 6. 캡처값을 Rust 픽스처로 고정

대조가 끝나면 `src/lighter/sign.rs` `#[cfg(test)]`에 다음을 추가한다:

```rust
#[test]
fn create_order_hash_matches_official_capture() {
    // 3단계 (A) 입력으로 만든 CreateOrderMsg.
    let msg = sign::CreateOrderMsg { /* 고정 입력 */ };
    let got = hex::encode(msg.hash(/* 메인넷 chain_id */));
    assert_eq!(got, "<Python txHash hex>");
}

#[test]
fn pubkey_matches_official() {
    let sk = sign::parse_private_key("0x1111...").unwrap();
    let pk = hex::encode(sign::gfp5::to_le_bytes(&sign::schnorr::pk_from_sk(sk)));
    assert_eq!(pk, "<Python derived pubkey hex>");
}
```

이 테스트가 통과하고 테스트넷 `POST /sendTx`가 주문을 수락하면, 그때 비로소
`config.allow_unverified_signing=true`를 켜도 된다. **그 전까진 실자금 금지.**
