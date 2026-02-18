# omega-quant — Technical Specification

## Overview

Quantitative trading engine crate providing real-time market analysis and advisory signals. Connects to Binance (testnet + mainnet) via WebSocket and REST APIs.

## Modules

| Module | Purpose |
|--------|---------|
| `signal.rs` | Output types: `QuantSignal`, `Regime`, `Direction`, `Action`, `ExecutionStrategy` |
| `kalman.rs` | Kalman filter (2D state: price + trend, plain f64 math, no nalgebra) |
| `hmm.rs` | Hidden Markov Model (3-state: Bull/Bear/Lateral, 5 observations, Baum-Welch training) |
| `kelly.rs` | Fractional Kelly criterion (position sizing with safety clamps) |
| `market_data.rs` | Binance WebSocket kline feed + REST historical data |
| `binance_auth.rs` | HMAC-SHA256 signing, order placement, ticker queries |
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

1. Testnet by default (`default_quant_network()` returns "testnet")
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

- `QuantEngine` stored as `Option<Arc<Mutex<QuantEngine>>>` in Gateway
- WebSocket kline feed spawned in `Gateway::run()` (processes closed candles)
- Latest signal injected into system prompt via `try_lock()` (non-blocking)
- Advisory block: `[QUANT ADVISORY — NOT FINANCIAL ADVICE]`

## Dependencies

tokio, serde, serde_json, tracing, anyhow, chrono, reqwest, uuid, tokio-tungstenite, futures-util, hmac, sha2, hex, rand
