# 토스증권 Open API 엔드포인트 요약

- 권위 스펙: `openapi.json` (토스증권 Open API v1.0.3, `https://openapi.tossinvest.com/openapi-docs/latest/openapi.json` 2026-06-02 스냅샷).
- Base URL: `https://openapi.tossinvest.com` (`servers` 단일 — 실전/모의 분기 없음).
- 인증: OAuth2 Client Credentials. `POST /oauth2/token` (form-urlencoded) → `Authorization: Bearer {access_token}`.

## 공통 규약

- **성공 응답 envelope**: `ApiResponse` = `{ "result": <payload> }`. 모든 비-OAuth 200 응답은 `result` 단일 키.
- **성공/실패 판정**: HTTP 상태 코드. 2xx 성공, 4xx/5xx 실패. (KIS의 body `rt_cd` 방식과 다름.)
- **에러 envelope (BFF, 4xx/5xx)**: `ErrorResponse` = `{ "error": { requestId, code, message, data? } }`.
  `code`는 flat string 식별자 (예: `order-not-found`, `invalid-request`). 클라이언트는 unknown code 허용.
- **OAuth2 에러 (/oauth2/token 4xx/5xx)**: `{ error(enum), error_description?, error_uri? }`. BFF envelope 아님.
- **계좌 헤더**: asset/order/order-history/order-info 엔드포인트는 `X-Tossinvest-Account: {accountSeq}` 필수.
  accountSeq는 `GET /api/v1/accounts` 응답의 `accountSeq`. `GET /accounts`에는 불필요.
- **레이트리밋**: 그룹별(MARKET_DATA, MARKET_DATA_CHART, ORDER, ASSET, ORDER_INFO, MARKET_INFO, STOCK, ACCOUNT, AUTH).
  429 시 `RateLimitExceeded` — 헤더 `Retry-After`(초), `X-RateLimit-Limit/Remaining/Reset`. body는 `code=rate-limit-exceeded`.
- **enum unknown 허용**: 거의 모든 enum 스키마가 "unknown enum 값 허용" 명시. 응답 enum은 문자열로 보존 권장.
- 모든 decimal/price/quantity/금액 필드는 `format: decimal`의 **문자열** (maxLength 30). 정밀도 보존용.

## 인증

| 메서드 | 경로 | 요청 | 응답 |
|--------|------|------|------|
| POST | `/oauth2/token` | form: `grant_type=client_credentials`, `client_id`, `client_secret` | `{access_token, token_type:"Bearer", expires_in(초)}` |

- refresh token 없음. 만료 시 동일 엔드포인트 재발급.
- **client당 유효 access token 1개. 재발급 시 이전 토큰 즉시 무효화.**

## 시세 (Market Data)

| 메서드 | 경로 | 쿼리 | 결과(`result`) | rate |
|--------|------|------|----------------|------|
| GET | `/api/v1/orderbook` | `symbol`(필수) | `OrderbookResponse` {timestamp?, currency, asks[], bids[]} | MARKET_DATA |
| GET | `/api/v1/prices` | `symbols`(필수, ≤200 콤마구분) | `PriceResponse[]` {symbol, timestamp?, lastPrice, currency} | MARKET_DATA |
| GET | `/api/v1/trades` | `symbol`(필수), `count`(≤50, 기본 50) | `Trade[]` {price, volume, timestamp, currency} | MARKET_DATA |
| GET | `/api/v1/price-limits` | `symbol`(필수) | `PriceLimitResponse` {timestamp, upperLimitPrice?, lowerLimitPrice?, currency} | MARKET_DATA |
| GET | `/api/v1/candles` | `symbol`(필수), `interval`(`1m`\|`1d`, 필수), `count`(≤200, 기본100), `before`?, `adjusted`(기본true) | `CandlePageResponse` {candles[], nextBefore?} | MARKET_DATA_CHART |

`OrderbookEntry` {price, volume}. `Candle` {timestamp, openPrice, highPrice, lowPrice, closePrice, volume, currency}.

## 종목정보 (Stock Info)

| 메서드 | 경로 | 파라미터 | 결과 | rate |
|--------|------|----------|------|------|
| GET | `/api/v1/stocks` | `symbols`(필수, 콤마구분) | `StockInfo[]` | STOCK |
| GET | `/api/v1/stocks/{symbol}/warnings` | `symbol`(path) | `StockWarning[]` | STOCK |

- `StockInfo`: symbol, name, englishName, isinCode, market(KOSPI/KOSDAQ/NYSE/NASDAQ/AMEX/KR_ETC/US_ETC),
  securityType, isCommonShare, status(SCHEDULED/ACTIVE/DELISTED), currency, listDate?, delistDate?,
  sharesOutstanding, leverageFactor?, koreanMarketDetail?(KrMarketDetail).
- `KrMarketDetail`: liquidationTrading, nxtSupported, krxTradingSuspended, nxtTradingSuspended?.
- `StockWarning`: warningType(LIQUIDATION_TRADING/OVERHEATED/INVESTMENT_WARNING/INVESTMENT_RISK/VI_*/STOCK_WARRANTS),
  exchange, startDate?, endDate?.

## 시장정보 (Market Info)

