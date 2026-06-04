use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::global::bitunix::config::BitunixConfig;
use crate::global::bitunix::error::{BitunixError, Result};
use crate::ratelimit::RateLimiter;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/api/v1/futures/..." 경로.
    pub path: String,
    /// GET/DELETE=query 파라미터(스칼라), POST=JSON body로 직렬화. JSON object.
    pub params: Value,
    /// true면 `api-key`/`nonce`/`timestamp`/`sign` 헤더를 부착한다.
    pub signed: bool,
}

/// 내부 도메인 호출 명세. `params`는 JSON object이며 GET은 query, POST는 body로 쓴다.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub params: Value,
    pub signed: bool,
}

impl ApiCall {
    /// 키 불필요 시세 호출 (GET).
    pub(crate) fn public_get(path: impl Into<String>, params: Value) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            params,
            signed: false,
        }
    }

    /// 서명 필요 조회 호출 (GET).
    pub(crate) fn signed_get(path: impl Into<String>, params: Value) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            params,
            signed: true,
        }
    }

    /// 서명 필요 변경 호출 (POST). Bitunix Futures 거래는 모두 POST.
    pub(crate) fn signed_post(path: impl Into<String>, params: Value) -> Self {
        Self {
            method: Method::POST,
            path: path.into(),
            params,
            signed: true,
        }
    }
}

/// HTTP 응답 — Bitunix는 `{ code, data, msg }` envelope의 `data`를 보존.
#[derive(Debug)]
pub(crate) struct RawResponse {
    pub data: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.data.clone())?)
    }
}

