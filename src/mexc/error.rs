use thiserror::Error;

/// MEXC Contract 어댑터 전역 에러.
///
/// KIS/Toss/Binance와 동일하게 형제 모듈로 병렬 신설한다. MEXC Contract는 비즈니스
/// 실패를 **HTTP 200 + 본문 `{ "success": false, "code": <n>, "message"|"msg": ... }`**
/// 로 내려준다(KIS의 `rt_cd` 방식에 가깝고 Binance의 음수 코드 + HTTP status 방식과 다름).
/// `code`는 양의 정수 식별자다.
#[derive(Error, Debug)]
pub enum MexcError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC 서명 생성 실패 (시크릿 키 길이 등).
    #[error("sign: {0}")]
    Sign(String),

    /// MEXC 에러 응답 (`{success:false, code, message}`). `code`는 양수 정수 식별자.
    /// 예: 600 파라미터 오류, 602 서명 실패, 1002 contract 미존재, 점검(주문 중단) 코드 등.
    /// `http`는 실제 HTTP status(대개 200 — 비즈니스 에러도 200으로 온다).
    #[error("api error (http {http}) code={code}: {message}")]
    Api {
        http: u16,
        code: i64,
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

/// MEXC 어댑터 전용 결과 타입. KIS/Toss/Binance와 분리.
pub type Result<T> = std::result::Result<T, MexcError>;
