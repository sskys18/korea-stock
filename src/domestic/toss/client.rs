use std::time::Duration;

use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::ratelimit::RateLimiter;
use crate::domestic::toss::auth::Auth;
use crate::domestic::toss::config::TossConfig;
use crate::domestic::toss::error::{Result, TossError};

/// 도메인 응답 + 메타 envelope. 대부분의 메서드는 `T`만 반환하지만, 응답 헤더
/// `X-Request-Id`까지 필요한 호출자를 위해 `raw_call`이 이 타입을 노출한다.
#[derive(Debug, Clone)]
pub struct TossResponse<T> {
    pub data: T,
    /// 응답 헤더 `X-Request-Id`. 토스 CS 문의용 식별자.
    pub request_id: Option<String>,
}

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/api/v1/..." 경로.
    pub path: String,
    /// GET=query 파라미터, POST=JSON body. JSON object.
    pub params: Value,
    pub is_post: bool,
    /// 계좌 스코프 호출이면 `Some(accountSeq)` — `X-Tossinvest-Account` 헤더로 첨부.
    pub account_seq: Option<i64>,
}

/// 내부 도메인 호출 명세.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub params: Value,
    pub is_post: bool,
    /// 계좌 스코프 액세서가 주입. `Some`일 때만 `X-Tossinvest-Account` 헤더를 붙인다.
    pub account_seq: Option<i64>,
}

/// HTTP 응답 — 성공 envelope의 `result`를 raw로 보존.
pub(crate) struct RawResponse {
    /// `ApiResponse.result` 페이로드.
    pub result: Value,
    pub request_id: Option<String>,
}

impl RawResponse {
    /// `result`를 타입 T로 역직렬화.
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.result.clone())?)
    }
}

/// JSON 스칼라를 query 파라미터 문자열로. (객체/배열은 호출부가 직렬화 후 넣음.)
fn json_scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// 토스증권 Open API 클라이언트.
pub struct TossClient {
    config: TossConfig,
    http: reqwest::Client,
    auth: Auth,
    /// 선택적 클라이언트측 글로벌 캡. `None`이면 사전 throttle 없이 429 루프에만 의존.
    limiter: Option<RateLimiter>,
}

impl TossClient {
    /// 클라이언트 생성. 토큰은 첫 호출 시 lazy 발급.
    pub fn new(config: TossConfig) -> Result<Self> {
        let http = reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(Duration::from_secs(10))
            .build()?;
        let auth = Auth::new(&config, http.clone());
        let limiter = config.rate_limit.map(RateLimiter::new);
        Ok(Self {
            config,
            http,
            auth,
            limiter,
        })
    }

