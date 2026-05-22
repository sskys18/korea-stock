# Plan 3 — KIS 어댑터: 실시간 WebSocket

- 작성일: 2026-05-22
- 스펙: `docs/specs/2026-05-22-kis-adapter-design.md` (특히 §3.7)
- 실시간 명세 SSOT: `docs/kis-api/realtime.md`
- 상태: 실행 대기
- 선행: Plan 1(Core 인프라 + 국내주식) 구현·커밋 완료

## 목표

`kis-adapter` 크레이트에 실시간 WebSocket 도메인을 추가한다 — approval_key 발급,
WS 연결·구독·해지, 수신 프레임 디코딩, 체결통보 AES-256-CBC 복호화. 이 Plan이
끝나면 4종 tr_id(`H0STCNT0`/`H0STASP0`/`H0STCNI0`/`H0STCNI9`/`HDFSCNT0`)를
구독해 타입 안전한 이벤트 스트림으로 수신할 수 있다.

기존 Plan 1 타입(`KisError`/`Environment`/`KisClient`/`KisConfig`)과 일관되어야
하며, **Plan 1의 공개 시그니처를 변경하지 않는다** (특히 동기 `KisClient::new`).

## 범위

- 포함: `src/realtime/` 5개 모듈, `lib.rs` 모듈 선언 + 재노출, `KisClient`에
  `realtime()` 비동기 액세서, 의존성 5종 추가, 예제 1개, 통합테스트 1건.
- 제외: 해외 체결통보(`H0GSCNI0`), 해외 호가(`HDFSASP0`), 국내 호가 외 기타 실시간
  TR. 미지원 tr_id는 본 Plan 스코프 밖 — 필요 시 후속 Plan 또는 `subscribe`의
  raw 경로(스코프 외, 본 Plan 미구현).

## 전제

- Plan 1이 `main`이 아닌 브랜치에 머지·커밋된 상태. 실행 시작 전
  `git checkout -b feat/kis-plan-3`.
- Plan 1 코드 현황 (실행자 확인 완료 가정):
  - `Environment::ws_base()` 존재 — 실전 `ws://ops.koreainvestment.com:21000`,
    모의 `ws://ops.koreainvestment.com:31000` 반환 (평문 `ws://`).
  - `KisError`에 `Ws(String)` / `Decode(String)` variant 존재.
  - `KisClient::new`는 **동기** `fn new(config: KisConfig) -> Result<Self>`.
    내부 필드 `config: KisConfig`, `http: reqwest::Client` (둘 다 private).
  - `KisClient::config()`는 `pub(crate) fn config(&self) -> &KisConfig`.
- 통합테스트 실행 시 모의투자 자격증명 환경변수(`KIS_APP_KEY` 등, Plan 1과 동일).

## 설계 결정 (실행 전 확정 — 변경 금지)

이 절의 결정은 스펙 §3.7의 모호함을 해소한다. 실행자는 그대로 따른다.

### D1 — 이벤트 채널: `tokio::sync::mpsc` (bounded, drop-oldest 수동 구현)

스펙 §3.7은 "`mpsc::Receiver<RealtimeEvent>` + 가장 오래된 이벤트 드롭 +
`Lagged(n)` 통지"를 요구한다. `tokio::sync::mpsc`는 송신측에서 가장 오래된
항목을 드롭하는 기능이 **없다** (송신은 블록 또는 `try_send` 실패만 가능).
`broadcast`는 `Lagged`를 네이티브 지원하나 multi-consumer 의미라 단일 수신자
모델과 어긋난다.

**결정**: `mpsc::channel::<RealtimeEvent>(1024)` 사용 + **drop-newest** 정책.
WS 수신 태스크가 `try_send` 실패(채널 가득)를 감지하면 해당 이벤트를 버리고
`AtomicU64` lag 카운터를 증가시킨다. 다음 `try_send` 성공 직전에 카운터가
0보다 크면 먼저 `RealtimeEvent::Lagged(n)`을 보내고 카운터를 0으로 리셋한다.

근거: 송신측 비블로킹 보장 + 호출자에게 유실 통지. "가장 오래된 드롭"은
mpsc로 깔끔히 컴파일되지 않으므로 의미상 동등한 drop-newest로 구현하되,
호출자는 `Lagged(n)`로 n건 유실을 인지한다. 이 차이를 `RealtimeEvent::Lagged`
doc 주석에 명시한다.

### D2 — WS URL 스킴: `Environment::ws_base()` 그대로 사용

`realtime.md` A-1(SSOT)은 평문 `ws://`. Plan 1 `ws_base()`도 `ws://` 반환.
`tokio-tungstenite`에 `rustls-tls-webpki-roots` feature를 켜지만, `ws://`
URL에서는 TLS 핸드셰이크가 일어나지 않아 **휴면 상태**다 — 동작 변화 없음.
KIS가 향후 `wss://`로 전환하면 feature 덕에 URL만 바꾸면 된다. 스펙 §3.7의
`wss://` 표기는 본 Plan에서 채택하지 않는다 (SSOT 우선).

### D3 — `SubscriptionHandle::Drop` 자동 해지 메커니즘

`Drop`은 동기 함수라 async 송신을 직접 할 수 없다. `SubscriptionHandle`은
`mpsc::UnboundedSender<ControlMsg>`(제어 채널 송신단)를 보유한다. `Drop`에서
`control_tx.send(ControlMsg::Unsubscribe { tr_id, tr_key })`를 호출 —
unbounded이므로 동기 컨텍스트에서 즉시 성공(또는 백그라운드 태스크 종료 시
조용히 실패, best-effort). 백그라운드 writer 태스크가 `ControlMsg`를 소비해
실제 해지 프레임을 WS sink에 쓴다.

### D4 — 체결통보 key/iv 비동기 도착 처리

체결통보 TR 구독 시 key/iv는 구독 **응답(JSON 제어 메시지)** 으로 나중에
비동기 도착한다. 수신 태스크는 `Option<AesCreds>`를 tr_id별로 보관한다.
key/iv 미설치 상태에서 암호화 프레임(`data[0]=='1'`)이 도착하면 그 프레임은
드롭하고 `tracing::warn`만 남긴다 (race 허용 — 구독 직후 1~2 프레임 한정,
정상 상태에서는 응답이 데이터보다 먼저 도착). 이 동작을 decode 태스크 주석에
명시.

### D5 — 다건 프레임 처리

수신 프레임 `[2]`는 데이터 건수(`data_cnt`), `[3]`은 본문에 `필드수 × 건수`개
값이 `^`로 연속된다. `decode.rs`는 본문을 `^`로 split 후 `tr_id별 필드수`로
청크 분할해 건당 이벤트 1개를 생성한다. **프레임당 이벤트 1개로 가정하지 말 것.**

### D6 — 해지 프레임 `tr_type` = `"2"` (SSOT 우선)

해지 프레임의 `header.tr_type` 값에 대해 두 문서가 충돌한다:
- `realtime.md` §A-3 (실시간 SSOT): `1` 등록 / `2` 등록해제.
- 스펙 §3.7 line 190: 해지 `tr_type:"0"`.

**결정**: SSOT인 `realtime.md`를 따라 해지는 `"2"`. 본 Plan 문서 상단이
실시간 명세 SSOT를 `realtime.md`로 명시했으므로 스펙 §3.7의 `"0"`보다 우선한다.
`build_frame`은 `subscribe=true → "1"`, `subscribe=false → "2"`.

### D7 — approval_key는 연결마다 재발급

`approval_key`는 캐싱하지 않고 **WS 연결을 새로 열 때마다** 발급한다 (최초
연결·재연결 공통). `ConnectionCtx`가 `reqwest::Client`를 보유하고
`run_one_connection` 진입 직후 `issue_approval_key`를 호출한다. 근거: 만료
정책이 `[미확인]`이고, 발급 빈도(연결당 1회)가 낮아 비용이 무시 가능하며,
재연결 시 만료된 키 재사용 위험을 원천 차단한다.

## 파일 맵

| 파일 | 책임 | 생성/수정 태스크 |
|------|------|------------------|
| `Cargo.toml` | 의존성 5종 추가 | T1 |
| `src/lib.rs` | `pub mod realtime` 선언 + 재노출 | T1, T8 |
| `src/realtime/mod.rs` | `RealtimeClient`, 이벤트 타입, 연결·재연결 루프 | T6, T7 |
| `src/realtime/approval.rs` | approval_key 발급 (`POST /oauth2/Approval`) | T2 |
| `src/realtime/crypto.rs` | AES-256-CBC 복호화 | T3 |
| `src/realtime/decode.rs` | 프레임 파싱, tr_id별 필드 매핑, 이벤트 생성 | T4 |
| `src/realtime/subscribe.rs` | 구독/해지 프레임, `SubscriptionHandle`, `ControlMsg` | T5 |
| `src/client.rs` | `KisClient::realtime()` 액세서 추가 | T6 |
| `examples/realtime_feed.rs` | 실시간 체결가 구독 CLI | T9 |
| `tests/integration.rs` | 모의 WS 구독 1건 스모크 (`#[ignore]`) | T10 |
| `README.md` | 실시간 사용법 절 추가 | T11 |

