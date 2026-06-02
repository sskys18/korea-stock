//! 주문 도메인 — 생성·정정·취소·목록·상세. 모두 `X-Tossinvest-Account` 필수.

use serde::Deserialize;
use serde_json::{json, Map, Value};

use crate::toss::client::{ApiCall, TossClient};
use crate::toss::error::Result;

/// 주문 방향. 요청 파라미터 — 값을 통제하므로 타입 enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}

impl Side {
    fn code(self) -> &'static str {
        match self {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        }
    }
}

/// 호가 유형. 요청 파라미터 — 타입 enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderType {
    /// 지정가.
    Limit,
    /// 시장가.
    Market,
}

impl OrderType {
    fn code(self) -> &'static str {
        match self {
            OrderType::Limit => "LIMIT",
            OrderType::Market => "MARKET",
        }
    }
}

/// 주문 유효 조건 (Time In Force). 요청 파라미터 — 타입 enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeInForce {
    /// 당일 유효.
    Day,
    /// 장 마감 주문 (US LIMIT 전용).
    Cls,
}

impl TimeInForce {
    fn code(self) -> &'static str {
        match self {
            TimeInForce::Day => "DAY",
            TimeInForce::Cls => "CLS",
        }
    }
}

/// 주문 생성 요청. 스펙 `oneOf` [수량기반, 금액기반] 을 두-변형으로 모델링.
///
/// 전부-Optional 단일 struct로 평탄화하면 "orderAmount ⇒ US MARKET" 불변식이
/// 사라지므로 enum으로 분리한다.
#[derive(Debug, Clone)]
pub enum OrderCreate {
    /// 수량 기반 주문. `quantity`(정수)로 주문 수량 지정.
    Quantity {
        symbol: String,
        side: Side,
        order_type: OrderType,
        /// 주문 수량 (정수 문자열).
        quantity: String,
        /// 주문 가격. `Limit`이면 필수, `Market`이면 None.
        price: Option<String>,
        /// 유효 조건. None이면 서버 기본 DAY.
        time_in_force: Option<TimeInForce>,
        /// 멱등성 키 (≤36자, 10분 유효).
        client_order_id: Option<String>,
        /// 1억원 이상 주문 확인 플래그.
        confirm_high_value_order: bool,
    },
    /// 금액 기반 주문 (US MARKET 전용). `order_amount`(달러)로 주문 금액 지정.
    Amount {
        symbol: String,
        side: Side,
        /// 주문 금액 (달러). 체결 수량은 시장가에 따라 결정.
        order_amount: String,
        client_order_id: Option<String>,
        confirm_high_value_order: bool,
    },
}

impl OrderCreate {
    /// 스펙 oneOf에 맞는 JSON body 생성. (orderType은 변형에 따라 고정.)
    fn to_body(&self) -> Value {
        let mut m = Map::new();
        match self {
            OrderCreate::Quantity {
                symbol,
                side,
                order_type,
                quantity,
                price,
                time_in_force,
                client_order_id,
                confirm_high_value_order,
            } => {
                m.insert("symbol".into(), json!(symbol));
                m.insert("side".into(), json!(side.code()));
                m.insert("orderType".into(), json!(order_type.code()));
                m.insert("quantity".into(), json!(quantity));
                if let Some(p) = price {
                    m.insert("price".into(), json!(p));
                }
                if let Some(tif) = time_in_force {
                    m.insert("timeInForce".into(), json!(tif.code()));
                }
                if let Some(id) = client_order_id {
                    m.insert("clientOrderId".into(), json!(id));
                }
                if *confirm_high_value_order {
                    m.insert("confirmHighValueOrder".into(), json!(true));
                }
            }
            OrderCreate::Amount {
                symbol,
                side,
                order_amount,
                client_order_id,
                confirm_high_value_order,
            } => {
                m.insert("symbol".into(), json!(symbol));
                m.insert("side".into(), json!(side.code()));
                // 금액 기반은 MARKET 고정.
                m.insert("orderType".into(), json!(OrderType::Market.code()));
                m.insert("orderAmount".into(), json!(order_amount));
                if let Some(id) = client_order_id {
                    m.insert("clientOrderId".into(), json!(id));
                }
                if *confirm_high_value_order {
                    m.insert("confirmHighValueOrder".into(), json!(true));
                }
            }
        }
        Value::Object(m)
    }
}

