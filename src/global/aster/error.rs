use thiserror::Error;

/// Aster fapi 어댑터 전역 에러.
///
/// Aster의 fapi 에러 본문은 Binance와 동일하게 `{ "code": -1121, "msg": "..." }`
/// 형태의 flat 정수 코드 + 메시지이며(Binance fapi 호환 표면), KIS/Toss와 필드
/// 구조가 겹치지 않는다. 공유 트레이트 없음 — venue 독립 에러.
#[derive(Error, Debug)]
pub enum AsterError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC 서명 생성 실패 (시크릿 키 길이 등).
    #[error("sign: {0}")]
    Sign(String),

    /// Aster 에러 응답 (`{code,msg}`). `code`는 음수 정수 식별자.
    /// 예: -1121 Invalid symbol, -2019 Margin is insufficient, -1021 timestamp.
    #[error("api error (http {status}) code={code}: {msg}")]
    Api {
        status: u16,
        code: i64,
        msg: String,
    },

    /// 역직렬화 단계 실패.
    #[error("decode: {0}")]
    Decode(String),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Aster 어댑터 전용 결과 타입. 다른 venue와 분리.
pub type Result<T> = std::result::Result<T, AsterError>;