---

## T1 — 의존성 추가 + 모듈 선언

`Cargo.toml`의 `[dependencies]`에 다음 5종 추가 (스펙 §4와 일치):

```toml
tokio-tungstenite = { version = "0.24", features = ["rustls-tls-webpki-roots"] }
futures-util = "0.3"
aes = "0.8"
cbc = { version = "0.1", features = ["alloc", "block-padding"] }
base64 = "0.22"
```

> `cbc` feature 주의: PKCS#7 언패딩에 `block-padding`이 필요. `cbc` 0.1은
> `Decryptor::decrypt_padded_vec_mut::<Pkcs7>` 사용 — `alloc` + `block-padding`
> feature 둘 다 켠다. `aes`는 기본 feature로 충분.

> `tokio` feature 수정 (필수): Plan 1은 `["rt-multi-thread","macros","sync","time","fs"]`.
> 예제 `realtime_feed.rs`가 `tokio::signal::ctrl_c()`를 쓰므로 `"signal"` feature를
> 추가해야 한다. `Cargo.toml`의 `tokio` 줄을 다음으로 교체:
> ```toml
> tokio = { version = "1", features = ["rt-multi-thread", "macros", "sync", "time", "fs", "signal"] }
> ```
> `[dev-dependencies]`의 `tokio` 줄에도 동일하게 `"signal"` 추가. `tokio::spawn`/
> `mpsc`/`time`은 기존 feature로 충분.

`src/lib.rs` 수정 — 모듈 선언과 재노출 추가:

```rust
pub mod realtime;
```

는 `pub mod domestic_stock;` 다음 줄에 추가. 재노출은 `pub use error::...` 블록
근처에:

```rust
pub use realtime::{
    RealtimeClient, RealtimeEvent, SubscriptionHandle, SubscriptionKind,
};
```

검증: `cargo build` — 이 시점엔 `realtime` 모듈 본문이 없어 실패. T8 끝에서 통과.
이 태스크는 `Cargo.toml`과 `lib.rs`만 수정.

커밋: `chore: 실시간 WebSocket 의존성 + 모듈 선언`

---

## T2 — `src/realtime/approval.rs`

approval_key 발급. `POST /oauth2/Approval` — REST 호출이며 WS 도메인이 아닌
REST base URL 사용. **요청 Body 필드명은 `secretkey`** (REST 토큰 발급의
`appsecret`과 다름 — `realtime.md` A-2).

```rust
//! WebSocket 접속키(approval_key) 발급.
//!
//! `POST /oauth2/Approval` — access token과 별개의 인증 수단.
//! 요청 Body 필드명이 `secretkey`임에 주의 (REST 토큰의 `appsecret`과 다름,
//! docs/kis-api/realtime.md §A-2).

use serde::Deserialize;

use crate::config::KisConfig;
use crate::error::{KisError, Result};

/// approval_key를 발급한다. 만료/캐싱은 하지 않음 — WS 연결을 열 때마다
/// (최초·재연결 공통) 새로 발급한다 (D7). 발급 빈도가 낮아 토큰버킷 불필요.
pub(crate) async fn issue_approval_key(
    http: &reqwest::Client,
    config: &KisConfig,
) -> Result<String> {
    #[derive(Deserialize)]
    struct ApprovalResponse {
        approval_key: String,
    }

    let url = format!("{}/oauth2/Approval", config.environment.rest_base());
    let body = serde_json::json!({
        "grant_type": "client_credentials",
        "appkey": config.app_key,
        "secretkey": config.app_secret,
    });

    let resp = http
        .post(&url)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let txt = resp.text().await.unwrap_or_default();
        return Err(KisError::Auth(format!(
            "approval_key issue failed (http {status}): {txt}"
        )));
    }

    let parsed: ApprovalResponse = resp.json().await?;
    if parsed.approval_key.is_empty() {
        return Err(KisError::Auth("approval_key empty in response".into()));
    }
    Ok(parsed.approval_key)
}
```

검증: T8에서 빌드.
커밋: `feat: approval_key 발급`

---

## T3 — `src/realtime/crypto.rs`

체결통보(`H0STCNI0`/`H0STCNI9`) 데이터 AES-256-CBC 복호화. 처리 순서는
`realtime.md` C-2: `Base64 디코드 → AES-256-CBC 복호화 → PKCS#7 언패딩 → UTF-8`.

key/iv는 구독 응답의 `body.output.{key,iv}`에서 받은 **문자열을 UTF-8
바이트열로** 사용 (key 32바이트, iv 16바이트).

```rust
//! 체결통보 실시간 데이터 AES-256-CBC 복호화.
//!
//! 알고리즘 출처: docs/kis-api/realtime.md §C-2 (`aes_cbc_base64_dec`).
//! 처리: Base64 디코드 → AES-256-CBC 복호화 → PKCS#7 언패딩 → UTF-8 디코드.

use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
use base64::Engine;

use crate::error::{KisError, Result};

type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;

/// 체결통보 구독 응답에서 받은 AES key/iv.
#[derive(Debug, Clone)]
pub(crate) struct AesCreds {
    /// AES-256 secret key — 응답 `body.output.key` 문자열 (UTF-8 32바이트).
    pub key: String,
    /// AES-256 IV — 응답 `body.output.iv` 문자열 (UTF-8 16바이트).
    pub iv: String,
}

impl AesCreds {
    /// Base64 암호문을 복호화해 UTF-8 평문 문자열로 반환.
    pub fn decrypt(&self, cipher_b64: &str) -> Result<String> {
        let key_bytes = self.key.as_bytes();
        let iv_bytes = self.iv.as_bytes();
        if key_bytes.len() != 32 {
            return Err(KisError::Decode(format!(
                "aes key must be 32 bytes, got {}",
                key_bytes.len()
            )));
        }
        if iv_bytes.len() != 16 {
            return Err(KisError::Decode(format!(
                "aes iv must be 16 bytes, got {}",
                iv_bytes.len()
            )));
        }

        let cipher_bytes = base64::engine::general_purpose::STANDARD
            .decode(cipher_b64.trim())
            .map_err(|e| KisError::Decode(format!("base64 decode failed: {e}")))?;

        let dec = Aes256CbcDec::new(key_bytes.into(), iv_bytes.into());
        let plain = dec
            .decrypt_padded_vec_mut::<Pkcs7>(&cipher_bytes)
            .map_err(|e| KisError::Decode(format!("aes-cbc decrypt failed: {e}")))?;

        String::from_utf8(plain)
            .map_err(|e| KisError::Decode(format!("decrypted bytes not utf-8: {e}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aes::cipher::{block_padding::Pkcs7, BlockEncryptMut};

    type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;

    /// 라운드트립 자체검증 벡터.
    ///
    /// realtime.md에 KIS 공식 KAT(known-answer-test) 벡터가 없으므로,
    /// 본 테스트는 자체 일관성 검증이다: 알려진 key/iv/평문을 동일 crate로
    /// 암호화 → Base64 → `AesCreds::decrypt`로 복호화 → 원문 일치 확인.
    /// 암호화 결과 Base64 문자열은 한 번 계산해 픽스처로 박아넣어, 복호화
    /// 경로(Base64 디코드·언패딩·UTF-8)가 회귀하면 즉시 깨지도록 한다.
    ///
    /// 실행자 작업: 아래 `encrypt_fixture()`를 1회 실행해 출력된 Base64
    /// 문자열을 `EXPECTED_CIPHER_B64` 상수에 전사할 것.
    const KEY: &str = "0123456789abcdef0123456789abcdef"; // 32 bytes
    const IV: &str = "abcdef9876543210"; // 16 bytes
    const PLAIN: &str = "user01^00000000-01^0000123456^^^^^^005930^10^71500^093015^";

    fn encrypt_fixture() -> String {
        let enc = Aes256CbcEnc::new(KEY.as_bytes().into(), IV.as_bytes().into());
        let ct = enc.encrypt_padded_vec_mut::<Pkcs7>(PLAIN.as_bytes());
        base64::engine::general_purpose::STANDARD.encode(ct)
    }

    /// 실행자: `cargo test realtime::crypto -- --nocapture print_fixture`로
    /// 1회 출력 → 아래 EXPECTED_CIPHER_B64에 전사.
    #[test]
    #[ignore = "fixture generator — run once to capture EXPECTED_CIPHER_B64"]
    fn print_fixture() {
        println!("EXPECTED_CIPHER_B64 = {}", encrypt_fixture());
    }

    // ── 실행자가 print_fixture 출력값으로 채울 것 ──
    const EXPECTED_CIPHER_B64: &str = "<<print_fixture 출력 Base64 전사>>";

    #[test]
    fn roundtrip_decrypt() {
        let creds = AesCreds {
            key: KEY.into(),
            iv: IV.into(),
        };
        // 픽스처 회귀 검사: 박아넣은 Base64를 복호화하면 원문이어야 함.
        let out = creds.decrypt(EXPECTED_CIPHER_B64).unwrap();
        assert_eq!(out, PLAIN);
        // 동적 라운드트립도 동시 검증.
        let dynamic = encrypt_fixture();
        assert_eq!(dynamic, EXPECTED_CIPHER_B64, "암호화 결정성 확인");
        assert_eq!(creds.decrypt(&dynamic).unwrap(), PLAIN);
    }

    #[test]
    fn rejects_wrong_key_length() {
        let creds = AesCreds {
            key: "tooshort".into(),
            iv: IV.into(),
        };
        assert!(matches!(
            creds.decrypt(EXPECTED_CIPHER_B64),
            Err(KisError::Decode(_))
        ));
    }
}
```

