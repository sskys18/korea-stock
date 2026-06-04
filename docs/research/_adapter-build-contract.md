# Venue Adapter Build Contract (신규 perp venue 어댑터)

신규 거래소 어댑터를 추가할 때 따르는 모듈 계약. **`src/global/binance/`가 정본 템플릿** —
구조·네이밍·에러·액세서 패턴을 그대로 미러링한다. 읽고 시작하라.

## 모듈 레이아웃 (`src/global/<venue>/`)
binance와 동일한 6파일:
- `mod.rs` — 모듈 doc, `mod`/`pub use` 재노출, 심볼 상수, `KR_SYMBOLS`, `symbol(KrStock)` 매핑
- `config.rs` — `<Venue>Config` (api key/secret/base_url/recv_window/rate_limit), `new`/`public`/`testnet`/`from_env`
- `error.rs` — `<Venue>Error` (thiserror), `Result` 별칭. binance/mexc error.rs 복제 후 변형
- `client.rs` — `<Venue>Client`, `ApiCall`/`RawRequest`, HMAC(또는 venue 서명) 헤더 구성, `market()`/`trade()` 액세서
- `market.rs` — 시세 도메인(키 불필요): ticker/depth/funding/contracts(or instruments). 응답 struct는 `#[serde]`
- `trade.rs` — 서명 거래: `OrderRequest`(limit/market 생성자), `place`/`cancel`/positions/balance. `Side` enum

## 필수 사항
1. **가격·수량은 String** 보존(정밀도). binance market.rs 참조.
2. **시세 = 키 불필요**, 거래 = 서명. `Config::public()`로 시세만.
3. **`symbol(crate::KrStock)` 매핑** — mod.rs에 필수. 미상장 종목/지수는 `=> return None`.
   `KrStock`은 `SamsungElec/SkHynix/HyundaiMotor/Kospi200`. Kospi200은 지수(`is_index()`).
4. **에러는 venue 독립** — 공유 trait 없음. `crate::global::<venue>::error::<Venue>Error`.
5. 경로는 `crate::global::<venue>::...` (절대), 모듈내는 `super::`/`self::` OK.

## 가용 의존성 (Cargo.toml — 신규 추가 최소화, 추가 시 보고)
- HMAC-SHA256/512: `hmac` + `sha2`(Sha256/Sha512) + `hex` ✅
- EIP-712/secp256k1: `k256`(ecdsa) + `sha3`(keccak256) + `rmp-serde` ✅ (hyperliquid 참조)
- 난수: `getrandom` ✅
- **Ed25519(Solana/Pacifica): `ed25519-dalek` 미존재 → 신규 필요. 보고할 것.**

## 검증 (worktree에서 반드시)
1. 자기 모듈을 `src/global/mod.rs`에 `pub mod <venue>;` 임시 배선 + lib.rs 재노출.
2. **라이브 시세 probe** — `examples/<venue>_kr_quote.rs` 작성 후 `cargo run --example <venue>_kr_quote`
   실행해 실제 공개 API에서 KR 심볼 호가/마크가가 응답함을 **출력으로 증명**(키 불필요).
   리서치 `docs/research/kr-stock-perp-venues-2026-06.md`의 base URL·심볼 사용.
3. **서명 단위테스트** — venue 공식 문서에 서명 예제 벡터가 있으면 `#[test]`로 바이트 대조.
   없으면 self-consistency 테스트(서명→검증 라운드트립 또는 알려진 입력 고정).
4. `cargo build --all-targets` + `cargo clippy --all-targets` clean + `cargo test` green.
5. 브랜치 `feat/venue-<venue>`에 커밋.

## 반환 (최종 메시지)
- 브랜치명, 생성 파일 목록
- **라이브 probe 출력**(KR 심볼 시세 — 핵심 증거)
- 서명 테스트 상태(벡터 검증/self-consistency/미검증 사유)
- 신규 Cargo 의존성(있으면)
- 중앙배선 라인: `global::mod.rs` mod decl, `lib.rs` 재노출, kr_perp_live_check 스니펫
- **거래 코드는 키 없어 실주문 미검증** — 명시.
