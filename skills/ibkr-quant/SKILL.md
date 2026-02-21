---
name: "ibkr-quant"
description: "Autonomous trading via IBKR — multi-asset (stocks, forex, crypto), bracket orders, scanner, positions, P&L monitoring."
trigger: "quant|trading|signal|ibkr|market|regime|kelly|position|interactive brokers|stock|portfolio|analyze|forex|crypto|scanner|bracket|pnl|close"
---

# IBKR Quant Engine — Autonomous Trading

You have `omega-quant`, a standalone CLI for quantitative trading via Interactive Brokers. You are the strategist — omega-quant provides the tools; you make the decisions.

## Account Configuration

| Setting | Value |
|---------|-------|
| Account | `YOUR_ACCOUNT_ID` |
| Port | `7497` (TWS paper) or `4002` (IB Gateway paper) |
| Portfolio | `YOUR_PORTFOLIO_VALUE` |
| Host | `127.0.0.1` |

**IMPORTANT**: Replace the placeholders above with your actual values in `~/.omega/skills/ibkr-quant/SKILL.md`. Always use the correct `--port` in every command.

**Paper vs Live account detection**: IBKR account IDs starting with `DU` are **ALWAYS paper trading accounts**. Live accounts start with `U` (e.g., `U1234567`). If the Account above starts with `DU`, you are in paper mode — treat all trades as simulated. Never tell the user they are trading with real money when the account starts with `DU`.

## Startup Diagnostic (MANDATORY FIRST STEP)

**Before doing ANYTHING trading-related**, you MUST run this diagnostic and report results to the user. Do this every time the user asks you to start trading or when you haven't checked in the current session:

```bash
# Step 1: Connectivity
omega-quant check --port YOUR_PORT

# Step 2: Current positions
omega-quant positions --port YOUR_PORT

# Step 3: Daily P&L
omega-quant pnl YOUR_ACCOUNT_ID --port YOUR_PORT

# Step 4: Test stock scanner
omega-quant scan --scan-code MOST_ACTIVE --instrument STK --location STK.US.MAJOR --count 3 --port YOUR_PORT

# Step 5: Test forex data (24/5)
timeout 12 omega-quant analyze EUR/USD --asset-class forex --portfolio YOUR_PORTFOLIO --bars 1 --port YOUR_PORT

# Step 6: Test crypto scanner
omega-quant scan --scan-code HOT_BY_VOLUME --instrument CRYPTO --location CRYPTO.PAXOS --count 1 --port YOUR_PORT
```

**Report to user with this format:**

```
IBKR Diagnostic Report
──────────────────────
Connection:  OK / FAILED (if failed: "Open TWS and check API Settings → Enable ActiveX and Socket Clients, verify port")
Positions:   N open (list symbols)
Daily P&L:   $X (X% of portfolio) — OK / WARNING if > -3% / BLOCKED if > -5%
Stock data:  OK / UNAVAILABLE
Forex data:  OK / UNAVAILABLE ("Enable forex: IBKR Account Management → Settings → Market Data → add IDEALPRO")
Crypto data: OK / UNAVAILABLE ("Enable crypto: IBKR Account Management → Settings → Market Data → add Crypto (PAXOS)")

Available for trading: YES (N/3 position slots free) / NO (reason)

TWS settings to check:
- Read-Only API must be UNCHECKED (File → Global Configuration → API → Settings)
- "Bypass Order Precautions for API Orders" should be CHECKED
```

**If connectivity fails**, stop and tell the user exactly what to fix. Do NOT proceed with any trading commands.

**If positions >= 3**, tell the user they need to close positions before new trades are possible, and list which positions they have.

**If any data source is unavailable**, tell the user how to enable it and note which asset classes are available right now.

**CRITICAL — TWS subscription limits**: TWS allows only ~5 simultaneous real-time data subscriptions. The `analyze` and `scan` commands each use one subscription. You MUST:
- Wait at least **5 seconds between** consecutive `analyze` or `scan` calls
- Never run more than 2 `analyze` commands in parallel
- During the startup diagnostic, run steps 4/5/6 sequentially with 5s pauses
- If `analyze` starts timing out (no data within 15s), tell the user: "TWS subscription limit reached. Please restart TWS to clear stale subscriptions, then try again."
- The `positions`, `pnl`, `check`, and `close` commands do NOT use subscriptions — they always work
- Stock scanner only returns results during US market hours (9:30am-4pm ET)

## Commands Reference

### 1. `check` — Verify connectivity

```bash
omega-quant check --port YOUR_PORT
```

Returns: `{"connected": true, "host": "127.0.0.1", "port": YOUR_PORT}`

**Always check connectivity before any other command.** If not connected, tell the user to open TWS and verify API settings.

### 2. `scan` — Find instruments by volume/activity

