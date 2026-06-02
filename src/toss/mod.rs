//! 토스증권 Open API 어댑터.
//!
//! 기존 KIS 어댑터(`KisClient`)와 동일 크레이트 내 형제 모듈. KIS의 파일 레이아웃·
//! 문서 주석·액세서 패턴을 미러링하되, 인증(OAuth2 form)·envelope(HTTP status + `result`)·
//! 에러(`TossError`)·레이트리밋(429 반응형)이 다르다. 설계 근거는
//! `docs/specs/2026-06-02-toss-adapter-design.md` 참조.
//!
//! ```ignore
//! use korea_stock::{TossClient, TossConfig};
//!
//! let client = TossClient::new(TossConfig::from_env()?)?;
//! let price = client.market_data().orderbook("005930").await?;
//! let accounts = client.accounts().list().await?;
//! let seq = accounts[0].account_seq;
//! let holdings = client.asset(seq).holdings(None).await?;
//! ```

mod auth;
mod client;
mod config;
mod error;

pub mod account;
pub mod market_data;
pub mod market_info;
pub mod order;
pub mod order_info;
pub mod stock_info;

pub use client::{RawRequest, TossClient, TossResponse};
pub use config::{TossConfig, DEFAULT_BASE_URL};
pub use error::{Result as TossResult, TossError};

#[cfg(test)]
mod tests {
    use super::*;
    use client::{map_bff_error, RawResponse};
    use serde_json::json;

    // RawResponse는 pub(crate) — envelope 언래핑·필드 파싱을 직접 검증한다.
    #[test]
    fn result_envelope_parses_typed() {
        // 성공 응답의 result를 타입 struct로 언래핑.
        let result = json!({
            "currency": "KRW",
            "asks": [{"price": "72300", "volume": "1200"}],
            "bids": [{"price": "72000", "volume": "5200"}]
        });
        let resp = RawResponse {
            result,
            request_id: Some("req-1".into()),
        };
        let ob: market_data::OrderbookResponse = resp.parse().unwrap();
        assert_eq!(ob.currency, "KRW");
        assert_eq!(ob.bids[0].volume, "5200");
    }

    // 스펙 example 기반 round-trip — camelCase 키가 struct 필드에 정확히 매핑되는지
    // 검증한다. rename_all 누락 시 hard-fail(필수 필드)하거나 silently-None(Option)으로
    // 회귀하므로, 각 도메인 응답 struct마다 example을 한 번씩 통과시킨다.
    #[test]
    fn price_response_parses_spec_example() {
        let v = json!({
            "symbol": "005930",
            "timestamp": "2026-03-25T09:30:00.123+09:00",
            "lastPrice": "72000",
            "currency": "KRW"
        });
        let p: market_data::PriceResponse = serde_json::from_value(v).unwrap();
        assert_eq!(p.last_price, "72000");
        assert_eq!(p.symbol, "005930");
    }

    #[test]
    fn candle_page_parses_spec_example() {
        let v = json!({
            "candles": [{
                "timestamp": "2026-03-25T09:00:00+09:00",
                "openPrice": "71600",
                "highPrice": "72300",
                "lowPrice": "71500",
                "closePrice": "72000",
                "volume": "3521000",
                "currency": "KRW"
            }],
            "nextBefore": "2026-03-25T09:00:00+09:00"
        });
        let page: market_data::CandlePage = serde_json::from_value(v).unwrap();
        assert_eq!(page.candles[0].open_price, "71600");
        assert_eq!(page.candles[0].close_price, "72000");
        assert_eq!(
            page.next_before.as_deref(),
            Some("2026-03-25T09:00:00+09:00"),
            "nextBefore가 camelCase 매핑되어야 페이징이 동작"
        );
    }

    #[test]
    fn price_limit_parses_spec_example() {
        let v = json!({
            "timestamp": "2026-03-25T09:30:00.123+09:00",
            "upperLimitPrice": "93000",
            "lowerLimitPrice": "50400",
            "currency": "KRW"
        });
        let pl: market_data::PriceLimitResponse = serde_json::from_value(v).unwrap();
        assert_eq!(pl.upper_limit_price.as_deref(), Some("93000"));
        assert_eq!(pl.lower_limit_price.as_deref(), Some("50400"));
    }

    #[test]
    fn order_detail_parses_spec_example() {
        let v = json!({
            "orderId": "abc",
            "symbol": "005930",
            "side": "BUY",
            "orderType": "LIMIT",
            "timeInForce": "DAY",
            "status": "FILLED",
            "price": "70000",
            "quantity": "10",
            "currency": "KRW",
            "orderedAt": "2026-03-29T09:30:00+09:00",
            "execution": {
                "filledQuantity": "10",
                "averageFilledPrice": "70000",
                "filledAmount": "700000",
                "commission": "1400",
                "tax": "0",
                "filledAt": "2026-03-28T09:31:15+09:00",
                "settlementDate": "2026-03-30"
            }
        });
        let o: order::OrderDetail = serde_json::from_value(v).unwrap();
        assert_eq!(o.order_id, "abc");
        assert_eq!(o.status, "FILLED");
        assert_eq!(o.execution.filled_quantity, "10");
        assert_eq!(o.execution.settlement_date.as_deref(), Some("2026-03-30"));
    }

    #[test]
    fn bff_error_maps_to_api_variant() {
        // 4xx/5xx BFF envelope → TossError::Api. 프로덕션 매핑 함수를 직접 호출.
        let body = json!({
            "error": {
                "requestId": "01HXYZ",
                "code": "order-not-found",
                "message": "주문을 찾을 수 없습니다."
            }
        });
        match map_bff_error(404, &body, None) {
            TossError::Api {
                status,
                code,
                request_id,
                message,
            } => {
                assert_eq!(status, 404);
                assert_eq!(code, "order-not-found");
                assert_eq!(request_id.as_deref(), Some("01HXYZ"));
                assert_eq!(message, "주문을 찾을 수 없습니다.");
            }
            _ => panic!("expected Api variant"),
        }
    }

    #[test]
    fn bff_error_falls_back_to_header_request_id() {
        // body에 error 객체가 없으면 unknown code + 헤더 request_id fallback.
        let body = json!({});
        match map_bff_error(500, &body, Some("hdr-id".into())) {
            TossError::Api {
                code, request_id, ..
            } => {
                assert_eq!(code, "unknown");
                assert_eq!(request_id.as_deref(), Some("hdr-id"));
            }
            _ => panic!("expected Api variant"),
        }
    }

    #[test]
    fn oauth2_error_maps_to_oauth2_variant() {
        // /oauth2/token 실패 → TossError::OAuth2.
        let body = json!({
            "error": "invalid_client",
            "error_description": "Client authentication failed."
        });
        let error = body["error"].as_str().unwrap().to_string();
        let description = body["error_description"].as_str().map(String::from);
        let e = TossError::OAuth2 { error, description };
        match e {
            TossError::OAuth2 { error, description } => {
                assert_eq!(error, "invalid_client");
                assert_eq!(description.as_deref(), Some("Client authentication failed."));
            }
            _ => panic!("expected OAuth2 variant"),
        }
    }

    #[test]
    fn config_defaults_to_prod_base() {
        let cfg = TossConfig::new("c_id", "s_secret");
        assert_eq!(cfg.base_url, DEFAULT_BASE_URL);
        assert!(cfg.rate_limit.is_none());
    }
}
