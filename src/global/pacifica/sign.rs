//! Pacifica 주문 서명 — Ed25519 over base58-encoded sorted-JSON 메시지.
//!
//! # 스킴 (공식 `python-sdk/common/utils.py` 충실 포팅)
//!
//! Pacifica는 모든 POST(거래) 요청을 트레이더의 Solana 키(Ed25519)로 서명한다.
//! 서명 대상 메시지는 다음으로 정규화한다:
//!
//! 1. `header` = `{type, timestamp, expiry_window}` 와 `data: payload`를 한 객체로 병합.
//!    → `{type, timestamp, expiry_window, data: {payload...}}`
//! 2. **모든 dict 키를 재귀적으로 사전순 정렬**(`sort_json_keys`). 내부 `data` 객체도 정렬.
//! 3. **compact JSON 직렬화** — 구분자 `(",", ":")` (공백 없음).
//! 4. UTF-8 바이트로 인코딩 → Ed25519 서명 → **base58** 인코딩.
//!
//! HTTP 본문은 서명된 구조와 **다르다**: `data` 래퍼를 풀어
//! `{account, signature, timestamp, expiry_window, ...payload}`로 평탄화한다.
//!
//! ## 검증 상태 — **메시지+암호 코어 검증(골든 벡터) / e2e 미검증(게이트)**
//!
//! [`tests::matches_sdk_golden_vector`]는 공식 `python-sdk`의 실제 `prepare_message`
//! 출력 문자열과, 고정 seed(0x01..0x20)로 Ed25519 서명한 base58 결과를 **바이트 단위로
//! 대조**한다(이 세션에서 SDK를 클론해 캡처). 즉 메시지 정규화·Ed25519는 공식 구현과
//! 일치함이 증명됐다. 다만 실키로 서버가 주문을 수락하는 end-to-end 경로는 키 없이
//! 검증 불가라, 런타임 제출은 `allow_unverified_signing` 게이트(기본 false) 뒤에 둔다.

use ed25519_dalek::{Signer, SigningKey};
#[cfg(test)]
use ed25519_dalek::VerifyingKey;
use serde_json::Value;
use std::collections::BTreeMap;

/// 서명 헤더 — 모든 작업 공통. SDK `signature_header`.
pub(crate) struct SignHeader {
    /// 작업 타입. limit `"create_order"`, market `"create_market_order"`, cancel `"cancel_order"`.
    pub kind: &'static str,
    /// epoch ms.
    pub timestamp: u64,
    /// 서명 만료창(ms).
    pub expiry_window: u64,
}

/// Pacifica 정규 서명 메시지 문자열을 만든다.
///
/// `{type, timestamp, expiry_window, data: payload}`를 재귀 키정렬 후 compact JSON으로
/// 직렬화한다. `payload`의 키도 정렬된다(SDK `sort_json_keys`). 직렬화는 `BTreeMap`을
/// 거쳐 serde_json 기능 플래그(`preserve_order`)와 무관하게 항상 정렬·compact를 보장한다.
pub(crate) fn build_message(header: &SignHeader, payload: &BTreeMap<String, Value>) -> String {
    // 내부 data 객체도 정렬 보장 위해 BTreeMap → Value로 재구성(중첩 객체까지 재귀 정렬).
    let data = Value::Object(
        payload
            .iter()
            .map(|(k, v)| (k.clone(), sort_value(v)))
            .collect(),
    );

    let mut top: BTreeMap<String, Value> = BTreeMap::new();
    top.insert("type".into(), Value::String(header.kind.to_string()));
    top.insert("timestamp".into(), Value::Number(header.timestamp.into()));
    top.insert(
        "expiry_window".into(),
        Value::Number(header.expiry_window.into()),
    );
    top.insert("data".into(), data);

    // BTreeMap → serde_json::to_string: 키 정렬 + compact(공백 없음) = SDK separators=(",",":").
    serde_json::to_string(&top).expect("BTreeMap<String,Value> serializes")
}

/// `serde_json::Value`를 재귀적으로 키정렬한다(SDK `sort_json_keys`와 동치).
/// 객체는 `serde_json::Map`을 정렬 키로 재삽입(serde_json Map은 IndexMap이 아닌 한
/// 정렬 순서 유지), 배열은 원소별 재귀, 스칼라는 그대로.
fn sort_value(v: &Value) -> Value {
    match v {
        Value::Object(m) => {
            let sorted: BTreeMap<String, Value> =
                m.iter().map(|(k, x)| (k.clone(), sort_value(x))).collect();
            Value::Object(sorted.into_iter().collect())
        }
        Value::Array(a) => Value::Array(a.iter().map(sort_value).collect()),
        other => other.clone(),
    }
}