> **실행자 지시 (필수, 플레이스홀더 제거)**: `EXPECTED_CIPHER_B64`는 진짜
> 상수여야 한다. 절차: (1) `print_fixture`의 `#[ignore]`를 잠시 제거하거나
> `cargo test --lib print_fixture -- --ignored --nocapture` 실행, (2) 출력된
> Base64 문자열을 `EXPECTED_CIPHER_B64`에 전사, (3) `print_fixture`는
> `#[ignore]` 유지. 커밋 시 `<<...>>` 플레이스홀더가 남아 있으면 안 됨 —
> `roundtrip_decrypt`가 컴파일·통과해야 커밋한다.

검증: T8 빌드 후 `cargo test realtime::crypto` — `roundtrip_decrypt`,
`rejects_wrong_key_length` 2건 통과 (`print_fixture`는 ignored).
커밋: `feat: 체결통보 AES-256-CBC 복호화`

---

## T4 — `src/realtime/decode.rs`

수신 프레임 파싱 + tr_id별 필드 매핑 + 이벤트 변환.

### 프레임 판별 (`realtime.md` A-5)

`recv()`한 텍스트 `data`의 첫 글자로 분기:
- `'0'` — 평문 실시간 데이터 → `|`로 split.
- `'1'` — 암호화 실시간 데이터(체결통보) → `|`로 split 후 `[3]` 본문 AES 복호화.
- 그 외 — JSON 제어 메시지 (구독 응답 / PINGPONG / 에러).

실시간 데이터 프레임 `|` 구조: `[0]`=암호화유무, `[1]`=tr_id, `[2]`=data_cnt,
`[3]`=본문. 본문은 `^`로 split, `tr_id별 필드수 × data_cnt`개 값.

### tr_id별 필드 매핑 — 필드표 verbatim 전사 지시

각 tr_id 구조체는 `realtime.md` B 섹션의 번호 매긴 필드표를 **순번 순서대로
전 행** 전사해 만든다. 모든 필드는 `String` (KIS 실시간 데이터는 전부 문자열).
필드명은 영문 약어 컬럼 사용, 한글 의미는 doc 주석으로 병기.

```rust
//! 수신 프레임 파싱 + tr_id별 필드 매핑.
//!
//! 프레임 구분자 2단계: 프레임 `|`, 필드 `^` (docs/kis-api/realtime.md §A-5).
//! 한 프레임에 data_cnt건이 올 수 있어 본문을 `필드수 × 건수`로 청크 분할한다.

use crate::error::{KisError, Result};
use crate::realtime::crypto::AesCreds;

/// 국내주식 실시간체결가 (H0STCNT0). 46개 필드.
/// 필드표 전체는 docs/kis-api/realtime.md §B-1 — 순번 1~46 verbatim.
#[derive(Debug, Clone)]
pub struct StockTrade {
    pub mksc_shrn_iscd: String,        // 1 유가증권 단축 종목코드
    pub stck_cntg_hour: String,        // 2 주식 체결 시간
    pub stck_prpr: String,             // 3 주식 현재가
    pub prdy_vrss_sign: String,        // 4 전일 대비 부호
    pub prdy_vrss: String,             // 5 전일 대비
    // ── 6~46: docs/kis-api/realtime.md §B-1 표의 순번 6~46 행을
    //    `필드명: String, // N 한글의미` 형식으로 전 행 전사. ──
}

/// 국내주식 실시간호가 (H0STASP0). 59개 필드 (idx 0~58).
/// 필드표 전체는 docs/kis-api/realtime.md §B-2 — 순번 1~59 verbatim.
/// 영문 필드명은 KIS 호가 표준 약어 사용 (askp01~10, bidp01~10,
/// askp_rsqn01~10, bidp_rsqn01~10 등) — doc 한글 의미를 주석 병기.
#[derive(Debug, Clone)]
pub struct StockAsking {
    pub mksc_shrn_iscd: String,        // idx 0  유가증권 단축 종목코드
    pub bsop_hour: String,             // idx 1  영업시간
    pub hour_cls_code: String,         // idx 2  시간구분코드
    pub askp1: String,                 // idx 3  매도호가01
    // ── idx 4~58: docs/kis-api/realtime.md §B-2 표의 순번 4~59(idx 3~58)
    //    행을 순서대로 전 행 전사. 필드 의미는 §B-2 한글 컬럼 그대로. ──
}

/// 체결통보 (H0STCNI0/H0STCNI9) — 체결/접수 공통 26필드.
/// idx 13(체결여부)이 "2"면 체결, 그 외면 접수. idx 9·10·25 의미가
/// 유형에 따라 바뀜 (docs/kis-api/realtime.md §B-3 (a)/(b)).
/// 본 struct는 위치 기준 raw 값을 그대로 보존하고, 의미 해석은 호출자 몫.
#[derive(Debug, Clone)]
pub struct OrderNotice {
    pub cust_id: String,               // idx 0  고객 ID
    pub acnt_no: String,               // idx 1  계좌번호
    pub oder_no: String,               // idx 2  주문번호
    pub ooder_no: String,              // idx 3  원주문번호
    pub seln_byov_cls: String,         // idx 4  매도매수구분
    // ── idx 5~25: docs/kis-api/realtime.md §B-3 (a) 표 순번 6~26(idx 5~25)
    //    행을 순서대로 전 행 전사. idx 9/10/25는 §B-3 (a) 명칭 사용
    //    (체결수량/체결단가/주문가격) — (b) 접수 통보 시 의미 차이는 doc 참조. ──
    /// idx 13. "2"=체결, 그 외=접수.
    pub cntg_yn: String, // (전사 시 idx 13 위치 — 위 묶음에 포함, 별도 필드 아님)
}
// 주의: 위 cntg_yn 줄은 설명용. 실제로는 idx 13이 §B-3 전사 묶음 안에
// `cntg_yn: String, // idx 13 체결여부 (2=체결/그외=접수)`로 들어간다.
// 중복 정의하지 말 것 — 전사 시 한 번만.

/// 해외주식 실시간체결가 (HDFSCNT0). 26개 필드.
/// 필드표 전체는 docs/kis-api/realtime.md §B-4 — 순번 1~26 verbatim.
#[derive(Debug, Clone)]
pub struct OverseasTrade {
    pub rsym: String,                  // 1  실시간 종목코드
    pub symb: String,                  // 2  종목코드
    pub zdiv: String,                  // 3  소수점 자리수
    pub tymd: String,                  // 4  현지영업일자
    // ── 5~26: docs/kis-api/realtime.md §B-4 표 순번 5~26 행 전 행 전사. ──
}
```

> **전사 규칙 (필수)**: 위 4개 struct의 `── ... ──` 주석 자리는 모두
> `realtime.md` B 섹션 해당 표의 남은 행을 `필드명: String, // 의미` 형식으로
> 전 행 채운다. 필드 개수가 표와 정확히 일치해야 한다(46/59/26/26). 커밋 전
> 표 행 수와 struct 필드 수를 대조 확인.

### tr_id별 필드 수 상수 + 디코드 함수

