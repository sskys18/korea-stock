use thiserror::Error;

/// Bitget v2 mix 어댑터 전역 에러.
///
/// KIS/Toss/Binance/Bybit/MEXC와 동일하게 형제 모듈로 병렬 신설한다. Bitget v2는 비즈니스
/// 실패를 **HTTP 200 + 본문 `{ "code": "<n>", "msg": "..", "data": {..} }`**로 내려준다
/// (Bybit의 `retCode` 방식과 동형, Binance의 음수 코드 + HTTP status 방식과 다름).
/// **`code=="00000"`이 성공**이고, 그 외 문자열 코드는 에러 식별자다.
#[derive(Error, Debug)]
pub enum BitgetError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿/패스프레이즈 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC 서명 생성 실패 (시크릿 키 길이 등).
    #[error("sign: {0}")]
    Sign(String),

    /// Bitget 에러 응답 (`{code, msg}`). `code`는 "00000"이 아닌 문자열 식별자.
    /// 예: 40009 서명 오류, 40037 잘못된 apiKey, 40034 파라미터 오류 등.
    /// `http`는 실제 HTTP status(대개 200 — 비즈니스 에러도 200으로 온다).
    #[error("api error (http {http}) code={code}: {msg}")]
    Api { http: u16, code: String, msg: String },

    /// 역직렬화 단계 실패.
    #[error("decode: {0}")]
    Decode(String),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Bitget 어댑터 전용 결과 타입. KIS/Toss/Binance/Bybit/MEXC와 분리.
pub type Result<T> = std::result::Result<T, BitgetError>;
