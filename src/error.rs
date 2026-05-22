use thiserror::Error;

/// KIS 어댑터 전역 에러.
#[derive(Error, Debug)]
pub enum KisError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    #[error("auth: {0}")]
    Auth(String),

    /// KIS 응답 `rt_cd != "0"`.
    #[error("api error rt_cd={rt_cd} msg_cd={msg_cd}: {msg}")]
    Api {
        rt_cd: String,
        msg_cd: String,
        msg: String,
    },

    #[error("rate limited")]
    RateLimit,

    #[error("websocket: {0}")]
    Ws(String),

    #[error("decode: {0}")]
    Decode(String),

    /// 모의투자 환경에서 미지원 TR 호출.
    #[error("unsupported in mock environment: {tr_id}")]
    UnsupportedInMock { tr_id: String },

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, KisError>;
