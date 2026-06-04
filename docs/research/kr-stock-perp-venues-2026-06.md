# Korean-Stock Perpetual-Futures Venues — Census

**As of 2026-06-04.** Goal: every crypto venue (CEX or on-chain perp DEX) that lists at least one **Korean** equity or **Korean** index perpetual swap. US/Japan/proxy products do **not** qualify a venue for the main count.

**Methodology note (evidence standard):** Every "Live?" + "KR symbols" claim below was verified **directly against the venue's live public instrument/market-data API on 2026-06-04** (curl + grep of the full symbol list), not from news summaries. News URLs are cited for listing dates and context only. Live-API verification is the authoritative live-status evidence; where I could not date the listing precisely from primary sources, the row is marked "live (API-verified 2026-06-04), exact list date uncertain."

**Inclusion rule (the one judgment call):** "Korean" = a Korean single stock (Samsung Electronics, SK Hynix, Hyundai Motor, NAVER, Kakao, LG Energy Solution, Celltrion, POSCO…) **or** a Korean index (KOSPI/KOSPI 200/KOSDAQ, or a synthetic Korea-composite like Lighter's `KRCOMP` / Hyperliquid's `KR200`). A **US-domiciled Korea-proxy ETF** (`EWY` = iShares MSCI South Korea ETF) does **not** by itself qualify a venue — those venues are listed in "Korea-proxy only (excluded from main count)."

---

## 1. Summary Table — venues WITH Korean-stock/index perps

| # | Venue | Type | Chain | KR symbols (exact, from API) | Live? (2026-06-04) | Market-data API base | Order auth model | Source |
|---|-------|------|-------|------------------------------|--------------------|----------------------|------------------|--------|
| 1 | **Binance** | CEX (USDⓈ-M Futures) | — | `SAMSUNGUSDT`, `SKHYNIXUSDT`, `HYUNDAIUSDT` | Yes | `https://fapi.binance.com` | HMAC-SHA256 | [B1] |
| 2 | **Hyperliquid (xyz dex)** | Perp DEX (HIP-3 builder) | Hyperliquid L1 | `xyz:SMSN`, `xyz:SKHX`, `xyz:HYUNDAI`, **`xyz:KR200`** (KOSPI 200) | Yes | `https://api.hyperliquid.xyz` | EIP-712 (signed actions) | [H1] |
| 3 | **Lighter** | Perp DEX (zk-rollup, order-book) | Lighter zk-L2 (Ethereum) | `SAMSUNGUSD`(162), `SKHYNIXUSD`(161), `HYUNDAIUSD`(160), `SAMSUNG`(140), `HYUNDAI`(141), `SKHYNIX`(143), **`KRCOMP`(142)** | Yes | `https://mainnet.zklighter.elliot.ai` | zk Schnorr/Poseidon signature | [L1] |
| 4 | **MEXC** | CEX (contract/futures) | — | `SAMSUNGSTOCK_USDT`, `SKHYNIXSTOCK_USDT`, `HYUNDAISTOCK_USDT` | Yes | `https://contract.mexc.com` | HMAC-SHA256 | [M1] |
| 5 | **Bybit** | CEX (USDT perp, linear) | — | `SAMSUNGUSDT`, `SKHYNIXUSDT`, `HYUNDAIUSDT` | Yes | `https://api.bybit.com` | HMAC-SHA256 | [BY1] |
| 6 | **Bitget** | CEX (USDT-M futures) | — | `SAMSUNGUSDT`, `SKHYNIXUSDT`, `HYUNDAIUSDT` | Yes | `https://api.bitget.com` | HMAC-SHA256 | [BG1] |
| 7 | **Gate.io** | CEX (USDT perp) | — | `SAMSUNG_USDT`, `SKHYNIX_USDT`, `HYUNDAI_USDT` | Yes | `https://api.gateio.ws` | HMAC-SHA512 | [G1] |
| 8 | **KuCoin** | CEX (futures) | — | `SAMSUNGUSDTM`, `SKHYNIXUSDTM`, `HYUNDAIUSDTM` | Yes | `https://api-futures.kucoin.com` | HMAC-SHA256 (+passphrase) | [K1] |
| 9 | **BingX** | CEX (perp swap) | — | `NCSKSAMSUNG2USD-USDT`, `NCSKSKHYNIX2USD-USDT`, **`NCSIKOSPI2USD-USDT`** (KOSPI) | Yes | `https://open-api.bingx.com` | HMAC-SHA256 | [BX1] |
| 10 | **HTX** | CEX (USDT-M linear swap) | — | `SAMSUNG-USDT`, `SKHYNIX-USDT`, `HYUNDAI-USDT` | Yes | `https://api.hbdm.com` | HMAC-SHA256 | [HT1] |
| 11 | **Bitunix** | CEX (USDT perp) | — | `SAMSUNGUSDT`, `SKHYNIXUSDT`, `HYUNDAIUSDT` | Yes | `https://fapi.bitunix.com` | HMAC-SHA256 | [BU1] |
| 12 | **Phemex** | CEX (USDT perp) | — | `SAMSUNGUSDT`, `SKHYNIXUSDT`, `HYUNDAIUSDT` | Yes | `https://api.phemex.com` | HMAC-SHA256 | [P1] |
| 13 | **WEEX** | CEX (USDT perp) | — | `cmt_samsungusdt`, `cmt_skhynixusdt`, `cmt_hyundaiusdt` | Yes | `https://api-contract.weex.com` | HMAC-SHA256 | [W1] |
| 14 | **Toobit** | CEX (USDT perp swap) | — | `SAMSUNG-SWAP-USDT`, `SKHYNIX-SWAP-USDT`, `HYUNDAI-SWAP-USDT` | Yes | `https://api.toobit.com` | HMAC-SHA256 | [T1] |
| 15 | **Aster** | Perp DEX | BNB Chain (multi-chain) | `SAMSUNGUSDT`, `SKHYNIXUSDT` (no Hyundai) | Yes | `https://fapi.asterdex.com` | EIP-712 / API key (Binance-style fapi) | [A1] |
| 16 | **Pacifica** | Perp DEX | Solana | `SAMSUNG`, `SKHYNIX` (no Hyundai) | Yes | `https://api.pacifica.fi` | Ed25519 (Solana key) signature | [PA1] |

