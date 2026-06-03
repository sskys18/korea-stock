use std::time::Duration;

use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use sha3::{Digest, Keccak256};

use crate::hyperliquid::config::HyperliquidConfig;
use crate::hyperliquid::error::{HyperliquidError, Result};
use crate::ratelimit::RateLimiter;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
///
/// Hyperliquid는 시세·거래 모두 POST + JSON 바디다. `/info`로 보내면 키 없이,
/// `/exchange`로 보내면 서명 필요. **`/exchange`는 직접 서명하지 않으므로** 이
/// escape hatch는 이미 서명·조립된 완성 바디(`{action,nonce,signature,...}`)를
/// 넣어야 한다. 서명이 필요하면 [`HyperliquidClient::post_signed`]를 쓴다.
#[derive(Debug, Clone)]
pub struct RawRequest {
    /// "/info" 또는 "/exchange".
    pub path: String,
    /// POST JSON 바디 전체.
    pub body: Value,
}

/// `/exchange` 전송 바디. action을 **타입 그대로** 담아 직렬화한다 — `serde_json::Value`를
/// 거치면 키가 알파벳 재정렬되어 "전송한 바디 ≠ 서명한 바디"가 될 수 있다(서버가
/// 이름 기준 역직렬화한다면 무해하나, 그 가정에 의존하지 않는다).
#[derive(Serialize)]
struct ExchangeRequest<'a, T: Serialize> {
    action: &'a T,
    nonce: u64,
    signature: Signature,
    #[serde(rename = "vaultAddress")]
    vault_address: Option<String>,
}

/// secp256k1 서명 `{r,s,v}`. r/s는 Ethereum `to_hex` 규칙의 최소 hex("0x"+선행0
/// 제거), v는 27|28.
#[derive(Debug, Clone, Serialize)]
pub(crate) struct Signature {
    pub r: String,
    pub s: String,
    pub v: u16,
}

/// Hyperliquid 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이 `POST /info`, 거래 액세서(`trade`)는 EIP-712
/// 서명으로 `POST /exchange`를 호출한다.
pub struct HyperliquidClient {
    config: HyperliquidConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl HyperliquidClient {
    /// 클라이언트 생성.
    pub fn new(config: HyperliquidConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(Duration::from_secs(10))
            .build()?;
        let limiter = config.rate_limit.map(RateLimiter::new);
        Ok(Self {
            config,
            http,
            limiter,
        })
    }

    /// 시세 도메인 액세서 (키 불필요, `POST /info`).
    pub fn market(&self) -> crate::hyperliquid::market::Market<'_> {
        crate::hyperliquid::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (`POST /exchange` 서명 + `POST /info` 조회 혼합).
    pub fn trade(&self) -> crate::hyperliquid::trade::Trade<'_> {
        crate::hyperliquid::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 이미 조립된 바디를 그대로 POST.
    pub async fn raw_call(&self, req: RawRequest) -> Result<Value> {
        self.post_json(&req.path, &req.body).await
    }

    /// 현재 UTC epoch milliseconds. 서명 nonce 용.
    pub(crate) fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// `/info` 또는 `/exchange`로 JSON 바디 POST. 429 반응형 백오프 재시도.
    ///
    /// 성공 판정은 **HTTP status로만** 하고, `/exchange` 바디 내부의
    /// `{"status":"err"}`는 호출부([`trade`])에서 별도 해석한다(원시 JSON 보존).
    pub(crate) async fn post_json(&self, path: &str, body: &Value) -> Result<Value> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.post_once(path, body).await {
                Err(HyperliquidError::Api { status, .. })
                    if status == 429 && attempt < MAX_RETRIES =>
                {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 POST. (선택)레이트캡 → 전송 → status 분기. 429는 대기 후 `Api{429}` 반환.
    async fn post_once(&self, path: &str, body: &Value) -> Result<Value> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }
        let url = format!("{}{}", self.config.base_url, path);
        let resp = self.http.post(&url).json(body).send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = retry_after_secs(&resp);
            tracing::warn!("429 — {wait}s 대기 후 재시도");
            tokio::time::sleep(Duration::from_secs(wait)).await;
            return Err(HyperliquidError::Api {
                status: 429,
                body: "rate-limit-exceeded".into(),
            });
        }

        if status.is_success() {
            return Ok(resp.json().await?);
        }

        let http_status = status.as_u16();
        let text = resp.text().await.unwrap_or_default();
        Err(HyperliquidError::Api {
            status: http_status,
            body: text,
        })
    }