```rust
/// tr_id별 `^` 분해 후 한 건당 필드 수.
fn fields_per_record(tr_id: &str) -> Option<usize> {
    match tr_id {
        "H0STCNT0" => Some(46),
        "H0STASP0" => Some(59),
        "H0STCNI0" | "H0STCNI9" => Some(26),
        "HDFSCNT0" => Some(26),
        _ => None,
    }
}

/// 한 수신 프레임을 디코드해 0건 이상의 이벤트로 변환.
///
/// `creds`: 체결통보 tr_id의 AES key/iv. 평문 tr_id이면 무시.
///   체결통보인데 `None`이면 빈 Vec 반환 + 호출자가 warn (D4 race).
pub(crate) fn decode_frame(
    raw: &str,
    creds: Option<&AesCreds>,
) -> Result<Vec<DecodedRecord>> {
    let parts: Vec<&str> = raw.split('|').collect();
    if parts.len() < 4 {
        return Err(KisError::Decode(format!(
            "realtime frame has {} segments, expected >=4",
            parts.len()
        )));
    }
    let encrypted = parts[0] == "1";
    let tr_id = parts[1];
    let data_cnt: usize = parts[2]
        .trim()
        .parse()
        .map_err(|e| KisError::Decode(format!("invalid data_cnt {}: {e}", parts[2])))?;

    // 본문 — 암호화면 복호화 (parts[3]만; '|'가 암호문에 없다고 가정,
    // 공식 샘플도 split('|')[3]만 사용).
    let body_owned;
    let body: &str = if encrypted {
        let creds = match creds {
            Some(c) => c,
            None => return Ok(Vec::new()), // D4: key/iv 미도착 — 드롭
        };
        body_owned = creds.decrypt(parts[3])?;
        &body_owned
    } else {
        parts[3]
    };

    let n_fields = fields_per_record(tr_id)
        .ok_or_else(|| KisError::Decode(format!("unsupported tr_id {tr_id}")))?;
    let fields: Vec<&str> = body.split('^').collect();
    let expected = n_fields * data_cnt.max(1);
    if fields.len() < expected {
        return Err(KisError::Decode(format!(
            "tr_id {tr_id}: got {} fields, expected {} ({}x{})",
            fields.len(),
            expected,
            n_fields,
            data_cnt
        )));
    }

    let mut out = Vec::with_capacity(data_cnt.max(1));
    for chunk in fields.chunks(n_fields).take(data_cnt.max(1)) {
        out.push(record_from_fields(tr_id, chunk)?);
    }
    Ok(out)
}

/// `^` 분해된 한 건의 필드 슬라이스를 tr_id별 struct로.
#[derive(Debug, Clone)]
pub(crate) enum DecodedRecord {
    StockTrade(StockTrade),
    StockAsking(StockAsking),
    OrderNotice(OrderNotice),
    OverseasTrade(OverseasTrade),
}

fn record_from_fields(tr_id: &str, f: &[&str]) -> Result<DecodedRecord> {
    // 헬퍼: 위치 g(i)로 String 추출. 길이는 decode_frame에서 이미 검증됨.
    let g = |i: usize| f[i].to_string();
    Ok(match tr_id {
        "H0STCNT0" => DecodedRecord::StockTrade(StockTrade {
            mksc_shrn_iscd: g(0),
            stck_cntg_hour: g(1),
            stck_prpr: g(2),
            prdy_vrss_sign: g(3),
            prdy_vrss: g(4),
            // ── g(5)..g(45): §B-1 전사한 필드 순서대로 g(i) 대입. ──
        }),
        "H0STASP0" => DecodedRecord::StockAsking(StockAsking {
            mksc_shrn_iscd: g(0),
            bsop_hour: g(1),
            hour_cls_code: g(2),
            askp1: g(3),
            // ── g(4)..g(58): §B-2 전사한 필드 순서대로 g(i) 대입. ──
        }),
        "H0STCNI0" | "H0STCNI9" => DecodedRecord::OrderNotice(OrderNotice {
            cust_id: g(0),
            acnt_no: g(1),
            oder_no: g(2),
            ooder_no: g(3),
            seln_byov_cls: g(4),
            // ── g(5)..g(25): §B-3 전사한 필드 순서대로 g(i) 대입.
            //    idx 13은 cntg_yn. ──
        }),
        "HDFSCNT0" => DecodedRecord::OverseasTrade(OverseasTrade {
            rsym: g(0),
            symb: g(1),
            zdiv: g(2),
            tymd: g(3),
            // ── g(4)..g(25): §B-4 전사한 필드 순서대로 g(i) 대입. ──
        }),
        other => {
            return Err(KisError::Decode(format!("unsupported tr_id {other}")))
        }
    })
}
```

> **전사 규칙 (필수)**: `record_from_fields`의 각 arm `── ... ──` 자리는
> 해당 struct 전사 시 정한 필드 순서대로 `필드명: g(i),`를 빠짐없이 채운다.
> struct 필드 개수 = `g(i)` 대입 개수 = `fields_per_record` 반환값이 셋 다
> 일치해야 한다.

### 제어 메시지 파싱

```rust
/// JSON 제어 메시지 파싱 결과.
#[derive(Debug, Clone)]
pub(crate) enum ControlMessage {
    /// PINGPONG — 받은 raw를 그대로 pong으로 되돌려야 함.
    PingPong,
    /// 구독 응답. rt_cd=="0"이면 ok. 체결통보면 key/iv 동봉.
    SubscribeAck {
        tr_id: String,
        tr_key: String,
        rt_cd: String,
        msg: String,
        creds: Option<AesCreds>,
    },
}

/// 첫 글자가 `0`/`1`이 아닌 텍스트 프레임(JSON)을 파싱.
pub(crate) fn decode_control(raw: &str) -> Result<ControlMessage> {
    let v: serde_json::Value = serde_json::from_str(raw)
        .map_err(|e| KisError::Decode(format!("control json parse: {e}")))?;
    let tr_id = v
        .pointer("/header/tr_id")
        .and_then(|x| x.as_str())
        .unwrap_or_default();
    if tr_id == "PINGPONG" {
        return Ok(ControlMessage::PingPong);
    }
    let tr_key = v
        .pointer("/header/tr_key")
        .and_then(|x| x.as_str())
        .unwrap_or_default()
        .to_string();
    let body_str = |p: &str| {
        v.pointer(p)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_string()
    };
    let creds = match (
        v.pointer("/body/output/key").and_then(|x| x.as_str()),
        v.pointer("/body/output/iv").and_then(|x| x.as_str()),
    ) {
        (Some(k), Some(iv)) => Some(AesCreds {
            key: k.to_string(),
            iv: iv.to_string(),
        }),
        _ => None,
    };
    Ok(ControlMessage::SubscribeAck {
        tr_id: tr_id.to_string(),
        tr_key,
        rt_cd: body_str("/body/rt_cd"),
        msg: body_str("/body/msg1"),
        creds,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_frame_single_record() {
        // H0STCNT0 평문 1건. 46필드를 ^로 — 값은 자리표시 숫자.
        let body: String = (0..46)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("^");
        let raw = format!("0|H0STCNT0|001|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        assert_eq!(recs.len(), 1);
        match &recs[0] {
            DecodedRecord::StockTrade(t) => assert_eq!(t.stck_prpr, "2"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn plain_frame_multi_record() {
        // data_cnt=2 — 46*2 필드.
        let body: String = (0..92)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("^");
        let raw = format!("0|H0STCNT0|002|{body}");
        let recs = decode_frame(&raw, None).unwrap();
        assert_eq!(recs.len(), 2, "다건 프레임은 건수만큼 이벤트");
    }

    #[test]
    fn encrypted_frame_without_creds_is_dropped() {
        let raw = "1|H0STCNI0|001|<<base64>>";
        let recs = decode_frame(raw, None).unwrap();
        assert!(recs.is_empty(), "key/iv 미도착 시 드롭");
    }

    #[test]
    fn pingpong_control() {
        let raw = r#"{"header":{"tr_id":"PINGPONG"}}"#;
        assert!(matches!(
            decode_control(raw).unwrap(),
            ControlMessage::PingPong
        ));
    }

    #[test]
    fn subscribe_ack_with_creds() {
        let raw = r#"{"header":{"tr_id":"H0STCNI0","tr_key":"htsid"},
            "body":{"rt_cd":"0","msg1":"OK",
            "output":{"key":"k","iv":"v"}}}"#;
        match decode_control(raw).unwrap() {
            ControlMessage::SubscribeAck { creds, rt_cd, .. } => {
                assert_eq!(rt_cd, "0");
                assert!(creds.is_some());
            }
            _ => panic!("wrong variant"),
        }
    }
}
```

