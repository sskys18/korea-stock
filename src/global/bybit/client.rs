use std::time::Duration;

use hmac::{Hmac, Mac};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::Sha256;

use crate::global::bybit::config::BybitConfig;
use crate::global::bybit::error::{BybitError, Result};
use crate::ratelimit::RateLimiter;

type HmacSha256 = Hmac<Sha256>;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/v5/..." 경로.
    pub path: String,
    /// 요청 파라미터. GET=query 문자열로, POST=JSON body로 직렬화. **순서 보존**
    /// (서명 문자열과 전송 페이로드가 바이트 단위로 같아야 한다).
    pub params: Vec<(String, String)>,
    /// true면 `X-BAPI-*` 서명 헤더를 부착한다.
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

    /// 서명 필요 변경 호출 (POST). Bybit v5 거래는 모두 POST.
    pub(crate) fn signed_post(path: impl Into<String>, params: Vec<(String, String)>) -> Self {
        Self {
            method: Method::POST,
            path: path.into(),
            params,
            signed: true,
        }
    }
}

/// HTTP 응답 — Bybit v5는 `{ retCode, retMsg, result, retExtInfo, time }` envelope의
/// `result`를 보존한다.
#[derive(Debug)]
pub(crate) struct RawResponse {
    pub result: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.result.clone())?)
    }
}