/// base58 Solana 시크릿 키를 [`SigningKey`]로 파싱한다.
///
/// Solana 키는 64바이트(`seed‖pubkey`) 또는 32바이트 seed다. 64바이트면 앞 32바이트가
/// seed, 뒤 32바이트는 공개키 — 파싱 후 유도 공개키가 뒤 32바이트와 **일치하는지 검증**해
/// 키 절반을 잘못 쓰는 사고를 막는다.
pub(crate) fn parse_secret_key(b58: &str) -> Result<SigningKey, String> {
    let bytes = bs58::decode(b58.trim())
        .into_vec()
        .map_err(|e| format!("invalid base58 secret key: {e}"))?;
    match bytes.len() {
        32 => {
            let seed: [u8; 32] = bytes.as_slice().try_into().unwrap();
            Ok(SigningKey::from_bytes(&seed))
        }
        64 => {
            let seed: [u8; 32] = bytes[0..32].try_into().unwrap();
            let sk = SigningKey::from_bytes(&seed);
            let derived = sk.verifying_key().to_bytes();
            if derived[..] != bytes[32..64] {
                return Err(
                    "64-byte secret key: derived pubkey != trailing 32 bytes (corrupt key)".into(),
                );
            }
            Ok(sk)
        }
        n => Err(format!("secret key must be 32 or 64 bytes, got {n}")),
    }
}

/// 시크릿 키에서 base58 공개키(Solana `account` 주소)를 얻는다.
pub(crate) fn account_address(sk: &SigningKey) -> String {
    bs58::encode(sk.verifying_key().to_bytes()).into_string()
}

/// 정규 메시지를 Ed25519 서명하고 base58로 인코딩한다(SDK `sign_message`).
pub(crate) fn sign_message(sk: &SigningKey, message: &str) -> String {
    let sig = sk.sign(message.as_bytes());
    bs58::encode(sig.to_bytes()).into_string()
}

