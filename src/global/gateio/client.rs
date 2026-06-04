use std::time::Duration;

use hmac::{Hmac, Mac};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha512};

use crate::global::gateio::config::GateioConfig;
use crate::global::gateio::error::{GateioError, Result};
use crate::ratelimit::RateLimiter;

type HmacSha512 = Hmac<Sha512>;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// 프리픽스 포함 전체 경로 "/api/v4/futures/usdt/...". 서명 PATH로도 쓰인다.
    pub path: String,
    /// 쿼리 파라미터. 순서 보존(서명·전송이 동일 문자열을 봐야 한다).
    pub params: Vec<(String, String)>,
    /// 요청 본문(JSON 직렬화된 문자열). GET/공개는 `None`(빈 본문).
    pub body: Option<String>,
    /// true면 `KEY`/`Timestamp`/`SIGN` 서명 헤더를 부착한다.
    pub signed: bool,
}

/// 내부 도메인 호출 명세.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub params: Vec<(String, String)>,
    /// JSON 본문 문자열. **서명 해시와 전송 바이트가 동일해야 하므로** 한 번
    /// 직렬화한 이 문자열을 그대로 해시하고 그대로 전송한다.
    pub body: Option<String>,
    pub signed: bool,
}

impl ApiCall {
    /// 키 불필요 시세 호출 (GET, 빈 본문).
    pub(crate) fn public(
        method: Method,
        path: impl Into<String>,
        params: Vec<(String, String)>,
    ) -> Self {
        Self {
            method,
            path: path.into(),
            params,
            body: None,
            signed: false,
        }
    }

    /// 서명 필요 거래/계좌 호출.
    pub(crate) fn signed(
        method: Method,
        path: impl Into<String>,
        params: Vec<(String, String)>,
        body: Option<String>,
    ) -> Self {
        Self {
            method,
            path: path.into(),
            params,
            body,
            signed: true,
        }
    }
}

/// HTTP 응답 — Gate는 envelope 없이 페이로드를 직접 반환한다.
pub(crate) struct RawResponse {
    pub body: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.body.clone())?)
    }
}