검증: T8 빌드 후 `cargo test realtime::decode` — 5건 통과.
커밋: `feat: 실시간 프레임 디코딩 + tr_id 필드 매핑`

---

## T5 — `src/realtime/subscribe.rs`

구독/해지 프레임 생성, `SubscriptionKind`, `SubscriptionHandle`, 제어 채널 메시지.

```rust
//! 구독/해지 프레임 + SubscriptionHandle.
//!
//! 구독 프레임 구조: docs/kis-api/realtime.md §A-3.
//! tr_type "1"=등록(구독), "2"=등록해제(해지).

use tokio::sync::mpsc;

/// 구독 가능한 실시간 데이터 종류. 본 Plan 지원 4종.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubscriptionKind {
    /// H0STCNT0 — 국내주식 실시간체결가. tr_key=종목코드.
    DomesticTrade,
    /// H0STASP0 — 국내주식 실시간호가. tr_key=종목코드.
    DomesticAsking,
    /// H0STCNI0(실전)/H0STCNI9(모의) — 체결통보. tr_key=HTS ID.
    OrderNotice,
    /// HDFSCNT0 — 해외주식 실시간체결가. tr_key=실시간종목코드.
    OverseasTrade,
}

impl SubscriptionKind {
    /// 환경별 tr_id. OrderNotice만 실전/모의가 다름.
    pub(crate) fn tr_id(self, env: crate::config::Environment) -> &'static str {
        use crate::config::Environment::*;
        match (self, env) {
            (SubscriptionKind::DomesticTrade, _) => "H0STCNT0",
            (SubscriptionKind::DomesticAsking, _) => "H0STASP0",
            (SubscriptionKind::OrderNotice, Real) => "H0STCNI0",
            (SubscriptionKind::OrderNotice, Mock) => "H0STCNI9",
            (SubscriptionKind::OverseasTrade, _) => "HDFSCNT0",
        }
    }

    /// 체결통보 여부 — AES 복호화 필요.
    pub(crate) fn is_encrypted(self) -> bool {
        matches!(self, SubscriptionKind::OrderNotice)
    }
}

/// 구독/해지 프레임 JSON 문자열 생성. tr_type "1"=구독, "2"=해지.
pub(crate) fn build_frame(
    approval_key: &str,
    tr_id: &str,
    tr_key: &str,
    subscribe: bool,
) -> String {
    serde_json::json!({
        "header": {
            "approval_key": approval_key,
            "custtype": "P",
            "tr_type": if subscribe { "1" } else { "2" },
            "content-type": "utf-8",
        },
        "body": {
            "input": { "tr_id": tr_id, "tr_key": tr_key }
        }
    })
    .to_string()
}

/// 백그라운드 writer 태스크로 보내는 제어 메시지.
#[derive(Debug, Clone)]
pub(crate) enum ControlMsg {
    Subscribe { tr_id: String, tr_key: String },
    Unsubscribe { tr_id: String, tr_key: String },
}

/// 활성 구독 핸들. Drop 시 자동 해지 프레임 전송 (best-effort).
///
/// 이벤트는 `RealtimeClient` 생성 시 받은 단일 `mpsc::Receiver`로 흐른다 —
/// 핸들은 이벤트를 직접 노출하지 않고 "구독 수명"만 관리한다.
pub struct SubscriptionHandle {
    tr_id: String,
    tr_key: String,
    control_tx: mpsc::UnboundedSender<ControlMsg>,
    /// unsubscribe() 명시 호출 시 true — Drop에서 중복 해지 방지.
    released: bool,
}

impl SubscriptionHandle {
    pub(crate) fn new(
        tr_id: String,
        tr_key: String,
        control_tx: mpsc::UnboundedSender<ControlMsg>,
    ) -> Self {
        Self {
            tr_id,
            tr_key,
            control_tx,
            released: false,
        }
    }

    /// 명시적 구독 해지. 해지 프레임 전송 요청을 큐에 넣는다.
    pub async fn unsubscribe(mut self) {
        let _ = self.control_tx.send(ControlMsg::Unsubscribe {
            tr_id: self.tr_id.clone(),
            tr_key: self.tr_key.clone(),
        });
        self.released = true;
    }
}

impl Drop for SubscriptionHandle {
    fn drop(&mut self) {
        if self.released {
            return;
        }
        // best-effort — writer 태스크가 죽었으면 조용히 실패.
        let _ = self.control_tx.send(ControlMsg::Unsubscribe {
            tr_id: self.tr_id.clone(),
            tr_key: self.tr_key.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Environment;

    #[test]
    fn order_notice_tr_id_per_env() {
        assert_eq!(
            SubscriptionKind::OrderNotice.tr_id(Environment::Real),
            "H0STCNI0"
        );
        assert_eq!(
            SubscriptionKind::OrderNotice.tr_id(Environment::Mock),
            "H0STCNI9"
        );
    }

    #[test]
    fn subscribe_frame_shape() {
        let f = build_frame("ak", "H0STCNT0", "005930", true);
        let v: serde_json::Value = serde_json::from_str(&f).unwrap();
        assert_eq!(v["header"]["tr_type"], "1");
        assert_eq!(v["header"]["approval_key"], "ak");
        assert_eq!(v["body"]["input"]["tr_id"], "H0STCNT0");
        assert_eq!(v["body"]["input"]["tr_key"], "005930");
    }

    #[test]
    fn unsubscribe_frame_shape() {
        let f = build_frame("ak", "H0STCNT0", "005930", false);
        let v: serde_json::Value = serde_json::from_str(&f).unwrap();
        assert_eq!(v["header"]["tr_type"], "2");
    }

    #[test]
    fn drop_sends_unsubscribe() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        {
            let _h = SubscriptionHandle::new("H0STCNT0".into(), "005930".into(), tx);
        } // drop
        match rx.try_recv().unwrap() {
            ControlMsg::Unsubscribe { tr_id, tr_key } => {
                assert_eq!(tr_id, "H0STCNT0");
                assert_eq!(tr_key, "005930");
            }
            _ => panic!("expected Unsubscribe"),
        }
    }
}
```

> **해지 프레임 `tr_type` (D6 참조)**: `build_frame`은 `subscribe=false`일 때
> `"2"`를 쓴다 — `realtime.md` §A-3 표(`1` 등록 / `2` 등록해제)가 실시간 SSOT.
> 스펙 §3.7 line 190의 `"0"` 표기와 충돌하나 SSOT 우선. 통합테스트(T10)에서
> 모의 환경 실호출로 검증.

검증: T8 빌드 후 `cargo test realtime::subscribe` — 4건 통과.
커밋: `feat: 구독 프레임 + SubscriptionHandle`

---

## T6 — `src/realtime/mod.rs` (RealtimeClient 골격) + `KisClient::realtime()`

`RealtimeClient` 구조체, 이벤트 타입, 구독 상태, `KisClient::realtime()` 액세서.
연결·재연결 루프 본문은 T7. 본 태스크는 타입 정의 + 연결 진입점까지.

