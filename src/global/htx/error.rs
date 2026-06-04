use thiserror::Error;

/// HTX(Huobi) USDT-M Linear Swap 어댑터 전역 에러.
///
/// KIS/Toss/Binance/MEXC와 동일하게 형제 모듈로 병렬 신설한다. HTX 에러 본문은
/// envelope `{ "status": "error", "err_code": "...", "err_msg": "..." }`(또는
/// 일부 시세 채널은 `"err-code"`/`"err-msg"` 하이픈) 형태이며, Binance(flat
/// `{code,msg}`)·MEXC(`{success,code}`)와 필드 구조가 겹치지 않는다.
#[derive(Error, Debug)]
pub enum HtxError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC 서명 생성 실패 (시크릿 키 길이 등).
    #[error("sign: {0}")]
    Sign(String),

    /// HTX 에러 응답 (`{status:"error", err_code, err_msg}`).
    /// `code`는 문자열 식별자(예: "1003" invalid signature, "1002" auth required).
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

/// HTX 어댑터 전용 결과 타입. KIS/Toss/Binance/MEXC와 분리.
pub type Result<T> = std::result::Result<T, HtxError>;
