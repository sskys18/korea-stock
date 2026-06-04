use thiserror::Error;

/// Gate.io APIv4 USDT Futures 어댑터 전역 에러.
///
/// 다른 venue와 동일하게 형제 모듈로 병렬 신설한다. Gate 에러 본문은
/// `{ "label": "CONTRACT_NOT_FOUND", "message": "..." }` 형태의 문자열 라벨 +
/// 메시지이며(Binance의 음수 정수 `code`와 다름), HTTP status로도 구분된다.
#[derive(Error, Debug)]
pub enum GateioError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC-SHA512 서명 생성 실패.
    #[error("sign: {0}")]
    Sign(String),

    /// Gate 에러 응답 (`{label,message}`). `label`은 문자열 식별자.
    /// 예: `CONTRACT_NOT_FOUND`, `INVALID_PARAM_VALUE`, `BALANCE_NOT_ENOUGH`.
    #[error("api error (http {status}) label={label}: {message}")]
    Api {
        status: u16,
        label: String,
        message: String,
    },

    /// 역직렬화 단계 실패.
    #[error("decode: {0}")]
    Decode(String),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Gate.io 어댑터 전용 결과 타입. 다른 venue와 분리.
pub type Result<T> = std::result::Result<T, GateioError>;