**Total venues with ≥1 Korean stock/index perp: 16** (12 CEX, 4 perp DEX).
Of these, **12 are new beyond the originally-known 4** (Binance / Hyperliquid-xyz / Lighter / MEXC). New: Bybit, Bitget, Gate.io, KuCoin, BingX, HTX, Bitunix, Phemex, WEEX, Toobit, Aster, Pacifica.

**Korean index products found:** Hyperliquid `xyz:KR200` (KOSPI 200), Lighter `KRCOMP` (Korea-composite synthetic), BingX `NCSIKOSPI2USD-USDT` (KOSPI). No venue lists a KOSDAQ perp.

---

## 2. Per-venue detail

### Already-known venues (re-verified live 2026-06-04)

#### 1. Binance — USDⓈ-M Futures (CEX)
- API base: `https://fapi.binance.com`; REST `GET /fapi/v1/exchangeInfo` (verified, 765 symbols), WS at `wss://fstream.binance.com`. Documented REST/WS. [B1]
- KR symbols present in live exchangeInfo: `SAMSUNGUSDT`, `SKHYNIXUSDT`, `HYUNDAIUSDT`. USDT-margined, up to 20x, 8h funding.
- Auth: HMAC-SHA256 signed query (`X-MBX-APIKEY` + `signature`).
- Listing date: **2026-06-02, 03:00 UTC**. Geofenced out of South Korea (FSC). [B2][B3]

