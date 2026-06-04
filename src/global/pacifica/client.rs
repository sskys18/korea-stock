use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::global::pacifica::config::PacificaConfig;
use crate::global::pacifica::error::{PacificaError, Result};
use crate::ratelimit::RateLimiter;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/api/v1/..." 경로.
    pub path: String,
    /// GET 쿼리 파라미터(순서 보존).
    pub query: Vec<(String, String)>,
    /// POST 본문(서명 포함된 완성 JSON). GET이면 `None`.
    pub body: Option<Value>,
}

/// 내부 도메인 호출 명세.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub body: Option<Value>,
}

impl ApiCall {
    /// 키 불필요 GET 시세 호출.
    pub(crate) fn get(path: impl Into<String>, query: Vec<(String, String)>) -> Self {
        Self {
            method: Method::GET,
            path: path.into(),
            query,
            body: None,
        }
    }

    /// 서명된 본문을 싣는 POST 거래 호출.
    pub(crate) fn post(path: impl Into<String>, body: Value) -> Self {
        Self {
            method: Method::POST,
            path: path.into(),
            query: vec![],
            body: Some(body),
        }
    }
}

/// Pacifica 응답 봉투 `{success, data, error, code}`. `data`만 떼어 파싱한다.
pub(crate) struct RawResponse {
    pub data: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.data.clone())?)
    }
}

/// Pacifica(Solana perp DEX) 클라이언트.
///
/// 시세 액세서(`market`)는 GET·키 불필요, 거래 액세서(`trade`)는 Ed25519 서명 본문을
/// POST한다(게이트 통과 시). REST 봉투(`{success,data,error,code}`)를 클라이언트가 벗긴다.
pub struct PacificaClient {
    config: PacificaConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl PacificaClient {
    /// 클라이언트 생성.
    pub fn new(config: PacificaConfig) -> Result<Self> {
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

    /// 설정 참조 (trade 모듈이 키·게이트·만료창을 읽는다).
    pub(crate) fn config(&self) -> &PacificaConfig {
        &self.config
    }

    /// 시세 도메인 액세서 (키 불필요).
    pub fn market(&self) -> crate::global::pacifica::market::Market<'_> {
        crate::global::pacifica::market::Market::new(self)
    }

    /// 거래 도메인 액세서 (Ed25519 서명, 게이트).
    pub fn trade(&self) -> crate::global::pacifica::trade::Trade<'_> {
        crate::global::pacifica::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 봉투의 `data` raw JSON.
    pub async fn raw_call(&self, req: RawRequest) -> Result<Value> {
        let resp = self
            .call(ApiCall {
                method: req.method,
                path: req.path,
                query: req.query,
                body: req.body,
            })
            .await?;
        Ok(resp.data)
    }

    /// 현재 UTC epoch milliseconds. 서명 `timestamp` 용.
    pub(crate) fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// 도메인 공용 호출. 429(레이트) 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(PacificaError::Api { status, .. })
                    if status == 429 && attempt < MAX_RETRIES =>
                {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 요청 조립 → 전송 → 봉투 분기.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let url = format!("{}{}", self.config.base_url, c.path);
        let mut req = self.http.request(c.method.clone(), &url);
        if !c.query.is_empty() {
            req = req.query(&c.query);
        }
        if let Some(body) = &c.body {
            req = req.json(body);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait = retry_after_secs(&resp);
            tracing::warn!("429 — {wait}s 대기 후 재시도");
            let (code, error) = parse_api_error(resp).await;
            tokio::time::sleep(Duration::from_secs(wait)).await;
            return Err(PacificaError::Api {
                status: 429,
                code,
                error,
            });
        }

        let http_status = status.as_u16();
        let text = resp.text().await?;
        let envelope: Value = serde_json::from_str(&text)
            .map_err(|e| PacificaError::Decode(format!("non-JSON body (http {http_status}): {e}")))?;

        // 봉투 `success`로 성공 판정(HTTP 200에 에러 봉투가 실리는 경우 방어).
        let success = envelope
            .get("success")
            .and_then(Value::as_bool)
            .unwrap_or(status.is_success());
        if !success || !status.is_success() {
            let code = envelope.get("code").and_then(Value::as_i64);
            let error = envelope
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| text.clone());
            return Err(PacificaError::Api {
                status: http_status,
                code,
                error,
            });
        }

        let data = envelope.get("data").cloned().unwrap_or(Value::Null);
        Ok(RawResponse { data })
    }
}

/// Pacifica 에러 봉투 `{error, code}` 파싱. 비-JSON이면 code=None + 원문.
async fn parse_api_error(resp: reqwest::Response) -> (Option<i64>, String) {
    let text = resp.text().await.unwrap_or_default();
    match serde_json::from_str::<Value>(&text) {
        Ok(v) => {
            let code = v.get("code").and_then(Value::as_i64);
            let error = v
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or(&text)
                .to_string();
            (code, error)
        }
        Err(_) => (None, text),
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
