use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::global::lighter::config::LighterConfig;
use crate::global::lighter::error::{LighterError, Result};
use crate::ratelimit::RateLimiter;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/api/v1/..." 경로.
    pub path: String,
    /// 쿼리 파라미터 (GET). 순서 보존.
    pub query: Vec<(String, String)>,
    /// POST 폼 바디 (`application/x-www-form-urlencoded`). sendTx 등. GET이면 빈 벡터.
    pub form: Vec<(String, String)>,
}

/// 내부 도메인 호출 명세. Lighter는 시세=GET(query), 트랜잭션=POST(form-urlencoded).
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub form: Vec<(String, String)>,
}

impl ApiCall {
    /// 키 불필요 GET 시세 호출.
    pub(crate) fn get(path: impl Into<String>, query: Vec<(String, String)>) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            query,
            form: Vec::new(),
        }
    }

    /// POST form 호출 (sendTx 등). 서명 구현 시 트랜잭션 제출 경로가 쓴다. 현재는 서명
    /// 단계에서 차단되어 호출되지 않으므로 dead_code를 허용한다(예약).
    #[allow(dead_code)]
    pub(crate) fn post_form(path: impl Into<String>, form: Vec<(String, String)>) -> Self {
        Self {
            method: Method::POST,
            path: path.into(),
            query: Vec::new(),
            form,
        }
    }
}

/// HTTP 응답 — Lighter는 envelope 없이 페이로드를 직접 반환하되 `code` 필드를 포함한다.
/// `call_once`가 body `code`를 검사해 성공/에러를 가른 뒤, 성공 body 전체를 보존한다.
pub(crate) struct RawResponse {
    pub body: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.body.clone())?)
    }
}

/// Lighter(zkLighter) REST 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이 완전 동작한다. 거래 액세서(`trade`)는 타입드
/// 구조체·호출 경로까지 완비하나, 서명을 검증하지 못해 실제 제출은
/// [`LighterError::SignerUnavailable`]로 거부한다(data-only).
pub struct LighterClient {
    config: LighterConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl LighterClient {
    /// 클라이언트 생성.
    pub fn new(config: LighterConfig) -> Result<Self> {
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

    /// 설정 참조 (액세서가 account_index/키 상태를 본다).
    pub(crate) fn config(&self) -> &LighterConfig {
        &self.config
    }

    /// 시세 도메인 액세서 (키 불필요).
    pub fn market(&self) -> crate::global::lighter::market::Market<'_> {
        crate::global::lighter::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (서명 미검증 — 제출은 차단).
    pub fn trade(&self) -> crate::global::lighter::trade::Trade<'_> {
        crate::global::lighter::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 raw JSON body.
    pub async fn raw_call(&self, req: RawRequest) -> Result<Value> {
        let resp = self
            .call(ApiCall {
                method: req.method,
                path: req.path,
                query: req.query,
                form: req.form,
            })
            .await?;
        Ok(resp.body)
    }

    /// 도메인 공용 호출. 429(레이트) 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(LighterError::Api { status, .. })
                    if status == 429 && attempt < MAX_RETRIES =>
                {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 요청 조립 → 전송 → status·body code 분기.
    ///
    /// Lighter는 정상 응답도 `code`(예: 200)를 싣고, 일부 잘못된 파라미터는 HTTP 200에
    /// 에러 envelope(`code` != 200)로 온다. 따라서 HTTP status와 body `code`를 함께 본다.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let url = format!("{}{}", self.config.base_url, c.path);
        let mut req = self
            .http
            .request(c.method.clone(), &url)
            .header("Accept", "application/json");

        if !c.query.is_empty() {
            req = req.query(&c.query);
        }
        if c.method == Method::POST {
            // form-urlencoded 바디 (sendTx 계열).
            req = req.form(&c.form);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = retry_after_secs(&resp);
            tracing::warn!("429 — {wait}s 대기 후 재시도");
            let (code, message) = parse_api_error(resp).await;
            tokio::time::sleep(Duration::from_secs(wait)).await;
            return Err(LighterError::Api {
                status: 429,
                code,
                message,
            });
        }

        if !status.is_success() {
            let http_status = status.as_u16();
            let (code, message) = parse_api_error(resp).await;
            return Err(LighterError::Api {
                status: http_status,
                code,
                message,
            });
        }

        // 2xx: body를 받아 `code`를 검사한다. Lighter 성공 code는 200.
        let body: Value = resp.json().await?;
        if let Some((code, message)) = error_in_body(&body) {
            return Err(LighterError::Api {
                status: status.as_u16(),
                code,
                message,
            });
        }
        Ok(RawResponse { body })
    }
}

/// 성공 HTTP status인데 body가 에러 envelope면 `(code, message)`를 반환.
/// Lighter는 성공 시 `code:200`(또는 `code` 부재)이고, 실패 시 `code` != 200 + `message`.
fn error_in_body(body: &Value) -> Option<(i64, String)> {
    let code = body.get("code").and_then(Value::as_i64)?;
    if code == 200 {
        return None;
    }
    let message = body
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    Some((code, message))
}

/// Lighter 에러 본문 `{code,message}` 파싱. 비-JSON이면 code=0 + 원문.
async fn parse_api_error(resp: reqwest::Response) -> (i64, String) {
    let text = resp.text().await.unwrap_or_default();
    match serde_json::from_str::<Value>(&text) {
        Ok(v) => {
            let code = v.get("code").and_then(Value::as_i64).unwrap_or(0);
            let message = v
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or(&text)
                .to_string();
            (code, message)
        }
        Err(_) => (0, text),
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

    #[test]
    fn body_code_200_is_success() {
        let body = serde_json::json!({ "code": 200, "candlesticks": [] });
        assert!(error_in_body(&body).is_none());
    }

    #[test]
    fn body_missing_code_is_success() {
        // code 필드가 없는 페이로드도 성공 취급(파싱은 각 도메인 struct가 검증).
        let body = serde_json::json!({ "order_books": [] });
        assert!(error_in_body(&body).is_none());
    }

    #[test]
    fn body_error_envelope_in_200() {
        // 라이브에서 잘못된 파라미터는 HTTP 200 + {code:20001,message:"invalid param "}.
        let body = serde_json::json!({ "code": 20001, "message": "invalid param " });
        let (code, msg) = error_in_body(&body).unwrap();
        assert_eq!(code, 20001);
        assert_eq!(msg, "invalid param ");
    }
}