```rust
//! 실시간 WebSocket 클라이언트.
//!
//! 설계: docs/specs/2026-05-22-kis-adapter-design.md §3.7.

mod approval;
mod crypto;
mod decode;
mod subscribe;

pub use subscribe::{SubscriptionHandle, SubscriptionKind};

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};

use crate::config::KisConfig;
use crate::error::{KisError, Result};
use crate::realtime::decode::DecodedRecord;
use crate::realtime::subscribe::ControlMsg;

/// 이벤트 채널 용량 (D1).
const EVENT_CHANNEL_CAP: usize = 1024;
/// 동시 구독 한도 (스펙 §3.7, §8 — 약 41건).
const MAX_SUBSCRIPTIONS: usize = 41;

/// 호출자에게 전달되는 실시간 이벤트.
///
/// 모든 데이터 이벤트에 `tr_id`+`tr_key`가 포함되어 호출자가 분기 가능
/// (스펙 §3.7). `tr_id`는 수신 프레임 `[1]`에서 추출 — 체결통보의 실전
/// (`H0STCNI0`)/모의(`H0STCNI9`) 구분도 이 값으로 가능.
#[derive(Debug, Clone)]
pub enum RealtimeEvent {
    /// 국내주식 체결가 (H0STCNT0).
    DomesticTrade {
        tr_id: String,
        tr_key: String,
        data: decode::StockTrade,
    },
    /// 국내주식 호가 (H0STASP0).
    DomesticAsking {
        tr_id: String,
        tr_key: String,
        data: decode::StockAsking,
    },
    /// 체결통보 (H0STCNI0/9).
    OrderNotice {
        tr_id: String,
        tr_key: String,
        data: decode::OrderNotice,
    },
    /// 해외주식 체결가 (HDFSCNT0).
    OverseasTrade {
        tr_id: String,
        tr_key: String,
        data: decode::OverseasTrade,
    },
    /// 채널 포화로 n건 유실 (D1 — drop-newest + 이 통지).
    Lagged(u64),
    /// 연결 끊김 — 자동 재연결·재구독 진행 중.
    Reconnecting,
    /// 재연결·재구독 완료.
    Reconnected,
}

/// 구독 1건의 상태. 재연결 시 재전송에 사용.
#[derive(Debug, Clone)]
pub(crate) struct SubState {
    pub kind: SubscriptionKind,
    pub tr_key: String,
}

/// 구독 상태 공유 — 재연결 중 subscribe/unsubscribe 직렬화 (스펙 §3.7).
pub(crate) type SubMap = Arc<Mutex<HashMap<(String, String), SubState>>>;

/// 실시간 WebSocket 클라이언트.
///
/// `KisClient::realtime()`으로 생성. 생성 시 자격증명 검증 + 연결 루프
/// 태스크 spawn (WS 연결·approval_key 발급은 루프 태스크가 수행). 이벤트는
/// `take_events()`로 한 번 꺼내는 단일 `mpsc::Receiver`로 흐른다.
pub struct RealtimeClient {
    config: KisConfig,
    subs: SubMap,
    control_tx: mpsc::UnboundedSender<ControlMsg>,
    /// 이벤트 수신단 — `take_events()`로 한 번만 꺼낼 수 있음.
    events: Option<mpsc::Receiver<RealtimeEvent>>,
}

impl RealtimeClient {
    /// 실시간 클라이언트 생성. 검증용 approval_key 1회 발급(자격증명 사전
    /// 확인) → 연결 루프 태스크 spawn. 실제 연결마다 approval_key를 재발급
    /// 하므로(D7), `connect`의 발급은 자격증명 유효성 조기 검증 목적이다.
    /// 내부 호출: `KisClient::realtime()`.
    pub(crate) async fn connect(
        config: KisConfig,
        http: reqwest::Client,
    ) -> Result<Self> {
        // 자격증명 조기 검증 — 실패 시 여기서 즉시 에러 반환.
        let _ = approval::issue_approval_key(&http, &config).await?;
        let (event_tx, event_rx) = mpsc::channel(EVENT_CHANNEL_CAP);
        let (control_tx, control_rx) = mpsc::unbounded_channel();
        let subs: SubMap = Arc::new(Mutex::new(HashMap::new()));

        // 연결 + 재연결 루프 태스크 spawn (T7에서 본문 구현).
        // approval_key는 ConnectionCtx가 http로 연결마다 재발급한다.
        run::spawn_connection_loop(run::ConnectionCtx {
            config: config.clone(),
            http,
            subs: subs.clone(),
            event_tx,
            control_rx,
            lag: Arc::new(AtomicU64::new(0)),
        });

        Ok(Self {
            config,
            subs,
            control_tx,
            events: Some(event_rx),
        })
    }

    /// 이벤트 수신 채널을 꺼낸다. 최초 1회만 `Some` — 이후 `None`.
    pub fn take_events(&mut self) -> Option<mpsc::Receiver<RealtimeEvent>> {
        self.events.take()
    }

    /// 실시간 데이터 구독. 한도(41건) 초과 시 `KisError::Ws`.
    pub async fn subscribe(
        &self,
        kind: SubscriptionKind,
        key: &str,
    ) -> Result<SubscriptionHandle> {
        let tr_id = kind.tr_id(self.config.environment).to_string();
        let tr_key = key.to_string();
        {
            let mut map = self.subs.lock().await;
            if map.len() >= MAX_SUBSCRIPTIONS
                && !map.contains_key(&(tr_id.clone(), tr_key.clone()))
            {
                return Err(KisError::Ws(format!(
                    "subscription limit {MAX_SUBSCRIPTIONS} reached"
                )));
            }
            map.insert(
                (tr_id.clone(), tr_key.clone()),
                SubState {
                    kind,
                    tr_key: tr_key.clone(),
                },
            );
        }
        self.control_tx
            .send(ControlMsg::Subscribe {
                tr_id: tr_id.clone(),
                tr_key: tr_key.clone(),
            })
            .map_err(|_| KisError::Ws("connection task gone".into()))?;
        Ok(SubscriptionHandle::new(tr_id, tr_key, self.control_tx.clone()))
    }
}

// 연결 루프 — T7에서 구현.
mod run;
```

`src/client.rs`에 `realtime()` 액세서 추가 (기존 `domestic_stock()` 근처):

```rust
    /// 실시간 WebSocket 클라이언트 생성.
    ///
    /// approval_key 발급(REST) + WS 연결을 수행하므로 비동기·실패 가능.
    /// 반환된 `RealtimeClient`는 독립 — `KisClient` 수명과 무관.
    pub async fn realtime(&self) -> Result<crate::realtime::RealtimeClient> {
        crate::realtime::RealtimeClient::connect(
            self.config.clone(),
            self.http.clone(),
        )
        .await
    }
```

> `KisClient::new`는 Plan 1대로 **동기** 유지. `realtime()`만 비동기.
> `self.http`는 `KisClient`의 private 필드 — 같은 crate(`client.rs`)이므로
> 직접 접근 가능. `self.config`도 동일.

> `lib.rs` 재노출은 T1에서 `RealtimeClient, RealtimeEvent, SubscriptionHandle,
> SubscriptionKind`로 이미 선언 — `RealtimeEvent`는 `mod.rs`에 정의되므로
> `pub use realtime::RealtimeEvent` 경로 유효. 확인.

검증: T8에서 빌드.
커밋: `feat: RealtimeClient 골격 + KisClient::realtime() 액세서`

---

## T7 — `src/realtime/run.rs` (연결·재연결 루프) + 빌드 통과

WS 연결, 수신/송신 처리, PINGPONG 응답, 자동 재연결+재구독, 채널 포화 처리.

