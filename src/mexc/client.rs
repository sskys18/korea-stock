use std::time::Duration;

use hmac::{Hmac, Mac};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::Sha256;

use crate::mexc::config::MexcConfig;
use crate::mexc::error::{MexcError, Result};
use crate::ratelimit::RateLimiter;

type HmacSha256 = Hmac<Sha256>;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/api/v1/..." 경로.
    pub path: String,
    /// GET/DELETE=query 파라미터(스칼라 문자열), POST=JSON body로 직렬화. JSON object.
    pub params: Value,
    /// true면 `ApiKey`/`Request-Time`/`Signature` 헤더를 부착한다.
    pub signed: bool,
}

/// 내부 도메인 호출 명세. `params`는 JSON object이며 GET/DELETE는 query, POST는 body로 쓴다.
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

    /// 서명 필요 변경 호출 (POST). MEXC Contract 거래는 모두 POST.
    pub(crate) fn signed_post(path: impl Into<String>, params: Value) -> Self {
        Self {
            method: Method::POST,
            path: path.into(),
            params,
            signed: true,
        }
    }
}

/// HTTP 응답 — MEXC Contract는 `{ success, code, data }` envelope의 `data`를 보존.
#[derive(Debug)]
pub(crate) struct RawResponse {
    pub data: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.data.clone())?)
    }
}