    /// 시세 도메인 액세서.
    pub fn market_data(&self) -> crate::domestic::toss::market_data::MarketData<'_> {
        crate::domestic::toss::market_data::MarketData::new(self)
    }

    /// 종목정보 도메인 액세서.
    pub fn stock_info(&self) -> crate::domestic::toss::stock_info::StockInfoApi<'_> {
        crate::domestic::toss::stock_info::StockInfoApi::new(self)
    }

    /// 시장정보 도메인 액세서.
    pub fn market_info(&self) -> crate::domestic::toss::market_info::MarketInfoApi<'_> {
        crate::domestic::toss::market_info::MarketInfoApi::new(self)
    }

    /// 계좌 목록 액세서 (계좌 헤더 불필요).
    pub fn accounts(&self) -> crate::domestic::toss::account::Accounts<'_> {
        crate::domestic::toss::account::Accounts::new(self)
    }

    /// 자산(보유주식) 액세서. `account_seq`는 `X-Tossinvest-Account`로 자동 주입.
    pub fn asset(&self, account_seq: i64) -> crate::domestic::toss::account::Asset<'_> {
        crate::domestic::toss::account::Asset::new(self, account_seq)
    }

    /// 주문 액세서. `account_seq`는 `X-Tossinvest-Account`로 자동 주입.
    pub fn order(&self, account_seq: i64) -> crate::domestic::toss::order::Order<'_> {
        crate::domestic::toss::order::Order::new(self, account_seq)
    }

    /// 주문정보 액세서. `account_seq`는 `X-Tossinvest-Account`로 자동 주입.
    pub fn order_info(&self, account_seq: i64) -> crate::domestic::toss::order_info::OrderInfo<'_> {
        crate::domestic::toss::order_info::OrderInfo::new(self, account_seq)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 raw `result` JSON.
    pub async fn raw_call(&self, req: RawRequest) -> Result<TossResponse<Value>> {
        let resp = self
            .call(ApiCall {
                method: req.method,
                path: req.path,
                params: req.params,
                is_post: req.is_post,
                account_seq: req.account_seq,
            })
            .await?;
        Ok(TossResponse {
            data: resp.result,
            request_id: resp.request_id,
        })
    }

    /// 도메인 모듈 공용 호출. 429(요청 한도 초과) 재시도 + 401(토큰 거부) 1회 복구.
    ///
    /// 429: 실제 대기는 `call_once`가 응답의 `Retry-After`만큼 직접 수행한 뒤 429 에러를
    /// 던진다. 이 루프는 그 직후 재시도를 `MAX_RETRIES`회 상한으로 반복한다. KIS의
    /// EGW00201 백오프 루프 구조를 그대로 가져오되 트리거를 429로 교체한 형태다.
    ///
    /// 401: 토스는 재발급 시 이전 토큰을 무효화하므로 메모리의 "만료 전" 토큰이 거부될 수
    /// 있다. 첫 401에 한해 토큰을 강제 재발급([`Auth::force_refresh`])하고 1회 재시도한다.
    /// 자격증명 자체가 틀리면 재발급은 OAuth2 에러로 실패하고, 재발급 후에도 401이면
    /// 그대로 반환한다(무한 루프 방지).
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        let mut auth_retried = false;
        loop {
            match self.call_once(&c).await {
                Err(TossError::Api { status, .. }) if status == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                Err(TossError::Api { status, .. }) if status == 401 && !auth_retried => {
                    tracing::warn!("401 — 토큰 거부 추정, 강제 재발급 후 1회 재시도");
                    self.auth.force_refresh().await?;
                    auth_retried = true;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 토큰 → 헤더 → 전송 → HTTP status 분기.
    ///
    /// 성공/실패를 **HTTP status로 판정**한다 (KIS의 body `rt_cd` 방식과 다름).
    /// 2xx면 `ApiResponse.result` 언래핑, 비2xx면 에러 envelope 파싱.
    /// 429는 응답의 `Retry-After`/`X-RateLimit-Reset`만큼 대기 후 `Api{status:429}` 반환 →
    /// 상위 `call`이 재시도 루프를 돈다.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }
        let token = self.auth.token().await?;
        let url = format!("{}{}", self.config.base_url, c.path);

        let mut req = self
            .http
            .request(c.method.clone(), &url)
            .header("authorization", format!("Bearer {token}"));

        if let Some(seq) = c.account_seq {
            req = req.header("X-Tossinvest-Account", seq.to_string());
        }

        if c.is_post {
            // POST body가 null이면(취소 등 본문 없는 호출) JSON body를 생략.
            if !c.params.is_null() {
                req = req.json(&c.params);
            }
        } else {
            let obj = c
                .params
                .as_object()
                .ok_or_else(|| TossError::Decode("query params must be object".into()))?;
            let pairs: Vec<(String, String)> = obj
                .iter()
                // 값이 null인 선택 파라미터는 query에서 제외.
                .filter(|(_, v)| !v.is_null())
                .map(|(k, v)| (k.clone(), json_scalar_to_string(v)))
                .collect();
            req = req.query(&pairs);
        }

        let resp = req.send().await?;
        let status = resp.status();
        let request_id = resp
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        // 429: 권위 throttle. Retry-After → X-RateLimit-Reset(초) 순으로 대기.
        if status == StatusCode::TOO_MANY_REQUESTS {
            let wait_secs = retry_after_secs(&resp);
            tracing::warn!("429 — {wait_secs}s 대기 후 재시도");
            tokio::time::sleep(Duration::from_secs(wait_secs)).await;
            return Err(TossError::Api {
                status: 429,
                request_id,
                code: "rate-limit-exceeded".into(),
                message: "요청 한도를 초과했습니다.".into(),
            });
        }

        if status.is_success() {
            let body: Value = resp.json().await?;
            let result = body
                .get("result")
                .cloned()
                .ok_or_else(|| TossError::Decode(format!("no result in response: {body}")))?;
            return Ok(RawResponse { result, request_id });
        }

        // 4xx/5xx: BFF 에러 envelope `{ "error": { requestId, code, message, data? } }`.
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        Err(map_bff_error(status.as_u16(), &body, request_id))
    }
}

/// BFF 에러 envelope → `TossError::Api` 매핑. 헤더 `X-Request-Id`는 body `requestId`가
/// 없을 때의 fallback. `call_once`에서 분리해 단위 테스트가 검증 가능하도록 한다.
pub(crate) fn map_bff_error(
    status: u16,
    body: &Value,
    header_request_id: Option<String>,
) -> TossError {
    let err = body.get("error");
    let code = err
        .and_then(|e| e.get("code"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let message = err
        .and_then(|e| e.get("message"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string();
    let request_id = err
        .and_then(|e| e.get("requestId"))
        .and_then(|v| v.as_str())
        .map(String::from)
        .or(header_request_id);
    TossError::Api {
        status,
        request_id,
        code,
        message,
    }
}

/// 429 대기 시간(초)을 응답 헤더에서 산출한다.
///
/// `Retry-After`는 RFC 7231상 ①delta-seconds(정수) 또는 ②HTTP-date 두 형식을 가진다.
/// 정수 우선 파싱, 실패 시 RFC2822 날짜로 파싱해 현재시각과의 차(초)로 환산한다 —
/// 정수만 받던 기존 구현은 날짜 형식을 만나면 조용히 1초로 떨어져 과소 대기했다.
/// 둘 다 실패하면 `X-RateLimit-Reset`(초) → 기본 1초 순. 최종값은 [1, 300]초로 클램프.
fn retry_after_secs(resp: &reqwest::Response) -> u64 {
    if let Some(raw) = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
    {
        if let Ok(secs) = raw.parse::<u64>() {
            return secs.clamp(1, 300);
        }
        if let Ok(when) = chrono::DateTime::parse_from_rfc2822(raw) {
            let delta = when.timestamp() - chrono::Utc::now().timestamp();
            return delta.clamp(1, 300) as u64;
        }
        tracing::warn!("Retry-After 파싱 실패('{raw}') — 폴백 대기 적용");
    }
    header_u64(resp, "x-ratelimit-reset")
        .unwrap_or(1)
        .clamp(1, 300)
}

/// 응답 헤더에서 u64 값을 읽는다 (X-RateLimit-Reset 등).
fn header_u64(resp: &reqwest::Response, name: &str) -> Option<u64> {
    resp.headers()
        .get(name)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse().ok())
}
