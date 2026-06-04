use thiserror::Error;

/// WEEX Contract API 어댑터 전역 에러.
///
/// Binance/MEXC와 동일하게 형제 모듈로 병렬 신설한다. WEEX 에러 본문은
/// `{ "code": "40020", "msg": "参数limit错误", "requestTime": 1780548561597,
/// "data": null }` 형태로 **문자열 코드** + 메시지이며(Binance는 음수 정수,
/// MEXC는 `{code:int,...}`), 필드 구조가 겹치지 않는다.
#[derive(Error, Debug)]
pub enum WeexError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿/패스프레이즈 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC 서명 생성 실패 (시크릿 키 길이 등).
    #[error("sign: {0}")]
    Sign(String),

    /// WEEX 에러 응답 (`{code,msg}`). `code`는 문자열 식별자(예 "40020").
    #[error("api error (http {status}) code={code}: {msg}")]
    Api {
        status: u16,
        code: String,
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

/// WEEX 어댑터 전용 결과 타입.
pub type Result<T> = std::result::Result<T, WeexError>;
