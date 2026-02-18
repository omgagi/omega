# Quantitative Trading Engine

## Overview

The omega-quant crate provides real-time quantitative trading analysis and advisory signals. It connects to Binance market data and runs a pipeline of mathematical models to produce actionable signals.

## Enabling

Add to your `config.toml`:

```toml
[quant]
enabled = true
default_symbol = "BTCUSDT"
network = "testnet"
portfolio_value = 10000.0
```

Set API keys via environment variables:
- **Testnet**: `BINANCE_TESTNET_API_KEY`, `BINANCE_TESTNET_SECRET_KEY`
- **Mainnet**: `BINANCE_API_KEY`, `BINANCE_SECRET_KEY`

## How It Works

1. **WebSocket Feed**: Connects to Binance kline stream for the configured symbol
2. **Kalman Filter**: Smooths raw prices and estimates trend
3. **HMM Regime Detection**: Classifies market as Bull, Bear, or Lateral
4. **Merton Allocation**: Computes risk-adjusted optimal position fraction
5. **Kelly Sizing**: Determines position size in USD with safety caps
6. **Signal Injection**: Latest signal appears in AI system prompt as `[QUANT ADVISORY]`

## Signal Format

When you ask about trading or markets, the AI sees something like:

```
[QUANT ADVISORY — NOT FINANCIAL ADVICE]
Symbol: BTCUSDT | Price: $50123.45 (filtered: $50120.00)
Regime: 📈 Bull (Bull: 72% | Bear: 8% | Lateral: 20%)
Hurst: 0.50 (Random Walk)
Merton allocation: +0.65 | Kelly: 8.2% ($820)
Direction: Long | Action: LONG (urgency: 72%) | Execution: Immediate
Confidence: 40%
[END QUANT ADVISORY]
```

## Safety Guardrails

| Guardrail | Default | Purpose |
|-----------|---------|---------|
| Network | testnet | No real money by default |
| Confirmation | always on | Human approves every trade |
| Kelly fraction | 0.25 | Only 25% of theoretical optimal |
| Max position | 10% | Never risk more than 10% of portfolio |
| Daily trades | 10 | Cap on number of trades per day |
| Daily USD | $5,000 | Cap on total USD traded per day |
| Cooldown | 5 min | Minimum wait between trades |
| Circuit breaker | 2% | Abort TWAP if price deviates >2% |

## Configuration Reference

### `[quant]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | false | Enable the quant engine |
| `default_symbol` | string | "BTCUSDT" | Trading pair to track |
| `network` | string | "testnet" | "testnet" or "mainnet" |
| `kline_interval` | string | "1m" | Candlestick interval |
| `risk_aversion` | float | 2.0 | Merton risk parameter (higher = conservative) |
| `kelly_fraction` | float | 0.25 | Fraction of full Kelly |
| `max_position_pct` | float | 0.10 | Max position as % of portfolio |
| `portfolio_value` | float | 10000.0 | Portfolio value in USD |

### `[quant.safety]`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `max_daily_trades` | int | 10 | Max trades per day |
| `max_daily_usd` | float | 5000.0 | Max USD per day |
| `require_confirmation` | bool | true | Human confirms trades (always true) |
| `cooldown_minutes` | int | 5 | Min wait between trades |
