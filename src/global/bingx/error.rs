use thiserror::Error;

/// BingX Perpetual Swap V2 어댑터 전역 에러.
///
/// Binance와 동일하게 형제 모듈로 병렬 신설한다. BingX 응답은
/// `{ "code": 0, "msg": "", "data": {...} }` envelope를 쓰며 `code`는 **정수**
/// (성공=0). Binance의 flat `{code,msg}`(음수 코드)와 달리 페이로드가 `data`로
/// 한 겹 감싸진다 — [`crate::global::bingx::client`]에서 풀어준다.
#[derive(Error, Debug)]
pub enum BingxError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC 서명 생성 실패 (시크릿 키 길이 등).
    #[error("sign: {0}")]
    Sign(String),

    /// BingX 에러 응답 (`{code,msg,data}`). `code`는 0 이외의 정수 식별자.
    /// 예: 100400 파라미터 오류, 100413 잘못된 심볼, 80001 서명 오류.
    #[error("api error (http {status}) code={code}: {msg}")]
    Api {
        status: u16,
        code: i64,
        msg: String,
    },

    /// 역직렬화/envelope 단계 실패 (시세 stub 응답 포함).
    #[error("decode: {0}")]
    Decode(String),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// BingX 어댑터 전용 결과 타입. 다른 venue와 분리.
pub type Result<T> = std::result::Result<T, BingxError>;