/// Bitunix Futures 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 이중 SHA256 서명으로
/// 호출한다. 서명은 **헤더**(`api-key`/`nonce`/`timestamp`/`sign`)로 보낸다.
pub struct BitunixClient {
    config: BitunixConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl BitunixClient {
    /// 클라이언트 생성.
    pub fn new(config: BitunixConfig) -> Result<Self> {
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
    pub fn market(&self) -> crate::global::bitunix::market::Market<'_> {
        crate::global::bitunix::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (이중 SHA256 서명).
    pub fn trade(&self) -> crate::global::bitunix::trade::Trade<'_> {
        crate::global::bitunix::trade::Trade::new(self)
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

    /// 현재 UTC epoch milliseconds. 서명 `timestamp` 헤더 용.
    ///
    /// **포맷 주의:** Bitunix 공식 서명 문서는 `timestamp` 헤더를
    /// "Current timestamp, milliseconds"로 규정한다(=13자리 epoch-ms). 문서의 코드
    /// 예시(`"20241120123045"`)는 ms로 해석하면 서기 2611년이라 epoch-ms일 수 없는,
    /// 자릿수만 보여주는 placeholder다. 따라서 헤더 명세(epoch-ms)를 따른다. 거래
    /// 경로는 라이브 미검증이므로 만약 서버가 datetime 문자열을 요구하면 여기만 교체.
    fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// 32자리 소문자 hex 난수 nonce. 문서: "Random string, 32bits".
    fn nonce() -> Result<String> {
        let mut buf = [0u8; 16];
        getrandom::getrandom(&mut buf).map_err(|e| BitunixError::Sign(e.to_string()))?;
        Ok(hex::encode(buf))
    }

    /// queryParams 서명 문자열: 값이 null인 키는 제외하고, **키 ASCII 오름차순 정렬**
    /// 후 구분자 없이 `key`+`value`를 이어붙인다. 동시에 전송용 `(k,v)` 쌍도 반환한다
    /// (서명한 파라미터와 전송 파라미터가 같은 정렬 소스에서 나오게 한다).
    ///
    /// 예: `{id:1, uid:200}` → 서명 문자열 `"id1uid200"`, 전송 쌍 `[("id","1"),("uid","200")]`.
    /// (Binance의 `k=v&k=v`도 MEXC의 `k=v&k=v`도 아닌, **구분자 없는 k·v 연결**이다.)
    fn sorted_query(params: &Value) -> Result<(String, Vec<(String, String)>)> {
        let obj = params
            .as_object()
            .ok_or_else(|| BitunixError::Decode("query params must be a JSON object".into()))?;
        let mut pairs: Vec<(String, String)> = obj
            .iter()
            .filter(|(_, v)| !v.is_null())
            .map(|(k, v)| (k.clone(), json_scalar_to_string(v)))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let joined = pairs.iter().fold(String::new(), |mut s, (k, v)| {
            s.push_str(k);
            s.push_str(v);
            s
        });
        Ok((joined, pairs))
    }

    /// SHA256(input) → 소문자 hex 64자.
    fn sha256_hex(input: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Bitunix 이중 SHA256 서명.
    ///
    /// ```text
    /// digest = SHA256(nonce + timestamp + apiKey + queryParams + body)   // hex
    /// sign   = SHA256(digest_hex + secretKey)                            // hex
    /// ```
    ///
    /// `query_params`는 GET이면 [`Self::sorted_query`]의 정렬 연결 문자열, `body`는
    /// POST면 전송할 **JSON 본문 원문**(공백 없는 compact)이다. 한쪽이 비면 빈 문자열.
    /// **첫 digest의 hex 64자 문자열**을 그대로 secret 앞에 붙여 다시 해시한다(원시
    /// 바이트가 아니다 — 레퍼런스 Go/Python과 동일).
    fn sign(
        secret: &str,
        api_key: &str,
        nonce: &str,
        timestamp: u64,
        query_params: &str,
        body: &str,
    ) -> String {
        let digest_input = format!("{nonce}{timestamp}{api_key}{query_params}{body}");
        let digest = Self::sha256_hex(&digest_input);
        Self::sha256_hex(&format!("{digest}{secret}"))
    }

    /// 도메인 공용 호출. 429 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(BitunixError::Api { http, .. }) if http == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 헤더/서명 조립 → 전송 → 본문 `code` 분기.
    ///
    /// **성공/실패를 본문 `code==0`으로 판정**한다(MEXC식). Bitunix는 비즈니스
    /// 에러도 HTTP 200 + `{code,msg}`로 내려준다.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let is_post = c.method == Method::POST;
        let url = format!("{}{}", self.config.base_url, c.path);

        // 서명 대상 query_params/body와 실제 전송 페이로드를 동일 소스에서 만든다.
        // GET: 정렬 연결 query 문자열(서명) + (k,v) 쌍(전송). body 없음.
        // POST: query_params 없음 + JSON 본문 원문(직렬화 1회 — compact라 공백 없음).
        let (sign_query, query_pairs, sign_body, post_body): (
            String,
            Vec<(String, String)>,
            String,
            Option<String>,
        ) = if is_post {
            let body = serde_json::to_string(&c.params)?;
            (String::new(), Vec::new(), body.clone(), Some(body))
        } else {
            let (joined, pairs) = Self::sorted_query(&c.params)?;
            (joined, pairs, String::new(), None)
        };

        let mut req = self.http.request(c.method.clone(), &url);

        if c.signed {
            if self.config.api_key.is_empty() || self.config.api_secret.is_empty() {
                return Err(BitunixError::Auth(
                    "signed endpoint requires api_key/api_secret".into(),
                ));
            }
            let nonce = Self::nonce()?;
            let timestamp = Self::timestamp_ms();
            let sign = Self::sign(
                &self.config.api_secret,
                &self.config.api_key,
                &nonce,
                timestamp,
                &sign_query,
                &sign_body,
            );
            req = req
                .header("api-key", &self.config.api_key)
                .header("nonce", &nonce)
                .header("timestamp", timestamp.to_string())
                .header("sign", sign)
                .header("language", "en-US");
        }

        // 페이로드 부착: POST는 서명한 그 바이트열을 본문으로(reqwest 재직렬화 금지),
        // GET은 정렬된 query를 그대로.
        if let Some(body) = post_body {
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
        } else if !query_pairs.is_empty() {
            req = req.query(&query_pairs);
        }

        let resp = req.send().await?;
        let status = resp.status();

        // 429만 HTTP status로 선판정(레이트리밋은 envelope 밖일 수 있다).
        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = retry_after_secs(&resp);
            tracing::warn!("429 — {wait}s 대기 후 재시도");
            tokio::time::sleep(Duration::from_secs(wait)).await;
            return Err(BitunixError::Api {
                http: 429,
                code: 429,
                msg: "rate limit exceeded".into(),
            });
        }

        let http = status.as_u16();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        interpret_envelope(http, body)
    }
}

/// Bitunix envelope `{ code, data, msg }`를 해석한다.
///
/// `code==0`이면 `data`를 반환, 아니면 [`BitunixError::Api`]로 매핑한다. `call_once`에서
/// 분리해 단위 테스트가 검증 가능하도록 한다.
pub(crate) fn interpret_envelope(http: u16, body: Value) -> Result<RawResponse> {
    let code = body.get("code").and_then(Value::as_i64);

    let ok = match code {
        Some(c) => c == 0,
        // envelope 형태가 아니면(예: 비-JSON, 인프라 5xx) HTTP status로 폴백 판정.
        None => (200..300).contains(&http),
    };

    if ok {
        let data = body.get("data").cloned().unwrap_or(Value::Null);
        return Ok(RawResponse { data });
    }

    let msg = body
        .get("msg")
        .or_else(|| body.get("message"))
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| {
            if body.is_null() {
                "empty/non-JSON response".to_string()
            } else {
                body.to_string()
            }
        });
    Err(BitunixError::Api {
        http,
        code: code.unwrap_or(0),
        msg,
    })
}

