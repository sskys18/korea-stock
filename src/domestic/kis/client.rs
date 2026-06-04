use reqwest::Method;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::domestic::kis::auth::Auth;
use crate::domestic::kis::config::KisConfig;
use crate::domestic::kis::error::{KisError, Result};
use crate::ratelimit::RateLimiter;

/// 모든 도메인 응답 공통 envelope. 데이터 + 연속조회 메타.
#[derive(Debug, Clone)]
pub struct KisResponse<T> {
    pub data: T,
    /// 응답 헤더 `tr_cont`. "F"/"M"이면 다음 페이지 존재.
    pub tr_cont: Option<String>,
    /// body cursor — 다음 페이지 요청 시 그대로 전달.
    pub ctx_area_fk: Option<String>,
    pub ctx_area_nk: Option<String>,
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg: String,
}

impl<T> KisResponse<T> {
    /// 다음 페이지가 있으면 true.
    pub fn has_next(&self) -> bool {
        matches!(self.tr_cont.as_deref(), Some("F") | Some("M"))
    }
}

/// 미구현 TR 직접 호출용 저수준 요청.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/uapi/..." 경로.
    pub path: String,
    pub tr_id: String,
    pub tr_cont: Option<String>,
    /// GET=query 파라미터, POST=body. JSON object.
    pub params: Value,
    /// POST일 때 true면 body, false면 GET query.
    pub is_post: bool,
    /// 이 요청에 hashkey 발급·첨부 (config.use_hashkey와 OR). 기본 false.
    pub needs_hashkey: bool,
}

/// 내부 도메인 호출 명세.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub tr_id: String,
    pub tr_cont: Option<String>,
    pub params: Value,
    pub is_post: bool,
    /// 요청별 hashkey 강제. config.use_hashkey와 OR로 적용.
    pub needs_hashkey: bool,
}

/// HTTP 응답 — 공통 envelope 검사 후 raw body 보존.
pub(crate) struct RawResponse {
    pub body: Value,
    pub tr_cont: Option<String>,
}

impl RawResponse {
    /// body의 지정 키를 타입 T로 역직렬화.
    pub fn field<T: DeserializeOwned>(&self, key: &str) -> Result<T> {
        let v = self
            .body
            .get(key)
            .ok_or_else(|| KisError::Decode(format!("missing {key}")))?;
        Ok(serde_json::from_value(v.clone())?)
    }

    /// `data`를 envelope로 감싼다. ctx_area는 body에서 추출.
    /// 국내주식은 `*100` 체계, 해외·선물옵션은 `*200` 체계 — 둘 다 수용(100 우선, 없으면 200).
    pub fn envelope<T>(&self, data: T) -> KisResponse<T> {
        let s = |k: &str| self.body.get(k).and_then(|v| v.as_str()).map(String::from);
        KisResponse {
            data,
            tr_cont: self.tr_cont.clone(),
            ctx_area_fk: s("ctx_area_fk100").or_else(|| s("ctx_area_fk200")),
            ctx_area_nk: s("ctx_area_nk100").or_else(|| s("ctx_area_nk200")),
            rt_cd: s("rt_cd").unwrap_or_default(),
            msg_cd: s("msg_cd").unwrap_or_default(),
            msg: s("msg1").unwrap_or_default(),
        }
    }
}

/// JSON 스칼라를 query 파라미터 문자열로. KIS는 모든 값을 문자열로 받음.
fn json_scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// KIS OpenAPI 클라이언트.
pub struct KisClient {
    config: KisConfig,
    http: reqwest::Client,
    auth: Auth,
    limiter: RateLimiter,
}

impl KisClient {
    /// 클라이언트 생성. 토큰은 첫 호출 시 lazy 발급.
    pub fn new(config: KisConfig) -> Result<Self> {
        // 코어 TLS는 rustls 고정 — `external` feature가 reqwest/native-tls를
        // 더해도 코어 클라이언트 백엔드가 바뀌지 않도록(feature 통합 방어).
        let http = reqwest::Client::builder()
            .use_rustls_tls()
            .timeout(std::time::Duration::from_secs(10))
            .build()?;
        let auth = Auth::new(&config, http.clone());
        let limiter = RateLimiter::new(config.environment.rate_limit());
        Ok(Self {
            config,
            http,
            auth,
            limiter,
        })
    }