/// Bybit v5 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 HMAC-SHA256 서명으로
/// 호출한다. 서명은 **헤더**(`X-BAPI-*`)로 보내며, 서명 대상은
/// `timestamp + api_key + recv_window + (queryString|rawBody)`이다.
pub struct BybitClient {
    config: BybitConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl BybitClient {
    /// 클라이언트 생성.
    pub fn new(config: BybitConfig) -> Result<Self> {
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
    pub fn market(&self) -> crate::global::bybit::market::Market<'_> {
        crate::global::bybit::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (HMAC 서명).
    pub fn trade(&self) -> crate::global::bybit::trade::Trade<'_> {
        crate::global::bybit::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 envelope의 `result` raw JSON.
    pub async fn raw_call(&self, req: RawRequest) -> Result<Value> {
        let resp = self
            .call(ApiCall {
                method: req.method,
                path: req.path,
                params: req.params,
                signed: req.signed,
            })
            .await?;
        Ok(resp.result)
    }

    /// 현재 UTC epoch milliseconds. 서명 `X-BAPI-TIMESTAMP` 용.
    fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// 쿼리 파라미터를 `k=v&k=v` 문자열로 직렬화. **순서 보존** — Bybit는 정렬을
    /// 요구하지 않으며, 서명과 전송이 동일 문자열을 봐야 한다(삽입순 유지).
    fn encode_query(pairs: &[(String, String)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// HMAC-SHA256(secret, timestamp + api_key + recv_window + param_string) → hex 소문자.
    ///
    /// `param_string`은 GET이면 [`Self::encode_query`]의 query 문자열, POST면 전송할
    /// **JSON 본문 원문**과 바이트 단위로 동일해야 한다(Bybit v5 서명 규칙).
    fn sign(
        secret: &str,
        timestamp: u64,
        api_key: &str,
        recv_window: u64,
        param_string: &str,
    ) -> Result<String> {
        let target = format!("{timestamp}{api_key}{recv_window}{param_string}");
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| BybitError::Sign(e.to_string()))?;
        mac.update(target.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    /// 도메인 공용 호출. 429 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(BybitError::Api { http, .. }) if http == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 헤더/서명 조립 → 전송 → 본문 `retCode` 분기.
    ///
    /// **성공/실패를 본문 `retCode`로 판정**한다(Binance의 HTTP status 방식과 다름).
    /// Bybit v5는 비즈니스 에러도 HTTP 200으로 내려준다.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let is_post = c.method == Method::POST;
        let base = format!("{}{}", self.config.base_url, c.path);

        // 서명 대상 param_string과 실제 전송 페이로드를 동일 소스에서 만든다.
        // GET: query 문자열. POST: JSON 본문 원문(직렬화 1회).
        let (param_string, post_body): (String, Option<String>) = if is_post {
            let body = json_object_from_pairs(&c.params)?;
            (body.clone(), Some(body))
        } else {
            (Self::encode_query(&c.params), None)
        };

        // GET은 서명한 query 문자열을 **그대로** URL에 붙인다(reqwest의 query()는
        // 퍼센트 인코딩을 다시 적용해 서명한 바이트와 어긋날 수 있으므로 직접 조립).
        let url = if !is_post && !param_string.is_empty() {
            format!("{base}?{param_string}")
        } else {
            base
        };

        let mut req = self.http.request(c.method.clone(), &url);

        if c.signed {
            if self.config.api_key.is_empty() || self.config.api_secret.is_empty() {
                return Err(BybitError::Auth(
                    "signed endpoint requires api_key/api_secret".into(),
                ));
            }
            let timestamp = Self::timestamp_ms();
            let signature = Self::sign(
                &self.config.api_secret,
                timestamp,
                &self.config.api_key,
                self.config.recv_window,
                &param_string,
            )?;
            req = req
                .header("X-BAPI-API-KEY", &self.config.api_key)
                .header("X-BAPI-TIMESTAMP", timestamp.to_string())
                .header("X-BAPI-RECV-WINDOW", self.config.recv_window.to_string())
                .header("X-BAPI-SIGN", signature);
        }

        // POST는 서명한 그 바이트열을 본문으로 부착한다(reqwest 재직렬화 금지).
        // GET은 위에서 query를 이미 URL에 박았다.
        if let Some(body) = post_body {
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body);
        }

        let resp = req.send().await?;
        let status = resp.status();

        // 429만 HTTP status로 선판정(레이트리밋은 envelope 밖일 수 있다).
        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = retry_after_secs(&resp);
            tracing::warn!("429 — {wait}s 대기 후 재시도");
            tokio::time::sleep(Duration::from_secs(wait)).await;
            return Err(BybitError::Api {
                http: 429,
                ret_code: 429,
                ret_msg: "rate limit exceeded".into(),
            });
        }

        let http = status.as_u16();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        interpret_envelope(http, body)
    }
}

/// Bybit v5 요청 본문에서 **불리언 타입**으로 직렬화해야 하는 키.
/// 그 외 모든 값(qty·price·symbol 등)은 문자열로 보낸다.
const BOOL_KEYS: [&str; 1] = ["reduceOnly"];

/// 순서 보존 key/value 쌍을 JSON object 본문 문자열로 직렬화한다. POST 서명·전송 공용.
/// 대부분 문자열로 보내되(Bybit v5는 qty/price 등을 문자열로 받는다), [`BOOL_KEYS`]에
/// 해당하는 키는 `"true"`/`"false"`를 **JSON 불리언**으로 변환한다 — Bybit는 `reduceOnly`
/// 등을 불리언으로 문서화한다.
fn json_object_from_pairs(pairs: &[(String, String)]) -> Result<String> {
    let mut map = serde_json::Map::with_capacity(pairs.len());
    for (k, v) in pairs {
        let value = if BOOL_KEYS.contains(&k.as_str()) {
            match v.as_str() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                other => Value::String(other.to_string()),
            }
        } else {
            Value::String(v.clone())
        };
        map.insert(k.clone(), value);
    }
    Ok(serde_json::to_string(&Value::Object(map))?)
}

/// Bybit v5 envelope `{ retCode, retMsg, result, retExtInfo, time }`를 해석한다.
///
/// `retCode==0`이면 `result`를 반환, 아니면 [`BybitError::Api`]로 매핑한다.
/// `call_once`에서 분리해 단위 테스트가 검증 가능하도록 한다.
pub(crate) fn interpret_envelope(http: u16, body: Value) -> Result<RawResponse> {
    let ret_code = body.get("retCode").and_then(Value::as_i64);

    match ret_code {
        Some(0) => {
            let result = body.get("result").cloned().unwrap_or(Value::Null);
            Ok(RawResponse { result })
        }
        Some(code) => {
            let ret_msg = body
                .get("retMsg")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| body.to_string());
            Err(BybitError::Api {
                http,
                ret_code: code,
                ret_msg,
            })
        }
        // envelope 형태가 아니면(비-JSON, 인프라 5xx 등) HTTP status로 폴백 판정.
        None => {
            if (200..300).contains(&http) {
                let result = body.get("result").cloned().unwrap_or(Value::Null);
                Ok(RawResponse { result })
            } else {
                let ret_msg = if body.is_null() {
                    "empty/non-JSON response".to_string()
                } else {
                    body.to_string()
                };
                Err(BybitError::Api {
                    http,
                    ret_code: 0,
                    ret_msg,
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

    // Bybit v5 서명 회귀 벡터.
    // 공식 문서가 공개한 (실키 포함) 숫자 테스트 벡터는 없어, 서명 규칙
    // (timestamp + api_key + recv_window + paramString, HMAC-SHA256, hex)을
    // 고정 입력으로 손계산하여 회귀 고정한다. 기대 hex는 bash openssl로 독립 계산:
    //   printf '1658384314791test-api-key5000category=linear&symbol=SAMSUNGUSDT' \
    //     | openssl dgst -sha256 -hmac 'test-api-secret' -hex
    // → (아래 상수). 이 한 줄이 거래 안전의 핵심이다.
    #[test]
    fn sign_matches_independent_openssl_vector() {
        let param_string = "category=linear&symbol=SAMSUNGUSDT";
        let sig = BybitClient::sign(
            "test-api-secret",
            1658384314791,
            "test-api-key",
            5000,
            param_string,
        )
        .unwrap();
        assert_eq!(
            sig,
            "144b79057aa3fbfef49590a726c47308807087f29820c0387a95cf5e18462b04"
        );
    }

    #[test]
    fn encode_query_preserves_insertion_order() {
        // Bybit는 정렬을 요구하지 않는다 — 삽입순 그대로.
        let pairs = vec![
            ("category".to_string(), "linear".to_string()),
            ("symbol".to_string(), "SAMSUNGUSDT".to_string()),
        ];
        assert_eq!(
            BybitClient::encode_query(&pairs),
            "category=linear&symbol=SAMSUNGUSDT"
        );
    }

    #[test]
    fn post_body_is_string_valued_json() {
        let pairs = vec![
            ("category".to_string(), "linear".to_string()),
            ("symbol".to_string(), "SAMSUNGUSDT".to_string()),
            ("side".to_string(), "Buy".to_string()),
        ];
        let body = json_object_from_pairs(&pairs).unwrap();
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["category"], "linear");
        assert_eq!(v["side"], "Buy");
        // 전부 문자열로 직렬화.
        assert!(v["symbol"].is_string());
    }

    #[test]
    fn post_body_coerces_reduce_only_to_bool() {
        // reduceOnly는 Bybit가 불리언으로 문서화한다 — 문자열이 아니라 JSON bool로.
        let pairs = vec![
            ("category".to_string(), "linear".to_string()),
            ("reduceOnly".to_string(), "true".to_string()),
        ];
        let body = json_object_from_pairs(&pairs).unwrap();
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["reduceOnly"], Value::Bool(true));
        assert!(v["reduceOnly"].is_boolean());
        // category는 여전히 문자열.
        assert!(v["category"].is_string());
    }

    #[test]
    fn sign_concatenates_ts_key_recvwindow_params() {
        // 순서가 바뀌면 값이 달라짐을 확인(구조 회귀).
        let a = BybitClient::sign("s", 1000, "AK", 5000, "x=1").unwrap();
        let b = BybitClient::sign("s", 1001, "AK", 5000, "x=1").unwrap();
        let c = BybitClient::sign("s", 1000, "AK", 6000, "x=1").unwrap();
        let d = BybitClient::sign("s", 1000, "AK2", 5000, "x=1").unwrap();
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn envelope_success_returns_result() {
        let body = json!({ "retCode": 0, "retMsg": "OK", "result": { "list": [1, 2] } });
        let r = interpret_envelope(200, body).unwrap();
        assert_eq!(r.result, json!({ "list": [1, 2] }));
    }

    #[test]
    fn envelope_business_error_on_http_200() {
        // 비즈니스 에러도 HTTP 200 — 본문 retCode로 판정해야 한다.
        let body = json!({ "retCode": 10001, "retMsg": "params error", "result": {} });
        let err = interpret_envelope(200, body).unwrap_err();
        match err {
            BybitError::Api {
                http,
                ret_code,
                ret_msg,
            } => {
                assert_eq!(http, 200);
                assert_eq!(ret_code, 10001);
                assert_eq!(ret_msg, "params error");
            }
            _ => panic!("expected Api error"),
        }
    }
}