/// 주문 정정 요청.
#[derive(Debug, Clone)]
pub struct OrderModify {
    /// 변경할 호가 유형.
    pub order_type: OrderType,
    /// 변경할 수량. KR 주식 필수, US 주식 전달 불가.
    pub quantity: Option<String>,
    /// 변경할 가격. `Limit`이면 필수.
    pub price: Option<String>,
    /// 1억원 이상 주문 확인 플래그.
    pub confirm_high_value_order: bool,
}

impl OrderModify {
    fn to_body(&self) -> Value {
        let mut m = Map::new();
        m.insert("orderType".into(), json!(self.order_type.code()));
        if let Some(q) = &self.quantity {
            m.insert("quantity".into(), json!(q));
        }
        if let Some(p) = &self.price {
            m.insert("price".into(), json!(p));
        }
        if self.confirm_high_value_order {
            m.insert("confirmHighValueOrder".into(), json!(true));
        }
        Value::Object(m)
    }
}

/// 주문 생성 응답.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderResponse {
    /// 서버 생성 주문 식별자. 정정/취소 시 사용.
    pub order_id: String,
    /// 요청 시 전달한 멱등성 키 그대로 반환. 미전달 시 null.
    #[serde(default)]
    pub client_order_id: Option<String>,
}

/// 정정/취소 응답. 새로 발급된 주문 식별자 (원주문 ID와 다름).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderOperationResponse {
    pub order_id: String,
}

/// 체결 결과. 체결 내역 없으면 filledQuantity=0.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderExecution {
    pub filled_quantity: String,
    /// 평균 체결가 (native currency). 부분 체결 시 체결분 평균.
    #[serde(default)]
    pub average_filled_price: Option<String>,
    #[serde(default)]
    pub filled_amount: Option<String>,
    #[serde(default)]
    pub commission: Option<String>,
    #[serde(default)]
    pub tax: Option<String>,
    #[serde(default)]
    pub filled_at: Option<String>,
    /// 결제 예정일. 미결제 시 null.
    #[serde(default)]
    pub settlement_date: Option<String>,
}

/// 주문 1건. enum성 필드(side/orderType/timeInForce/status/currency)는 unknown 허용 위해 String.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderDetail {
    pub order_id: String,
    pub symbol: String,
    /// 주문 방향 (BUY/SELL).
    pub side: String,
    /// 호가 유형 (LIMIT/MARKET).
    pub order_type: String,
    /// 유효 조건 (DAY/CLS/OPG).
    pub time_in_force: String,
    /// 주문 상태 (PENDING/PARTIAL_FILLED/FILLED/CANCELED/...).
    pub status: String,
    /// 주문 가격 (native currency). MARKET이면 null.
    #[serde(default)]
    pub price: Option<String>,
    pub quantity: String,
    /// 주문 금액 (USD). 금액 기반 US 시장가 매수만 해당, 그 외 null.
    #[serde(default)]
    pub order_amount: Option<String>,
    pub currency: String,
    /// 주문 시간 (ISO 8601, KST).
    pub ordered_at: String,
    /// 취소 시간. 해당 없으면 null.
    #[serde(default)]
    pub canceled_at: Option<String>,
    pub execution: OrderExecution,
}

/// 주문 목록 페이징 응답.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedOrderResponse {
    pub orders: Vec<OrderDetail>,
    /// 다음 페이지 커서. 없으면 null.
    #[serde(default)]
    pub next_cursor: Option<String>,
    pub has_next: bool,
}

/// 주문 라이프사이클 그룹 필터. 요청 파라미터 — 타입 enum.
///
/// `orders[].status` 세부값과 체계가 다른 **그룹 라벨**이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderStatusFilter {
    /// 진행 중 주문 그룹.
    Open,
    /// 종료 주문 그룹. 현재 서버는 `400 closed-not-supported` 반환.
    Closed,
}

impl OrderStatusFilter {
    fn code(self) -> &'static str {
        match self {
            OrderStatusFilter::Open => "OPEN",
            OrderStatusFilter::Closed => "CLOSED",
        }
    }
}

/// 주문 목록 조회 파라미터.
#[derive(Debug, Clone)]
pub struct OrderListReq {
    pub status: OrderStatusFilter,
    /// 종목 필터 (선택).
    pub symbol: Option<String>,
    /// 조회 시작일 (inclusive, YYYY-MM-DD, KST).
    pub from: Option<String>,
    /// 조회 종료일 (inclusive).
    pub to: Option<String>,
    /// 페이지네이션 커서.
    pub cursor: Option<String>,
    /// 페이지 크기 (1~100).
    pub limit: Option<u32>,
}