    /// **타입드 action**을 서명해 `POST /exchange`로 제출. 응답 바디를 원시 JSON으로
    /// 반환한다.
    ///
    /// action(예: [`crate::hyperliquid::trade::OrderAction`])은 wire 필드 순서를
    /// 선언 순서로 고정한 struct여야 한다. 여기서 nonce를 찍고, action_hash→EIP-712
    /// 서명→타입드 [`ExchangeRequest`] 바디를 조립한다. **본문 내부 에러 해석은
    /// 하지 않는다** — 호출부가 매핑한다.
    pub(crate) async fn post_signed<T: Serialize>(&self, action: &T) -> Result<Value> {
        if self.config.secret_key.is_empty() {
            return Err(HyperliquidError::Auth(
                "signed endpoint requires secret_key".into(),
            ));
        }
        let nonce = Self::timestamp_ms();
        let vault = self.config.vault_address.as_deref();
        let signature = sign_l1_action(
            &self.config.secret_key,
            action,
            nonce,
            vault,
            self.config.is_mainnet(),
        )?;

        let req = ExchangeRequest {
            action,
            nonce,
            signature,
            vault_address: self.config.vault_address.clone(),
        };
        let body = serde_json::to_value(&req)?;
        self.post_json("/exchange", &body).await
    }
}

// ──────────────────────────────────────────────────────────────────────────
// L1 action 서명 (EIP-712 + secp256k1 ECDSA).
//
// hyperliquid-python-sdk `hyperliquid/utils/signing.py`를 1:1로 옮긴다:
//   1. action을 msgpack(named map, **struct 선언 순서**)으로 직렬화.
//   2. nonce(8B big-endian) 부착.
//   3. vault 마커: None→0x00, Some(addr)→0x01 + 20B 주소.
//   4. (expires_after 사용 시) 0x00 + 8B big-endian. **미사용이면 생략.**
//   5. keccak256(data) = connectionId(32B).
//   6. phantom agent = { source: "a"(mainnet)/"b"(testnet), connectionId }.
//   7. EIP-712 도메인 Exchange/1/chainId 1337/verifyingContract 0x0,
//      primaryType Agent(string source, bytes32 connectionId) 해시 서명.
//   8. 서명 → {r,s,v}, v = recovery_id + 27.
//
// **정수 인코딩 함정:** action의 모든 비음수 정수 필드는 반드시 `u64`로 타이핑해야
// 한다. rmp-serde의 `serialize_i64`는 양수에도 *signed* 마커(0xd2/0xd3)를 쓰지만
// Python `msgpack.packb`는 *unsigned* 마커(0xce/0xcf)를 쓴다 → 바이트 불일치 →
// 서명 무효. `serialize_u64`만 unsigned 마커를 낸다. (asset id 110034 등 큰 값에서
// 표면화; asset=1 같은 fixint은 우연히 동일해 테스트가 잡지 못한다.)
// ──────────────────────────────────────────────────────────────────────────

/// keccak256 헬퍼.
fn keccak256(data: &[u8]) -> [u8; 32] {
    let mut h = Keccak256::new();
    h.update(data);
    h.finalize().into()
}

/// "0x" 접두를 허용하는 hex 디코드.
fn hex_decode(s: &str) -> Result<Vec<u8>> {
    let s = s.strip_prefix("0x").unwrap_or(s);
    hex::decode(s).map_err(|e| HyperliquidError::Sign(format!("hex decode: {e}")))
}

/// 타입드 action의 msgpack 직렬화 + nonce/vault/expires 바이트 부착 후 keccak256.
///
/// `rmp_serde::to_vec_named`로 **맵(문자열 키, struct 선언 순서)** 인코딩한다 —
/// `to_vec`는 배열로 인코딩되어 Python `msgpack.packb(dict)`와 어긋난다. action을
/// `serde_json::Value`로 받지 않는 것이 핵심: `Value`(BTreeMap)는 키를 알파벳
/// 재정렬해 서명을 전부 무효화한다.
pub(crate) fn action_hash<T: Serialize>(
    action: &T,
    nonce: u64,
    vault: Option<&str>,
    expires_after: Option<u64>,
) -> Result<[u8; 32]> {
    let mut data = rmp_serde::to_vec_named(action)
        .map_err(|e| HyperliquidError::Sign(format!("msgpack: {e}")))?;
    data.extend_from_slice(&nonce.to_be_bytes());
    match vault {
        None => data.push(0x00),
        Some(addr) => {
            data.push(0x01);
            let bytes = hex_decode(addr)?;
            if bytes.len() != 20 {
                return Err(HyperliquidError::Sign(format!(
                    "vault address must be 20 bytes, got {}",
                    bytes.len()
                )));
            }
            data.extend_from_slice(&bytes);
        }
    }
    if let Some(exp) = expires_after {
        data.push(0x00);
        data.extend_from_slice(&exp.to_be_bytes());
    }
    Ok(keccak256(&data))
}