#### 2. Hyperliquid — "xyz" builder DEX (HIP-3 perp DEX)
- API base: `https://api.hyperliquid.xyz`; `POST /info {"type":"meta","dex":"xyz"}` returns the 85-name `xyz` universe (verified). Documented REST/WS (`wss://api.hyperliquid.xyz/ws`). [H1]
- KR names in the live `xyz` universe: `xyz:SMSN` (Samsung), `xyz:SKHX` (SK Hynix), `xyz:HYUNDAI`, and **`xyz:KR200`** (KOSPI 200 index). (Also present: `xyz:EWY` Korea ETF — proxy, and `xyz:EWJ`/`xyz:JP225`/`xyz:SOFTBANK` = Japan, not KR.)
- Chain: Hyperliquid L1. xyz/trade.xyz is the dominant HIP-3 deployment (~90%+ of HIP-3 OI). [H2][H3]
- Auth: EIP-712-signed actions posted to `/exchange`. HIP-3 went live on mainnet ~2025-10-13. [H2]

#### 3. Lighter (zk order-book perp DEX)
- API base: `https://mainnet.zklighter.elliot.ai`; `GET /api/v1/orderBooks` (verified). Documented REST + WS. [L1]
- KR markets in live orderBooks (with market_id): `HYUNDAIUSD`(160), `SKHYNIXUSD`(161), `SAMSUNGUSD`(162), plus base-quote variants `SAMSUNG`(140), `HYUNDAI`(141), `SKHYNIX`(143), and **`KRCOMP`(142)** = Korea-composite index. (Also `USDKRW`(105) FX, `SKR`(130), `MKR`(28) — not equities.)
- Chain: Lighter zk-rollup (Ethereum L2). Auth: zk signature (Poseidon2 hash + Schnorr-style scheme over the order payload), API-key registered on-chain.
- Marketed as "first" Korean-equity perp DEX; listing reported early-to-mid 2026. [L2][L3]

#### 4. MEXC — Futures/contract (CEX)
- API base: `https://contract.mexc.com`; `GET /api/v1/contract/detail` (verified, 884 contracts). Documented REST/WS. [M1]
- KR symbols: `SAMSUNGSTOCK_USDT`, `SKHYNIXSTOCK_USDT`, `HYUNDAISTOCK_USDT` (part of a large `*STOCK_USDT` TradFi family that is otherwise US equities). Auth: HMAC-SHA256.

### New venues (discovered + API-verified this run)

#### 5. Bybit (CEX, linear USDT perp)
- `GET https://api.bybit.com/v5/market/instruments-info?category=linear` → live list contains `SAMSUNGUSDT`, `SKHYNIXUSDT`, `HYUNDAIUSDT` (also `EWYUSDT` Korea-ETF proxy). [BY1]
- Auth: HMAC-SHA256 (v5 signed header). Bybit publicly announced a "TradFi Perp" push in May 2026; KR single-stock perps are live on the API as of 2026-06-04. [BY2]

#### 6. Bitget (CEX, USDT-M futures)
- `GET https://api.bitget.com/api/v2/mix/market/contracts?productType=USDT-FUTURES` → `SAMSUNGUSDT`, `SKHYNIXUSDT`, `HYUNDAIUSDT` (verified). [BG1]
- Auth: HMAC-SHA256 (+ passphrase). Marketed "20x Korean blue-chip perps." [BG2]

#### 7. Gate.io (CEX, USDT perp)
- `GET https://api.gateio.ws/api/v4/futures/usdt/contracts` → `SAMSUNG_USDT`, `SKHYNIX_USDT`, `HYUNDAI_USDT` (verified). [G1]
- Auth: HMAC-SHA512 (APIv4 signature).

#### 8. KuCoin (CEX, futures)
- `GET https://api-futures.kucoin.com/api/v1/contracts/active` → `SAMSUNGUSDTM`, `SKHYNIXUSDTM`, `HYUNDAIUSDTM` (verified). [K1]
- Note: KuCoin's 2026-05-22 "stock index perpetual" announcement [K2] covered only US names (BE/APLD/ASTS/VRT); the Korean single-stock perps are a **separate, later listing** present on the live API. Auth: HMAC-SHA256 + API passphrase.

