---
name = "binance-quant"
description = "Quantitative trading advisory — Kalman filter, HMM regime detection, Kelly sizing, Binance market data."
trigger = "quant|trading|signal|binance|market|regime|kelly|position"
---

# Binance Quant Advisory

Real-time quantitative trading signals powered by the omega-quant engine.

## What it provides

- **Kalman-filtered prices** — noise reduction on raw market data
- **HMM regime detection** — Bull / Bear / Lateral classification with probabilities
- **Merton optimal allocation** — risk-adjusted position recommendation
- **Fractional Kelly sizing** — position size in USD with safety caps
- **Execution strategy** — TWAP or Immediate recommendation

## How it works

When enabled (`[quant]` in config.toml), the engine:
1. Connects to Binance WebSocket for live kline data
2. Processes each closed candle through the quant pipeline
3. Injects the latest signal into the system prompt as `[QUANT ADVISORY]`

The AI uses this context to answer trading questions with data-driven insights.

## Safety

- Testnet by default — no real money until explicitly configured
- Human confirmation required for every trade
- Daily trade and USD limits enforced
- Circuit breaker on 2% price deviation
- 5-minute cooldown between trades
- Signals are advisory only — marked "NOT FINANCIAL ADVICE"

## Configuration

```toml
[quant]
enabled = true
default_symbol = "BTCUSDT"
network = "testnet"          # "testnet" or "mainnet"
portfolio_value = 10000.0
risk_aversion = 2.0
kelly_fraction = 0.25
max_position_pct = 0.10

[quant.safety]
max_daily_trades = 10
max_daily_usd = 5000.0
require_confirmation = true
cooldown_minutes = 5
```

Set API keys via environment variables:
- Testnet: `BINANCE_TESTNET_API_KEY`, `BINANCE_TESTNET_SECRET_KEY`
- Mainnet: `BINANCE_API_KEY`, `BINANCE_SECRET_KEY`