impl OrderListReq {
    /// status만 지정한 최소 파라미터.
    pub fn new(status: OrderStatusFilter) -> Self {
        Self {
            status,
            symbol: None,
            from: None,
            to: None,
            cursor: None,
            limit: None,
        }
    }
}

/// 주문 도메인 액세서. `client.order(account_seq)`로 획득.
/// `account_seq`를 구조적으로 보유해 `X-Tossinvest-Account` 누락이 불가능하다.
pub struct Order<'a> {
    client: &'a TossClient,
    account_seq: i64,
}

impl<'a> Order<'a> {
    pub(crate) fn new(client: &'a TossClient, account_seq: i64) -> Self {
        Self {
            client,
            account_seq,
        }
    }

    /// 주문 생성.
    pub async fn create(&self, req: OrderCreate) -> Result<OrderResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: "/api/v1/orders".into(),
                params: req.to_body(),
                is_post: true,
                account_seq: Some(self.account_seq),
            })
            .await?
            .parse()
    }

    /// 주문 정정. 정정/취소는 원주문 ID와 다른 새 주문 ID를 반환.
    pub async fn modify(
        &self,
        order_id: &str,
        req: OrderModify,
    ) -> Result<OrderOperationResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: format!("/api/v1/orders/{order_id}/modify"),
                params: req.to_body(),
                is_post: true,
                account_seq: Some(self.account_seq),
            })
            .await?
            .parse()
    }

    /// 주문 취소. 본문 없음.
    pub async fn cancel(&self, order_id: &str) -> Result<OrderOperationResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::POST,
                path: format!("/api/v1/orders/{order_id}/cancel"),
                params: Value::Null,
                is_post: true,
                account_seq: Some(self.account_seq),
            })
            .await?
            .parse()
    }

    /// 주문 목록 조회.
    pub async fn list(&self, req: OrderListReq) -> Result<PaginatedOrderResponse> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: "/api/v1/orders".into(),
                params: serde_json::json!({
                    "status": req.status.code(),
                    "symbol": req.symbol,
                    "from": req.from,
                    "to": req.to,
                    "cursor": req.cursor,
                    "limit": req.limit,
                }),
                is_post: false,
                account_seq: Some(self.account_seq),
            })
            .await?
            .parse()
    }

    /// 주문 상세 조회.
    pub async fn get(&self, order_id: &str) -> Result<OrderDetail> {
        self.client
            .call(ApiCall {
                method: reqwest::Method::GET,
                path: format!("/api/v1/orders/{order_id}"),
                params: serde_json::json!({}),
                is_post: false,
                account_seq: Some(self.account_seq),
            })
            .await?
            .parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantity_order_body_has_required_fields() {
        let req = OrderCreate::Quantity {
            symbol: "005930".into(),
            side: Side::Buy,
            order_type: OrderType::Limit,
            quantity: "10".into(),
            price: Some("70000".into()),
            time_in_force: None,
            client_order_id: None,
            confirm_high_value_order: false,
        };
        let body = req.to_body();
        assert_eq!(body["symbol"], "005930");
        assert_eq!(body["side"], "BUY");
        assert_eq!(body["orderType"], "LIMIT");
        assert_eq!(body["quantity"], "10");
        assert_eq!(body["price"], "70000");
        // 선택 필드는 미전달 시 키 자체가 없어야 한다.
        assert!(body.get("timeInForce").is_none());
        assert!(body.get("clientOrderId").is_none());
        assert!(body.get("confirmHighValueOrder").is_none());
        // 금액 기반 키가 섞이면 안 된다.
        assert!(body.get("orderAmount").is_none());
    }

    #[test]
    fn amount_order_forces_market_and_amount() {
        let req = OrderCreate::Amount {
            symbol: "AAPL".into(),
            side: Side::Buy,
            order_amount: "100.5".into(),
            client_order_id: Some("my-order-001".into()),
            confirm_high_value_order: false,
        };
        let body = req.to_body();
        assert_eq!(body["symbol"], "AAPL");
        assert_eq!(body["orderType"], "MARKET", "금액 기반은 MARKET 고정");
        assert_eq!(body["orderAmount"], "100.5");
        assert_eq!(body["clientOrderId"], "my-order-001");
        // 수량 기반 키가 섞이면 안 된다.
        assert!(body.get("quantity").is_none());
        assert!(body.get("price").is_none());
    }

    #[test]
    fn status_filter_codes() {
        assert_eq!(OrderStatusFilter::Open.code(), "OPEN");
        assert_eq!(OrderStatusFilter::Closed.code(), "CLOSED");
    }
}
