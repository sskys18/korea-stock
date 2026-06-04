use thiserror::Error;

/// Bitunix Futures 어댑터 전역 에러.
///
/// KIS/Toss/Binance/MEXC와 동일하게 형제 모듈로 병렬 신설한다. Bitunix는 비즈니스
/// 에러를 HTTP status가 아니라 본문 envelope `{ "code": <int>, "msg": "...", "data": ... }`
/// 로 내려준다(MEXC식). `code==0`이 성공이며, 그 외엔 [`BitunixError::Api`]로 매핑한다.
#[derive(Error, Debug)]
pub enum BitunixError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// 서명(이중 SHA256) 생성 중 실패. (nonce 난수 생성 실패 등.)
    #[error("sign: {0}")]
    Sign(String),

    /// Bitunix 에러 응답 (`{code,msg}`). `code`는 비-0 정수 식별자.
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

/// Bitunix 어댑터 전용 결과 타입. KIS/Toss/Binance/MEXC와 분리.
pub type Result<T> = std::result::Result<T, BitunixError>;
