use thiserror::Error;

/// Phemex Contract(USDT Perpetual v2) 어댑터 전역 에러.
///
/// KIS/Toss/Binance/MEXC와 동일하게 형제 모듈로 병렬 신설한다. Phemex 응답은
/// 도메인별로 envelope이 다르다 — 거래/계좌는 `{ "code": 0, "data": ..., "msg": "" }`
/// (비즈니스 에러도 HTTP 200으로 `code != 0`), 시세는 `{ "error": null, "id": 0,
/// "result": ... }`. 둘 다 본문으로 성공/실패를 판정한다(MEXC 방식).
#[derive(Error, Debug)]
pub enum PhemexError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC 서명 생성 실패 (시크릿 base64url 디코드/키 길이 등).
    #[error("sign: {0}")]
    Sign(String),

    /// Phemex 에러 응답. `code`는 비-0 비즈니스 코드(또는 HTTP status).
    /// 예: 10500 internal, 11001 symbol not found, 401 unauthorized.
    #[error("api error (http {http}) code={code}: {msg}")]
    Api { http: u16, code: i64, msg: String },

    /// 역직렬화 단계 실패.
    #[error("decode: {0}")]
    Decode(String),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Phemex 어댑터 전용 결과 타입. KIS/Toss/Binance/MEXC와 분리.
pub type Result<T> = std::result::Result<T, PhemexError>;