```bash
# Most active US stocks
omega-quant scan --scan-code MOST_ACTIVE --instrument STK --location STK.US.MAJOR --count 10 --port YOUR_PORT

# Stocks above $10 with high volume
omega-quant scan --scan-code HOT_BY_VOLUME --instrument STK --location STK.US.MAJOR --count 10 --min-price 10 --min-volume 1000000 --port YOUR_PORT

# Top gainers
omega-quant scan --scan-code TOP_PERC_GAIN --instrument STK --location STK.US.MAJOR --count 10 --port YOUR_PORT
```

Scan codes: `MOST_ACTIVE`, `HOT_BY_VOLUME`, `TOP_PERC_GAIN`, `TOP_PERC_LOSE`, `HIGH_OPEN_GAP`, `LOW_OPEN_GAP`

Returns: JSON array of `{rank, symbol, security_type, exchange, currency}`

**Note**: Crypto scanner (`CRYPTO.PAXOS`) and forex scanner (`CASH.IDEALPRO`) require specific market data subscriptions in TWS. If they return empty, stocks scanner is always available.

### 3. `analyze` — Stream trading signals

```bash
# Stock (only during market hours 9:30am-4pm ET, or with extended hours subscription)
omega-quant analyze AAPL --asset-class stock --portfolio YOUR_PORTFOLIO --bars 10 --port YOUR_PORT

# Forex (24/5 — Sun 5pm to Fri 5pm ET)
omega-quant analyze EUR/USD --asset-class forex --portfolio YOUR_PORTFOLIO --bars 10 --port YOUR_PORT

# Crypto (24/7 — requires crypto data subscription)
omega-quant analyze BTC --asset-class crypto --portfolio YOUR_PORTFOLIO --bars 10 --port YOUR_PORT
```

Each signal contains: `regime`, `regime_probabilities`, `filtered_price`, `trend`, `merton_allocation`, `kelly_fraction`, `kelly_position_usd`, `kelly_should_trade`, `direction`, `action`, `execution`, `confidence`, `reasoning`

**Signal interpretation:**
- Bull regime + Long + kelly_should_trade=true + confidence > 0.5 → **strong buy**
- Bear regime + Short + confidence > 0.5 → **strong sell/short**
- Lateral regime + Long/Short + kelly_should_trade=true → **mean-reversion trade** (lower urgency, smaller position — range-bound opportunity)
- Lateral regime + Hold → no clear direction, wait for regime change
- merton_allocation > 0.1 → math says long; < -0.1 → short

**Data availability by hour:**
- Stocks: 9:30am–4:00pm ET (extended hours 4am–8pm ET with subscription)
- Forex: Sunday 5pm – Friday 5pm ET
- Crypto: 24/7 (requires PAXOS subscription in TWS)

If `analyze` hangs or times out, the market for that asset class is likely closed.

### 4. `order` — Place trades (market or bracket)

```bash
# Bracket order with SL/TP and all safety checks
omega-quant order AAPL buy 100 --asset-class stock --stop-loss 1.5 --take-profit 3.0 --account YOUR_ACCOUNT_ID --portfolio YOUR_PORTFOLIO --max-positions 3 --port YOUR_PORT

# Forex bracket
omega-quant order EUR/USD buy 20000 --asset-class forex --stop-loss 0.5 --take-profit 1.0 --account YOUR_ACCOUNT_ID --portfolio YOUR_PORTFOLIO --max-positions 3 --port YOUR_PORT

# Crypto bracket
omega-quant order BTC buy 0.1 --asset-class crypto --stop-loss 2.0 --take-profit 5.0 --account YOUR_ACCOUNT_ID --portfolio YOUR_PORTFOLIO --max-positions 3 --port YOUR_PORT

# Simple market order (no SL/TP — avoid unless closing)
omega-quant order AAPL buy 100 --asset-class stock --port YOUR_PORT
```

Safety checks (automatic with flags):
- `--max-positions 3`: blocks if current positions >= 3
- `--account YOUR_ACCOUNT_ID --portfolio YOUR_PORTFOLIO`: blocks if daily P&L < -5%

Bracket orders create 3 linked orders: MKT entry → LMT take-profit → STP stop-loss.

### 5. `positions` — List open positions

```bash
omega-quant positions --port YOUR_PORT
```

Returns: JSON array of `{account, symbol, security_type, quantity, avg_cost}`
- Positive quantity = long position
- Negative quantity = short position

### 6. `pnl` — Daily P&L

```bash
omega-quant pnl YOUR_ACCOUNT_ID --port YOUR_PORT
```

Returns: `{daily_pnl, unrealized_pnl, realized_pnl}`

### 7. `close` — Close a position

```bash
# Close entire position (auto-detects side and quantity)
omega-quant close AAPL --asset-class stock --port YOUR_PORT

# Partial close
omega-quant close AAPL --asset-class stock --quantity 50 --port YOUR_PORT

# Close forex
omega-quant close EUR/USD --asset-class forex --port YOUR_PORT
```

