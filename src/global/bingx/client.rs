use std::time::Duration;

use hmac::{Hmac, Mac};
use reqwest::{Method, StatusCode};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::Sha256;

use crate::global::bingx::config::BingxConfig;
use crate::global::bingx::error::{BingxError, Result};
use crate::ratelimit::RateLimiter;

type HmacSha256 = Hmac<Sha256>;

/// 미구현/미래 엔드포인트 직접 호출용 저수준 요청. 타입 안전성 없음.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: Method,
    /// "/openApi/swap/v2/..." 경로 (쿼리 제외).
    pub path: String,
    /// 파라미터 (서명 대상 포함). **순서 보존** — BingX는 삽입순 queryString을
    /// 서명·전송 모두 동일하게 본다(알파벳 정렬 불필요).
    pub params: Vec<(String, String)>,
    /// true면 `timestamp`+HMAC `signature`를 부착하고 `X-BX-APIKEY` 헤더를 보낸다.
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
    pub(crate) fn public(
        method: Method,
        path: impl Into<String>,
        params: Vec<(String, String)>,
    ) -> Self {
        Self {
            method,
            path: path.into(),
            params,
            signed: false,
        }
    }

    /// 서명 필요 거래/계좌 호출.
    pub(crate) fn signed(
        method: Method,
        path: impl Into<String>,
        params: Vec<(String, String)>,
    ) -> Self {
        Self {
            method,
            path: path.into(),
            params,
            signed: true,
        }
    }
}

/// HTTP 응답 — BingX는 `{code,msg,data}` envelope의 **`data`만** 담는다.
/// (envelope 검사·언래핑은 [`BingxClient::call_once`]에서 끝낸다.)
pub(crate) struct RawResponse {
    pub data: Value,
}

impl RawResponse {
    pub fn parse<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_value(self.data.clone())?)
    }
}

/// BingX Perpetual Swap V2 클라이언트.
///
/// 시세 액세서(`market`)는 키 없이, 거래 액세서(`trade`)는 HMAC-SHA256(hex) 서명으로
/// 호출한다. Binance식 서명 — 전체 파라미터(`timestamp` 포함)를 삽입순 queryString으로
/// 만들고 `signature=HEX(HMAC_SHA256(secret, queryString))`을 덧붙인다. 매 서명
/// 요청마다 `timestamp`를 새로 찍는다.
pub struct BingxClient {
    config: BingxConfig,
    http: reqwest::Client,
    limiter: Option<RateLimiter>,
}

impl BingxClient {
    /// 클라이언트 생성.
    pub fn new(config: BingxConfig) -> Result<Self> {
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
    pub fn market(&self) -> crate::global::bingx::market::Market<'_> {
        crate::global::bingx::market::Market::new(self)
    }

