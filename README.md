# kis-adapter

한국투자증권(KIS) OpenAPI Rust 어댑터.

## 현재 범위 (Plan 1 + 2)

- 국내주식 12개 TR — 시세·주문·계좌·체결
- 해외주식 8개 TR — 현재가·기간시세·매수/매도/정정취소·잔고·미체결·체결내역
- 국내선물옵션 7개 TR — 현재가·호가·주문/정정취소·잔고·체결내역·매수가능
- 실전/모의투자 환경, 토큰 자동 발급·캐싱, 레이트리밋, 연속조회

실시간 WebSocket은 Plan 3에서 추가.

## 사용법

```rust
use kis_adapter::{KisClient, KisConfig};
use kis_adapter::overseas_stock::OverseasExchange;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = KisClient::new(KisConfig::from_env()?)?;
    let price = client.domestic_stock().current_price("005930").await?;
    println!("{}", price.stck_prpr);
    let overseas = client
        .overseas_stock()
        .current_price(OverseasExchange::Nasd, "AAPL")
        .await?;
    println!("{}", overseas.last);
    Ok(())
}
```

## 환경변수

| 변수 | 설명 |
|------|------|
| `KIS_APP_KEY` | 앱 키 |
| `KIS_APP_SECRET` | 앱 시크릿 |
| `KIS_ACCOUNT_NO` | 계좌번호 8자리 |
| `KIS_ACCOUNT_PRODUCT` | 계좌상품코드 2자리 |
| `KIS_ENV` | `real` 또는 `mock` (기본 mock) |

## 주문 hashkey

기본값은 `KisConfig.use_hashkey = false`다. 실제 주문 호출에서 KIS가 hashkey
관련 오류를 반환하면 설정을 `true`로 바꿔 재시도한다.

## 라이선스

MIT
