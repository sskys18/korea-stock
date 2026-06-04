use thiserror::Error;

/// Lighter(zkLighter) 어댑터 전역 에러.
///
/// KIS/Toss/Binance와 동일하게 형제 모듈로 병렬 신설한다. Lighter REST 에러 본문은
/// `{ "code": 20001, "message": "invalid param " }` 형태의 flat 정수 코드 + 메시지다
/// (HTTP 200에 에러 envelope를 싣는 경우도 있어 status만으로 성공을 판정하지 않고
/// body의 `code`도 함께 본다). 성공 응답도 `code`를 포함한다(예: 200).
#[derive(Error, Debug)]
pub enum LighterError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// 자격증명 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// **서명기 미검증** — 거래 서명 제출이 차단됐다.
    ///
    /// Lighter 주문 서명은 zk 친화 해시(poseidon)+schnorr 류 커스텀 스킴이며, 공식
    /// `lighter-python`·비공식 `lighter-rust` 모두 네이티브 Go 바이너리(`lighter-go`)를
    /// FFI로 호출해 서명한다. crates.io에 순수 Rust 구현이 없고 이 세션에서 알고리즘을
    /// 공식 문서로 재현 검증할 수 없어, **서명을 날조하지 않고**(HARD RULE 3) 제출을
    /// 명시적으로 거부한다. 서명이 추가되기 전까지 본 어댑터는 data-only다.
    #[error("signer unavailable: {0}")]
    SignerUnavailable(String),

    /// Lighter REST 에러 응답 (`{code,message}`). `code`는 정수 식별자.
    /// 예: 20001 invalid param. HTTP status도 함께 보존한다.
    #[error("api error (http {status}) code={code}: {message}")]
    Api {
        status: u16,
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

/// Lighter 어댑터 전용 결과 타입. KIS/Toss/Binance와 분리.
pub type Result<T> = std::result::Result<T, LighterError>;
