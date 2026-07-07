# Session Handoff
> Generated: 2026-06-04 KST

## Task
한국 주식(삼성전자·SK하이닉스·현대차) 무기한선물 perp을 거래하는 CEX·DEX 어댑터 4종
추가 — Binance·Hyperliquid·Lighter·MEXC. **완료·main 머지·푸시 끝.**

## Status
### Completed (origin/main `fcb2ec9`, 동기화됨 0 ahead)
- `src/binance/` — full: 시세 + 서명거래(HMAC-SHA256, 공식 doc벡터 검증). 심볼 라이브확정
  (`SAMSUNGUSDT`/`SKHYNIXUSDT`/`HYUNDAIUSDT`).
- `src/hyperliquid/` — full: 시세 + EIP-712/secp256k1 거래. 서명 primitive를 upstream
  hyperliquid-python-sdk dummy 벡터와 바이트 대조 검증. `place_by_coin`이 asset id를
  라이브 meta에서 재도출(wrong-instrument 가드). /exchange 바디 필드순서 버그 수정 완료.
- `src/lighter/` — 암호검증/봉투 미검증: 시세+realtime(WS) + 순수Rust Poseidon2/Schnorr
  서명(`sign.rs`). 암호코어는 upstream poseidon_crypto schnorr 벡터와 바이트 일치 검증.
  거래는 `allow_unverified_signing` 게이트(기본 false) 뒤.
- `src/mexc/` — data-only: 시세 + 서명조회. 신규주문 API는 MEXC 서버측 차단(2022~).
- docs 정합: README·`docs/specs/2026-06-02-kr-perp-venues-design.md`·lib.rs·Cargo.toml.
- 검증: 단위 142 passed, `cargo clippy --all-targets` clean.

### In Progress
- 없음. 코드·문서·머지·푸시 전부 끝.

## Resume Here
코드상 완료. 다음은 **사용자 키 필요한 실환경 검증**(이 머신에선 불가):
1. 각 venue **testnet 실주문 왕복** — 거래코드는 전부 오프라인 검증만, 어떤 venue도
   testnet/live로 주문된 적 없다. Binance `.testnet()`, HL testnet, Lighter testnet으로
   미체결 주문→취소 1사이클씩.
2. **Lighter tx_info 봉투 확정** — `scripts/lighter_capture_vector.md` 절차로 공식
   lighter-python SignerClient 출력(tx_info JSON·chain_id·order_expiry)을 캡처해
   `src/lighter/trade.rs` `build_create_order_tx_info`와 대조. 일치 시 게이트 해제.
3. (선택) Hyperliquid 실시간 WS(`subscribe`) 추가 — 현재 REST 시세만.

## Decisions (do NOT revisit)
- **공유 trait 없음**: 각 venue 독립 형제 모듈(kis/toss 패턴). CEX(HMAC)/DEX(EIP-712/zk)
  서명·주문모델 달라 추상화 누수. 구체 4구현 공통형태 나오기 전엔 trait 금지.
- **Lighter 서명 순수Rust 포팅 + 게이트**: lighter-go FFI 거부(Go 빌드 의존성·배포 복잡).
  암호코어만 벡터 검증, 봉투 미검증분은 날조 대신 `allow_unverified_signing` 게이트.
- **MEXC place/cancel 실서명 구현(스텁 아님)**: HMAC 검증 가능 → 실호출. 서버 차단은
  maintenance 에러로 자연 노출. data-only는 '신규주문 불가' 사실.
- **HL /exchange 바디는 typed struct 직송**: `serde_json::to_value`(BTreeMap 정렬)는 서명
  msgpack 선언순서와 어긋나 거부 위험 → reqwest `.json(&req)`로 순서 보존. (이번 세션 수정)

## Gotchas
- **gh 계정 403**: active 계정이 `yuseongkim-inbl`(쓰기권한 없음)로 자꾸 돌아감. push 실패 시
  첫 의심 → `gh auth switch --user sskys18`. repo 소유자 sskys18.
- **git chokepoint 훅**: commit/push/merge는 standalone Bash. `cd …;` 체이닝도 차단 →
  `git -C <path>`. 출력은 `> /tmp/x.log 2>&1` 후 별도 read. auto-review 훅이 merge/push 시
  재실행하며 blocking 발견 시 차단(이번엔 HL 필드순서 버그를 정확히 잡음).
- **serde_json 기본 BTreeMap**: 객체 키 알파벳 정렬. 서명 대상 JSON엔 치명적. 서명 바디는
  항상 serde 직렬화(struct 순서) 경로로(`.json(&req)`), `to_value` 금지.
- **KIS 키 `.env`(gitignore)**: 날아가면 `~/.claude/projects/-Users-sskys-Mine-korea-stock/*.jsonl`에서
  `KIS_(APP_KEY|APP_SECRET|ACCOUNT_NO)=` 재추출. `KIS_ENV=real`.
- `docs/handoff.md`는 untracked(커밋 안 됨). 이 파일 자체.

## 기타 미결(이전 작업, 별개)
- NXT WS decode 장중 실프레임 검증 미실행 — `cargo test --all-features -- --ignored
  realtime_nxt_asking_decode --nocapture`(08:00~20:00 KST + `.env`). 실패 시
  `src/kis/realtime/decode.rs` ASP 필드순서를 `docs/kis-api/nxt-ws-columns.txt`와 대조.

## Context
- **Branch**: `main` (feature `feat/kr-perp-venue-adapters`도 remote에 있음, 머지 완료)
- **Tests**: 142 passed / 30 ignored(KIS 네트워크·픽스처) / 0 failed. clippy clean.
- **Unknowns**(실행 전 확인): Lighter testnet chain_id, venue별 심볼 상태(운영 전
  `exchangeInfo`/`contracts()`/`orderBookDetails` 재확인). HL은 상수 말고 `place_by_coin` 경로.