```rust
//! WS 연결 수명 관리 — 연결, 수신·송신, PINGPONG, 재연결.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::config::KisConfig;
use crate::realtime::decode::{self, ControlMessage, DecodedRecord};
use crate::realtime::subscribe::{build_frame, ControlMsg};
use crate::realtime::{RealtimeEvent, SubMap, SubscriptionKind};

/// 연결 루프 컨텍스트.
pub(crate) struct ConnectionCtx {
    pub config: KisConfig,
    /// approval_key 재발급용 (D7 — 연결마다 새로 발급).
    pub http: reqwest::Client,
    pub subs: SubMap,
    pub event_tx: mpsc::Sender<RealtimeEvent>,
    pub control_rx: mpsc::UnboundedReceiver<ControlMsg>,
    pub lag: Arc<AtomicU64>,
}

/// 연결+재연결 루프를 백그라운드 태스크로 spawn.
pub(crate) fn spawn_connection_loop(ctx: ConnectionCtx) {
    tokio::spawn(async move { connection_loop(ctx).await });
}

async fn connection_loop(mut ctx: ConnectionCtx) {
    let mut backoff = Duration::from_secs(1);
    let max_backoff = Duration::from_secs(30);
    let url = ctx.config.environment.ws_base().to_string();

    loop {
        match run_one_connection(&mut ctx, &url).await {
            // control_rx 종료(RealtimeClient drop) — 루프 종료.
            ConnEnd::ClientGone => return,
            ConnEnd::Disconnected => {
                let _ = ctx.event_tx.send(RealtimeEvent::Reconnecting).await;
                tokio::time::sleep(backoff).await;
                backoff = (backoff * 2).min(max_backoff);
            }
        }
    }
}

enum ConnEnd {
    /// control_rx가 닫힘 — 클라이언트 drop, 영구 종료.
    ClientGone,
    /// 연결 끊김 — 재연결 대상.
    Disconnected,
}

/// 단일 연결의 수명. 끊기면 반환, control_rx 종료면 ClientGone.
async fn run_one_connection(ctx: &mut ConnectionCtx, url: &str) -> ConnEnd {
    // D7 — 연결마다 approval_key 새로 발급. 발급 실패도 재연결 대상.
    let approval_key = match crate::realtime::approval::issue_approval_key(
        &ctx.http,
        &ctx.config,
    )
    .await
    {
        Ok(k) => k,
        Err(e) => {
            tracing::warn!("approval_key reissue failed: {e}");
            return ConnEnd::Disconnected;
        }
    };

    let (ws, _) = match tokio_tungstenite::connect_async(url).await {
        Ok(ok) => ok,
        Err(e) => {
            tracing::warn!("ws connect failed: {e}");
            return ConnEnd::Disconnected;
        }
    };
    let (mut sink, mut stream) = ws.split();

    // 재연결이면 기존 구독 전체 재전송 (스펙 §3.7 자동 재구독).
    {
        let map = ctx.subs.lock().await;
        for ((tr_id, tr_key), _state) in map.iter() {
            let frame = build_frame(&approval_key, tr_id, tr_key, true);
            if sink.send(Message::Text(frame)).await.is_err() {
                return ConnEnd::Disconnected;
            }
        }
        if !map.is_empty() {
            let _ = ctx.event_tx.send(RealtimeEvent::Reconnected).await;
        }
    }

    // tr_id별 AES key/iv 캐시 (D4).
    let mut creds: std::collections::HashMap<String, crate::realtime::crypto::AesCreds> =
        std::collections::HashMap::new();

    loop {
        tokio::select! {
            // ── 제어 채널: subscribe/unsubscribe 프레임 송신 ──
            ctl = ctx.control_rx.recv() => {
                match ctl {
                    None => return ConnEnd::ClientGone,
                    Some(ControlMsg::Subscribe { tr_id, tr_key }) => {
                        let f = build_frame(&approval_key, &tr_id, &tr_key, true);
                        if sink.send(Message::Text(f)).await.is_err() {
                            return ConnEnd::Disconnected;
                        }
                    }
                    Some(ControlMsg::Unsubscribe { tr_id, tr_key }) => {
                        ctx.subs.lock().await.remove(&(tr_id.clone(), tr_key.clone()));
                        let f = build_frame(&approval_key, &tr_id, &tr_key, false);
                        // best-effort — 끊겨도 무시.
                        let _ = sink.send(Message::Text(f)).await;
                    }
                }
            }
            // ── WS 수신 ──
            msg = stream.next() => {
                let msg = match msg {
                    Some(Ok(m)) => m,
                    Some(Err(e)) => {
                        tracing::warn!("ws recv error: {e}");
                        return ConnEnd::Disconnected;
                    }
                    None => return ConnEnd::Disconnected,
                };
                match msg {
                    Message::Text(text) => {
                        if handle_text(ctx, &mut sink, &mut creds, &text)
                            .await
                            .is_err()
                        {
                            return ConnEnd::Disconnected;
                        }
                    }
                    Message::Ping(p) => {
                        let _ = sink.send(Message::Pong(p)).await;
                    }
                    Message::Close(_) => return ConnEnd::Disconnected,
                    _ => {}
                }
            }
        }
    }
}

/// 텍스트 프레임 처리. Err = 송신 실패(연결 끊김).
async fn handle_text<S>(
    ctx: &ConnectionCtx,
    sink: &mut S,
    creds: &mut std::collections::HashMap<String, crate::realtime::crypto::AesCreds>,
    text: &str,
) -> std::result::Result<(), ()>
where
    S: futures_util::Sink<Message> + Unpin,
{
    let first = text.as_bytes().first().copied();
    match first {
        Some(b'0') | Some(b'1') => {
            // 실시간 데이터 프레임.
            let tr_id = text.split('|').nth(1).unwrap_or_default();
            let cred = creds.get(tr_id);
            match decode::decode_frame(text, cred) {
                Ok(records) => {
                    for rec in records {
                        emit_record(ctx, tr_id, rec).await;
                    }
                }
                Err(e) => tracing::warn!("frame decode failed: {e}"),
            }
        }
        _ => {
            // JSON 제어 메시지.
            match decode::decode_control(text) {
                Ok(ControlMessage::PingPong) => {
                    // realtime.md §A-4 — 받은 데이터를 WebSocket Pong
                    // 프레임으로 그대로 되돌림 (`websocket.pong(data)`).
                    sink.send(Message::Pong(text.as_bytes().to_vec().into()))
                        .await
                        .map_err(|_| ())?;
                }
                Ok(ControlMessage::SubscribeAck {
                    tr_id,
                    rt_cd,
                    msg,
                    creds: ack_creds,
                    ..
                }) => {
                    if rt_cd != "0" && !rt_cd.is_empty() {
                        tracing::warn!("subscribe ack tr_id={tr_id} rt_cd={rt_cd}: {msg}");
                    }
                    if let Some(c) = ack_creds {
                        creds.insert(tr_id, c); // D4 — key/iv 설치.
                    }
                }
                Err(e) => tracing::warn!("control decode failed: {e}"),
            }
        }
    }
    Ok(())
}

/// 디코드된 레코드를 이벤트로 변환해 채널에 송신. 포화 시 drop-newest + lag (D1).
/// `tr_id`는 수신 프레임 `[1]`에서 추출한 값.
async fn emit_record(ctx: &ConnectionCtx, tr_id: &str, rec: DecodedRecord) {
    let tr_id = tr_id.to_string();
    let event = match rec {
        DecodedRecord::StockTrade(d) => RealtimeEvent::DomesticTrade {
            tr_id,
            tr_key: d.mksc_shrn_iscd.clone(),
            data: d,
        },
        DecodedRecord::StockAsking(d) => RealtimeEvent::DomesticAsking {
            tr_id,
            tr_key: d.mksc_shrn_iscd.clone(),
            data: d,
        },
        DecodedRecord::OrderNotice(d) => RealtimeEvent::OrderNotice {
            tr_id,
            tr_key: d.cust_id.clone(),
            data: d,
        },
        DecodedRecord::OverseasTrade(d) => RealtimeEvent::OverseasTrade {
            tr_id,
            tr_key: d.symb.clone(),
            data: d,
        },
    };

    // lag 누적분이 있으면 먼저 Lagged 통지 시도.
    let pending = ctx.lag.swap(0, Ordering::Relaxed);
    if pending > 0
        && ctx
            .event_tx
            .try_send(RealtimeEvent::Lagged(pending))
            .is_err()
    {
        // Lagged 자체도 못 보냄 — 카운터 복원.
        ctx.lag.fetch_add(pending, Ordering::Relaxed);
    }

    if ctx.event_tx.try_send(event).is_err() {
        // 채널 포화 — drop-newest + lag 증가 (D1).
        ctx.lag.fetch_add(1, Ordering::Relaxed);
    }
}
```

> **`tr_key` 추출 주의**: `emit_record`는 디코드 struct의 종목코드 필드를
> `tr_key`로 쓴다. 체결통보(`OrderNotice`)는 종목코드 대신 `cust_id`(고객 ID)를
> tr_key로 — 구독 시 tr_key가 HTS ID였으므로 일관. `OverseasTrade`는 `symb`
> 사용. 전사 후 struct 필드명이 위와 다르면 `emit_record`도 맞춰 수정.

`lib.rs` / 빌드 통과 검증. 이 시점에 `realtime` 5개 모듈 + run.rs 전부 존재.

```
cargo build
cargo build --examples       # examples/realtime_feed.rs는 T9에서 생성 — 그 전엔 생략
cargo clippy -- -D warnings
cargo test
```

기대: build 에러 0. `cargo test` — Plan 1 단위테스트 6건 +
realtime 단위테스트(crypto 2, decode 5, subscribe 4 = 11건) = 17 passed,
ignored 3건(integration 2 + crypto print_fixture 1).

> **clippy 주의**: `run.rs`의 `handle_text`는 제네릭 `S: Sink`. 실제 호출부는
> `SplitSink`로 단일화되므로 제네릭 불필요할 수 있음 — clippy가 지적하면
> 구체 타입(`futures_util::stream::SplitSink<WsStream, Message>`)으로 교체.
> `WsStream` 타입 별칭 정의:
> `type WsStream = tokio_tungstenite::WebSocketStream<
>     tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;`

커밋: `feat: WS 연결·재연결 루프 + PINGPONG + 채널 포화 처리`

---

## T8 — 빌드·단위테스트 통과 확인 (체크포인트)

T7 끝에서 이미 빌드했으나, 본 태스크는 전 모듈 통합 검증 전용 체크포인트다.
신규 코드 없음 — T1~T7 산출물이 함께 컴파일·테스트되는지 확인만.

```
cargo build
cargo clippy -- -D warnings
cargo test
```

문제 발견 시 해당 태스크로 돌아가 수정. 통과하면 다음.
커밋 없음 (T7 커밋에 포함). 이미 통과했으면 이 태스크는 no-op.

---

## T9 — `examples/realtime_feed.rs`

