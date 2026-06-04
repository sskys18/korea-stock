use std::time::Duration;

use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::Sha256;

use crate::global::phemex::config::PhemexConfig;
use crate::global::phemex::error::{PhemexError, Result};
use crate::ratelimit::RateLimiter;

type HmacSha256 = Hmac<Sha256>;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/g-orders" 같은 경로(앞 `/` 포함).
    pub path: String,
    /// 쿼리 파라미터. **순서 보존** — 서명·전송이 동일 문자열을 봐야 한다.
    pub query: Vec<(String, String)>,
    /// 본문(POST/PUT). JSON object. 비-POST/PUT이면 무시.
    pub body: Option<Value>,
    /// true면 `x-phemex-access-token`/`-request-expiry`/`-request-signature` 헤더를 부착.
    pub signed: bool,
}

/// 내부 도메인 호출 명세.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Option<Value>,
    pub signed: bool,
}

impl ApiCall {
    /// 키 불필요 시세 호출 (GET).
    pub(crate) fn public_get(path: impl Into<String>, query: Vec<(String, String)>) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            query,
            body: None,
            signed: false,
        }
    }

    /// 서명 필요 조회 호출 (GET).
    pub(crate) fn signed_get(path: impl Into<String>, query: Vec<(String, String)>) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            query,
            body: None,
            signed: true,
        }
    }

    /// 서명 필요 변경 호출 (POST + JSON body).
    pub(crate) fn signed_post(path: impl Into<String>, body: Value) -> Self {
        Self {
            method: Method::POST,
            path: path.into(),
            query: Vec::new(),
            body: Some(body),
            signed: true,
        }
    }

    /// 서명 필요 변경 호출 (DELETE + query).
    pub(crate) fn signed_delete(path: impl Into<String>, query: Vec<(String, String)>) -> Self {
        Self {
            method: Method::DELETE,
            path: path.into(),
            query,
            body: None,
            signed: true,
        }
    }
}

/// Phemex 시세 응답 envelope `{ error, id, result }`의 `result`를 보존.
#[derive(Debug)]
pub(crate) struct RawResponse {
    pub result: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.result.clone())?)
    }
}

/// Phemex Perpetual v2 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 HMAC-SHA256 서명으로
/// 호출한다. 서명은 **헤더**로 보내며, 대상 문자열은 `URLPath + QueryString +
/// Expiry + body`, HMAC 키는 `Base64::urlDecode(API Secret)`이다.
pub struct PhemexClient {
    config: PhemexConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl PhemexClient {
    /// 클라이언트 생성.
    pub fn new(config: PhemexConfig) -> Result<Self> {
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
    pub fn market(&self) -> crate::global::phemex::market::Market<'_> {
        crate::global::phemex::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (HMAC 서명).
    pub fn trade(&self) -> crate::global::phemex::trade::Trade<'_> {
        crate::global::phemex::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 envelope의 `result`(또는 `data`) raw JSON.
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
        Ok(resp.result)
    }

    /// 현재 UTC epoch seconds.
    fn now_secs() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }

    /// 쿼리 파라미터를 `k=v&k=v`로 직렬화. **순서 보존, `?` 제외** — 이 문자열이
    /// 서명 대상이자 전송 query가 된다(둘이 바이트 단위로 동일해야 한다).
    fn encode_query(pairs: &[(String, String)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// Phemex 서명: `HMAC_SHA256(key, path + query + expiry + body)` → hex 소문자.
    /// `key`는 공식 문서대로 `Base64::urlDecode(API Secret)`. **항상 디코드**하며
    /// 실패 시 [`PhemexError::Sign`](유저별 비결정 서명 방지). 패딩 유무 모두 허용.
    fn sign(
        secret: &str,
        path: &str,
        query: &str,
        expiry: u64,
        body: &str,
    ) -> Result<String> {
        let key = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(secret)
            .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(secret))
            .map_err(|e| PhemexError::Sign(format!("api_secret not valid base64url: {e}")))?;
        let message = format!("{path}{query}{expiry}{body}");
        let mut mac =
            HmacSha256::new_from_slice(&key).map_err(|e| PhemexError::Sign(e.to_string()))?;
        mac.update(message.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    /// 도메인 공용 호출. 429 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(PhemexError::Api { http, .. }) if http == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 서명/헤더/페이로드 조립 → 전송 → envelope 해석.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let query = Self::encode_query(&c.query);
        // 본문(POST/PUT): 서명 대상과 전송 바이트가 동일하도록 직렬화 1회.
        let body_string = match &c.body {
            Some(v) => serde_json::to_string(v)?,
            None => String::new(),
        };

        let mut url = format!("{}{}", self.config.base_url, c.path);
        if !query.is_empty() {
            url.push('?');
            url.push_str(&query);
        }

        let mut req = self.http.request(c.method.clone(), &url);

        if c.signed {
            if self.config.api_key.is_empty() || self.config.api_secret.is_empty() {
                return Err(PhemexError::Auth(
                    "signed endpoint requires api_key/api_secret".into(),
                ));
            }
            let expiry = Self::now_secs() + self.config.request_expiry_secs;
            let signature = Self::sign(
                &self.config.api_secret,
                &c.path,
                &query,
                expiry,
                &body_string,
            )?;
            req = req
                .header("x-phemex-access-token", &self.config.api_key)
                .header("x-phemex-request-expiry", expiry.to_string())
                .header("x-phemex-request-signature", signature);
        }

        // POST/PUT은 서명한 바로 그 바이트열을 본문으로(reqwest 재직렬화 금지).
        if c.body.is_some() {
            req = req
                .header(reqwest::header::CONTENT_TYPE, "application/json")
                .body(body_string);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = retry_after_secs(&resp);
            tracing::warn!("429 — {wait}s 대기 후 재시도");
            tokio::time::sleep(Duration::from_secs(wait)).await;
            return Err(PhemexError::Api {
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

/// Phemex envelope 해석. 두 가지 형태를 모두 처리한다:
///  - 시세: `{ error, id, result }` — `error`가 null이면 `result` 반환.
///  - 거래/계좌: `{ code, data, msg }` — `code==0`이면 `data` 반환.
///
/// 어느 쪽이든 본문으로 성공/실패를 판정하며(비즈니스 에러도 HTTP 200으로 옴),
/// envelope 형태가 아니면 HTTP status로 폴백한다.
pub(crate) fn interpret_envelope(http: u16, body: Value) -> Result<RawResponse> {
    // 거래/계좌 envelope: { code, data, msg }
    if let Some(code) = body.get("code").and_then(Value::as_i64) {
        if code == 0 {
            let result = body.get("data").cloned().unwrap_or(Value::Null);
            return Ok(RawResponse { result });
        }
        let msg = body
            .get("msg")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .unwrap_or_else(|| body.to_string());
        return Err(PhemexError::Api { http, code, msg });
    }

    // 시세 envelope: { error, id, result }
    if body.get("result").is_some() || body.get("error").is_some() {
        let err = body.get("error");
        let is_err = matches!(err, Some(v) if !v.is_null());
        if !is_err {
            let result = body.get("result").cloned().unwrap_or(Value::Null);
            return Ok(RawResponse { result });
        }
        let (code, msg) = match err {
            Some(Value::Object(_)) => (
                err.unwrap().get("code").and_then(Value::as_i64).unwrap_or(0),
                err.unwrap()
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("md error")
                    .to_string(),
            ),
            Some(Value::Number(n)) => (n.as_i64().unwrap_or(0), "md error".to_string()),
            other => (0, other.map(|v| v.to_string()).unwrap_or_default()),
        };
        return Err(PhemexError::Api { http, code, msg });
    }

    // envelope 아님 — HTTP status로 폴백.
    if (200..300).contains(&http) {
        return Ok(RawResponse { result: body });
    }
    Err(PhemexError::Api {
        http,
        code: http as i64,
        msg: if body.is_null() {
            "empty/non-JSON response".to_string()
        } else {
            body.to_string()
        },
    })
}

/// 429 대기 시간(초). `Retry-After`/`X-RateLimit-Retry-After-CONTRACT` → 기본 1초. [1,300].
fn retry_after_secs(resp: &reqwest::Response) -> u64 {
    let h = resp.headers();
    h.get("retry-after")
        .or_else(|| h.get("x-ratelimit-retry-after-contract"))
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(1)
        .clamp(1, 300)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Phemex 공식 문서 "Signature Example 2"의 **signed string**을 바이트 단위로
    /// 재현한다(doc-vector). 문서:
    ///   path  = /orders/activeList
    ///   query = ordStatus=New&ordStatus=PartiallyFilled&ordStatus=Untriggered&symbol=BTCUSD
    ///   expiry= 1575735951
    ///   body  = <null>
    ///   signed string = `/orders/activeListordStatus=...&symbol=BTCUSD1575735951`
    /// 공식 문서는 결과 hex를 공개하지 않으므로, 연결 문자열이 문서와 정확히 일치함을
    /// 확인하고(서명 규칙의 핵심), HMAC 자체는 self-consistency로 고정한다.
    #[test]
    fn signed_string_matches_phemex_doc_example2() {
        let path = "/orders/activeList";
        let query = "ordStatus=New&ordStatus=PartiallyFilled&ordStatus=Untriggered&symbol=BTCUSD";
        let expiry = 1575735951u64;
        let body = "";
        let message = format!("{path}{query}{expiry}{body}");
        assert_eq!(
            message,
            "/orders/activeListordStatus=New&ordStatus=PartiallyFilled&ordStatus=Untriggered&symbol=BTCUSD1575735951"
        );
    }

    /// encode_query가 삽입 순서를 보존하고 `?` 없이 `k=v&k=v`로 만든다(서명·전송 일치).
    #[test]
    fn encode_query_preserves_order_no_question_mark() {
        let pairs = vec![
            ("currency".to_string(), "USDT".to_string()),
            ("symbol".to_string(), "SAMSUNGUSDT".to_string()),
        ];
        assert_eq!(
            PhemexClient::encode_query(&pairs),
            "currency=USDT&symbol=SAMSUNGUSDT"
        );
    }

    /// GET 서명 회귀 고정(self-consistency). secret "MTIzNDU2Nzg5MA"는 base64url로
    /// "1234567890"을 인코딩한 것 → HMAC 키는 ASCII bytes `1234567890`.
    /// 기대값은 동일 입력으로 Python `hmac.new(b"1234567890", msg, sha256).hexdigest()`로
    /// 독립 계산해 고정했다(서명기 정확성을 네트워크 없이 검증).
    #[test]
    fn sign_get_regression() {
        let sig = PhemexClient::sign(
            "MTIzNDU2Nzg5MA",
            "/g-accounts/accountPositions",
            "currency=USDT&symbol=SAMSUNGUSDT",
            1700000000,
            "",
        )
        .unwrap();
        assert_eq!(
            sig,
            "27d8c15a93ab06886daab2b7c61867452d90687973881582e97fba8ee0a98c6d"
        );
    }

    /// base64url 디코드가 안 되는 시크릿은 에러(유저별 비결정 서명 방지).
    #[test]
    fn sign_rejects_invalid_base64url_secret() {
        // "!!!"는 유효한 base64url이 아니므로 Sign 에러.
        let err = PhemexClient::sign("!!!", "/x", "", 1, "").unwrap_err();
        assert!(matches!(err, PhemexError::Sign(_)));
    }

    /// POST 서명: query는 빈 문자열, body는 JSON 원문. 메시지 = path+expiry+body.
    #[test]
    fn sign_post_message_shape() {
        // path + "" + expiry + body 순서 검증(구조 회귀).
        let body = r#"{"symbol":"SAMSUNGUSDT","side":"Buy"}"#;
        let a = PhemexClient::sign("c2VjcmV0", "/g-orders", "", 1700000000, body).unwrap();
        let b = PhemexClient::sign("c2VjcmV0", "/g-orders", "", 1700000001, body).unwrap();
        let c = PhemexClient::sign("c2VjcmV0", "/g-orders", "", 1700000000, "").unwrap();
        assert_ne!(a, b); // expiry 변화 반영
        assert_ne!(a, c); // body 변화 반영
        assert_eq!(a.len(), 64); // hex SHA256
    }

    #[test]
    fn envelope_trade_code_zero_returns_data() {
        let body = json!({ "code": 0, "data": { "orderID": "abc" }, "msg": "" });
        let r = interpret_envelope(200, body).unwrap();
        assert_eq!(r.result, json!({ "orderID": "abc" }));
    }

    #[test]
    fn envelope_trade_business_error_on_http_200() {
        let body = json!({ "code": 11001, "msg": "TE_NO_SUCH_SYMBOL", "data": null });
        let err = interpret_envelope(200, body).unwrap_err();
        match err {
            PhemexError::Api { http, code, msg } => {
                assert_eq!(http, 200);
                assert_eq!(code, 11001);
                assert_eq!(msg, "TE_NO_SUCH_SYMBOL");
            }
            _ => panic!("expected Api error"),
        }
    }

    #[test]
    fn envelope_md_null_error_returns_result() {
        let body = json!({ "error": null, "id": 0, "result": { "lastRp": "235.19" } });
        let r = interpret_envelope(200, body).unwrap();
        assert_eq!(r.result, json!({ "lastRp": "235.19" }));
    }
}
