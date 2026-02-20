# omega-quant — Technical Specification

## Overview

Quantitative trading engine crate providing real-time market analysis, advisory signals, multi-asset order execution, and portfolio monitoring. Connects to Interactive Brokers (IBKR) via the TWS API (ibapi crate). Exposed as a standalone CLI binary (`omega-quant`) invoked by the AI through the `ibkr-quant` skill — zero gateway coupling.

## Architecture

```
skills/ibkr-quant/SKILL.md          ← teaches the AI when/how to use omega-quant
         │
         │ AI reads instructions, invokes via bash:
         ▼
omega-quant check                    ← TCP connectivity check
omega-quant scan                     ← market scanner (volume, activity)
omega-quant analyze AAPL ...         ← stream signals (Kalman→HMM→Kelly→JSON)
omega-quant order AAPL buy 100 ...   ← place order (market or bracket SL/TP)
omega-quant positions                ← list open positions + avg cost
omega-quant pnl DU1234567            ← daily P&L for account
omega-quant close AAPL               ← close position (auto-detect side/qty)
         │
         │ uses library:
         ▼
crates/omega-quant/src/lib.rs         ← math core (QuantEngine)
crates/omega-quant/src/market_data.rs  ← IBKR TWS feed, scanner, multi-asset contracts
crates/omega-quant/src/executor.rs     ← order execution, bracket orders, positions, P&L
```

## Modules

| Module | Purpose |
|--------|---------|
| `bin/main.rs` | CLI binary: 7 subcommands (`check`, `scan`, `analyze`, `order`, `positions`, `pnl`, `close`) via clap |
| `signal.rs` | Output types: `QuantSignal`, `Regime`, `Direction`, `Action`, `ExecutionStrategy` |
| `kalman.rs` | Kalman filter (2D state: price + trend, plain f64 math, no nalgebra) |
| `hmm.rs` | Hidden Markov Model (3-state: Bull/Bear/Lateral, 5 observations, Baum-Welch training) |
| `kelly.rs` | Fractional Kelly criterion (position sizing with safety clamps) |
| `market_data.rs` | `AssetClass` enum (Stock/Forex/Crypto), `build_contract()`, IBKR TWS real-time price feed with auto-reconnect (stocks/crypto: `realtime_bars`, forex: `tick_by_tick_midpoint`), `run_scanner()`, `ScanResult` |
| `execution.rs` | TWAP + Immediate execution plan types |
| `executor.rs` | Live order execution with circuit breaker, daily limits, crash recovery, bracket orders (`place_bracket_order`), position queries (`get_positions`), P&L queries via snapshot API (`get_daily_pnl`), price queries via `market_data().snapshot()` (`get_ibkr_price`), close positions (`close_position`), safety checks (`check_max_positions`, `check_daily_pnl_cutoff`) |
| `lib.rs` | `QuantEngine` orchestrator + inline Merton allocation |

## Multi-Asset Support

| Asset Class | CLI flag | Symbol format | Contract builder | WhatToShow |
|-------------|----------|---------------|------------------|------------|
| Stock | `--asset-class stock` | `AAPL` | `Contract::stock(symbol)` | `realtime_bars` (Trades) |
| Forex | `--asset-class forex` | `EUR/USD` | `Contract::forex(base, quote)` | `tick_by_tick_midpoint` (realtime_bars not supported for CASH contracts) |
| Crypto | `--asset-class crypto` | `BTC` | `Contract::crypto(symbol)` | `realtime_bars` (Trades) |

`build_contract()` in `market_data.rs` handles parsing and construction. Forex symbols are split on `/` (must be `BASE/QUOTE` format). Price queries (`get_ibkr_price`) use `market_data().snapshot()` for all asset classes.

## CLI Subcommands

### `omega-quant check`
TCP connectivity check to IB Gateway.
```
omega-quant check --port 4002 --host 127.0.0.1
→ {"connected": true, "host": "127.0.0.1", "port": 4002}
```

### `omega-quant scan`
Market scanner — find instruments by volume/activity via IBKR scanner subscription.
```
omega-quant scan --scan-code MOST_ACTIVE --instrument STK --location STK.US.MAJOR --count 10
→ [{"rank":0,"symbol":"AAPL","security_type":"STK","exchange":"NASDAQ","currency":"USD"},...]
```
Scan codes: `MOST_ACTIVE`, `HOT_BY_VOLUME`, `TOP_PERC_GAIN`, `TOP_PERC_LOSE`, `HIGH_OPEN_GAP`, `LOW_OPEN_GAP`. Optional filters: `--min-price`, `--min-volume`.