#### 9. BingX (CEX, perp swap)
- `GET https://open-api.bingx.com/openApi/swap/v2/quote/contracts` → `NCSKSAMSUNG2USD-USDT`, `NCSKSKHYNIX2USD-USDT`, and index **`NCSIKOSPI2USD-USDT`** (verified). No Hyundai. Also Korea-ETF proxy `NCSIEWY2USD-USDT`. [BX1]
- BingX uses an `NCSK…2USD` (single-stock) / `NCSI…2USD` (index) TradFi naming convention. Auth: HMAC-SHA256.

#### 10. HTX (CEX, USDT-M linear swap)
- `GET https://api.hbdm.com/linear-swap-api/v1/swap_contract_info` → `SAMSUNG-USDT`, `SKHYNIX-USDT`, `HYUNDAI-USDT` (verified). [HT1]
- HTX's public "TradFi Zone" headline emphasized US stocks [HT2], but the live linear-swap contract list includes the three Korean names. Auth: HMAC-SHA256.

#### 11. Bitunix (CEX, USDT perp)
- `GET https://fapi.bitunix.com/api/v1/futures/market/trading_pairs` → `SAMSUNGUSDT`, `SKHYNIXUSDT`, `HYUNDAIUSDT` (verified). [BU1] Auth: HMAC-SHA256.

#### 12. Phemex (CEX, USDT perp)
- `GET https://api.phemex.com/public/products` → `SAMSUNGUSDT`, `SKHYNIXUSDT`, `HYUNDAIUSDT` (verified, 820 products). [P1]
- Phemex publicized a "70+ TradFi assets incl. global equities" expansion, June 2026. [P2] Auth: HMAC-SHA256.

#### 13. WEEX (CEX, USDT perp)
- `GET https://api-contract.weex.com/capi/v2/market/contracts` → `cmt_samsungusdt`, `cmt_skhynixusdt`, `cmt_hyundaiusdt` (verified). [W1] Auth: HMAC-SHA256.

#### 14. Toobit (CEX, USDT perp swap)
- `GET https://api.toobit.com/api/v1/exchangeInfo` → `SAMSUNG-SWAP-USDT`, `SKHYNIX-SWAP-USDT`, `HYUNDAI-SWAP-USDT` (verified in raw symbol list). [T1] Auth: HMAC-SHA256.

#### 15. Aster (perp DEX, BNB Chain)
- `GET https://fapi.asterdex.com/fapi/v1/exchangeInfo` (Binance-style fapi) → `SAMSUNGUSDT`, `SKHYNIXUSDT` (verified, 460 symbols). **No Hyundai.** [A1]
- Chain: Aster runs primarily on BNB Chain (multi-chain). Marketed "5x leverage Samsung & SK Hynix AI perps," May/June 2026. [A2][A3]
- Auth: EIP-712 wallet signature for on-chain settlement; the fapi also supports API-key/HMAC for off-chain order placement (Binance-compatible surface).

#### 16. Pacifica (perp DEX, Solana)
- `GET https://api.pacifica.fi/api/v1/info` → `SAMSUNG`, `SKHYNIX` (verified, 69 markets). **No Hyundai.** [PA1]
- Chain: Solana (CEX-like order-book DEX). Auth: Ed25519 signature with the trader's Solana key over the order payload.

---

## 3. Excluded — venues checked that do NOT list Korean stock/index perps

All checked against live APIs on 2026-06-04 unless noted.