/// JSON 스칼라를 query 파라미터 문자열로.
fn json_scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
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

    // Bitunix 이중 SHA256 서명 doc 벡터.
    //
    // 공식 문서(www.bitunix.com/api-docs/futures/common/sign.html)는 Go/Python
    // 레퍼런스 코드의 입력값만 공개하고 기대 출력은 싣지 않았다. 그 레퍼런스 코드는
    // 출력을 1:1로 결정하므로, 동일 입력을 본 구현(sha256_hex)에 통과시켜 회귀 고정한다.
    // **이 기대값은 공식 발표 벡터가 아니라 공식 레퍼런스 코드로 유도한 회귀 고정값이다.**
    #[test]
    fn sign_matches_bitunix_doc_reference_vector() {
        // **프로덕션 sign()을 직접 호출**해 컴포넌트 순서까지 벡터로 고정한다(수기
        // 재구성이 아님 — sign()의 format! 순서가 바뀌면 이 테스트가 깨져야 한다).
        // doc 예시 timestamp `20241120123045`는 u64에 들어가고 to_string이 동일
        // 문자열을 내므로 그대로 흘려보낼 수 있다.
        let sign = BitunixClient::sign(
            "yourSecretKey", // secret
            "yourApiKey",    // api_key
            "123456",        // nonce
            20241120123045,  // timestamp → "20241120123045"
            "id1uid200",     // query_params
            r#"{"uid":"2899","arr":[{"id":1,"name":"maple"},{"id":2,"name":"lily"}]}"#,
        );
        // 레퍼런스 Go/Python 코드가 유일하게 결정하는 출력.
        assert_eq!(
            sign,
            "00397cd1e52c7dce3258067324363b6361fabc9178a0912b330c138db8745655"
        );
    }

    /// queryParams 빌더(정렬+구분자 없는 연결)를 독립 검증한다. doc 예시 `{id:1,uid:200}`
    /// → `"id1uid200"`. 이 변환이 서명 정확성의 진짜 위험 지점이다(합성 벡터로는 미검증).
    #[test]
    fn sorted_query_concatenates_sorted_kv_no_separator() {
        let (s, pairs) = BitunixClient::sorted_query(&json!({ "uid": 200, "id": 1 })).unwrap();
        assert_eq!(s, "id1uid200");
        // 전송 쌍도 정렬되어 있어야 한다(서명 소스와 동일).
        assert_eq!(
            pairs,
            vec![
                ("id".to_string(), "1".to_string()),
                ("uid".to_string(), "200".to_string())
            ]
        );
    }

    #[test]
    fn sorted_query_drops_null_keys() {
        let (s, pairs) =
            BitunixClient::sorted_query(&json!({ "symbol": "SAMSUNGUSDT", "limit": Value::Null }))
                .unwrap();
        assert_eq!(s, "symbolSAMSUNGUSDT");
        assert!(!pairs.iter().any(|(k, _)| k == "limit"));
    }

    /// digest는 hex 64자 **문자열**로 secret 앞에 붙는다(원시 바이트 아님). 구조 회귀.
    #[test]
    fn sign_feeds_hex_digest_into_second_hash() {
        let digest = BitunixClient::sha256_hex("abc");
        assert_eq!(digest.len(), 64);
        let full = BitunixClient::sign("sec", "ak", "n", 1000, "x1", "");
        // 입력 한 글자만 바꿔도 값이 달라짐(서명이 모든 컴포넌트에 의존).
        let other = BitunixClient::sign("sec", "ak", "n", 1001, "x1", "");
        assert_ne!(full, other);
        let other2 = BitunixClient::sign("sec2", "ak", "n", 1000, "x1", "");
        assert_ne!(full, other2);
    }

    #[test]
    fn post_body_signs_compact_json_no_spaces() {
        // serde_json::to_string은 compact(공백 없음) — "remove all spaces" 충족.
        let body = serde_json::to_string(&json!({ "symbol": "BTCUSDT", "qty": "1" })).unwrap();
        assert!(!body.contains(' '));
    }

    #[test]
    fn nonce_is_32_hex_chars() {
        let n = BitunixClient::nonce().unwrap();
        assert_eq!(n.len(), 32);
        assert!(n.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn envelope_success_returns_data() {
        let body = json!({ "code": 0, "data": { "x": 1 }, "msg": "Success" });
        let r = interpret_envelope(200, body).unwrap();
        assert_eq!(r.data, json!({ "x": 1 }));
    }

    #[test]
    fn envelope_business_error_on_http_200() {
        // 비즈니스 에러도 HTTP 200 — 본문 code로 판정해야 한다.
        let body = json!({ "code": 10007, "msg": "param invalid", "data": null });
        let err = interpret_envelope(200, body).unwrap_err();
        match err {
            BitunixError::Api { http, code, msg } => {
                assert_eq!(http, 200);
                assert_eq!(code, 10007);
                assert_eq!(msg, "param invalid");
            }
            _ => panic!("expected Api error"),
        }
    }
}
