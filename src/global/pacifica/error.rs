use thiserror::Error;

/// Pacifica(Solana perp DEX) 어댑터 전역 에러.
///
/// KIS/Toss/Binance/Lighter와 동일하게 형제 모듈로 병렬 신설한다. Pacifica REST
/// 본문은 `{ "success": bool, "data": ..., "error": string|null, "code": int|null }`
/// 봉투다. 성공도 `success:true`로 표시되며 실패 시 `error`/`code`에 사유가 담긴다 —
/// HTTP status만으로 성공을 판정하지 않고 `success`도 함께 본다.
#[derive(Error, Debug)]
pub enum PacificaError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// 시크릿 키 누락·형식 오류 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// **서명기 미검증 게이트** — 거래 서명 제출이 차단됐다.
    ///
    /// Pacifica 주문 서명(Ed25519 over sorted-JSON)은 공식 `python-sdk`와 교차
    /// 검증된 골든 벡터로 메시지 정규화·암호 코어가 바이트 일치함을 확인했다. 다만
    /// 실제 키 없이 서버가 주문을 수락하는 end-to-end 경로는 미검증이다. 따라서
    /// 실주문은 [`crate::global::pacifica::PacificaConfig::allow_unverified_signing`]
    /// 게이트(기본 false) 뒤에 두고, 닫혀 있으면 이 에러로 거부한다.
    #[error("signer gated (unverified e2e): {0}")]
    SignerUnavailable(String),

    /// Ed25519 서명 생성 실패 (키 길이·base58 디코드 등).
    #[error("sign: {0}")]
    Sign(String),

    /// Pacifica 에러 응답 (`{success:false, error, code}`). HTTP status도 보존한다.
    #[error("api error (http {status}) code={code:?}: {error}")]
    Api {
        status: u16,
        code: Option<i64>,
        error: String,
    },

    /// 역직렬화 단계 실패.
    #[error("decode: {0}")]
    Decode(String),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Pacifica 어댑터 전용 결과 타입. 타 venue와 분리.
pub type Result<T> = std::result::Result<T, PacificaError>;