    pub(crate) fn config(&self) -> &KisConfig {
        &self.config
    }

    /// 국내주식 도메인 액세서.
    pub fn domestic_stock(&self) -> crate::domestic::kis::domestic_stock::DomesticStock<'_> {
        crate::domestic::kis::domestic_stock::DomesticStock::new(self)
    }

    /// 해외주식 도메인 액세서.
    pub fn overseas_stock(&self) -> crate::domestic::kis::overseas_stock::OverseasStock<'_> {
        crate::domestic::kis::overseas_stock::OverseasStock::new(self)
    }

    /// 국내선물옵션 도메인 액세서.
    pub fn futureoption(&self) -> crate::domestic::kis::futureoption::FutureOption<'_> {
        crate::domestic::kis::futureoption::FutureOption::new(self)
    }

    /// 실시간 WebSocket 클라이언트 생성.
    ///
    /// approval_key 발급(REST) + WS 연결을 수행하므로 비동기·실패 가능.
    /// 반환된 `RealtimeClient`는 독립 — `KisClient` 수명과 무관.
    pub async fn realtime(&self) -> Result<crate::domestic::kis::realtime::RealtimeClient> {
        crate::domestic::kis::realtime::RealtimeClient::connect(self.config.clone(), self.http.clone()).await
    }

    /// 미구현 TR 직접 호출. 응답은 raw JSON envelope.
    pub async fn raw_call(&self, req: RawRequest) -> Result<KisResponse<Value>> {
        let resp = self
            .call(ApiCall {
                method: req.method,
                path: req.path,
                tr_id: req.tr_id,
                tr_cont: req.tr_cont,
                params: req.params,
                is_post: req.is_post,
                needs_hashkey: req.needs_hashkey,
            })
            .await?;
        let data = resp.body.clone();
        Ok(resp.envelope(data))
    }

    /// 도메인 모듈 공용 호출. EGW00201(초당 거래건수 초과) 시 지수 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(KisError::Api { msg_cd, .. })
                    if msg_cd == "EGW00201" && attempt < MAX_RETRIES =>
                {
                    let backoff = std::time::Duration::from_millis(200u64 << attempt);
                    tracing::warn!(
                        "EGW00201 rate limit — retry {}/{} after {:?}",
                        attempt + 1,
                        MAX_RETRIES,
                        backoff
                    );
                    tokio::time::sleep(backoff).await;
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. 레이트리밋 → 토큰 → 헤더 → 전송 → rt_cd 검사.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        self.limiter.acquire().await;
        let token = self.auth.token().await?;
        let url = format!("{}{}", self.config.environment.rest_base(), c.path);

        let mut req = self
            .http
            .request(c.method.clone(), &url)
            .header("authorization", format!("Bearer {token}"))
            .header("appkey", &self.config.app_key)
            .header("appsecret", &self.config.app_secret)
            .header("tr_id", &c.tr_id)
            .header("custtype", "P");

        if let Some(tc) = &c.tr_cont {
            req = req.header("tr_cont", tc);
        }

        if c.is_post {
            if self.config.use_hashkey || c.needs_hashkey {
                let hash = self.auth.hashkey(&c.params).await?;
                req = req.header("hashkey", hash);
            }
            req = req.json(&c.params);
        } else {
            let obj = c
                .params
                .as_object()
                .ok_or_else(|| KisError::Decode("query params must be object".into()))?;
            let pairs: Vec<(String, String)> = obj
                .iter()
                .map(|(k, v)| (k.clone(), json_scalar_to_string(v)))
                .collect();
            req = req.query(&pairs);
        }

        let resp = req.send().await?;
        let tr_cont = resp
            .headers()
            .get("tr_cont")
            .and_then(|v| v.to_str().ok())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from);

        let status = resp.status();
        let body: Value = resp.json().await?;

        let rt_cd = body.get("rt_cd").and_then(|v| v.as_str());
        match rt_cd {
            Some("0") => Ok(RawResponse { body, tr_cont }),
            Some(code) => Err(KisError::Api {
                rt_cd: code.to_string(),
                msg_cd: body
                    .get("msg_cd")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                msg: body
                    .get("msg1")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
            }),
            None => Err(KisError::Decode(format!(
                "no rt_cd in response (http {status}): {body}"
            ))),
        }
    }
}