/// EIP-712 `keccak256("\x19\x01" || domainSeparator || hashStruct(message))`.
fn eip712_signing_hash(source: &str, connection_id: &[u8; 32]) -> [u8; 32] {
    // domain typeHash: EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)
    let domain_type_hash = keccak256(
        b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)",
    );
    let name_hash = keccak256(b"Exchange");
    let version_hash = keccak256(b"1");
    // chainId = 1337 (uint256, 32B big-endian) → 0x...0539.
    let mut chain_id = [0u8; 32];
    chain_id[30] = (1337u16 >> 8) as u8; // 0x05
    chain_id[31] = (1337u16 & 0xff) as u8; // 0x39
                                           // verifyingContract = 0x0..0 (address) → 전부 0.
    let verifying_contract = [0u8; 32];

    let mut domain_enc = Vec::with_capacity(32 * 5);
    domain_enc.extend_from_slice(&domain_type_hash);
    domain_enc.extend_from_slice(&name_hash);
    domain_enc.extend_from_slice(&version_hash);
    domain_enc.extend_from_slice(&chain_id);
    domain_enc.extend_from_slice(&verifying_contract);
    let domain_separator = keccak256(&domain_enc);

    // Agent struct: keccak256(typeHash || keccak256(source) || connectionId)
    let agent_type_hash = keccak256(b"Agent(string source,bytes32 connectionId)");
    let source_hash = keccak256(source.as_bytes());
    let mut struct_enc = Vec::with_capacity(32 * 3);
    struct_enc.extend_from_slice(&agent_type_hash);
    struct_enc.extend_from_slice(&source_hash);
    struct_enc.extend_from_slice(connection_id);
    let hash_struct = keccak256(&struct_enc);

    let mut final_enc = Vec::with_capacity(2 + 32 + 32);
    final_enc.extend_from_slice(&[0x19, 0x01]);
    final_enc.extend_from_slice(&domain_separator);
    final_enc.extend_from_slice(&hash_struct);
    keccak256(&final_enc)
}

/// 타입드 L1 action 서명. [`Signature`]를 반환한다.
pub(crate) fn sign_l1_action<T: Serialize>(
    secret_key: &str,
    action: &T,
    nonce: u64,
    vault: Option<&str>,
    is_mainnet: bool,
) -> Result<Signature> {
    let connection_id = action_hash(action, nonce, vault, None)?;
    let source = if is_mainnet { "a" } else { "b" };
    let digest = eip712_signing_hash(source, &connection_id);
    sign_digest(secret_key, &digest)
}

/// 32B prehash를 secp256k1로 서명 → [`Signature`] (v = recovery_id + 27).
fn sign_digest(secret_key: &str, digest: &[u8; 32]) -> Result<Signature> {
    use k256::ecdsa::SigningKey;

    let key_bytes = hex_decode(secret_key)?;
    let signing_key = SigningKey::from_slice(&key_bytes)
        .map_err(|e| HyperliquidError::Sign(format!("invalid secret key: {e}")))?;

    // EIP-712는 이미 해시된 32B를 서명한다 → prehash 서명.
    let (sig, recid) = signing_key
        .sign_prehash_recoverable(digest)
        .map_err(|e| HyperliquidError::Sign(format!("ecdsa: {e}")))?;

    // k256은 기본 low-S 정규화(EIP-2)를 적용한다 — Hyperliquid 요구 형태.
    let r = sig.r().to_bytes(); // 32B big-endian
    let s = sig.s().to_bytes();
    let v = recid.to_byte() as u16 + 27;

    // Python SDK는 `eth_utils.to_hex(int)`로 r/s를 직렬화한다 — **선행 0 제거된 최소
    // hex**(예 0x53749d5b..., 63자). 32B 제로패딩 hex를 보내면 SDK 출력과 달라지므로
    // 동일 규칙으로 맞춘다. (서버 수용 형태를 SDK와 일치시킨다.)
    Ok(Signature {
        r: to_eth_hex(&r),
        s: to_eth_hex(&s),
        v,
    })
}