### `omega-quant analyze`
Stream trading signals as JSONL (one JSON object per price update).
```
omega-quant analyze AAPL --asset-class stock --portfolio 50000 --port 4002 --bars 30
→ {"timestamp":"...","symbol":"AAPL","raw_price":185.50,"regime":"Bull",...}
```
Uses `QuantEngine.process_price()` for each price tick. Stops after `--bars` count. Has a 15-second timeout for first data — reports error if no data received (market closed or subscription missing). Supports all asset classes.

### `omega-quant order`
Place a market order or bracket order (entry + SL + TP) via IBKR TWS API.
```
# Market order
omega-quant order AAPL buy 100 --asset-class stock --port 4002
→ {"type":"market","status":"Completed","filled_qty":100.0,...}

# Bracket order
omega-quant order AAPL buy 100 --asset-class stock --stop-loss 1.5 --take-profit 3.0 --port 4002
→ {"type":"bracket","status":"Completed","entry_price":185.50,"stop_loss_price":182.72,"take_profit_price":191.07,...}
```
Safety flags: `--max-positions N` (default 3), `--account` + `--portfolio` (P&L cutoff at -5%).

Bracket orders create 3 linked IBKR orders: parent MKT (transmit=false) → TP LMT (transmit=false) → SL STP (transmit=true).

### `omega-quant positions`
List all open positions from IBKR.
```
omega-quant positions --port 4002
→ [{"account":"DU1234567","symbol":"AAPL","security_type":"STK","quantity":100.0,"avg_cost":185.50},...]
```
Filters out zero-quantity positions.

### `omega-quant pnl`
Get daily P&L for an account.
```
omega-quant pnl DU1234567 --port 4002
→ {"daily_pnl":-250.50,"unrealized_pnl":-100.0,"realized_pnl":-150.50}
```

### `omega-quant close`
Close an open position. Auto-detects side (long→sell, short→buy) and quantity from current position.
```
omega-quant close AAPL --asset-class stock --port 4002
→ {"status":"Completed","side":"SELL","closed_qty":100.0,"filled_usd":18552.0,...}

# Partial close
omega-quant close AAPL --asset-class stock --quantity 50 --port 4002
```

## Pipeline

```
Price tick → Kalman filter → Returns → EWMA volatility
                                    → HMM regime detection
                                    → Merton optimal allocation (inlined)
                                    → Kelly sizing
                                    → Action + Direction
                                    → Execution strategy
                                    → QuantSignal output
```

## Safety Invariants

1. Paper trading by default (port 4002)
2. Kelly fraction <= 1.0 (clamped in `KellyCriterion::new()`)
3. Max allocation <= 50% (clamped in `KellyCriterion::new()`)
4. Daily trade limit checked in `Executor::execute()`
5. Daily USD limit checked in `Executor::execute()`
6. Cooldown enforced via `DailyLimits::check()`
7. Circuit breaker at 2% deviation in `execute_twap()`
8. Crash recovery via `ExecutionState` JSON serde
9. Disclaimer in `format_signal()` includes "NOT FINANCIAL ADVICE"
10. Max positions check via `check_max_positions()` (default 3)
11. Daily P&L cutoff via `check_daily_pnl_cutoff()` (default -5%)
12. Bracket orders always linked (parent + TP + SL transmitted atomically)

## Skill Integration

The `ibkr-quant` skill (`skills/ibkr-quant/SKILL.md`) teaches the AI:
- All 7 subcommands with examples for stocks, forex, and crypto
- Strategy rules (bracket orders, max positions, pre-entry checklist)
- Autonomous loop via SCHEDULE_ACTION (scan every 5min, monitor every 1min)
- Safety rules (paper first, P&L cutoff, not financial advice)
- No gateway wiring needed — removing the skill folder removes quant entirely

## Prerequisites

IB Gateway or TWS must be running locally:
- Paper trading: port 4002
- Live trading: port 4001
- Auth handled by IB Gateway app (no API keys in code)

## Dependencies

tokio, serde, serde_json, tracing, anyhow, chrono, uuid, clap, ibapi, rand, futures
