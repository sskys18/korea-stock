use thiserror::Error;

/// Bybit v5 어댑터 전역 에러.
///
/// KIS/Toss/Binance/MEXC와 동일하게 형제 모듈로 병렬 신설한다. Bybit v5는 비즈니스
/// 실패를 **HTTP 200 + 본문 `{ "retCode": <n>, "retMsg": "..", "result": {..} }`**
/// 로 내려준다(MEXC의 `success`/`code` 방식과 동형, Binance의 음수 코드 + HTTP status
/// 방식과 다름). `retCode==0`이 성공이고 그 외는 에러 식별자다.
#[derive(Error, Debug)]
pub enum BybitError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// API 키/시크릿 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// HMAC 서명 생성 실패 (시크릿 키 길이 등).
    #[error("sign: {0}")]
    Sign(String),

    /// Bybit 에러 응답 (`{retCode, retMsg}`). `ret_code`는 0이 아닌 정수 식별자.
    /// 예: 10001 파라미터 오류, 10003/10004 키/서명 오류, 110001 주문 미존재 등.
    /// `http`는 실제 HTTP status(대개 200 — 비즈니스 에러도 200으로 온다).
    #[error("api error (http {http}) retCode={ret_code}: {ret_msg}")]
    Api {
        http: u16,
        ret_code: i64,
        ret_msg: String,
    },

    /// 역직렬화 단계 실패.
    #[error("decode: {0}")]
    Decode(String),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Bybit 어댑터 전용 결과 타입. KIS/Toss/Binance/MEXC와 분리.
pub type Result<T> = std::result::Result<T, BybitError>;
