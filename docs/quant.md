# Quantitative Trading Engine

## Overview

The omega-quant crate provides real-time quantitative trading analysis, multi-asset order execution, and portfolio monitoring via Interactive Brokers (IBKR). It is exposed as a standalone CLI binary (`omega-quant`) that the AI invokes through the `ibkr-quant` skill — no gateway wiring, no config.toml section needed.

## How It Works

The AI learns about omega-quant from the `ibkr-quant` skill (`skills/ibkr-quant/SKILL.md`). When a user asks about trading, stocks, forex, crypto, or market analysis, the AI invokes the CLI tool via bash:

1. **Check connectivity**: `omega-quant check --port 4002`
2. **Scan market**: `omega-quant scan --scan-code MOST_ACTIVE --count 10`
3. **Analyze signals**: `omega-quant analyze AAPL --asset-class stock --portfolio 50000 --bars 10`
4. **Place orders**: `omega-quant order AAPL buy 100 --asset-class stock --stop-loss 1.5 --take-profit 3.0`
5. **Monitor positions**: `omega-quant positions`
6. **Check P&L**: `omega-quant pnl DU1234567`
7. **Close positions**: `omega-quant close AAPL --asset-class stock`
8. **List open orders**: `omega-quant orders`
9. **Cancel orders**: `omega-quant cancel --order-id 42` or `omega-quant cancel` (all)

## CLI Commands

### Check IB Gateway connectivity

```bash
omega-quant check --port 4002
# → {"connected": true, "host": "127.0.0.1", "port": 4002}
```

- Port 4002 = paper trading (default, safe)
- Port 4001 = live trading (real money)

### Scan — find instruments by volume/activity

```bash
# Most active US stocks
omega-quant scan --scan-code MOST_ACTIVE --instrument STK --location STK.US.MAJOR --count 10

# Hot crypto
omega-quant scan --scan-code HOT_BY_VOLUME --instrument CRYPTO --location CRYPTO.PAXOS --count 5

# Stocks above $10 with high volume
omega-quant scan --scan-code HOT_BY_VOLUME --instrument STK --location STK.US.MAJOR --min-price 10 --min-volume 1000000
```

Scan codes: `MOST_ACTIVE`, `HOT_BY_VOLUME`, `TOP_PERC_GAIN`, `TOP_PERC_LOSE`, `HIGH_OPEN_GAP`, `LOW_OPEN_GAP`

### Analyze — stream trading signals

```bash
# Stock
omega-quant analyze AAPL --asset-class stock --portfolio 50000 --bars 10

# Forex
omega-quant analyze EUR/USD --asset-class forex --portfolio 50000 --bars 10

# Crypto
omega-quant analyze BTC --asset-class crypto --portfolio 50000 --bars 10
```

Each signal contains:
- `regime`: Bull / Bear / Lateral (HMM-detected market state)
- `filtered_price`: Kalman-filtered price (noise removed)
- `merton_allocation`: optimal portfolio allocation [-0.5, 1.5]
- `kelly_fraction`: fractional Kelly bet size
- `kelly_position_usd`: recommended position in dollars
- `direction`: Long / Short / Hold
- `action`: Long/Short/Hold/ReducePosition/Exit with urgency
- `confidence`: signal confidence score [0, 1]
- `reasoning`: human-readable summary


### Order — place a trade (market or bracket)

```bash
# Simple market order
omega-quant order AAPL buy 100 --asset-class stock --port 4002

# Bracket order with stop-loss and take-profit
omega-quant order AAPL buy 100 --asset-class stock --stop-loss 1.5 --take-profit 3.0 --port 4002

# Bracket with safety checks
omega-quant order AAPL buy 100 --asset-class stock --stop-loss 1.5 --take-profit 3.0 --account DU1234567 --portfolio 50000 --max-positions 3

# Forex bracket
omega-quant order EUR/USD buy 20000 --asset-class forex --stop-loss 0.5 --take-profit 1.0

# Crypto bracket
omega-quant order BTC buy 0.1 --asset-class crypto --stop-loss 2.0 --take-profit 5.0
```

Bracket orders create 3 linked orders: MKT entry → LMT take-profit → STP stop-loss.

### Positions — list open positions

```bash
omega-quant positions --port 4002
# → [{"account":"DU1234567","symbol":"AAPL","security_type":"STK","quantity":100.0,"avg_cost":185.50}]
```

### P&L — daily profit/loss

```bash
omega-quant pnl DU1234567 --port 4002
# → {"daily_pnl":-250.50,"unrealized_pnl":-100.0,"realized_pnl":-150.50}
```

### Close — close a position

```bash
# Close entire position (auto-detects side and quantity)
omega-quant close AAPL --asset-class stock --port 4002

# Partial close
omega-quant close AAPL --asset-class stock --quantity 50 --port 4002
```

### Orders — list open/pending orders

```bash
omega-quant orders --port 4002
# → [{"order_id":42,"symbol":"AAPL","action":"BUY","quantity":100.0,"order_type":"MKT","status":"Submitted","filled":0.0,"remaining":100.0,"parent_id":0}]
```

Always check open orders before placing new ones to avoid duplicates.

### Cancel — cancel orders

```bash
# Cancel a specific order
omega-quant cancel --order-id 42 --port 4002

# Cancel ALL open orders
omega-quant cancel --port 4002
```

## Signal Interpretation

- **Bull regime + Long direction + kelly_should_trade=true** → strong buy signal
- **Bear regime + Short direction** → consider reducing exposure
- **Lateral regime + Long/Short + kelly_should_trade=true** → mean-reversion trade (range-bound opportunity, lower urgency, smaller position)
- **Lateral regime + Hold** → no clear direction, wait for regime change
- **confidence > 0.5** → higher conviction signal
- **merton_allocation > 0.1** → math says go long; < -0.1 → go short

## Multi-Asset Support

| Asset Class | Flag | Symbol Format | Example |
|-------------|------|---------------|---------|
| Stock | `--asset-class stock` | `AAPL`, `MSFT` | `omega-quant analyze AAPL --asset-class stock` |
| Forex | `--asset-class forex` | `EUR/USD`, `GBP/JPY` | `omega-quant order EUR/USD buy 20000 --asset-class forex` (uses tick-by-tick midpoint) |
| Crypto | `--asset-class crypto` | `BTC`, `ETH` | `omega-quant analyze BTC --asset-class crypto` (uses realtime bars) |

## Prerequisites

IB Gateway or TWS must be running locally:
- Paper trading: port 4002 (default)
- Live trading: port 4001
- Docker: `docker run -d -p 4002:4002 ghcr.io/gnzsnz/ib-gateway:latest`
- Auth is handled by the IB Gateway app — no API keys in code

## Safety Guardrails

| Guardrail | Default | Purpose |
|-----------|---------|---------|
| Mode | paper | Paper trading by default (port 4002) |
| Daily trades | 10 | Cap on number of trades per day |
| Daily USD | $50,000 | Cap on total USD traded per day |
| Cooldown | 5 min | Minimum wait between trades |
| Circuit breaker | 2% | Abort TWAP if price deviates >2% |
| Max positions | 3 | Block new trades if too many open positions |
| P&L cutoff | -5% | Halt trading if daily loss exceeds 5% of portfolio |
| Bracket orders | always | Every entry gets SL + TP (enforced by skill) |
| Signals | advisory | Marked "NOT FINANCIAL ADVICE" |

## Removing Quant

To completely remove quant from Omega, simply delete `skills/ibkr-quant/`. Zero gateway changes needed.