/// Gate.io APIv4 USDT Futures 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 HMAC-SHA512 서명으로
/// 호출한다. 서명은 요청마다 `Timestamp`(초)를 새로 찍는다.
pub struct GateioClient {
    config: GateioConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl GateioClient {
    /// 클라이언트 생성.
    pub fn new(config: GateioConfig) -> Result<Self> {
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
    pub fn market(&self) -> crate::global::gateio::market::Market<'_> {
        crate::global::gateio::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (HMAC-SHA512 서명).
    pub fn trade(&self) -> crate::global::gateio::trade::Trade<'_> {
        crate::global::gateio::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 raw JSON.
    pub async fn raw_call(&self, req: RawRequest) -> Result<Value> {
        let resp = self
            .call(ApiCall {
                method: req.method,
                path: req.path,
                params: req.params,
                body: req.body,
                signed: req.signed,
            })
            .await?;
        Ok(resp.body)
    }

    /// 현재 UTC epoch **초**. Gate 서명 `Timestamp` 용 (Binance는 ms, Gate는 초).
    fn timestamp_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// 쿼리 파라미터를 `k=v&k=v` 문자열로 직렬화. **순서 보존** — 서명·전송이
    /// 동일 문자열을 봐야 한다.
    fn encode_query(pairs: &[(String, String)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// 본문의 hex(SHA512). 빈 본문은 빈 문자열을 해시한다(Gate 규약).
    fn hash_body(body: &str) -> String {
        let mut h = Sha512::new();
        h.update(body.as_bytes());
        hex::encode(h.finalize())
    }

    /// Gate APIv4 서명: `HEX(HMAC_SHA512(secret, sign_string))`.
    /// `sign_string = METHOD\nPATH\nQUERY\nHEX(SHA512(body))\nTIMESTAMP`.
    fn sign(secret: &str, sign_string: &str) -> Result<String> {
        let mut mac = HmacSha512::new_from_slice(secret.as_bytes())
            .map_err(|e| GateioError::Sign(e.to_string()))?;
        mac.update(sign_string.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    /// 도메인 공용 호출. 429(레이트) 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(GateioError::Api { status, .. }) if status == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → URL/본문/서명 조립 → 전송 → status 분기.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let query = Self::encode_query(&c.params);
        let url = format!("{}{}", self.config.base_url, c.path);
        let full_url = if query.is_empty() {
            url
        } else {
            format!("{url}?{query}")
        };

        // 본문은 한 번 직렬화한 문자열을 해시·전송에 **동일하게** 사용한다.
        let body = c.body.clone().unwrap_or_default();

        let mut req = self.http.request(c.method.clone(), &full_url);

        if c.signed {
            if self.config.api_key.is_empty() || self.config.api_secret.is_empty() {
                return Err(GateioError::Auth(
                    "signed endpoint requires api_key/api_secret".into(),
                ));
            }
            let ts = Self::timestamp_secs().to_string();
            let hashed = Self::hash_body(&body);
            // PATH는 프리픽스 포함 전체 경로, QUERY는 `?` 없는 raw.
            let sign_string =
                format!("{}\n{}\n{}\n{}\n{}", c.method.as_str(), c.path, query, hashed, ts);
            let sign = Self::sign(&self.config.api_secret, &sign_string)?;
            req = req
                .header("KEY", &self.config.api_key)
                .header("Timestamp", ts)
                .header("SIGN", sign);
        }

        if !body.is_empty() {
            // .json() 대신 .body() — 재직렬화로 바이트가 달라져 서명이 깨지는 것을 막는다.
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = retry_after_secs(&resp);
            tracing::warn!("429 — {wait}s 대기 후 재시도");
            let (label, message) = parse_api_error(resp).await;
            tokio::time::sleep(Duration::from_secs(wait)).await;
            return Err(GateioError::Api {
                status: 429,
                label,
                message,
            });
        }

        if status.is_success() {
            let body: Value = resp.json().await?;
            return Ok(RawResponse { body });
        }

        let http_status = status.as_u16();
        let (label, message) = parse_api_error(resp).await;
        Err(GateioError::Api {
            status: http_status,
            label,
            message,
        })
    }
}

/// Gate 에러 본문 `{label,message}` 파싱. 비-JSON이면 label="" + 원문.
async fn parse_api_error(resp: reqwest::Response) -> (String, String) {
    let text = resp.text().await.unwrap_or_default();
    match serde_json::from_str::<Value>(&text) {
        Ok(v) => {
            let label = v
                .get("label")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let message = v
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| v.get("detail").and_then(Value::as_str))
                .unwrap_or(&text)
                .to_string();
            (label, message)
        }
        Err(_) => (String::new(), text),
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

/// 최소 RFC 3986 unreserved 외 % 이스케이프. Gate 쿼리 값은 심볼·영숫자 위주라
/// 대부분 그대로지만, 안전을 위해 인코딩한다. **서명·전송 동일 인코딩을 보장.**
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 공개 상수: 빈 문자열의 hex(SHA512). Gate GET 요청의 body 해시로 쓰인다.
    /// 본문 해시 프리미티브가 정확함을 네트워크 없이 검증한다.
    #[test]
    fn empty_body_hash_matches_published_constant() {
        assert_eq!(
            GateioClient::hash_body(""),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
             47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
    }

    // 레퍼런스 구현(Gate 공식 `gen_sign` Python) 대조 벡터.
    // 고정 입력으로 Python `hashlib`/`hmac`이 산출한 SIGN을 하드코딩하고,
    // Rust `sha2`/`hmac` 구현이 동일 바이트를 재현하는지 검증한다.
    // 이것은 self-consistency가 아니라 교차구현(Python↔Rust) 바이트 일치다.
    //
    // 생성 스크립트(고정 입력):
    //   secret='mysecret'
    //   POST /api/v4/futures/usdt/orders  query=''
    //   body='{"contract":"SAMSUNG_USDT","size":3,"price":"0","tif":"ioc"}'
    //   ts='1780550000'
    #[test]
    fn sign_matches_python_reference_vector_post() {
        let secret = "mysecret";
        let method = "POST";
        let path = "/api/v4/futures/usdt/orders";
        let query = "";
        let body = r#"{"contract":"SAMSUNG_USDT","size":3,"price":"0","tif":"ioc"}"#;
        let ts = "1780550000";

        let hashed = GateioClient::hash_body(body);
        assert_eq!(
            hashed,
            "991dfd12f5ec147f48934f6084a583f4bd888a32656ff713006b91249bf323d7\
             634465d9da839194c7108f373e3699d850110b0b5dab27ca09de2ad3c169ce52"
        );

        let sign_string = format!("{method}\n{path}\n{query}\n{hashed}\n{ts}");
        let sign = GateioClient::sign(secret, &sign_string).unwrap();
        assert_eq!(
            sign,
            "14c45b0c120a7880160c38274aef1d20b2d0daa01421d80bd43e22cbd04791d3\
             832d1fe25cabd3e08c21dd2a992341cd3a2fee68fb099ea844f9a488078cc9f6"
        );
    }

    // GET(빈 본문 + 쿼리) 레퍼런스 벡터.
    //   GET /api/v4/futures/usdt/orders  query='contract=SAMSUNG_USDT&status=open'
    //   ts='1780550000'
    #[test]
    fn sign_matches_python_reference_vector_get() {
        let secret = "mysecret";
        let hashed = GateioClient::hash_body("");
        let sign_string = format!(
            "GET\n/api/v4/futures/usdt/orders\ncontract=SAMSUNG_USDT&status=open\n{hashed}\n1780550000"
        );
        let sign = GateioClient::sign(secret, &sign_string).unwrap();
        assert_eq!(
            sign,
            "0d04eab57dc72e673fe5690fc45a296db3c67931d07c2811c272aa6a0f17f161\
             ef185aaf2eeea6f94b50b53bd6301e864ccbe0e53599a750fc652d627328768f"
        );
    }

    #[test]
    fn encode_query_preserves_order() {
        let pairs = vec![
            ("contract".to_string(), "SAMSUNG_USDT".to_string()),
            ("limit".to_string(), "5".to_string()),
        ];
        assert_eq!(
            GateioClient::encode_query(&pairs),
            "contract=SAMSUNG_USDT&limit=5"
        );
    }

    #[test]
    fn urlencode_escapes_reserved() {
        assert_eq!(urlencode("A,B"), "A%2CB");
        assert_eq!(urlencode("SAMSUNG_USDT"), "SAMSUNG_USDT");
    }
}
