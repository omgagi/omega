---
name = "ibkr-quant"
description = "Quantitative trading advisory — Kalman filter, HMM regime detection, Kelly sizing, IBKR market data."
trigger = "quant|trading|signal|ibkr|market|regime|kelly|position|interactive brokers"
---

# IBKR Quant Advisory

Real-time quantitative trading signals powered by the omega-quant engine with Interactive Brokers market data.

## What it provides

- **Kalman-filtered prices** — noise reduction on raw market data
- **HMM regime detection** — Bull / Bear / Lateral classification with probabilities
- **Merton optimal allocation** — risk-adjusted position recommendation
- **Fractional Kelly sizing** — position size in USD with safety caps
- **Execution strategy** — TWAP or Immediate recommendation

## How it works

When enabled via `/quant enable`, the engine:
1. Connects to IB Gateway via TWS API for live price data
2. Processes each 5-second bar through the quant pipeline
3. Injects the latest signal into the system prompt as `[QUANT ADVISORY]`

The AI uses this context to answer trading questions with data-driven insights.

## Prerequisites

- **IB Gateway** or **TWS** must be running locally
  - Paper trading: port 4002 (default)
  - Live trading: port 4001
- Docker option: `docker run -d -p 4002:4002 ghcr.io/gnzsnz/ib-gateway:latest`

## Bot Commands

```
/quant              — Show current quant status
/quant enable       — Start IBKR quant (paper mode by default)
/quant disable      — Stop quant engine
/quant symbol AAPL  — Change tracked symbol
/quant portfolio 50000 — Set portfolio value
/quant paper        — Use paper trading (port 4002)
/quant live         — Use live trading (port 4001)
```

Settings are stored in the database, no restart needed.

## Safety

- Paper trading by default — no real money until explicitly switched to live
- Human confirmation required for every trade
- Daily trade and USD limits enforced
- Circuit breaker on 2% price deviation
- 5-minute cooldown between trades
- Signals are advisory only — marked "NOT FINANCIAL ADVICE"
