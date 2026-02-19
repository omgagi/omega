# Quantitative Trading Engine

## Overview

The omega-quant crate provides real-time quantitative trading analysis and advisory signals. It connects to Interactive Brokers (IBKR) via the TWS API and runs a pipeline of mathematical models to produce actionable signals.

## Enabling

Quant is configured via the `/quant` Telegram bot command — no config.toml section needed.

```
/quant enable         — Start the quant engine (connects to IB Gateway)
/quant disable        — Stop the quant engine
/quant symbol AAPL    — Change tracked symbol (default: AAPL)
/quant portfolio 50000 — Set portfolio value (default: 10000)
/quant paper          — Use paper trading, port 4002 (default)
/quant live           — Use live trading, port 4001
```

**Prerequisite**: IB Gateway or TWS must be running locally. Paper trading uses port 4002, live uses port 4001. Auth is handled by the IB Gateway app — no API keys in code.

## How It Works

1. **IBKR Price Feed**: Connects to IB Gateway via TWS API, streams 5-second real-time bars
2. **Kalman Filter**: Smooths raw prices and estimates trend
3. **HMM Regime Detection**: Classifies market as Bull, Bear, or Lateral
4. **Merton Allocation**: Computes risk-adjusted optimal position fraction
5. **Kelly Sizing**: Determines position size in USD with safety caps
6. **Signal Injection**: Latest signal appears in AI system prompt as `[QUANT ADVISORY]`

## Signal Format

When you ask about trading or markets, the AI sees something like:

```
[QUANT ADVISORY — NOT FINANCIAL ADVICE]
Symbol: AAPL | Price: $185.50 (filtered: $185.45)
Regime: Bull (Bull: 72% | Bear: 8% | Lateral: 20%)
Hurst: 0.50 (Random Walk)
Merton allocation: +0.65 | Kelly: 8.2% ($820)
Direction: Long | Action: LONG (urgency: 72%) | Execution: Immediate
Confidence: 40%
[END QUANT ADVISORY]
```

## Safety Guardrails

| Guardrail | Default | Purpose |
|-----------|---------|---------|
| Mode | paper | Paper trading by default (port 4002) |
| Confirmation | always on | Human approves every trade |
| Kelly fraction | 0.25 | Only 25% of theoretical optimal |
| Max position | 10% | Never risk more than 10% of portfolio |
| Daily trades | 10 | Cap on number of trades per day |
| Daily USD | $5,000 | Cap on total USD traded per day |
| Cooldown | 5 min | Minimum wait between trades |
| Circuit breaker | 2% | Abort TWAP if price deviates >2% |