/// MEXC Contract(선물) 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 HMAC-SHA256 서명으로
/// 호출한다. 서명은 **헤더**(`ApiKey`/`Request-Time`/`Signature`)로 보내며, 서명
/// 대상은 `accessKey + reqTime + paramString`이다.
pub struct MexcClient {
    config: MexcConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl MexcClient {
    /// 클라이언트 생성.
    pub fn new(config: MexcConfig) -> Result<Self> {
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
    pub fn market(&self) -> crate::mexc::market::Market<'_> {
        crate::mexc::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (HMAC 서명).
    pub fn trade(&self) -> crate::mexc::trade::Trade<'_> {
        crate::mexc::trade::Trade::new(self)
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

    /// 현재 UTC epoch milliseconds. 서명 `Request-Time` 용.
    fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// GET/DELETE 파라미터 문자열: 값이 null인 키는 제외하고, **키 사전순 정렬** 후
    /// `k=v&k=v`로 결합. MEXC는 서명·전송 모두 이 정렬된 문자열을 봐야 한다
    /// (Binance는 삽입순 유지 — MEXC는 사전순 정렬이 필수다).
    fn sorted_query(params: &Value) -> Result<(String, Vec<(String, String)>)> {
        let obj = params
            .as_object()
            .ok_or_else(|| MexcError::Decode("query params must be a JSON object".into()))?;
        let mut pairs: Vec<(String, String)> = obj
            .iter()
            .filter(|(_, v)| !v.is_null())
            .map(|(k, v)| (k.clone(), json_scalar_to_string(v)))
            .collect();
        pairs.sort_by(|a, b| a.0.cmp(&b.0));
        let joined = pairs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&");
        Ok((joined, pairs))
    }

    /// HMAC-SHA256(secret, accessKey + reqTime + paramString) → hex 소문자.
    ///
    /// `param_string`은 GET/DELETE면 [`Self::sorted_query`]의 정렬 결합 문자열,
    /// POST면 전송할 **JSON 본문 원문**과 바이트 단위로 동일해야 한다.
    fn sign(secret: &str, access_key: &str, req_time: u64, param_string: &str) -> Result<String> {
        let target = format!("{access_key}{req_time}{param_string}");
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| MexcError::Sign(e.to_string()))?;
        mac.update(target.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    /// 도메인 공용 호출. 429 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(MexcError::Api { http, .. }) if http == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 헤더/서명 조립 → 전송 → 본문 `success` 분기.
    ///
    /// **성공/실패를 본문 `success`/`code`로 판정**한다(Binance의 HTTP status 방식과 다름).
    /// MEXC Contract는 비즈니스 에러도 HTTP 200으로 내려준다.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let is_post = c.method == Method::POST;
        let url = format!("{}{}", self.config.base_url, c.path);

        // 서명 대상 param_string과 실제 전송 페이로드를 동일 소스에서 만든다.
        // GET/DELETE: 정렬 query 문자열. POST: JSON 본문 원문(직렬화 1회).
        let (param_string, query_pairs, post_body): (String, Vec<(String, String)>, Option<String>) =
            if is_post {
                let body = serde_json::to_string(&c.params)?;
                (body.clone(), Vec::new(), Some(body))
            } else {
                let (joined, pairs) = Self::sorted_query(&c.params)?;
                (joined, pairs, None)
            };

        let mut req = self.http.request(c.method.clone(), &url);

        if c.signed {
            if self.config.access_key.is_empty() || self.config.secret_key.is_empty() {
                return Err(MexcError::Auth(
                    "signed endpoint requires access_key/secret_key".into(),
                ));
            }
            let req_time = Self::timestamp_ms();
            let signature = Self::sign(
                &self.config.secret_key,
                &self.config.access_key,
                req_time,
                &param_string,
            )?;
            req = req
                .header("ApiKey", &self.config.access_key)
                .header("Request-Time", req_time.to_string())
                .header("Recv-Window", self.config.recv_window.to_string())
                .header("Signature", signature);
        }

        // 페이로드 부착: POST는 서명한 그 바이트열을 본문으로(reqwest 재직렬화 금지),
        // GET/DELETE는 정렬된 query를 그대로.
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
            return Err(MexcError::Api {
                http: 429,
                code: 429,
                message: "rate limit exceeded".into(),
            });
        }

        let http = status.as_u16();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        interpret_envelope(http, body)
    }
}

/// MEXC Contract envelope `{ success, code, data, message|msg }`를 해석한다.
///
/// `success==true`(또는 `code==0`)면 `data`를 반환, 아니면 [`MexcError::Api`]로 매핑한다.
/// `call_once`에서 분리해 단위 테스트가 검증 가능하도록 한다.
pub(crate) fn interpret_envelope(http: u16, body: Value) -> Result<RawResponse> {
    let success = body.get("success").and_then(Value::as_bool);
    let code = body.get("code").and_then(Value::as_i64);

    let ok = match (success, code) {
        (Some(s), _) => s,
        (None, Some(c)) => c == 0,
        // envelope 형태가 아니면(예: 비-JSON, 인프라 5xx) HTTP status로 폴백 판정.
        (None, None) => (200..300).contains(&http),
    };

    if ok {
        let data = body.get("data").cloned().unwrap_or(Value::Null);
        return Ok(RawResponse { data });
    }

    let message = body
        .get("message")
        .or_else(|| body.get("msg"))
        .and_then(Value::as_str)
        .map(String::from)
        .unwrap_or_else(|| {
            if body.is_null() {
                "empty/non-JSON response".to_string()
            } else {
                body.to_string()
            }
        });
    Err(MexcError::Api {
        http,
        code: code.unwrap_or(0),
        message,
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

    // MEXC Contract 서명 doc-vector.
    // 공식 문서가 공개한 숫자 테스트 벡터는 없어, 서명 규칙(accessKey+reqTime+paramString,
    // HMAC-SHA256, hex)을 고정 입력으로 손계산하여 회귀 고정한다.
    // **주의: 이 기대값은 MEXC 공식 발표 벡터가 아니라 본 구현의 회귀 고정값이다.**
    // HMAC-SHA256("mexc_secret", "mexc_access" + "1609740600000" +
    //   "price=33016.5&symbol=BTC_USDT&vol=1") 의 hex.
    #[test]
    fn sign_regression_vector() {
        // GET 정렬 문자열을 sorted_query로 만들어 동일 경로를 검증.
        let params = json!({ "symbol": "BTC_USDT", "vol": 1, "price": 33016.5 });
        let (param_string, _) = MexcClient::sorted_query(&params).unwrap();
        // 사전순: price < symbol < vol.
        assert_eq!(param_string, "price=33016.5&symbol=BTC_USDT&vol=1");

        let sig = MexcClient::sign("mexc_secret", "mexc_access", 1609740600000, &param_string)
            .unwrap();
        assert_eq!(
            sig,
            "be059b28daa937e484ae703505a6988725b5516dc2e921a66f9823b30b25229d"
        );
    }

    #[test]
    fn sorted_query_drops_null_and_sorts() {
        let params = json!({ "symbol": "SAMSUNG_USDT", "page": 2, "opt": Value::Null });
        let (s, pairs) = MexcClient::sorted_query(&params).unwrap();
        assert_eq!(s, "page=2&symbol=SAMSUNG_USDT");
        assert!(!pairs.iter().any(|(k, _)| k == "opt"));
    }

    #[test]
    fn sign_concatenates_access_time_params() {
        // accessKey+reqTime+paramString 순서가 바뀌면 값이 달라짐을 확인(구조 회귀).
        let a = MexcClient::sign("s", "AK", 1000, "x=1").unwrap();
        let b = MexcClient::sign("s", "AK", 1001, "x=1").unwrap();
        let c = MexcClient::sign("s", "AK2", 1000, "x=1").unwrap();
        assert_ne!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn envelope_success_returns_data() {
        let body = json!({ "success": true, "code": 0, "data": { "x": 1 } });
        let r = interpret_envelope(200, body).unwrap();
        assert_eq!(r.data, json!({ "x": 1 }));
    }

    #[test]
    fn envelope_business_error_on_http_200() {
        // 비즈니스 에러도 HTTP 200 — 본문으로 판정해야 한다.
        let body = json!({ "success": false, "code": 1002, "message": "contract not exist" });
        let err = interpret_envelope(200, body).unwrap_err();
        match err {
            MexcError::Api { http, code, message } => {
                assert_eq!(http, 200);
                assert_eq!(code, 1002);
                assert_eq!(message, "contract not exist");
            }
            _ => panic!("expected Api error"),
        }
    }

    #[test]
    fn envelope_code_zero_without_success_is_ok() {
        let body = json!({ "code": 0, "data": [1, 2, 3] });
        let r = interpret_envelope(200, body).unwrap();
        assert_eq!(r.data, json!([1, 2, 3]));
    }
}
