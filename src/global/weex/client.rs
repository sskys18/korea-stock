use std::time::Duration;

use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::Sha256;

use crate::global::weex::config::WeexConfig;
use crate::global::weex::error::{Result, WeexError};
use crate::ratelimit::RateLimiter;

type HmacSha256 = Hmac<Sha256>;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/capi/v2/..." 경로 (쿼리 제외).
    pub path: String,
    /// 쿼리 파라미터 (GET). 순서 보존 — 서명·전송이 같은 문자열을 본다.
    pub query: Vec<(String, String)>,
    /// POST 바디 JSON (없으면 `Value::Null`).
    pub body: Value,
    /// true면 `ACCESS-*` 서명 헤더를 부착한다.
    pub signed: bool,
}

/// 내부 도메인 호출 명세.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Value,
    pub signed: bool,
}

impl ApiCall {
    /// 키 불필요 시세 호출 (GET).
    pub(crate) fn public_get(path: impl Into<String>, query: Vec<(String, String)>) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            query,
            body: Value::Null,
            signed: false,
        }
    }

    /// 서명 GET (조회).
    pub(crate) fn signed_get(path: impl Into<String>, query: Vec<(String, String)>) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            query,
            body: Value::Null,
            signed: true,
        }
    }

    /// 서명 POST (주문 등).
    pub(crate) fn signed_post(path: impl Into<String>, body: Value) -> Self {
        Self {
            method: Method::POST,
            path: path.into(),
            query: Vec::new(),
            body,
            signed: true,
        }
    }

}

/// HTTP 응답 — WEEX 시세는 envelope 없이 페이로드를 직접 반환한다(거래 응답도 동일).
pub(crate) struct RawResponse {
    pub body: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.body.clone())?)
    }
}