    /// 거래·계좌 도메인 액세서 (HMAC 서명).
    pub fn trade(&self) -> crate::global::bingx::trade::Trade<'_> {
        crate::global::bingx::trade::Trade::new(self)
    }

    /// 미구현/미래 엔드포인트 직접 호출. 응답은 envelope의 `data`(raw JSON).
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

    /// 현재 UTC epoch milliseconds. 서명 `timestamp` 용.
    fn timestamp_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// 파라미터를 `k=v&k=v` 문자열로 직렬화. **삽입순 보존** — 서명·전송이 동일
    /// 문자열을 봐야 한다(BingX는 알파벳 정렬을 요구하지 않는다).
    fn encode_query(pairs: &[(String, String)]) -> String {
        pairs
            .iter()
            .map(|(k, v)| format!("{}={}", urlencode(k), urlencode(v)))
            .collect::<Vec<_>>()
            .join("&")
    }

    /// HMAC-SHA256(secret, payload) → hex 소문자. BingX 서명기.
    fn sign(secret: &str, payload: &str) -> Result<String> {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
            .map_err(|e| BingxError::Sign(e.to_string()))?;
        mac.update(payload.as_bytes());
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    /// 도메인 공용 호출. 429(레이트) 반응형 백오프 + 시세 stub(스키마 문서 스텁)
    /// 재시도.
    pub(crate) async fn call(&self, c: ApiCall) -> Result<RawResponse> {
        const MAX_RETRIES: u32 = 4;
        let mut attempt = 0;
        loop {
            match self.call_once(&c).await {
                Err(BingxError::Api { status, .. }) if status == 429 && attempt < MAX_RETRIES => {
                    tracing::warn!("429 rate limited — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
                // BingX CDN이 간헐적으로 시세 응답 대신 비-JSON "스키마 스텁"
                // (`{ code: int, data: {...}, msg: string }` 형태의 타입 설명)을
                // 돌려준다. JSON 디코드 실패 시 재시도.
                Err(BingxError::Decode(_)) if attempt < MAX_RETRIES => {
                    tracing::warn!("decode/stub response — retry {}/{}", attempt + 1, MAX_RETRIES);
                    attempt += 1;
                    tokio::time::sleep(Duration::from_millis(150)).await;
                }
                other => return other,
            }
        }
    }

    /// 단일 호출. (선택)레이트캡 → 파라미터/서명 조립 → 전송 → status 분기 →
    /// envelope 언래핑.
    async fn call_once(&self, c: &ApiCall) -> Result<RawResponse> {
        if let Some(limiter) = &self.limiter {
            limiter.acquire().await;
        }

        let mut pairs = c.params.clone();
        if c.signed {
            if self.config.api_key.is_empty() || self.config.api_secret.is_empty() {
                return Err(BingxError::Auth(
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
        // 키가 있으면 항상 첨부 (시세도 키가 있으면 더 높은 레이트 한도).
        if !self.config.api_key.is_empty() {
            req = req.header("X-BX-APIKEY", &self.config.api_key);
        }

        let resp = req.send().await?;
        let status = resp.status();

        if status == StatusCode::TOO_MANY_REQUESTS {
            let (code, msg) = parse_api_error(resp).await;
            return Err(BingxError::Api {
                status: status.as_u16(),
                code,
                msg,
            });
        }

        if status.is_success() {
            let text = resp.text().await?;
            let body: Value = serde_json::from_str(&text)
                .map_err(|e| BingxError::Decode(format!("{e}: {}", truncate(&text, 200))))?;
            // 비즈니스 에러 envelope: `{code,msg,data}` with non-zero integer code.
            let code = body.get("code").and_then(Value::as_i64);
            if let Some(code) = code {
                if code != 0 {
                    let msg = body
                        .get("msg")
                        .and_then(Value::as_str)
                        .unwrap_or("")
                        .to_string();
                    return Err(BingxError::Api {
                        status: status.as_u16(),
                        code,
                        msg,
                    });
                }
            } else {
                // `code`가 정수가 아니면 envelope가 아니라 스키마 스텁 — 재시도 유도.
                return Err(BingxError::Decode(format!(
                    "non-envelope response: {}",
                    truncate(&text, 200)
                )));
            }
            // 성공 — `data`만 꺼내 반환. data가 없으면 Null(빈 응답 허용).
            let data = body.get("data").cloned().unwrap_or(Value::Null);
            return Ok(RawResponse { data });
        }

        let http_status = status.as_u16();
        let (code, msg) = parse_api_error(resp).await;
        Err(BingxError::Api {
            status: http_status,
            code,
            msg,
        })
    }
}

/// BingX 에러 본문 `{code,msg}` 파싱. `code`는 정수. 비-JSON이면 code=0 + 원문.
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

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        let mut end = n;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}…", &s[..end])
    }
}

/// 최소 application/x-www-form-urlencoded 인코딩 (RFC 3986 unreserved 외 % 이스케이프).
/// BingX 심볼은 하이픈을 포함하지만 unreserved라 그대로 통과한다.
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

    // BingX 공식 문서(BingX-Standard-Contract-doc/REST API.md)의 HMAC-SHA256 서명
    // 테스트 벡터. 서명기가 정확함을 네트워크 없이 검증한다 — 거래 안전의 핵심.
    // 독립 검산: printf '%s' "<payload>" | openssl dgst -sha256 -hmac "<secret>" -hex
    #[test]
    fn hmac_matches_bingx_doc_vector() {
        let secret = "UuGuyEGt6ZEkpUObCYCmIfh0elYsZVh80jlYwpJuRZEw70t6vomMH7Sjmf94ztSI";
        let payload =
            "quoteOrderQty=20&side=BUY&symbol=ETHUSDT&timestamp=1649404670162&type=MARKET";
        let sig = BingxClient::sign(secret, payload).unwrap();
        assert_eq!(
            sig,
            "428a3c383bde514baff0d10d3c20e5adfaacaf799e324546dafe5ccc480dd827"
        );
    }

    #[test]
    fn encode_query_preserves_insertion_order() {
        // BingX는 알파벳 정렬을 요구하지 않는다 — 삽입순 그대로 서명·전송.
        let pairs = vec![
            ("symbol".to_string(), "NCSKSAMSUNG2USD-USDT".to_string()),
            ("side".to_string(), "BUY".to_string()),
        ];
        assert_eq!(
            BingxClient::encode_query(&pairs),
            "symbol=NCSKSAMSUNG2USD-USDT&side=BUY"
        );
    }

    #[test]
    fn urlencode_passes_hyphen_escapes_reserved() {
        // KR 심볼의 하이픈은 unreserved라 그대로.
        assert_eq!(urlencode("NCSKSAMSUNG2USD-USDT"), "NCSKSAMSUNG2USD-USDT");
        // 콤마는 %2C로 이스케이프.
        assert_eq!(urlencode("A,B"), "A%2CB");
    }
}
