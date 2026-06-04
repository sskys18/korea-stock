use thiserror::Error;

/// KuCoin Futures 어댑터 전역 에러.
///
/// Binance/Bybit/MEXC와 동일하게 형제 모듈로 병렬 신설한다(공유 트레이트 없음).
/// KuCoin은 비즈니스 결과를 **본문 envelope `{ "code": "<문자열>", "data": .., "msg": .. }`**
/// 로 내려준다. **`code == "200000"`(문자열)이 성공**, 그 외는 [`KucoinError::Api`].
/// (Bybit의 정수 `retCode==0`와 달리 KuCoin의 `code`는 문자열임에 주의.)
#[derive(Error, Debug)]
pub enum KucoinError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿/패스프레이즈 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC 서명 생성 실패 (시크릿 키 길이 등).
    #[error("sign: {0}")]
    Sign(String),

    /// KuCoin 에러 응답 (`{code, msg}`). `code`는 "200000"이 아닌 문자열 식별자.
    /// 예: "400100" 파라미터 오류, "400003" 키 오류, "404" not exist 등.
    /// `http`는 실제 HTTP status (KuCoin은 일부 에러를 4xx로도 내려준다).
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

/// KuCoin 어댑터 전용 결과 타입. 다른 venue와 분리.
pub type Result<T> = std::result::Result<T, KucoinError>;
