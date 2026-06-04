use std::time::Duration;

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::Sha256;

use crate::global::htx::config::HtxConfig;
use crate::global::htx::error::{HtxError, Result};
use crate::ratelimit::RateLimiter;

type HmacSha256 = Hmac<Sha256>;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/linear-swap-api/v1/..." 경로.
    pub path: String,
    /// 비서명 query 파라미터(GET 시세) 또는 POST 시 query에 합쳐질 추가 파라미터.
    pub query: Vec<(String, String)>,
    /// POST 본문(JSON). 서명 대상 아님. None이면 본문 없음.
    pub body: Option<Value>,
    /// true면 `AccessKeyId`/`SignatureMethod`/`SignatureVersion`/`Timestamp`/`Signature`를
    /// query에 부착한다.
    pub signed: bool,
}

/// 내부 도메인 호출 명세.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    /// 비서명 query 파라미터(시세 조회용). 서명 호출에서는 비어 있고 서명 4종만 붙는다.
    pub query: Vec<(String, String)>,
    /// POST 본문(JSON). HTX는 본문을 서명하지 않는다.
    pub body: Option<Value>,
    pub signed: bool,
}

impl ApiCall {
    /// 키 불필요 시세 호출 (GET + query).
    pub(crate) fn public(
        method: Method,
        path: impl Into<String>,
        query: Vec<(String, String)>,
    ) -> Self {
        Self {
            method,
            path: path.into(),
            query,
            body: None,
            signed: false,
        }
    }

    /// 서명 필요 거래/계좌 호출 (POST + JSON 본문). 서명 파라미터는 query로 간다.
    pub(crate) fn signed_post(path: impl Into<String>, body: Value) -> Self {
        Self {
            method: Method::POST,
            path: path.into(),
            query: Vec::new(),
            body: Some(body),
            signed: true,
        }
    }
}

/// HTTP 응답 — HTX는 envelope `{status, data|tick, err_code, err_msg}`의 페이로드를 보존.
#[derive(Debug)]
pub(crate) struct RawResponse {
    pub data: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.data.clone())?)
    }
}

/// HTX(Huobi) USDT-M Linear Swap 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 HMAC-SHA256 서명으로
/// 호출한다. HTX 서명은 **POST에도 GET-스타일 정규화 문자열**을 서명하며, 서명
/// 파라미터(`AccessKeyId`/`SignatureMethod`/`SignatureVersion`/`Timestamp`/`Signature`)는
/// **쿼리 스트링**으로 가고 JSON 본문은 서명하지 않는다.
pub struct HtxClient {
    config: HtxConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl HtxClient {
    /// 클라이언트 생성.
    pub fn new(config: HtxConfig) -> Result<Self> {
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
    pub fn market(&self) -> crate::global::htx::market::Market<'_> {
        crate::global::htx::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (HMAC 서명).
    pub fn trade(&self) -> crate::global::htx::trade::Trade<'_> {
        crate::global::htx::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 envelope의 페이로드 raw JSON.
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
        Ok(resp.data)
    }

    /// 현재 UTC 시각을 HTX 서명 `Timestamp` 포맷(`YYYY-MM-DDTHH:MM:SS`, UTC)으로.
    fn timestamp() -> String {
        Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string()
    }

    /// RFC 3986 unreserved 외 % 이스케이프. HTX prehash는 값뿐 아니라 키도
    /// 이 규칙으로 인코딩한 뒤 ASCII 키 사전순 정렬한다. 콜론(`:`)→`%3A` 등.
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

    /// HTX 서명 정규화 query: 각 (k,v)를 url-encode → **인코딩된 키 ASCII 사전순 정렬**
    /// → `k=v&k=v` 결합. 서명 대상이자 실제 전송 query와 동일 문자열이어야 한다.
    fn canonical_query(pairs: &[(String, String)]) -> String {
        let mut encoded: Vec<(String, String)> = pairs
            .iter()
            .map(|(k, v)| (Self::urlencode(k), Self::urlencode(v)))
            .collect();
        encoded.sort_by(|a, b| a.0.cmp(&b.0));
        encoded
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// HTX prehash: `METHOD\nHOST\nPATH\ncanonical_query` 4라인(개행 결합, 끝 개행 없음).
    /// HOST는 소문자 도메인부, PATH는 선행 슬래시 포함 전체 경로.
    fn prehash(method: &str, host: &str, path: &str, canonical_query: &str) -> String {
        format!("{method}\n{host}\n{path}\n{canonical_query}")
    }

    /// Base64(HMAC-SHA256(secret, prehash)). **hex가 아니라 Base64** — HTX 규칙.
    fn sign(secret: &str, prehash: &str) -> Result<String> {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| HtxError::Sign(e.to_string()))?;
        mac.update(prehash.as_bytes());
        Ok(BASE64.encode(mac.finalize().into_bytes()))
    }

    /// 도메인 공용 호출. 429 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(HtxError::Api { status, .. }) if status == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → query/서명 조립 → 전송 → envelope 분기.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        // 전송 query 문자열을 **우리 인코더로 직접** 만든다(reqwest의 query() 재인코딩에
        // 의존하지 않음). 서명 호출은 canonical_query(정렬+인코딩)가 곧 와이어 query라
        // 서명 대상과 전송 바이트가 정확히 일치한다.
        let query_string = if c.signed {
            if self.config.access_key.is_empty() || self.config.secret_key.is_empty() {
                return Err(HtxError::Auth(
                    "signed endpoint requires access_key/secret_key".into(),
                ));
            }
            let mut pairs = c.query.clone();
            pairs.push(("AccessKeyId".into(), self.config.access_key.clone()));
            pairs.push(("SignatureMethod".into(), "HmacSHA256".into()));
            pairs.push(("SignatureVersion".into(), "2".into()));
            pairs.push(("Timestamp".into(), Self::timestamp()));

            let canonical = Self::canonical_query(&pairs);
            let prehash = Self::prehash(c.method.as_str(), &self.config.host, &c.path, &canonical);
            let signature = Self::sign(&self.config.secret_key, &prehash)?;
            // Signature를 인코딩해 canonical 뒤에 부착(서명 자체는 서명 대상에서 제외).
            format!("{canonical}&Signature={}", Self::urlencode(&signature))
        } else {
            // 비서명 GET: 삽입순 유지(시세는 정렬 불필요), 동일 인코더로 직렬화.
            c.query
                .iter()
                .map(|(k, v)| format!("{}={}", Self::urlencode(k), Self::urlencode(v)))
                .collect::<Vec<_>>()
                .join("&")
        };

        let base = format!("{}{}", self.config.base_url, c.path);
        let url = if query_string.is_empty() {
            base
        } else {
            format!("{base}?{query_string}")
        };
        let mut req = self.http.request(c.method.clone(), &url);
        if let Some(body) = &c.body {
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .json(body);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = retry_after_secs(&resp);
            tracing::warn!("429 — {wait}s 대기 후 재시도");
            tokio::time::sleep(Duration::from_secs(wait)).await;
            return Err(HtxError::Api {
                status: 429,
                code: "429".into(),
                msg: "rate limit exceeded".into(),
            });
        }

        let http = status.as_u16();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        interpret_envelope(http, body)
    }
}