| Venue | Type | Why excluded | Evidence |
|-------|------|--------------|----------|
| **OKX** | CEX | No KR equity in SWAP instruments (361 markets, 0 hits) | `GET www.okx.com/api/v5/public/instruments?instType=SWAP` (API-verified) |
| **Kraken** | CEX | Kraken Futures = crypto-only; 331 instruments, 0 KR | `GET futures.kraken.com/derivatives/api/v3/instruments` (API-verified) |
| **Coinbase (International)** | CEX | 297 SPOT/PERP instruments, all crypto, 0 KR | `GET api.international.coinbase.com/api/v1/instruments` (API-verified) |
| **Backpack** | CEX/DEX (Solana) | 84 PERP markets, all crypto; tokenized-stock platform is US equities (spot), not KR perps | `GET api.backpack.exchange/api/v1/markets` (API-verified, 0 KR) [BP1] |
| **dYdX v4** | Perp DEX (dYdX Chain) | 296 perp markets, 0 KR | `GET indexer.dydx.trade/v4/perpetualMarkets` (API-verified) |
| **Drift** | Perp DEX (Solana) | Crypto + pre-launch token perps only; lists **no equities**; relaunching as USDT perps DEX mid-2026 | product-scope source [DR1] (API endpoints returned 403/HTML on this run) |
| **Aevo** | Perp DEX (OP/Aevo L2) | 1,904 instruments, 0 KR | `GET api.aevo.xyz/markets` (API-verified) |
| **Paradex** | Perp DEX (Starknet appchain) | 597 markets, 0 KR | `GET api.prod.paradex.trade/v1/markets` (API-verified) |
| **Vertex** | Perp DEX (Arbitrum) | Crypto perps only; lists **no equities** | product-scope (no equity product line) [DR1] (API endpoints unreachable this run) |
| **GMX** | Perp DEX (Arbitrum/Avax) | 126 tokens, no equities at all (no synthetic stock markets) | `GET arbitrum-api.gmxinfra.io/tokens` (API-verified) |
| **Ostium** | Perp DEX (Arbitrum, RWA) | 50+ RWA markets incl. US single-stocks (NVDA/MSFT/TSLA/COIN), indices, FX, commodities — **no Korean stock or KOSPI/KOSDAQ** | [OS1][OS2] (product-scope; subgraph not reachable this run) |
| **Injective / Helix** | Perp DEX (Injective) | No KR equity in derivative markets list | `GET sentry.lcd.injective.network/.../derivative/markets` (API-verified, 0 KR) |
| **Bluefin** | Perp DEX (Sui) | Infra supports tokenized-stock markets but none KR launched; crypto perps only | [BL1] (API "no healthy upstream" this run; secondary-sourced) |
| **Apex (Omni)** | Perp DEX | No KR equity markets | `GET omni.apex.exchange/api/v3/symbols` (API-verified, 0 KR) |
| **Extended** | Perp DEX (Starknet) | 130 markets incl. a "TradFi" category, but 0 Korean names | `GET api.starknet.extended.exchange/api/v1/info/markets` (API-verified) |
| **Avantis** | Perp DEX (Base) | RWA perps = FX / commodities / indices; **no equities / no KR** | product-scope source [DR1] (`/pairs` returned 404 this run) |
| **edgeX** | Perp DEX | 292 contracts, 0 KR | `GET pro.edgex.exchange/api/v1/public/meta/getMetaData` (API-verified) |
| **Hibachi** | Perp DEX | 0 KR in exchange-info | `GET data-api.hibachi.xyz/market/exchange-info` (API-verified) |

### Korea-proxy only (excluded from main count — EWY ETF, not a Korean instrument)
- **Bybit** *also* lists `EWYUSDT` — but Bybit qualifies for the **main count** anyway via its direct Samsung/SKHynix/Hyundai perps (row 5).
- **BingX** *also* lists `NCSIEWY2USD-USDT` — qualifies via direct names + KOSPI (row 9).
- **Hyperliquid xyz** *also* lists `xyz:EWY` — qualifies via direct names + KR200 (row 2).
- A venue whose *only* Korea exposure was `EWY` would be excluded; none of the checked venues fell into that category alone.

---

## 4. Uncertain / needs key or further confirmation

