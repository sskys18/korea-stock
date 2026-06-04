use std::time::Duration;

use base64::Engine;
use hmac::{Hmac, Mac};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::Sha256;

use crate::global::bitget::config::BitgetConfig;
use crate::global::bitget::error::{BitgetError, Result};
use crate::ratelimit::RateLimiter;

type HmacSha256 = Hmac<Sha256>;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/api/v2/..." 경로.
    pub path: String,
    /// 요청 파라미터. GET=query 문자열로, POST=JSON body로 직렬화.
    pub params: Vec<(String, String)>,
    /// true면 `ACCESS-*` 서명 헤더를 부착한다.
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

    /// 서명 필요 변경 호출 (POST). Bitget v2 거래는 모두 POST.
    pub(crate) fn signed_post(path: impl Into<String>, params: Vec<(String, String)>) -> Self {
        Self {
            method: Method::POST,
            path: path.into(),
            params,
            signed: true,
        }
    }
}

/// HTTP 응답 — Bitget v2는 `{ code, msg, requestTime, data }` envelope의 `data`를 보존한다.
#[derive(Debug)]
pub(crate) struct RawResponse {
    pub data: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.data.clone())?)
    }
}

/// 요청 본문에서 **YES/NO 문자열이 아닌 JSON 값**으로 직렬화해야 하는 키는 없다 —
/// Bitget v2 mix는 size/price/reduceOnly(YES/NO 문자열) 등을 전부 **문자열**로 받는다.
/// 따라서 POST 본문은 모든 값을 문자열로 직렬화한다.
///
/// Bitget v2 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 HMAC-SHA256 + passphrase로
/// 호출한다. 서명은 **헤더**(`ACCESS-KEY`/`ACCESS-SIGN`/`ACCESS-TIMESTAMP`/
/// `ACCESS-PASSPHRASE`)로 보내며, 서명 대상은
/// `timestamp + method.upper + requestPath + (?queryString | rawBody)`이다.
/// 결과는 Base64(HMAC-SHA256)다(Binance/Bybit의 hex와 다름).
pub struct BitgetClient {
    config: BitgetConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl BitgetClient {
    /// 클라이언트 생성.
    pub fn new(config: BitgetConfig) -> Result<Self> {
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
    pub fn market(&self) -> crate::global::bitget::market::Market<'_> {
        crate::global::bitget::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (HMAC + passphrase 서명).
    pub fn trade(&self) -> crate::global::bitget::trade::Trade<'_> {
        crate::global::bitget::trade::Trade::new(self)
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

    /// 현재 UTC epoch milliseconds. 서명 `ACCESS-TIMESTAMP` 용.
    fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// 쿼리 파라미터를 `k=v&k=v` 문자열로 직렬화. **순서 보존**(삽입순) — 공개 GET용.
    fn encode_query(pairs: &[(String, String)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// 서명 GET용 쿼리: **키 사전순 정렬**(Bitget 서명 규칙 — ccxt `keysort`).
    /// 정렬한 결과는 서명 문자열과 실제 전송 URL이 동일해야 한다.
    fn encode_query_sorted(pairs: &[(String, String)]) -> String {
        let mut sorted: Vec<&(String, String)> = pairs.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));
        sorted
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// 순서 보존 key/value 쌍을 JSON object 본문 문자열로 직렬화. POST 서명·전송 공용.
    /// Bitget v2 mix는 모든 본문 값을 **문자열**로 받는다(size/price/reduceOnly="YES" 등).
    fn json_object_from_pairs(pairs: &[(String, String)]) -> Result<String> {
        let mut map = serde_json::Map::with_capacity(pairs.len());
        for (k, v) in pairs {
            map.insert(k.clone(), Value::String(v.clone()));
        }
        Ok(serde_json::to_string(&Value::Object(map))?)
    }

    /// Base64(HMAC-SHA256(secret, timestamp + method + requestPath + suffix)).
    ///
    /// `suffix`는 GET이면 `?`+정렬 query 문자열(비어 있으면 생략), POST면 전송할
    /// **JSON 본문 원문**과 바이트 단위로 동일해야 한다(Bitget v2 서명 규칙). `method`는
    /// 대문자("GET"/"POST"), `request_path`는 `/api/v2/...` 전체 경로.
    fn sign(
        secret: &str,
        timestamp: u64,
        method: &str,
        request_path: &str,
        suffix: &str,
    ) -> Result<String> {
        let prehash = format!("{timestamp}{method}{request_path}{suffix}");
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| BitgetError::Sign(e.to_string()))?;
        mac.update(prehash.as_bytes());
        Ok(base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
    }

    /// 도메인 공용 호출. 429 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(BitgetError::Api { http, .. }) if http == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 헤더/서명 조립 → 전송 → 본문 `code` 분기.
    ///
    /// **성공/실패를 본문 `code`로 판정**한다(Binance의 HTTP status 방식과 다름).
    /// Bitget v2는 비즈니스 에러도 HTTP 200 + `code!="00000"`으로 내려준다.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let is_post = c.method == Method::POST;
        let base = format!("{}{}", self.config.base_url, c.path);

        // 공개 GET: 삽입순 query. 서명 GET: 정렬 query(서명·URL 동일). POST: JSON 본문.
        let (suffix, post_body): (String, Option<String>) = if is_post {
            let body = Self::json_object_from_pairs(&c.params)?;
            (body.clone(), Some(body))
        } else if c.signed {
            let q = Self::encode_query_sorted(&c.params);
            (if q.is_empty() { String::new() } else { format!("?{q}") }, None)
        } else {
            let q = Self::encode_query(&c.params);
            (if q.is_empty() { String::new() } else { format!("?{q}") }, None)
        };

        // GET은 서명한(또는 공개) query 문자열을 **그대로** URL에 붙인다(reqwest의 query()는
        // 퍼센트 인코딩을 다시 적용해 서명한 바이트와 어긋날 수 있으므로 직접 조립).
        let url = if !is_post {
            format!("{base}{suffix}")
        } else {
            base
        };

        let mut req = self.http.request(c.method.clone(), &url);

        if c.signed {
            if self.config.api_key.is_empty()
                || self.config.api_secret.is_empty()
                || self.config.passphrase.is_empty()
            {
                return Err(BitgetError::Auth(
                    "signed endpoint requires api_key/api_secret/passphrase".into(),
                ));
            }
            let timestamp = Self::timestamp_ms();
            let method_str = c.method.as_str();
            let signature = Self::sign(
                &self.config.api_secret,
                timestamp,
                method_str,
                &c.path,
                &suffix,
            )?;
            req = req
                .header("ACCESS-KEY", &self.config.api_key)
                .header("ACCESS-SIGN", signature)
                .header("ACCESS-TIMESTAMP", timestamp.to_string())
                .header("ACCESS-PASSPHRASE", &self.config.passphrase)
                .header("locale", "en-US");
        }

        // POST는 서명한 그 바이트열을 본문으로 부착한다(reqwest 재직렬화 금지).
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
            return Err(BitgetError::Api {
                http: 429,
                code: "429".into(),
                msg: "rate limit exceeded".into(),
            });
        }

        let http = status.as_u16();
        let body: Value = resp.json().await.unwrap_or(Value::Null);
        interpret_envelope(http, body)
    }
}

/// Bitget v2 envelope `{ code, msg, requestTime, data }`를 해석한다.
///
/// `code=="00000"`이면 `data`를 반환, 아니면 [`BitgetError::Api`]로 매핑한다.
/// `call_once`에서 분리해 단위 테스트가 검증 가능하도록 한다.
pub(crate) fn interpret_envelope(http: u16, body: Value) -> Result<RawResponse> {
    let code = body.get("code").and_then(Value::as_str);

    match code {
        Some("00000") => {
            let data = body.get("data").cloned().unwrap_or(Value::Null);
            Ok(RawResponse { data })
        }
        Some(c) => {
            let msg = body
                .get("msg")
                .and_then(Value::as_str)
                .map(String::from)
                .unwrap_or_else(|| body.to_string());
            Err(BitgetError::Api {
                http,
                code: c.to_string(),
                msg,
            })
        }
        // envelope 형태가 아니면(비-JSON, 인프라 5xx 등) HTTP status로 폴백 판정.
        None => {
            if (200..300).contains(&http) {
                let data = body.get("data").cloned().unwrap_or(Value::Null);
                Ok(RawResponse { data })
            } else {
                let msg = if body.is_null() {
                    "empty/non-JSON response".to_string()
                } else {
                    body.to_string()
                };
                Err(BitgetError::Api {
                    http,
                    code: String::new(),
                    msg,
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

    // Bitget v2 서명 회귀 벡터.
    // 공식 문서가 공개한 (실키 포함) 숫자 테스트 벡터는 없어, 서명 규칙
    // (timestamp + method + requestPath + ?sortedQuery, HMAC-SHA256, Base64)을
    // 고정 입력으로 잡아 회귀 고정한다. 기대 Base64는 bash openssl로 독립 계산:
    //   printf '1717400000000GET/api/v2/mix/market/ticker?productType=USDT-FUTURES&symbol=SAMSUNGUSDT' \
    //     | openssl dgst -sha256 -hmac 'test-api-secret' -binary | openssl base64
    // → (아래 상수). 이 한 줄이 거래 안전의 핵심이다.
    #[test]
    fn sign_matches_independent_openssl_vector() {
        // 정렬된 GET query suffix(키 사전순: productType < symbol).
        let suffix = "?productType=USDT-FUTURES&symbol=SAMSUNGUSDT";
        let sig = BitgetClient::sign(
            "test-api-secret",
            1717400000000,
            "GET",
            "/api/v2/mix/market/ticker",
            suffix,
        )
        .unwrap();
        assert_eq!(sig, "9Ev/Bw3O+OEb6MG/IMmSElDQFTCr8HBq/XlHIt6UJ5c=");
    }

    #[test]
    fn encode_query_sorted_orders_keys_alphabetically() {
        // 삽입순은 symbol→productType지만 서명 query는 사전순으로 재정렬해야 한다.
        let pairs = vec![
            ("symbol".to_string(), "SAMSUNGUSDT".to_string()),
            ("productType".to_string(), "USDT-FUTURES".to_string()),
        ];
        assert_eq!(
            BitgetClient::encode_query_sorted(&pairs),
            "productType=USDT-FUTURES&symbol=SAMSUNGUSDT"
        );
    }

    #[test]
    fn encode_query_preserves_insertion_order() {
        // 공개 GET은 정렬 불필요 — 삽입순 그대로.
        let pairs = vec![
            ("symbol".to_string(), "SAMSUNGUSDT".to_string()),
            ("productType".to_string(), "USDT-FUTURES".to_string()),
        ];
        assert_eq!(
            BitgetClient::encode_query(&pairs),
            "symbol=SAMSUNGUSDT&productType=USDT-FUTURES"
        );
    }

    #[test]
    fn post_body_is_string_valued_json() {
        let pairs = vec![
            ("symbol".to_string(), "SAMSUNGUSDT".to_string()),
            ("size".to_string(), "0.1".to_string()),
            ("reduceOnly".to_string(), "YES".to_string()),
        ];
        let body = BitgetClient::json_object_from_pairs(&pairs).unwrap();
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["symbol"], "SAMSUNGUSDT");
        // 전부 문자열로 직렬화(Bitget v2 mix 규칙).
        assert!(v["size"].is_string());
        assert!(v["reduceOnly"].is_string());
        assert_eq!(v["reduceOnly"], "YES");
    }

    #[test]
    fn sign_concatenates_ts_method_path_suffix() {
        // 어느 한 컴포넌트가 바뀌면 서명이 달라짐을 확인(구조 회귀).
        let a = BitgetClient::sign("s", 1000, "GET", "/p", "?x=1").unwrap();
        let b = BitgetClient::sign("s", 1001, "GET", "/p", "?x=1").unwrap();
        let c = BitgetClient::sign("s", 1000, "POST", "/p", "?x=1").unwrap();
        let d = BitgetClient::sign("s", 1000, "GET", "/q", "?x=1").unwrap();
        let e = BitgetClient::sign("s", 1000, "GET", "/p", "?x=2").unwrap();
        assert_ne!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
        assert_ne!(a, e);
    }

    #[test]
    fn envelope_success_returns_data() {
        let body = json!({ "code": "00000", "msg": "success", "data": { "x": 1 } });
        let r = interpret_envelope(200, body).unwrap();
        assert_eq!(r.data, json!({ "x": 1 }));
    }

    #[test]
    fn envelope_business_error_on_http_200() {
        // 비즈니스 에러도 HTTP 200 — 본문 code로 판정해야 한다.
        let body = json!({ "code": "40009", "msg": "sign signature error", "data": null });
        let err = interpret_envelope(200, body).unwrap_err();
        match err {
            BitgetError::Api { http, code, msg } => {
                assert_eq!(http, 200);
                assert_eq!(code, "40009");
                assert_eq!(msg, "sign signature error");
            }
            _ => panic!("expected Api error"),
        }
    }
}
