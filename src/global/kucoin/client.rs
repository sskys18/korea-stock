use std::time::Duration;

use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::Sha256;

use crate::global::kucoin::config::KucoinConfig;
use crate::global::kucoin::error::{KucoinError, Result};
use crate::ratelimit::RateLimiter;

type HmacSha256 = Hmac<Sha256>;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/api/v1/..." 경로.
    pub path: String,
    /// 요청 파라미터. GET=query 문자열로, POST=JSON body로 직렬화. **순서 보존**
    /// (서명 prehash와 전송 페이로드가 바이트 단위로 같아야 한다).
    pub params: Vec<(String, String)>,
    /// true면 `KC-API-*` 서명 헤더를 부착한다.
    pub signed: bool,
}

/// 내부 도메인 호출 명세. `params`는 순서 보존 key/value. GET은 query, POST는 body.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub params: Vec<(String, String)>,
    pub signed: bool,
}

impl ApiCall {
    /// 키 불필요 시세 호출 (GET).
    pub(crate) fn public_get(path: impl Into<String>, params: Vec<(String, String)>) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            params,
            signed: false,
        }
    }

    /// 서명 필요 조회 호출 (GET).
    pub(crate) fn signed_get(path: impl Into<String>, params: Vec<(String, String)>) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            params,
            signed: true,
        }
    }

    /// 서명 필요 변경 호출 (POST). KuCoin Futures 주문/취소는 POST/DELETE.
    pub(crate) fn signed_post(path: impl Into<String>, params: Vec<(String, String)>) -> Self {
        Self {
            method: Method::POST,
            path: path.into(),
            params,
            signed: true,
        }
    }

    /// 서명 필요 삭제 호출 (DELETE). KuCoin Futures 주문 취소는 DELETE.
    pub(crate) fn signed_delete(path: impl Into<String>, params: Vec<(String, String)>) -> Self {
        Self {
            method: Method::DELETE,
            path: path.into(),
            params,
            signed: true,
        }
    }
}

/// HTTP 응답 — KuCoin은 `{ code, data, msg }` envelope의 `data`를 보존한다.
#[derive(Debug)]
pub(crate) struct RawResponse {
    pub data: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.data.clone())?)
    }
}