| 메서드 | 경로 | 쿼리 | 결과 | rate |
|--------|------|------|------|------|
| GET | `/api/v1/exchange-rate` | `baseCurrency`(필수), `quoteCurrency`(필수), `dateTime`? | `ExchangeRateResponse` | MARKET_INFO |
| GET | `/api/v1/market-calendar/KR` | `date`? | `KrMarketCalendarResponse` | MARKET_INFO |
| GET | `/api/v1/market-calendar/US` | `date`? | `UsMarketCalendarResponse` | MARKET_INFO |

- `ExchangeRateResponse`: baseCurrency, quoteCurrency, rate, midRate, basisPoint, rateChangeType(UP/EQUAL/DOWN), validFrom, validUntil.
- `Kr/UsMarketCalendarResponse`: today, previousBusinessDay, nextBusinessDay (각 `Kr/UsMarketDay`).
- `KrMarketDay`: date, integrated?(IntegratedHour: preMarket?/regularMarket?/afterMarket? 각 세션 nullable).
- `UsMarketDay`: date, dayMarket?/preMarket?/regularMarket?/afterMarket? (각 세션 startTime/endTime).

## 계좌 (Account)

| 메서드 | 경로 | 결과 | rate |
|--------|------|------|------|
| GET | `/api/v1/accounts` | `Account[]` {accountNo, accountSeq(int64), accountType(BROKERAGE/...)} | ACCOUNT |

- 계좌 헤더 불필요. `accountSeq`가 이후 모든 계좌 스코프 호출의 `X-Tossinvest-Account` 값.

## 자산 (Asset)

| 메서드 | 경로 | 헤더/쿼리 | 결과 | rate |
|--------|------|-----------|------|------|
| GET | `/api/v1/holdings` | `X-Tossinvest-Account`(필수), `symbol`? | `HoldingsOverview` | ASSET |

- `HoldingsOverview`: totalPurchaseAmount(Price), marketValue(OverviewMarketValue), profitLoss(OverviewProfitLoss),
  dailyProfitLoss(OverviewDailyProfitLoss), items(`HoldingsItem[]`).
- `Price` {krw, usd?}. 통화별 합산(환산 미포함).
- `HoldingsItem`: symbol, name, marketCountry(KR/US), currency, quantity, lastPrice, averagePurchasePrice,
  marketValue(MarketValue), profitLoss(ProfitLoss), dailyProfitLoss(DailyProfitLoss), cost(Cost).

## 주문 (Order) — 모두 `X-Tossinvest-Account` 필수

| 메서드 | 경로 | 요청 | 결과 | rate |
|--------|------|------|------|------|
| POST | `/api/v1/orders` | `OrderCreateRequest`(oneOf 수량/금액) | `OrderResponse` {orderId, clientOrderId?} | ORDER |
| POST | `/api/v1/orders/{orderId}/modify` | `OrderModifyRequest` {orderType, quantity?, price?, confirmHighValueOrder?} | `OrderOperationResponse` {orderId(신규)} | ORDER |
| POST | `/api/v1/orders/{orderId}/cancel` | (없음) | `OrderOperationResponse` {orderId(신규)} | ORDER |
| GET | `/api/v1/orders` | `status`(OPEN/CLOSED 필수), `symbol`?, `from`?, `to`?, `cursor`?, `limit`? | `PaginatedOrderResponse` {orders[], nextCursor?, hasNext} | ORDER_HISTORY |
| GET | `/api/v1/orders/{orderId}` | `orderId`(path) | `Order` | ORDER_HISTORY |

- `OrderCreateRequest` oneOf:
  - **Quantity-based**: symbol, side(BUY/SELL), orderType(LIMIT/MARKET), timeInForce?(DAY/CLS, 기본 DAY),
    quantity(정수문자열), price?(LIMIT 필수·MARKET 금지), clientOrderId?(멱등키 ≤36, 10분 유효), confirmHighValueOrder?.
  - **Amount-based** (US MARKET 전용): symbol, side, orderType(MARKET), orderAmount(달러), clientOrderId?, confirmHighValueOrder?.
- `Order`: orderId, symbol, side, orderType, timeInForce(DAY/CLS/OPG), status(OrderStatus), price?, quantity,
  orderAmount?, currency, orderedAt, canceledAt?, execution(OrderExecution).
- `OrderStatus`: PENDING/PENDING_CANCEL/PENDING_REPLACE/PARTIAL_FILLED/FILLED/CANCELED/REJECTED/CANCEL_REJECTED/REPLACE_REJECTED/REPLACED.
- `OrderExecution`: filledQuantity, averageFilledPrice?, filledAmount?, commission?, tax?, filledAt?, settlementDate?.
- `status` 필터는 라이프사이클 그룹(OPEN/CLOSED)이며 `orders[].status` 세부값과 체계가 다름. `CLOSED`는 현재 `400 closed-not-supported`.

## 주문정보 (Order Info) — 모두 `X-Tossinvest-Account` 필수

| 메서드 | 경로 | 쿼리 | 결과 | rate |
|--------|------|------|------|------|
| GET | `/api/v1/buying-power` | `currency`(필수) | `BuyingPowerResponse` {currency, cashBuyingPower} | ORDER_INFO |
| GET | `/api/v1/sellable-quantity` | `symbol`(필수) | `SellableQuantityResponse` {sellableQuantity} | ORDER_INFO |
| GET | `/api/v1/commissions` | (헤더만) | `Commission[]` {marketCountry, commissionRate, startDate?, endDate?} | ORDER_INFO |
