use thiserror::Error;

/// Hyperliquid 어댑터 전역 에러.
///
/// KIS/Toss/Binance와 동일하게 형제 모듈로 병렬 신설한다. Hyperliquid는 두 종류의
/// 실패 표면을 가진다:
/// - `POST /info`(시세): HTTP 200 + JSON 페이로드. 잘못된 요청은 HTTP 4xx + 평문/JSON.
/// - `POST /exchange`(거래): HTTP 200이라도 바디가
///   `{"status":"err","response":"..."}`이거나
///   `{"status":"ok","response":{"type":"order","data":{"statuses":[{"error":"..."}]}}}`
///   처럼 **본문 내부에** 에러를 담는다. 따라서 HTTP status만으로 성공을 판정하면 안 된다.
#[derive(Error, Debug)]
pub enum HyperliquidError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),

    /// 비밀키 누락 등 서명 전 단계 실패.
    #[error("auth: {0}")]
    Auth(String),

    /// 서명 생성 실패 (키 파싱·msgpack·해시·ECDSA).
    #[error("sign: {0}")]
    Sign(String),

    /// `POST /exchange` 거부 — 바디 `{"status":"err", ...}` 또는 개별 주문 status의 error.
    #[error("exchange error: {0}")]
    Exchange(String),

    /// HTTP 4xx/5xx (요청 형식 오류·서버 장애 등). 바디 원문 보존.
    #[error("api error (http {status}): {body}")]
    Api { status: u16, body: String },

    /// 역직렬화 단계 실패.
    #[error("decode: {0}")]
    Decode(String),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Hyperliquid 어댑터 전용 결과 타입. KIS/Toss/Binance와 분리.
pub type Result<T> = std::result::Result<T, HyperliquidError>;
