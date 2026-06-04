use std::time::Duration;

use hmac::{Hmac, Mac};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::Sha256;

use crate::binance::config::BinanceConfig;
use crate::binance::error::{BinanceError, Result};
use crate::ratelimit::RateLimiter;

type HmacSha256 = Hmac<Sha256>;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/fapi/v1/..." 경로.
    pub path: String,
    /// 쿼리 파라미터 (서명 대상 포함). 순서 보존.
    pub params: Vec<(String, String)>,
    /// true면 `timestamp`+`recvWindow`+HMAC `signature`를 부착하고 키 헤더를 보낸다.
    pub signed: bool,
}

/// 내부 도메인 호출 명세.
pub(crate) struct ApiCall {
    pub method: Method,
    pub path: String,
    pub params: Vec<(String, String)>,
    pub signed: bool,
}

impl ApiCall {
    /// 키 불필요 시세 호출.
    pub(crate) fn public(method: Method, path: impl Into<String>, params: Vec<(String, String)>) -> Self {
        Self {
            method,
            path: path.into(),
            params,
            signed: false,
        }
    }

    /// 서명 필요 거래/계좌 호출.
    pub(crate) fn signed(method: Method, path: impl Into<String>, params: Vec<(String, String)>) -> Self {
        Self {
            method,
            path: path.into(),
            params,
            signed: true,
        }
    }
}

/// HTTP 응답 — Binance는 envelope 없이 페이로드를 직접 반환한다.
pub(crate) struct RawResponse {
    pub body: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.body.clone())?)
    }
}

