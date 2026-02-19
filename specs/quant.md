# omega-quant — Technical Specification

## Overview

Quantitative trading engine crate providing real-time market analysis and advisory signals. Connects to Interactive Brokers (IBKR) via the TWS API (ibapi crate).

## Modules

| Module | Purpose |
|--------|---------|
| `signal.rs` | Output types: `QuantSignal`, `Regime`, `Direction`, `Action`, `ExecutionStrategy` |
| `kalman.rs` | Kalman filter (2D state: price + trend, plain f64 math, no nalgebra) |
| `hmm.rs` | Hidden Markov Model (3-state: Bull/Bear/Lateral, 5 observations, Baum-Welch training) |
| `kelly.rs` | Fractional Kelly criterion (position sizing with safety clamps) |
| `market_data.rs` | IBKR TWS real-time price feed via `ibapi` crate with auto-reconnect |
| `execution.rs` | TWAP + Immediate execution plan types |
| `executor.rs` | Live order execution with circuit breaker, daily limits, crash recovery |
| `lib.rs` | `QuantEngine` orchestrator + inline Merton allocation |

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
2. Human confirms every trade (`require_confirmation` always true)
3. Kelly fraction <= 1.0 (clamped in `KellyCriterion::new()`)
4. Max allocation <= 50% (clamped in `KellyCriterion::new()`)
5. Daily trade limit checked in `Executor::execute()`
6. Daily USD limit checked in `Executor::execute()`
7. Cooldown enforced via `DailyLimits::check()`
8. Circuit breaker at 2% deviation in `execute_twap()`
9. Crash recovery via `ExecutionState` JSON serde
10. Disclaimer in `format_signal()` includes "NOT FINANCIAL ADVICE"

## Gateway Integration

- `QuantEngine` stored as `Mutex<Option<Arc<Mutex<QuantEngine>>>>` in Gateway (lazy init)
- Quant engine started on demand via `/quant enable` bot command
- IBKR price feed spawned when engine starts (processes 5-second real-time bars)
- Latest signal injected into system prompt via `try_lock()` (non-blocking)
- Advisory block: `[QUANT ADVISORY — NOT FINANCIAL ADVICE]`
- Configuration stored in SQLite `facts` table (quant_enabled, quant_symbol, quant_portfolio, quant_mode)

## Bot Command

`/quant` — Telegram bot command for configuration:

| Subcommand | Effect |
|------------|--------|
| `/quant` | Show current quant status |
| `/quant enable` | Start IBKR price feed + quant engine |
| `/quant disable` | Stop quant engine |
| `/quant symbol AAPL` | Change tracked symbol |
| `/quant portfolio 50000` | Set portfolio value |
| `/quant paper` | Use paper trading (port 4002) |
| `/quant live` | Use live trading (port 4001) |

## Prerequisites

IB Gateway or TWS must be running locally:
- Paper trading: port 4002
- Live trading: port 4001
- Auth handled by IB Gateway app (no API keys in code)

## Dependencies

tokio, serde, serde_json, tracing, anyhow, chrono, uuid, ibapi, rand