```rust
//! 실시간 체결가 구독 예제.
//! 실행: KIS_* 환경변수 설정 후
//! `cargo run --example realtime_feed -- 005930 000660`
//! (인자 없으면 005930 기본. Ctrl-C로 종료.)

use kis_adapter::{KisClient, KisConfig, RealtimeEvent, SubscriptionKind};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let codes: Vec<String> = {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.is_empty() {
            vec!["005930".to_string()]
        } else {
            args
        }
    };

    let client = KisClient::new(KisConfig::from_env()?)?;
    let mut rt = client.realtime().await?;
    let mut events = rt.take_events().expect("events receiver");

    // 종목별 체결가 구독 — 핸들은 살려둬야 Drop 자동해지가 안 일어남.
    let mut handles = Vec::new();
    for code in &codes {
        handles.push(rt.subscribe(SubscriptionKind::DomesticTrade, code).await?);
        println!("구독: {code}");
    }

    println!("실시간 수신 시작 — Ctrl-C로 종료");
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                println!("종료");
                break;
            }
            ev = events.recv() => {
                match ev {
                    Some(RealtimeEvent::DomesticTrade { tr_key, data, .. }) => {
                        println!(
                            "[{tr_key}] {} 현재가 {} 거래량 {}",
                            data.stck_cntg_hour, data.stck_prpr, data.cntg_vol
                        );
                    }
                    Some(RealtimeEvent::Lagged(n)) => {
                        eprintln!("경고: {n}건 유실 (채널 포화)");
                    }
                    Some(RealtimeEvent::Reconnecting) => println!("재연결 중..."),
                    Some(RealtimeEvent::Reconnected) => println!("재연결·재구독 완료"),
                    Some(_) => {}
                    None => {
                        println!("연결 종료됨");
                        break;
                    }
                }
            }
        }
    }
    // handles drop → 자동 해지.
    drop(handles);
    Ok(())
}
```

> `data.cntg_vol`은 `StockTrade`의 §B-1 idx 13(체결 거래량) 필드 — 전사 시
> 필드명이 다르면 맞춰 수정. `Cargo.toml`에 `[[example]] name = "realtime_feed"`
> 항목 추가.

검증: `cargo build --example realtime_feed` 에러 0.
(자격증명 있으면 수동 실행 — 장 시간대에 체결가 출력 확인.)
커밋: `docs: 실시간 체결가 구독 예제`

---

## T10 — `tests/integration.rs` 실시간 스모크 추가

기존 `tests/integration.rs`에 모의 WS 구독 1건 스모크를 **추가** (기존 테스트
유지). `#[ignore]` 게이트.

```rust
#[tokio::test]
#[ignore = "requires KIS_* credentials + market hours"]
async fn realtime_subscribe_one() {
    use kis_adapter::{KisClient, KisConfig, RealtimeEvent, SubscriptionKind};

    let config = KisConfig::from_env().expect("KIS_* env vars");
    let client = KisClient::new(config).expect("client");
    let mut rt = client.realtime().await.expect("realtime connect");
    let mut events = rt.take_events().expect("events");

    let _handle = rt
        .subscribe(SubscriptionKind::DomesticTrade, "005930")
        .await
        .expect("subscribe");

    // 5초 안에 이벤트 1건 이상 (또는 재연결 통지) 수신되면 통과.
    // 장 시간 외에는 데이터가 없을 수 있어 timeout을 fail로 보지 않음 —
    // 구독 호출이 에러 없이 끝났다는 점이 핵심 검증.
    let got = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            match events.recv().await {
                Some(RealtimeEvent::DomesticTrade { .. }) => return true,
                Some(_) => continue,
                None => return false,
            }
        }
    })
    .await;
    // timeout(Err) = 장 시간 외 — 허용. Ok(false) = 채널 종료 = 실패.
    if let Ok(false) = got {
        panic!("event channel closed unexpectedly");
    }
}
```

검증: `cargo test --test integration` — ignored 3건 표시(Plan 1의 2건 + 이번 1건).
자격증명 + 장 시간이면 `cargo test --test integration realtime -- --ignored` 통과.
커밋: `test: 모의 WS 구독 통합 스모크`

---

## T11 — `README.md` 실시간 절 추가 + 최종 검증

`README.md`에 실시간 사용법 절 추가 (Plan 1 README 유지, 절만 추가):

```markdown
## 실시간 WebSocket (Plan 3)

\`\`\`rust
use kis_adapter::{KisClient, KisConfig, RealtimeEvent, SubscriptionKind};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KisClient::new(KisConfig::from_env()?)?;
    let mut rt = client.realtime().await?;
    let mut events = rt.take_events().unwrap();
    let _h = rt.subscribe(SubscriptionKind::DomesticTrade, "005930").await?;
    while let Some(ev) = events.recv().await {
        if let RealtimeEvent::DomesticTrade { tr_key, data, .. } = ev {
            println!("{tr_key} {}", data.stck_prpr);
        }
    }
    Ok(())
}
\`\`\`

- 지원 tr_id: H0STCNT0(국내체결가), H0STASP0(국내호가),
  H0STCNI0/H0STCNI9(체결통보 실전/모의), HDFSCNT0(해외체결가).
- 체결통보는 AES-256-CBC 복호화 자동 처리. `tr_key`는 HTS ID 사용.
- `SubscriptionHandle` drop 시 자동 해지. 동시 구독 한도 약 41건.
- 연결 끊김 시 자동 재연결·재구독 — `RealtimeEvent::Reconnecting/Reconnected` 통지.
```

최종 검증 — 전체 명령 실행, 출력 확인:

```
cargo build
cargo build --examples
cargo clippy -- -D warnings
cargo test
```

기대: build 에러 0, clippy 경고 0, `cargo test` 17 passed + ignored 3.

커밋: `docs: README 실시간 절 + Plan 3 완료`

---

## Plan 3 완료 기준

- [ ] `cargo build` / `cargo build --examples` 에러 0
- [ ] `cargo clippy -- -D warnings` 경고 0
- [ ] `cargo test` — 단위 17건 통과(Plan 1 6 + crypto 2 + decode 5 + subscribe 4),
      ignored 3건(integration 2 + crypto print_fixture 1)
- [ ] 자격증명 있으면 `cargo test --test integration -- --ignored` 통과
- [ ] `KisClient::realtime()` 비동기 액세서 동작, Plan 1 `KisClient::new` 동기 시그니처 불변
- [ ] 4종 tr_id 구독·디코딩 — `H0STCNT0`/`H0STASP0`/`H0STCNI0`/`H0STCNI9`/`HDFSCNT0`
- [ ] 체결통보 AES-256-CBC 복호화 — `roundtrip_decrypt` 통과, 픽스처 플레이스홀더 제거
- [ ] `SubscriptionHandle` Drop 자동 해지 — `drop_sends_unsubscribe` 통과
- [ ] 다건 프레임 처리 — `plain_frame_multi_record` 통과
- [ ] 채널 포화 시 drop-newest + `RealtimeEvent::Lagged(n)` 통지
- [ ] PINGPONG 응답, 자동 재연결+재구독
- [ ] 스펙 §3.7 항목 전수 반영 — approval_key(`secretkey` 필드), WS endpoint
      21000/31000, 구독/해지 프레임, 2단계 구분자(`|`/`^`), 41건 한도

## 미해결/위험

- **해지 `tr_type` 값**: realtime.md §A-3 표(`"2"`)와 스펙 §3.7(`"0"`)이 충돌 →
  D6에서 SSOT(realtime.md) 우선으로 `"2"` 채택. T10 통합테스트에서 모의 실호출로
  검증, 해지가 거부되면 실행자가 SSOT를 재확인 후 조정.
- **AES KAT 부재**: realtime.md에 KIS 공식 known-answer 벡터 없음 →
  crypto 테스트는 자체 라운드트립 + 박아넣은 픽스처 회귀 검사. "공식 벡터"가
  아님을 테스트 주석에 명시.
- **장 시간 외 통합테스트**: 실시간 데이터는 장 시간에만 흐름 → T10은 timeout을
  실패로 보지 않고 "구독 호출 무에러 + 채널 미종료"를 핵심 검증으로 삼음.
- **암호문 내 `|` 가정**: `decode_frame`은 `split('|')[3]`만 본문으로 사용 —
  Base64 암호문에 `|`가 없다는 공식 샘플 가정에 의존. Base64 알파벳에 `|`
  없으므로 안전.

## 다음

Plan 3로 스펙 §3.7 + §5 실시간 4종 TR 커버리지 완료. 어댑터 전체 스코프
(국내주식 Plan 1 + 해외·선물 Plan 2 + 실시간 Plan 3) 구현 종료.
