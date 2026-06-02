use thiserror::Error;

/// 토스증권 어댑터 전역 에러.
///
/// KIS의 단일 `KisError`를 확장하지 않고 병렬 신설한다. 사유는 설계 문서
/// `docs/specs/2026-06-02-toss-adapter-design.md` §3.3 참조 —
/// 토스 에러(BFF `{requestId,code,message}` / OAuth2 `{error,error_description}`)는
/// KIS 에러(`{rt_cd,msg_cd,msg}`)와 필드 구조가 전혀 겹치지 않으며, `KisError`라는
/// 이름이 KIS 도메인 전용이기 때문이다.
#[derive(Error, Debug)]
pub enum TossError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// 토큰 발급·캐시 처리 실패 (전송 외 사유).
    #[error("auth: {0}")]
    Auth(String),

    /// `/oauth2/token` 4xx/5xx — OAuth2 표준 에러. BFF envelope 아님.
    #[error("oauth2 error {error}: {}", description.as_deref().unwrap_or(""))]
    OAuth2 {
        error: String,
        description: Option<String>,
    },

    /// BFF 공통 에러 envelope (`ErrorResponse.error`). 4xx/5xx.
    /// `code`는 flat string 식별자 — 클라이언트는 unknown code를 허용해야 한다.
    #[error("api error (http {status}) code={code}: {message}")]
    Api {
        status: u16,
        /// 응답 헤더 `X-Request-Id` / body `requestId`. 토스 CS 문의용.
        request_id: Option<String>,
        code: String,
        message: String,
    },

    /// envelope/필드 누락 등 역직렬화 단계 실패.
    #[error("decode: {0}")]
    Decode(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// 토스 도메인 전용 결과 타입. KIS의 `crate::error::Result`와 분리.
pub type Result<T> = std::result::Result<T, TossError>;