/// Binance USDM Futures(fapi) 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 HMAC-SHA256 서명으로
/// 호출한다. OAuth 토큰이 없으므로 Toss의 `Auth`에 대응하는 상태는 없고, 매 서명
/// 요청마다 `timestamp`를 새로 찍는다.
pub struct BinanceClient {
    config: BinanceConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl BinanceClient {
    /// 클라이언트 생성.
    pub fn new(config: BinanceConfig) -> Result<Self> {
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
    pub fn market(&self) -> crate::binance::market::Market<'_> {
        crate::binance::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (HMAC 서명).
    pub fn trade(&self) -> crate::binance::trade::Trade<'_> {
        crate::binance::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 raw JSON.
    pub async fn raw_call(&self, req: RawRequest) -> Result<Value> {
        let resp = self
            .call(ApiCall {
                method: req.method,
                path: req.path,
                params: req.params,
                signed: req.signed,
            })
            .await?;
        Ok(resp.body)
    }

    /// 현재 UTC epoch milliseconds. 서명 `timestamp` 용.
    fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// 쿼리 파라미터를 `k=v&k=v` 문자열로 직렬화. **순서 보존** — 서명·전송이 동일
    /// 문자열을 봐야 한다. 값은 이미 호출부에서 적절히 인코딩된 스칼라 문자열.
    fn encode_query(pairs: &[(String, String)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// HMAC-SHA256(secret, payload) → hex 소문자.
    fn sign(secret: &str, payload: &str) -> Result<String> {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| BinanceError::Sign(e.to_string()))?;
        mac.update(payload.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    /// 도메인 공용 호출. 429/418(레이트·차단) 반응형 백오프 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(BinanceError::Api { status, .. }) if (status == 429 || status == 418) && attempt < MAX_RETRIES => {
                    tracing::warn!("{status} rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 파라미터/서명 조립 → 전송 → status 분기.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let mut pairs = c.params.clone();
        if c.signed {
            if self.config.api_key.is_empty() || self.config.api_secret.is_empty() {
                return Err(BinanceError::Auth(
                    "signed endpoint requires api_key/api_secret".into(),
                ));
            }
            pairs.push(("recvWindow".into(), self.config.recv_window.to_string()));
            pairs.push(("timestamp".into(), Self::timestamp_ms().to_string()));
            let payload = Self::encode_query(&pairs);
            let signature = Self::sign(&self.config.api_secret, &payload)?;
            pairs.push(("signature".into(), signature));
        }

        let url = format!("{}{}", self.config.base_url, c.path);
        let query = Self::encode_query(&pairs);
        let full_url = if query.is_empty() {
            url
        } else {
            format!("{url}?{query}")
        };

        let mut req = self.http.request(c.method.clone(), &full_url);
        // 키가 있으면 항상 첨부 (시세도 키가 있으면 더 높은 레이트 한도를 받는다).
        if !self.config.api_key.is_empty() {
            req = req.header("X-MBX-APIKEY", &self.config.api_key);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS || status.as_u16() == 418 {
            let wait = retry_after_secs(&resp);
            tracing::warn!("{} — {wait}s 대기 후 재시도", status.as_u16());
            let (code, msg) = parse_api_error(resp).await;
            tokio::time::sleep(Duration::from_secs(wait)).await;
            return Err(BinanceError::Api {
                status: status.as_u16(),
                code,
                msg,
            });
        }

        if status.is_success() {
            let body: Value = resp.json().await?;
            return Ok(RawResponse { body });
        }

        let http_status = status.as_u16();
        let (code, msg) = parse_api_error(resp).await;
        Err(BinanceError::Api {
            status: http_status,
            code,
            msg,
        })
    }
}

/// Binance 에러 본문 `{code,msg}` 파싱. 비-JSON이면 code=0 + 원문.
async fn parse_api_error(resp: reqwest::Response) -> (i64, String) {
    let text = resp.text().await.unwrap_or_default();
    match serde_json::from_str::<Value>(&text) {
        Ok(v) => {
            let code = v.get("code").and_then(Value::as_i64).unwrap_or(0);
            let msg = v
                .get("msg")
                .and_then(Value::as_str)
                .unwrap_or(&text)
                .to_string();
            (code, msg)
        }
        Err(_) => (0, text),
    }
}

/// 429/418 대기 시간(초). `Retry-After`(정수초) → 기본 1초. [1,300] 클램프.
fn retry_after_secs(resp: &reqwest::Response) -> u64 {
    resp.headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(1)
        .clamp(1, 300)
}

/// 최소 application/x-www-form-urlencoded 인코딩 (RFC 3986 unreserved 외 % 이스케이프).
/// Binance 파라미터 값은 영숫자·심볼 위주라 대부분 그대로지만, 안전을 위해 인코딩한다.
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

    // Binance 공식 문서의 표준 HMAC-SHA256 서명 테스트 벡터.
    // (Signed Endpoint Examples for POST /api/v3/order)
    // 서명기가 정확함을 네트워크 없이 검증한다 — 이 한 줄이 거래 안전의 핵심.
    #[test]
    fn hmac_matches_binance_doc_vector() {
        let secret = "NhqPtmdSJYdKjVHjA7PZj4Mge3R5YNiP1e3UZjInClVN65XAbvqqM6A7H5fATj0j";
        let payload = "symbol=LTCBTC&side=BUY&type=LIMIT&timeInForce=GTC&quantity=1&price=0.1&recvWindow=5000&timestamp=1499827319559";
        let sig = BinanceClient::sign(secret, payload).unwrap();
        assert_eq!(
            sig,
            "c8db56825ae71d6d79447849e617115f4a920fa2acdcab2b053c4b2838bd6b71"
        );
    }

    #[test]
    fn encode_query_preserves_order() {
        let pairs = vec![
            ("symbol".to_string(), "SAMSUNGUSDT".to_string()),
            ("side".to_string(), "BUY".to_string()),
        ];
        assert_eq!(
            BinanceClient::encode_query(&pairs),
            "symbol=SAMSUNGUSDT&side=BUY"
        );
    }

    #[test]
    fn urlencode_escapes_reserved() {
        // 콤마(symbols 배열 등)는 %2C로 이스케이프.
        assert_eq!(urlencode("A,B"), "A%2CB");
        assert_eq!(urlencode("SAMSUNGUSDT"), "SAMSUNGUSDT");
    }
}