/// WEEX Contract 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 HMAC-SHA256 서명으로
/// 호출한다. 서명은 Bitget 계열 스킴: `Base64(HMAC_SHA256(secret,
/// timestamp + METHOD + requestPath[?query] + body))`. 매 요청마다 timestamp를
/// 새로 찍고 `ACCESS-KEY/SIGN/TIMESTAMP/PASSPHRASE` 헤더를 부착한다.
pub struct WeexClient {
    config: WeexConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl WeexClient {
    /// 클라이언트 생성.
    pub fn new(config: WeexConfig) -> Result<Self> {
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
    pub fn market(&self) -> crate::global::weex::market::Market<'_> {
        crate::global::weex::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (HMAC 서명).
    pub fn trade(&self) -> crate::global::weex::trade::Trade<'_> {
        crate::global::weex::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 raw JSON.
    pub async fn raw_call(&self, req: RawRequest) -> Result<Value> {
        let resp = self
            .call(ApiCall {
                method: req.method,
                path: req.path,
                query: req.query,
                body: req.body,
                signed: req.signed,
            })
            .await?;
        Ok(resp.body)
    }

    /// 현재 UTC epoch milliseconds. 서명 `ACCESS-TIMESTAMP` 용 (30초 후 만료).
    fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// 쿼리 파라미터를 `k=v&k=v` 문자열로 직렬화. **순서 보존** — 서명·전송이 동일
    /// 문자열을 봐야 한다.
    fn encode_query(pairs: &[(String, String)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// 서명 대상 prehash 문자열 조립:
    /// `timestamp + METHOD + requestPath (+ "?" + query, 있을 때만) + body`.
    /// GET은 body가 빈 문자열, POST는 query가 비어 compact JSON body가 붙는다.
    fn prehash(timestamp: u64, method: &Method, path: &str, query: &str, body: &str) -> String {
        let mut s = String::with_capacity(64 + path.len() + query.len() + body.len());
        s.push_str(&timestamp.to_string());
        s.push_str(method.as_str());
        s.push_str(path);
        if !query.is_empty() {
            s.push('?');
            s.push_str(query);
        }
        s.push_str(body);
        s
    }

    /// HMAC-SHA256(secret, prehash) → Base64(표준).
    fn sign(secret: &str, prehash: &str) -> Result<String> {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| WeexError::Sign(e.to_string()))?;
        mac.update(prehash.as_bytes());
        Ok(base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
    }

    /// 도메인 공용 호출. 429(레이트) 반응형 백오프 + 시세 stub(잘못된 JSON 문서
    /// 스텁) 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(WeexError::Api { status, .. }) if status == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                // WEEX CDN이 간헐적으로 시세 응답 대신 비-JSON "스키마 스텁"
                // (`{ base_volume: string, ... }`)을 돌려준다. 디코드 실패 시 재시도.
                Err(WeexError::Decode(_)) if attempt < MAX_RETRIES => {
                    tracing::warn!("decode/stub response — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(150)).await;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → URL/서명 조립 → 전송 → status 분기 → JSON 파싱.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let query = Self::encode_query(&c.query);
        // requestPath: 서명·전송 모두 쿼리 포함 경로를 본다.
        let request_path = if query.is_empty() {
            c.path.clone()
        } else {
            format!("{}?{}", c.path, query)
        };
        // POST/DELETE 바디: compact JSON (스페이스 없음). GET은 빈 문자열.
        let body_str = if c.body.is_null() {
            String::new()
        } else {
            serde_json::to_string(&c.body)?
        };

        let url = format!("{}{}", self.config.base_url, request_path);
        let mut req = self.http.request(c.method.clone(), &url);

        if c.signed {
            if self.config.api_key.is_empty()
                || self.config.api_secret.is_empty()
                || self.config.api_passphrase.is_empty()
            {
                return Err(WeexError::Auth(
                    "signed endpoint requires api_key/api_secret/api_passphrase".into(),
                ));
            }
            let ts = Self::timestamp_ms();
            // prehash는 query를 "?query"로 분리해 본다 (path는 query 미포함).
            let prehash = Self::prehash(ts, &c.method, &c.path, &query, &body_str);
            let signature = Self::sign(&self.config.api_secret, &prehash)?;
            req = req
                .header("ACCESS-KEY", &self.config.api_key)
                .header("ACCESS-SIGN", signature)
                .header("ACCESS-TIMESTAMP", ts.to_string())
                .header("ACCESS-PASSPHRASE", &self.config.api_passphrase);
        }

        if !body_str.is_empty() {
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body_str);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let (code, msg) = parse_api_error(resp).await;
            return Err(WeexError::Api {
                status: status.as_u16(),
                code,
                msg,
            });
        }

        if status.is_success() {
            let text = resp.text().await?;
            let body: Value = serde_json::from_str(&text)
                .map_err(|e| WeexError::Decode(format!("{e}: {}", truncate(&text, 200))))?;
            // 비즈니스 에러 envelope: `{code,msg,data}` with non-"00000" code.
            if let Some(code) = body.get("code").and_then(Value::as_str) {
                if code != "00000" && code != "0" {
                    let msg = body
                        .get("msg")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    return Err(WeexError::Api {
                        status: status.as_u16(),
                        code: code.to_string(),
                        msg,
                    });
                }
            }
            return Ok(RawResponse { body });
        }

        let http_status = status.as_u16();
        let (code, msg) = parse_api_error(resp).await;
        Err(WeexError::Api {
            status: http_status,
            code,
            msg,
        })
    }
}

/// WEEX 에러 본문 `{code,msg}` 파싱. `code`는 문자열. 비-JSON이면 code="" + 원문.
async fn parse_api_error(resp: reqwest::Response) -> (String, String) {
    let text = resp.text().await.unwrap_or_default();
    match serde_json::from_str::<Value>(&text) {
        Ok(v) => {
            let code = v
                .get("code")
                .map(|c| match c {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .unwrap_or_default();
            let msg = v
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or(&text)
                .to_string();
            (code, msg)
        }
        Err(_) => (String::new(), text),
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut end = n;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

/// 최소 RFC 3986 unreserved 외 % 이스케이프.
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

    // WEEX 공식 Signature 문서의 prehash 예시(GET) 구조를 그대로 재현해
    // 서명기 self-consistency를 네트워크 없이 고정한다.
    // 문서 예: timestamp=1591089508404, "GET" + "/api/v3/market/depth" + "?" +
    //          "symbol=BTCUSDT&limit=20" → prehash =
    //   "1591089508404GET/api/v3/market/depth?symbol=BTCUSDT&limit=20"
    #[test]
    fn prehash_matches_weex_doc_get_example() {
        let ts = 1591089508404u64;
        let ph = WeexClient::prehash(
            ts,
            &Method::GET,
            "/api/v3/market/depth",
            "symbol=BTCUSDT&limit=20",
            "",
        );
        assert_eq!(
            ph,
            "1591089508404GET/api/v3/market/depth?symbol=BTCUSDT&limit=20"
        );
    }

    #[test]
    fn prehash_post_example_no_query_compact_body() {
        // 문서 POST 예: "1561022985382POST/api/v3/order{...}".
        let ts = 1561022985382u64;
        let body = r#"{"symbol":"BTCUSDT","side":"BUY"}"#;
        let ph = WeexClient::prehash(ts, &Method::POST, "/api/v3/order", "", body);
        assert_eq!(
            ph,
            r#"1561022985382POST/api/v3/order{"symbol":"BTCUSDT","side":"BUY"}"#
        );
    }

    // 서명 round-trip: 고정 입력 → 고정 base64. 회귀 방지(known-answer).
    // 답은 독립 검산: `printf '%s' "<prehash>" | openssl dgst -sha256 -hmac
    // "test-secret-key" -binary | base64` 와 일치한다.
    #[test]
    fn sign_base64_known_answer() {
        let secret = "test-secret-key";
        let prehash = "1591089508404GET/api/v3/market/depth?symbol=BTCUSDT&limit=20";
        let sig = WeexClient::sign(secret, prehash).unwrap();
        // base64 표준 인코딩 출력(44자, SHA256 32바이트).
        assert_eq!(sig.len(), 44);
        // 결정적임을 보장 — 두 번 호출 동일.
        assert_eq!(sig, WeexClient::sign(secret, prehash).unwrap());
        // 고정 회귀 스냅샷 (openssl 검산값).
        assert_eq!(sig, "7u6rl8klWn9Q/dahijbLL8oLU6rNlUxO8II0ekgiQyE=");
    }

    #[test]
    fn encode_query_preserves_order() {
        let pairs = vec![
            ("symbol".to_string(), "cmt_samsungusdt".to_string()),
            ("limit".to_string(), "15".to_string()),
        ];
        assert_eq!(
            WeexClient::encode_query(&pairs),
            "symbol=cmt_samsungusdt&limit=15"
        );
    }
}