- **Exact listing dates for the secondary CEXs/DEXs** (Bybit, Bitget, Gate, KuCoin, BingX, HTX, Bitunix, Phemex, WEEX, Toobit, Aster, Pacifica): the KR symbols are **live on the API today (2026-06-04)**, but precise list dates are not all pinned to a primary announcement. They cluster in **late-May to early-June 2026** alongside Binance's 2026-06-02 launch and the broader "TradFi perp" wave. Treat list dates as "≈ May–Jun 2026, unconfirmed per-venue."
- **Aster chain specifics:** Aster is multi-chain; "Aster Chain" as a dedicated L1 was referenced in some coverage but not confirmed — primary settlement is BNB Chain. [A2]
- **Aster / Pacifica auth exact scheme:** confirmed signature-based (EIP-712 for Aster, Ed25519/Solana for Pacifica) from their public API surface; exact field-ordering needs the signing SDK to implement against.
- **KuCoin / HTX:** their *public announcements* emphasized US-only stock lists; the Korean perps are present on the live API but I did not locate a dedicated KR-listing announcement page (API is the evidence). Worth a follow-up to find the announcement URL.
- **Drift / Vertex / Avantis:** their public list endpoints returned 403/404/HTML on this run, so the exclusion rests on **product-scope evidence** (these protocols list crypto + pre-launch/RWA-FX/commodity/index markets but **no single-stock equities at all**) rather than a clean symbol-list grep. Exclusion is sound (no equity product line ⇒ no Korean equity), but a working API/key would harden it to the same standard as the other rows. [DR1]
- **"Live?" column semantics:** "Yes" means the symbol is **present in the venue's live instrument list on 2026-06-04** (and these all launched in the May–Jun 2026 TradFi-perp wave, so are actively trading). It is not a per-symbol check of an `active`/`PENDING_TRADING` status flag — read it as "listed & live as of 2026-06-04," spot-checks consistent with active trading.
- **Geo-restrictions:** Binance explicitly geofences South Korean users (FSC). Most other CEXs likely apply similar TradFi-perp geo-blocks; not enumerated here.

---

## Sources