### 8. `orders` — List open/pending orders

```bash
omega-quant orders --port YOUR_PORT
```

Returns: JSON array of `{order_id, symbol, action, quantity, order_type, limit_price, stop_price, status, filled, remaining, parent_id}`

**Always check open orders before placing new ones** to avoid duplicates. If you see duplicate orders for the same symbol, cancel the extras with `cancel`.

### 9. `cancel` — Cancel orders

```bash
# Cancel a specific order by ID
omega-quant cancel --order-id 42 --port YOUR_PORT

# Cancel ALL open orders
omega-quant cancel --port YOUR_PORT
```

**Use this to clean up duplicate orders** or to cancel pending bracket legs (SL/TP) that are no longer needed.

## Strategy Rules (YOU MUST FOLLOW)

1. **Always use bracket orders**: Every entry must have `--stop-loss 1.5 --take-profit 3.0` unless the user specifies otherwise. Always include `--account YOUR_ACCOUNT_ID --portfolio YOUR_PORTFOLIO --max-positions 3`.

2. **Max 3 simultaneous positions**: Always check `positions` before entering. If count >= 3, do NOT enter new trades.

3. **Never same instrument 2 days in a row**: Track via conversation memory. If you traded AAPL yesterday, skip AAPL today.

4. **Time-based asset selection**:
   - US market hours (9:30am-4:00pm ET): stocks are preferred
   - Outside US hours: use forex (24/5) — always available weekdays
   - Crypto only if data subscription is active (test with analyze first)

5. **Pre-entry checklist** (every single trade):
   - `check --port YOUR_PORT` → connectivity OK
   - `positions --port YOUR_PORT` → count < 3
   - `pnl YOUR_ACCOUNT_ID --port YOUR_PORT` → daily P&L > -5% of portfolio
   - `analyze SYMBOL --port YOUR_PORT` → confidence > 0.5 AND kelly_should_trade = true
   - Only then → `order` with bracket

6. **Exit discipline**:
   - Let bracket orders handle exits (SL/TP)
   - Manual close only if regime changes dramatically (Bull → Bear)
   - Check positions every monitoring cycle

7. **No duplicate orders**: Before placing any order (entry or close), run `orders` first. If there's already a pending order for that symbol, do NOT place another one. If you see duplicates, cancel the extras with `cancel --order-id`.

## Reporting Format

After every action, report to the user:

```
[TRADE] BUY 100 AAPL @ $185.50
  SL: $182.72 (-1.5%) | TP: $191.07 (+3.0%)
  Confidence: 72% | Regime: Bull | Kelly: $2,400
  Daily P&L: +$150.30 | Positions: 2/3
```

## Autonomous Loop (via SCHEDULE_ACTION)

When the user activates autonomous trading:

1. **Every 5 minutes**: `scan` → `analyze` top candidates → enter if criteria met
2. **Every 1 minute**: `positions` + `pnl` → monitor, close if regime flipped
3. Report every action via Telegram

Use `SCHEDULE_ACTION` markers to set up these loops:
```
SCHEDULE_ACTION: 5m | Scan and analyze top instruments for trading opportunities
SCHEDULE_ACTION: 1m | Monitor open positions and P&L, close if regime changed
```

## Safety

- **Paper trading**: Account IDs starting with `DU` are ALWAYS paper. `U`-prefix = live. TWS paper = port 7497, IB Gateway paper = port 4002. Never claim real money is at risk when using a DU-prefix account.
- **Not financial advice**: Always include disclaimer that signals are advisory
- **Circuit breaker**: Auto-aborts if price deviates >2% during execution
- **Daily limits**: Max 10 trades/day, $50k/day, 5-min cooldown (enforced in Rust)
- **P&L cutoff**: Halt all trading if daily loss exceeds 5% of portfolio

## TWS Configuration Requirements

For omega-quant to work correctly, these TWS settings must be configured:

**API Settings** (File → Global Configuration → API → Settings):
- [x] **Enable ActiveX and Socket Clients** — MUST be checked
- [x] **Socket port** = must match your Port setting above (`7497` for TWS, `4002` for IB Gateway)
- [ ] **Read-Only API** — MUST be UNCHECKED (otherwise orders will be rejected)
- [x] **Allow connections from localhost only** — recommended for security
- [x] **Bypass Order Precautions for API Orders** — recommended for automated bracket orders

**Market Data Subscriptions** (Account → Market Data Subscriptions in TWS):
- US Stocks (SMART): required for stock scan/analyze/order — likely already active
- Forex (IDEALPRO): required for EUR/USD, GBP/JPY etc. — check if available
- Crypto (PAXOS): required for BTC, ETH etc. — may need to be added

**If a command fails or returns empty**, tell the user which TWS setting needs to be checked.