/// base58 서명을 공개키로 검증한다(라운드트립 테스트·외부 검증용).
#[cfg(test)]
pub(crate) fn verify(pubkey: &VerifyingKey, message: &str, sig_b58: &str) -> Result<(), String> {
    let sig_bytes = bs58::decode(sig_b58)
        .into_vec()
        .map_err(|e| format!("bad sig base58: {e}"))?;
    let arr: [u8; 64] = sig_bytes
        .as_slice()
        .try_into()
        .map_err(|_| "sig not 64 bytes".to_string())?;
    let sig = ed25519_dalek::Signature::from_bytes(&arr);
    pubkey
        .verify_strict(message.as_bytes(), &sig)
        .map_err(|e| format!("verify failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 고정 32바이트 seed(0x01..0x20). 골든 벡터 생성에 쓴 값과 동일.
    const SEED: [u8; 32] = [
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32,
    ];

    /// **골든 벡터 (공식 python-sdk 교차검증).**
    ///
    /// 메시지 정규화 문자열·base58 서명을 공식 `python-sdk/common/utils.py`의 실제
    /// `prepare_message` 출력 + Python `cryptography` Ed25519 결과와 **바이트 일치**로 대조.
    /// 이 한 테스트가 메시지 레이아웃·키정렬·compact JSON·Ed25519·base58 인코딩을
    /// 전부 공식 구현 대비 고정한다 — 거래 안전의 핵심.
    #[test]
    fn matches_sdk_golden_vector() {
        let header = SignHeader {
            kind: "create_order",
            timestamp: 1_716_200_000_000,
            expiry_window: 5_000,
        };
        // SDK create_limit_order.py의 signature_payload 형태(순서는 무관 — 정렬됨).
        let mut payload: BTreeMap<String, Value> = BTreeMap::new();
        payload.insert("symbol".into(), json!("SAMSUNG"));
        payload.insert("price".into(), json!("234.20"));
        payload.insert("reduce_only".into(), json!(false));
        payload.insert("amount".into(), json!("0.1"));
        payload.insert("side".into(), json!("bid"));
        payload.insert("tif".into(), json!("GTC"));
        payload.insert(
            "client_order_id".into(),
            json!("f47ac10b-58cc-4372-a567-0e02b2c3d479"),
        );

        let msg = build_message(&header, &payload);
        // 공식 SDK prepare_message가 뱉은 정규 문자열(이 세션에서 캡처).
        assert_eq!(
            msg,
            r#"{"data":{"amount":"0.1","client_order_id":"f47ac10b-58cc-4372-a567-0e02b2c3d479","price":"234.20","reduce_only":false,"side":"bid","symbol":"SAMSUNG","tif":"GTC"},"expiry_window":5000,"timestamp":1716200000000,"type":"create_order"}"#
        );

        let sk = SigningKey::from_bytes(&SEED);
        // 공개키(base58)도 골든값과 대조 — 키 유도 경로 고정.
        assert_eq!(
            account_address(&sk),
            "9C6hybhQ6Aycep9jaUnP6uL9ZYvDjUp1aSkFWPUFJtpj"
        );
        let sig = sign_message(&sk, &msg);
        // Python cryptography Ed25519 + base58(직접 구현)으로 캡처한 결정적 서명.
        assert_eq!(
            sig,
            "fuGrgfRj8i1zgNBUTpd2N7g9hJRTfsjNP8yfkknDxNoyhCv7QHvvbHmea6RcmwjpNHbj2TSF8DsBAu1iw43gntG"
        );
    }

    /// 취소 작업 골든 벡터 — `order_id`가 JSON **number**(문자열 아님)임을 고정.
    #[test]
    fn cancel_golden_vector() {
        let header = SignHeader {
            kind: "cancel_order",
            timestamp: 1_716_200_000_000,
            expiry_window: 5_000,
        };
        let mut payload: BTreeMap<String, Value> = BTreeMap::new();
        payload.insert("symbol".into(), json!("SAMSUNG"));
        payload.insert("order_id".into(), json!(42069));

        let msg = build_message(&header, &payload);
        assert_eq!(
            msg,
            r#"{"data":{"order_id":42069,"symbol":"SAMSUNG"},"expiry_window":5000,"timestamp":1716200000000,"type":"cancel_order"}"#
        );
        let sk = SigningKey::from_bytes(&SEED);
        assert_eq!(
            sign_message(&sk, &msg),
            "wKi1wN7kdc1bhvoYqtiFxXAamNeNG4qUB5grABnsycbDbXHTWnpj7wNoWKQBAo8JSFcB1WhqpTTLzgjwKAfPmT9"
        );
    }

    /// 서명 → 검증 라운드트립 + 결정성(같은 입력 → 같은 서명).
    #[test]
    fn sign_verify_roundtrip_deterministic() {
        let sk = SigningKey::from_bytes(&SEED);
        let header = SignHeader {
            kind: "create_order",
            timestamp: 1,
            expiry_window: 5_000,
        };
        let mut payload: BTreeMap<String, Value> = BTreeMap::new();
        payload.insert("symbol".into(), json!("SKHYNIX"));
        payload.insert("amount".into(), json!("1"));
        let msg = build_message(&header, &payload);

        let sig1 = sign_message(&sk, &msg);
        let sig2 = sign_message(&sk, &msg);
        assert_eq!(sig1, sig2, "Ed25519 결정성");
        verify(&sk.verifying_key(), &msg, &sig1).expect("roundtrip verify");
        // 변조된 메시지는 검증 실패.
        assert!(verify(&sk.verifying_key(), "tampered", &sig1).is_err());
    }

    /// 64바이트 키 = seed‖pubkey 파싱 + 공개키 일치 검증.
    #[test]
    fn parse_64byte_key_and_validate_pubkey() {
        let sk = SigningKey::from_bytes(&SEED);
        let pubkey = sk.verifying_key().to_bytes();
        let mut full = Vec::with_capacity(64);
        full.extend_from_slice(&SEED);
        full.extend_from_slice(&pubkey);
        let b58 = bs58::encode(&full).into_string();
        let parsed = parse_secret_key(&b58).expect("64-byte parse");
        assert_eq!(parsed.to_bytes(), SEED);

        // 32바이트 seed 직접도 동작.
        let seed_b58 = bs58::encode(SEED).into_string();
        assert_eq!(parse_secret_key(&seed_b58).unwrap().to_bytes(), SEED);

        // 손상된 64바이트(뒤 절반 불일치)는 거부.
        let mut bad = full.clone();
        bad[40] ^= 0xff;
        let bad_b58 = bs58::encode(&bad).into_string();
        assert!(parse_secret_key(&bad_b58).is_err());
    }

    /// 중첩 객체 키정렬이 재귀적임을 확인(SDK sort_json_keys 동치).
    #[test]
    fn nested_keys_sorted() {
        let header = SignHeader {
            kind: "create_order",
            timestamp: 1,
            expiry_window: 2,
        };
        let mut payload: BTreeMap<String, Value> = BTreeMap::new();
        payload.insert("z".into(), json!({"b": 1, "a": 2}));
        payload.insert("a".into(), json!("x"));
        let msg = build_message(&header, &payload);
        // 외부·내부 모두 정렬: data 안에서 a 먼저, z 안의 {a,b} 정렬.
        assert_eq!(
            msg,
            r#"{"data":{"a":"x","z":{"a":2,"b":1}},"expiry_window":2,"timestamp":1,"type":"create_order"}"#
        );
    }
}