- [B1] Binance fapi exchangeInfo (live API, 2026-06-04): `https://fapi.binance.com/fapi/v1/exchangeInfo`
- [B2] Binance to launch Samsung/SK Hynix/Hyundai perps June 2 — PANews, 2026: `https://www.panewslab.com/en/articles/019e8390-d620-76b8-9fcd-022754daac93`
- [B3] Binance lists KR perps, not for South Korean users — CryptoRank, 2026: `https://cryptorank.io/news/feed/b7dbb-binance-perpetual-futures-samsung-sk-hynix-hyundai-south-korea`
- [H1] Hyperliquid xyz dex meta (live API, 2026-06-04): `POST https://api.hyperliquid.xyz/info {"type":"meta","dex":"xyz"}`
- [H2] Hyperliquid HIP-3 / TradeXYZ explainer — Datawallet, 2026: `https://www.datawallet.com/crypto/tradexyz-explained`
- [H3] HL//KR Samsung (xyz:SMSN) 24h price page: `https://hlkr.co.kr/stocks/smsn`
- [L1] Lighter orderBooks (live API, 2026-06-04): `https://mainnet.zklighter.elliot.ai/api/v1/orderBooks`
- [L2] Korean stocks meet DeFi: Lighter lists Samsung/SK Hynix/Hyundai perps — TradingView/Invezz, 2026: `https://www.tradingview.com/news/invezz:a627a32b7094b:0-korean-stocks-meet-defi-as-lighter-lists-samsung-sk-hynix-and-hyundai-perps/`
- [L3] Korean Equity Perpetual Futures Go Live on DEX — ourcryptotalk, 2026: `https://ourcryptotalk.com/news/korean-equity-perpetual-futures-dex/`
- [M1] MEXC contract detail (live API, 2026-06-04): `https://contract.mexc.com/api/v1/contract/detail`
- [BY1] Bybit linear instruments-info (live API, 2026-06-04): `https://api.bybit.com/v5/market/instruments-info?category=linear`
- [BY2] Bybit expands into TradFi perps — CryptoTimes, 2026-05-08: `https://www.cryptotimes.io/2026/05/08/bybit-expands-into-tradfi-with-7-new-stock-and-etf-perpetuals/`
- [BG1] Bitget mix contracts (live API, 2026-06-04): `https://api.bitget.com/api/v2/mix/market/contracts?productType=USDT-FUTURES`
- [BG2] Bitget 20x Korean stock perps live — Bitget News, 2026: `https://www.bitget.com/news/detail/12560605439568`
- [G1] Gate.io futures contracts (live API, 2026-06-04): `https://api.gateio.ws/api/v4/futures/usdt/contracts`
- [K1] KuCoin futures active contracts (live API, 2026-06-04): `https://api-futures.kucoin.com/api/v1/contracts/active`
- [K2] KuCoin stock-index perp listing (US names) — 2026-05-22: `https://www.kucoin.com/announcement/en-kucoin-futures-will-list-multiple-stock-index-perpetual-contracts-2026-05-22`
- [BX1] BingX swap contracts (live API, 2026-06-04): `https://open-api.bingx.com/openApi/swap/v2/quote/contracts`
- [HT1] HTX linear-swap contract info (live API, 2026-06-04): `https://api.hbdm.com/linear-swap-api/v1/swap_contract_info`
- [HT2] HTX launches US stock futures (TradFi Zone) — CryptoTimes, 2026-05-25: `https://www.cryptotimes.io/2026/05/25/htx-launches-us-stock-futures-trading-with-usdt/`
- [BU1] Bitunix trading pairs (live API, 2026-06-04): `https://fapi.bitunix.com/api/v1/futures/market/trading_pairs`
- [P1] Phemex products (live API, 2026-06-04): `https://api.phemex.com/public/products`
- [P2] Phemex expands TradFi 70+ assets — PRNewswire, 2026-06-03: `https://www.manilatimes.net/2026/06/03/tmt-newswire/pr-newswire/phemex-expands-tradfi-offering-beyond-70-assets-bringing-global-equities-to-247-crypto-markets/2357702`
- [W1] WEEX contracts (live API, 2026-06-04): `https://api-contract.weex.com/capi/v2/market/contracts`
- [T1] Toobit exchangeInfo (live API, 2026-06-04): `https://api.toobit.com/api/v1/exchangeInfo`
- [A1] Aster fapi exchangeInfo (live API, 2026-06-04): `https://fapi.asterdex.com/fapi/v1/exchangeInfo`
- [A2] Aster DEX adds 5x leverage on Samsung & SK Hynix — Bitget News, 2026: `https://www.bitget.com/asia/amp/news/detail/12560605438302`
- [A3] Aster brings Korean AI stocks on-chain — LiveBitcoinNews, 2026: `https://www.livebitcoinnews.com/aster-brings-korean-ai-stocks-onchain-heres-why-it-matters/`
- [PA1] Pacifica info (live API, 2026-06-04): `https://api.pacifica.fi/api/v1/info`
- [BP1] Backpack markets (live API, 2026-06-04): `https://api.backpack.exchange/api/v1/markets`
- [OS1] Ostium — Perp swaps for RWAs (stocks/indices/commodities/FX): `https://www.ostium.com/`
- [OS2] Ostium V2 / 50+ markets incl. single-name US equities: `https://www.ostium.com/blog/introducing-ostium-v2`
- [BL1] Bluefin (Sui) — infra for pre-IPO/tokenized-stock markets, no KR launched: `https://learn.backpack.exchange/articles/what-is-bluefin-a-high-performance-on-chain-trading-platform-on-sui`
- [DR1] Product-scope (Drift = Solana crypto/pre-launch perps, no equities; Avantis = Base RWA FX/commodities/indices, no equities; Vertex = crypto perps) — web search 2026-06-04; Drift: `https://www.drift.trade/`, Avantis: `https://cryptoslate.com/perp-dex-season-avantis-and-aster-defy-market-downturn-with-impressive-rallies/`
- [KR1] Kraken Futures instruments (live API, 2026-06-04): `https://futures.kraken.com/derivatives/api/v3/instruments`
- [CB1] Coinbase International instruments (live API, 2026-06-04): `https://api.international.coinbase.com/api/v1/instruments`