/// HTX envelope `{ status, data|tick, err_code|err-code, err_msg|err-msg }` 해석.
///
/// `status=="ok"`면 `data`(거래/계약정보) 또는 `tick`(시세 채널)을 반환, 아니면
/// [`HtxError::Api`]로 매핑한다. `call_once`에서 분리해 단위 테스트가 가능하도록 한다.
pub(crate) fn interpret_envelope(http: u16, body: Value) -> Result<RawResponse> {
    let status = body.get("status").and_then(Value::as_str);

    // status가 없으면(예: 비-JSON, 인프라 5xx) HTTP status로 폴백 판정.
    let ok = match status {
        Some(s) => s == "ok",
        None => (200..300).contains(&http),
    };

    if ok {
        // 시세 채널은 `tick`, REST API는 `data`. 둘 다 없으면 envelope 전체.
        let payload = body
            .get("data")
            .or_else(|| body.get("tick"))
            .cloned()
            .unwrap_or(body.clone());
        return Ok(RawResponse { data: payload });
    }

    let code = body
        .get("err_code")
        .or_else(|| body.get("err-code"))
        .map(|v| match v {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        })
        .unwrap_or_default();
    let msg = body
        .get("err_msg")
        .or_else(|| body.get("err-msg"))
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| {
            if body.is_null() {
                "empty/non-JSON response".to_string()
            } else {
                body.to_string()
            }
        });
    Err(HtxError::Api {
        status: http,
        code,
        msg,
    })
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

    // HTX 서명 self-consistency 벡터 (cross-tool).
    //
    // HTX 공식 문서는 access key·secret을 마스킹(`e2xxxxxx…`)해 검증 가능한 숫자
    // 테스트 벡터를 공개하지 않는다(spot/v1·usdt_swap/v1 모두 동일). 따라서 본
    // 어댑터의 prehash+HMAC 경로를, **독립 도구(openssl)**로 사전계산한 Base64
    // HMAC-SHA256 값과 대조한다. openssl 계산:
    //   SECRET='b0xxxxxx-c6xxxxxx-94xxxxxx-dxxxx'
    //   PREHASH=$'GET\napi.hbdm.com\n/linear-swap-api/v1/swap_contract_info\n'\
    //     'AccessKeyId=e2xxxxxx-99xxxxxx-84xxxxxx-7xxxx&SignatureMethod=HmacSHA256&'\
    //     'SignatureVersion=2&Timestamp=2017-05-11T15%3A19%3A30&contract_code=SAMSUNG-USDT'
    //   printf '%s' "$PREHASH" | openssl dgst -sha256 -hmac "$SECRET" -binary | openssl base64
    //   => oTeJdWvqRLyAo07/XbNaHgQKlgL4eU466P5rph/nAZI=
    // **주의: 이는 HTX 공식 발표 벡터가 아니라 openssl로 교차검증한 self-consistency
    // 벡터다(Rust HMAC ≡ openssl HMAC over the exact HTX prehash format).**
    const VEC_SECRET: &str = "b0xxxxxx-c6xxxxxx-94xxxxxx-dxxxx";
    const VEC_ACCESS: &str = "e2xxxxxx-99xxxxxx-84xxxxxx-7xxxx";
    const VEC_TIMESTAMP: &str = "2017-05-11T15:19:30";
    const VEC_EXPECTED_SIG: &str = "oTeJdWvqRLyAo07/XbNaHgQKlgL4eU466P5rph/nAZI=";

    #[test]
    fn sign_matches_openssl_cross_tool_vector() {
        // 서명 4종 + contract_code를 canonical_query로 만들어 동일 경로를 검증.
        let pairs = vec![
            ("contract_code".to_string(), "SAMSUNG-USDT".to_string()),
            ("AccessKeyId".to_string(), VEC_ACCESS.to_string()),
            ("SignatureMethod".to_string(), "HmacSHA256".to_string()),
            ("SignatureVersion".to_string(), "2".to_string()),
            ("Timestamp".to_string(), VEC_TIMESTAMP.to_string()),
        ];
        let canonical = HtxClient::canonical_query(&pairs);
        // ASCII 키 정렬: AccessKeyId < SignatureMethod < SignatureVersion < Timestamp < contract_code
        // (대문자 < 소문자). Timestamp의 콜론은 %3A로 인코딩된다.
        assert_eq!(
            canonical,
            "AccessKeyId=e2xxxxxx-99xxxxxx-84xxxxxx-7xxxx&SignatureMethod=HmacSHA256&\
             SignatureVersion=2&Timestamp=2017-05-11T15%3A19%3A30&contract_code=SAMSUNG-USDT"
        );

        let prehash = HtxClient::prehash(
            "GET",
            "api.hbdm.com",
            "/linear-swap-api/v1/swap_contract_info",
            &canonical,
        );
        assert_eq!(
            prehash,
            "GET\napi.hbdm.com\n/linear-swap-api/v1/swap_contract_info\n\
             AccessKeyId=e2xxxxxx-99xxxxxx-84xxxxxx-7xxxx&SignatureMethod=HmacSHA256&\
             SignatureVersion=2&Timestamp=2017-05-11T15%3A19%3A30&contract_code=SAMSUNG-USDT"
        );

        let sig = HtxClient::sign(VEC_SECRET, &prehash).unwrap();
        assert_eq!(sig, VEC_EXPECTED_SIG);
    }

    #[test]
    fn urlencode_escapes_colon_and_comma() {
        // Timestamp 콜론·배열 콤마가 인코딩되어야 prehash가 와이어와 일치.
        assert_eq!(HtxClient::urlencode("2017-05-11T15:19:30"), "2017-05-11T15%3A19%3A30");
        assert_eq!(HtxClient::urlencode("A,B"), "A%2CB");
        assert_eq!(HtxClient::urlencode("SAMSUNG-USDT"), "SAMSUNG-USDT");
    }

    #[test]
    fn canonical_query_sorts_by_encoded_key() {
        let pairs = vec![
            ("b".to_string(), "2".to_string()),
            ("a".to_string(), "1".to_string()),
        ];
        assert_eq!(HtxClient::canonical_query(&pairs), "a=1&b=2");
    }

    #[test]
    fn envelope_ok_returns_data() {
        let body = json!({ "status": "ok", "data": [{ "contract_code": "SAMSUNG-USDT" }] });
        let r = interpret_envelope(200, body).unwrap();
        assert_eq!(r.data, json!([{ "contract_code": "SAMSUNG-USDT" }]));
    }

    #[test]
    fn envelope_ok_returns_tick_for_market_channel() {
        let body = json!({ "ch": "market.SAMSUNG-USDT.detail.merged", "status": "ok",
            "tick": { "close": "235.38", "ask": [236.06, 3], "bid": [234.42, 21] } });
        let r = interpret_envelope(200, body).unwrap();
        assert_eq!(r.data["close"], json!("235.38"));
    }

    #[test]
    fn envelope_error_maps_to_api_err() {
        let body = json!({ "status": "error", "err_code": "1003", "err_msg": "invalid signature" });
        let err = interpret_envelope(200, body).unwrap_err();
        match err {
            HtxError::Api { status, code, msg } => {
                assert_eq!(status, 200);
                assert_eq!(code, "1003");
                assert_eq!(msg, "invalid signature");
            }
            _ => panic!("expected Api error"),
        }
    }

    #[test]
    fn envelope_error_hyphen_fields() {
        // 일부 시세 채널은 하이픈 키(err-code/err-msg)를 쓴다.
        let body = json!({ "status": "error", "err-code": "1010", "err-msg": "account not exist" });
        let err = interpret_envelope(200, body).unwrap_err();
        match err {
            HtxError::Api { code, msg, .. } => {
                assert_eq!(code, "1010");
                assert_eq!(msg, "account not exist");
            }
            _ => panic!("expected Api error"),
        }
    }
}
