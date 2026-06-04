use thiserror::Error;

/// Binance USDM Futures 어댑터 전역 에러.
///
/// KIS/Toss와 동일하게 형제 모듈로 병렬 신설한다. Binance 에러 본문은
/// `{ "code": -1121, "msg": "Invalid symbol." }` 형태의 flat 정수 코드 +
/// 메시지이며, KIS(`rt_cd/msg_cd`)·Toss(BFF `code/message`)와 필드 구조가
/// 전혀 겹치지 않는다.
#[derive(Error, Debug)]
pub enum BinanceError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC 서명 생성 실패 (시크릿 키 길이 등).
    #[error("sign: {0}")]
    Sign(String),

    /// Binance 에러 응답 (`{code,msg}`). `code`는 음수 정수 식별자.
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

/// Binance 어댑터 전용 결과 타입. KIS/Toss와 분리.
pub type Result<T> = std::result::Result<T, BinanceError>;