/// 32B big-endian을 Ethereum `to_hex(int)` 규칙으로 — 선행 0 바이트/니블 제거,
/// "0x" 접두. 전부 0이면 "0x0".
fn to_eth_hex(bytes: &[u8]) -> String {
    let hex = hex::encode(bytes);
    let trimmed = hex.trim_start_matches('0');
    if trimmed.is_empty() {
        "0x0".to_string()
    } else {
        format!("0x{trimmed}")
    }
}

/// 429 대기 시간(초). `Retry-After`(정수초) → 기본 1초. [1,300] 클램프.
fn retry_after_secs(resp: &reqwest::Response) -> u64 {
    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(1)
        .clamp(1, 300)
}

/// `/info` 응답(JSON)을 타입 T로 역직렬화. market/trade 모듈이 사용.
pub(crate) fn parse_json<T: DeserializeOwned>(v: Value) -> Result<T> {
    serde_json::from_value(v).map_err(HyperliquidError::Json)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_KEY: &str = "0x0123456789012345678901234567890123456789012345678901234567890123";

    // dummy action: 선언 순서 type → num. 실제 도메인엔 없는 합성 struct.
    // num은 u64 (float_to_int_for_hashing(1000) = 100000000000).
    #[derive(Serialize)]
    struct Dummy {
        #[serde(rename = "type")]
        kind: &'static str,
        num: u64,
    }

    // ── 서명 정확성 검증 (네트워크 없이) ──────────────────────────────────
    // hyperliquid-python-sdk tests/signing_test.py 공식 테스트 벡터.
    // msgpack→action_hash→EIP-712→ECDSA 전 파이프라인이 SDK와 바이트 단위로
    // 일치함을 검증한다. (order/cancel wire 자체 검증은 trade.rs 테스트에서.)
    #[test]
    fn sign_dummy_action_matches_sdk_vector() {
        let action = Dummy {
            kind: "dummy",
            num: 100000000000,
        };

        // mainnet (source="a")
        let sig = sign_l1_action(TEST_KEY, &action, 0, None, true).unwrap();
        assert_eq!(
            sig.r,
            "0x53749d5b30552aeb2fca34b530185976545bb22d0b3ce6f62e31be961a59298"
        );
        assert_eq!(
            sig.s,
            "0x755c40ba9bf05223521753995abb2f73ab3229be8ec921f350cb447e384d8ed8"
        );
        assert_eq!(sig.v, 27);

        // testnet (source="b")
        let sig_t = sign_l1_action(TEST_KEY, &action, 0, None, false).unwrap();
        assert_eq!(
            sig_t.r,
            "0x542af61ef1f429707e3c76c5293c80d01f74ef853e34b76efffcb57e574f9510"
        );
        assert_eq!(
            sig_t.s,
            "0x17b8b32f086e8cdede991f1e2c529f5dd5297cbe8128500e00cbaf766204a613"
        );
        assert_eq!(sig_t.v, 28);
    }

    #[test]
    fn action_hash_appends_vault_marker() {
        let action = Dummy {
            kind: "dummy",
            num: 1,
        };
        let no_vault = action_hash(&action, 0, None, None).unwrap();
        let with_vault =
            action_hash(&action, 0, Some("0x1234567890123456789012345678901234567890"), None)
                .unwrap();
        assert_ne!(no_vault, with_vault);
    }

    #[test]
    fn eth_hex_strips_leading_zeros_like_to_hex() {
        // 선행 0 바이트/니블을 eth_utils.to_hex와 동일하게 제거(나머지는 유지).
        // 32B = [0x05, 0x37, 0x00, ...] → "0x537" + 뒤따르는 0들.
        let mut b = [0u8; 32];
        b[0] = 0x05;
        b[1] = 0x37;
        assert_eq!(&to_eth_hex(&b)[..5], "0x537"); // 선행 0 니블 제거 확인
        assert!(!to_eth_hex(&b).starts_with("0x0"));
        // 선행 0이 없으면 그대로(64자 hex).
        let mut c = [0u8; 32];
        c[31] = 0x42;
        assert_eq!(to_eth_hex(&c), "0x42");
        // 전부 0.
        assert_eq!(to_eth_hex(&[0u8; 32]), "0x0");
    }

    #[test]
    fn vault_address_wrong_length_errors() {
        let action = Dummy {
            kind: "dummy",
            num: 1,
        };
        let err = action_hash(&action, 0, Some("0xdead"), None).unwrap_err();
        assert!(matches!(err, HyperliquidError::Sign(_)));
    }
}