/// KuCoin Futures 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 HMAC-SHA256 서명으로
/// 호출한다. 서명은 **헤더**(`KC-API-*`)로 보내며, prehash는
/// `timestamp + METHOD(대문자) + endpoint(?query 포함) + body`이다. SIGN·PASSPHRASE는
/// 모두 **base64(HMAC_SHA256(secret, ..))**.
pub struct KucoinClient {
    config: KucoinConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl KucoinClient {
    /// 클라이언트 생성.
    pub fn new(config: KucoinConfig) -> Result<Self> {
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

    /// 시세 도메인 액세서 (키 불필요).
    pub fn market(&self) -> crate::global::kucoin::market::Market<'_> {
        crate::global::kucoin::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (HMAC 서명).
    pub fn trade(&self) -> crate::global::kucoin::trade::Trade<'_> {
        crate::global::kucoin::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 envelope의 `data` raw JSON.
    pub async fn raw_call(&self, req: RawRequest) -> Result<Value> {
        let resp = self
            .call(ApiCall {
                method: req.method,
                path: req.path,
                params: req.params,
                signed: req.signed,
            })
            .await?;
        Ok(resp.data)
    }

    /// 현재 UTC epoch milliseconds. 서명 `KC-API-TIMESTAMP` 용.
    fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// 쿼리 파라미터를 `k=v&k=v` 문자열로 직렬화. **순서 보존** — 서명 prehash와 전송
    /// URL이 동일 문자열을 봐야 한다(삽입순 유지).
    fn encode_query(pairs: &[(String, String)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// `KC-API-SIGN`: base64(HMAC_SHA256(secret, timestamp + METHOD + endpoint + body)).
    ///
    /// `method`는 **대문자**, `endpoint`는 GET이면 `?query`까지 포함한 경로, `body`는 POST
    /// 본문 원문(GET이면 "")이다 — 전송 바이트와 prehash가 일치해야 한다.
    fn sign(secret: &str, timestamp: u64, method: &str, endpoint: &str, body: &str) -> Result<String> {
        let prehash = format!("{timestamp}{method}{endpoint}{body}");
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| KucoinError::Sign(e.to_string()))?;
        mac.update(prehash.as_bytes());
        Ok(base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
    }

    /// `KC-API-PASSPHRASE` (v2): base64(HMAC_SHA256(secret, rawPassphrase)).
    fn encrypt_passphrase(secret: &str, passphrase: &str) -> Result<String> {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| KucoinError::Sign(e.to_string()))?;
        mac.update(passphrase.as_bytes());
        Ok(base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
    }

    /// 32자 hex clientOid. KuCoin 주문 멱등 추적용 `clientOid`(필수)에 쓴다.
    pub(crate) fn client_oid() -> Result<String> {
        let mut buf = [0u8; 16];
        getrandom::getrandom(&mut buf).map_err(|e| KucoinError::Sign(e.to_string()))?;
        Ok(hex::encode(buf))
    }

    /// 도메인 공용 호출. 429 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(KucoinError::Api { http, .. }) if http == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 헤더/서명 조립 → 전송 → 본문 `code` 분기.
    ///
    /// **성공/실패를 본문 문자열 `code`로 판정**한다 (`"200000"`이 성공).
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let has_body = c.method == Method::POST || c.method == Method::PUT;
        let query = Self::encode_query(&c.params);

        // 서명 prehash의 endpoint는 GET이면 query까지 포함한다. body는 POST 본문 원문.
        // GET/DELETE는 query를 URL에, POST는 본문을 직렬화한다.
        let (endpoint, post_body): (String, Option<String>) = if has_body {
            let body = json_object_from_pairs(&c.params)?;
            (c.path.clone(), Some(body))
        } else if !query.is_empty() {
            (format!("{}?{}", c.path, query), None)
        } else {
            (c.path.clone(), None)
        };

        let url = format!("{}{}", self.config.base_url, endpoint);
        let mut req = self.http.request(c.method.clone(), &url);

        if c.signed {
            if self.config.api_key.is_empty()
                || self.config.api_secret.is_empty()
                || self.config.api_passphrase.is_empty()
            {
                return Err(KucoinError::Auth(
                    "signed endpoint requires api_key/api_secret/api_passphrase".into(),
                ));
            }
            let timestamp = Self::timestamp_ms();
            let body_str = post_body.as_deref().unwrap_or("");
            let signature = Self::sign(
                &self.config.api_secret,
                timestamp,
                c.method.as_str(),
                &endpoint,
                body_str,
            )?;
            let passphrase =
                Self::encrypt_passphrase(&self.config.api_secret, &self.config.api_passphrase)?;
            req = req
                .header("KC-API-KEY", &self.config.api_key)
                .header("KC-API-SIGN", signature)
                .header("KC-API-TIMESTAMP", timestamp.to_string())
                .header("KC-API-PASSPHRASE", passphrase)
                .header("KC-API-KEY-VERSION", "2");
        }

        // POST는 서명한 그 바이트열을 본문으로 부착한다(reqwest 재직렬화 금지).
        if let Some(body) = post_body {
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = retry_after_secs(&resp);
            tracing::warn!("429 — {wait}s 대기 후 재시도");
            tokio::time::sleep(Duration::from_secs(wait)).await;
            return Err(KucoinError::Api {
                http: 429,
                code: "429".into(),
                msg: "rate limit exceeded".into(),
            });
        }

        let http = status.as_u16();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        interpret_envelope(http, body)
    }
}

/// KuCoin Futures 요청 본문에서 **불리언**으로 직렬화할 키. 그 외는 [`NUMBER_KEYS`]에
/// 없으면 문자열.
const BOOL_KEYS: [&str; 2] = ["reduceOnly", "postOnly"];

/// KuCoin Futures 요청 본문에서 **JSON 숫자**로 직렬화할 키. 문서상 `size`(lots, int)와
/// `leverage`가 number 타입이다. price·심볼 등은 문자열로 보낸다(정밀도 보존).
const NUMBER_KEYS: [&str; 2] = ["size", "leverage"];

/// 순서 보존 key/value 쌍을 JSON object 본문 문자열로 직렬화한다. POST 서명·전송 공용.
/// 대부분 문자열로 보내되, [`BOOL_KEYS`]는 JSON 불리언, [`NUMBER_KEYS`]는 JSON 숫자로
/// 변환한다 — KuCoin Futures 주문 스키마(`size`/`leverage`=number, `reduceOnly`/
/// `postOnly`=bool)를 맞춘다.
fn json_object_from_pairs(pairs: &[(String, String)]) -> Result<String> {
    let mut map = serde_json::Map::with_capacity(pairs.len());
    for (k, v) in pairs {
        let value = if BOOL_KEYS.contains(&k.as_str()) {
            match v.as_str() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                other => Value::String(other.to_string()),
            }
        } else if NUMBER_KEYS.contains(&k.as_str()) {
            match serde_json::from_str::<serde_json::Number>(v) {
                Ok(n) => Value::Number(n),
                Err(_) => Value::String(v.clone()),
            }
        } else {
            Value::String(v.clone())
        };
        map.insert(k.clone(), value);
    }
    Ok(serde_json::to_string(&Value::Object(map))?)
}

/// KuCoin envelope `{ code, data, msg }`를 해석한다.
///
/// `code=="200000"`이면 `data`를 반환, 아니면 [`KucoinError::Api`]로 매핑한다.
/// `call_once`에서 분리해 단위 테스트가 검증 가능하도록 한다.
pub(crate) fn interpret_envelope(http: u16, body: Value) -> Result<RawResponse> {
    let code = body.get("code").and_then(Value::as_str);

    match code {
        Some("200000") => {
            let data = body.get("data").cloned().unwrap_or(Value::Null);
            Ok(RawResponse { data })
        }
        Some(other) => {
            let msg = body
                .get("msg")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| body.to_string());
            Err(KucoinError::Api {
                http,
                code: other.to_string(),
                msg,
            })
        }
        // envelope 형태가 아니면(비-JSON, 인프라 5xx 등) HTTP status로 폴백 판정.
        None => {
            if (200..300).contains(&http) {
                let data = body.get("data").cloned().unwrap_or(Value::Null);
                Ok(RawResponse { data })
            } else {
                let msg = if body.is_null() {
                    "empty/non-JSON response".to_string()
                } else {
                    body.to_string()
                };
                Err(KucoinError::Api {
                    http,
                    code: http.to_string(),
                    msg,
                })
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // KuCoin 서명 회귀 벡터 — 공식 문서가 공개한 숫자 키 벡터는 없어, 서명 규칙
    // (timestamp + METHOD + endpoint + body, HMAC-SHA256, **base64**)을 고정
    // 입력으로 손계산해 회귀 고정한다. 기대값은 bash openssl로 독립 계산:
    //   printf '%s' '1718000000000GET/api/v1/contracts/active' \
    //     | openssl dgst -sha256 -hmac 'test-api-secret' -binary | base64
    // → (아래 상수). 이 한 줄이 거래 안전의 핵심이다.
    #[test]
    fn sign_get_matches_independent_openssl_vector() {
        let sig = KucoinClient::sign(
            "test-api-secret",
            1718000000000,
            "GET",
            "/api/v1/contracts/active",
            "",
        )
        .unwrap();
        assert_eq!(sig, "nvMFy5pbkJnSdIGa1SVLOGRD/PHvJe/k3aKGa6Zj0dw=");
    }

    #[test]
    fn sign_post_matches_independent_openssl_vector() {
        // body 바이트가 prehash에 그대로 들어간다(POST).
        //   printf '%s' '1718000000000POST/api/v1/orders{"clientOid":"abc",...}' \
        //     | openssl dgst -sha256 -hmac 'test-api-secret' -binary | base64
        let body = r#"{"clientOid":"abc","symbol":"SAMSUNGUSDTM","side":"buy","type":"limit","size":1,"price":"233.00"}"#;
        let sig = KucoinClient::sign(
            "test-api-secret",
            1718000000000,
            "POST",
            "/api/v1/orders",
            body,
        )
        .unwrap();
        assert_eq!(sig, "IS0NtCrav2OrhnMyWRj4MMeGJbyKFN+OAtYgtTLrvQk=");
    }

    #[test]
    fn passphrase_v2_matches_independent_openssl_vector() {
        //   printf '%s' 'test-passphrase' \
        //     | openssl dgst -sha256 -hmac 'test-api-secret' -binary | base64
        let p = KucoinClient::encrypt_passphrase("test-api-secret", "test-passphrase").unwrap();
        assert_eq!(p, "fqkeR28GaLc+yzydx1yFfD3Jc46eSFoOwfG2YgL/Qos=");
    }

    #[test]
    fn sign_includes_query_in_endpoint() {
        // GET prehash endpoint는 ?query를 포함 → 다른 query면 서명이 달라져야 한다.
        let a =
            KucoinClient::sign("s", 1000, "GET", "/api/v1/orders?status=active", "").unwrap();
        let b = KucoinClient::sign("s", 1000, "GET", "/api/v1/orders?status=done", "").unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn post_body_coerces_size_leverage_to_number() {
        let pairs = vec![
            ("symbol".to_string(), "SAMSUNGUSDTM".to_string()),
            ("size".to_string(), "3".to_string()),
            ("leverage".to_string(), "5".to_string()),
            ("price".to_string(), "233.00".to_string()),
        ];
        let body = json_object_from_pairs(&pairs).unwrap();
        let v: Value = serde_json::from_str(&body).unwrap();
        assert!(v["size"].is_number());
        assert_eq!(v["size"], 3);
        assert!(v["leverage"].is_number());
        // price는 문자열(정밀도 보존), symbol도 문자열.
        assert!(v["price"].is_string());
        assert!(v["symbol"].is_string());
    }

    #[test]
    fn post_body_coerces_reduce_only_and_post_only_to_bool() {
        let pairs = vec![
            ("reduceOnly".to_string(), "true".to_string()),
            ("postOnly".to_string(), "false".to_string()),
        ];
        let body = json_object_from_pairs(&pairs).unwrap();
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["reduceOnly"], Value::Bool(true));
        assert_eq!(v["postOnly"], Value::Bool(false));
    }

    #[test]
    fn envelope_success_returns_data() {
        let body = json!({ "code": "200000", "data": { "orderId": "1" } });
        let r = interpret_envelope(200, body).unwrap();
        assert_eq!(r.data, json!({ "orderId": "1" }));
    }

    #[test]
    fn envelope_string_code_error() {
        // KuCoin code는 문자열 — "200000" 외는 에러. (404 not exist 등)
        let body = json!({ "code": "404", "msg": "not exist" });
        let err = interpret_envelope(404, body).unwrap_err();
        match err {
            KucoinError::Api { http, code, msg } => {
                assert_eq!(http, 404);
                assert_eq!(code, "404");
                assert_eq!(msg, "not exist");
            }
            _ => panic!("expected Api error"),
        }
    }
}
