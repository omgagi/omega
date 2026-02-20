//! omega-quant CLI — standalone quantitative trading tool.
//!
//! Subcommands:
//! - `check`     — verify IB Gateway connectivity
//! - `scan`      — scan market for instruments by volume/activity
//! - `analyze`   — stream trading signals as JSONL
//! - `order`     — place an order (market or bracket) via IBKR
//! - `positions` — list open positions
//! - `pnl`       — get daily P&L for an account
//! - `close`     — close a position

use clap::{Parser, Subcommand};
use omega_quant::execution::{ImmediatePlan, Side};
use omega_quant::executor::{
    cancel_all_orders, cancel_order_by_id, check_daily_pnl_cutoff, check_max_positions,
    close_position, get_daily_pnl, get_ibkr_price, get_open_orders, get_positions,
    place_bracket_order, CircuitBreaker, DailyLimits, Executor,
};
use omega_quant::market_data::{build_contract, AssetClass, IbkrConfig};

#[derive(Parser)]
#[command(
    name = "omega-quant",
    version,
    about = "Quantitative trading engine — Kalman filter, HMM regime detection, Kelly sizing, IBKR execution"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Check IB Gateway connectivity.
    Check {
        /// TWS/Gateway port (paper: 4002, live: 4001).
        #[arg(long, default_value_t = 4002)]
        port: u16,
        /// TWS/Gateway host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    /// Scan market for instruments by volume/activity.
    Scan {
        /// Scanner code (e.g. MOST_ACTIVE, HOT_BY_VOLUME, TOP_PERC_GAIN).
        #[arg(long, default_value = "MOST_ACTIVE")]
        scan_code: String,
        /// Instrument type (e.g. STK, CRYPTO, CASH.IDEALPRO).
        #[arg(long, default_value = "STK")]
        instrument: String,
        /// Location code (e.g. STK.US.MAJOR, CRYPTO.PAXOS).
        #[arg(long, default_value = "STK.US.MAJOR")]
        location: String,
        /// Number of results to return.
        #[arg(long, default_value_t = 10)]
        count: i32,
        /// Minimum price filter.
        #[arg(long)]
        min_price: Option<f64>,
        /// Minimum volume filter.
        #[arg(long)]
        min_volume: Option<i32>,
        /// TWS/Gateway port.
        #[arg(long, default_value_t = 4002)]
        port: u16,
        /// TWS/Gateway host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    /// Stream trading signals as JSONL (one JSON object per line).
    Analyze {
        /// Symbol (e.g. AAPL, EUR/USD, BTC).
        symbol: String,
        /// Asset class: stock, forex/fx, crypto.
        #[arg(long, default_value = "stock")]
        asset_class: String,
        /// Portfolio value in USD.
        #[arg(long, default_value_t = 10_000.0)]
        portfolio: f64,
        /// TWS/Gateway port.
        #[arg(long, default_value_t = 4002)]
        port: u16,
        /// TWS/Gateway host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Number of 5-second bars to process before stopping.
        #[arg(long, default_value_t = 30)]
        bars: u32,
    },
    /// Place an order via IBKR (market or bracket with SL/TP).
    Order {
        /// Symbol (e.g. AAPL, EUR/USD, BTC).
        symbol: String,
        /// Order side: buy or sell.
        side: String,
        /// Quantity.
        quantity: f64,
        /// Asset class: stock, forex/fx, crypto.
        #[arg(long, default_value = "stock")]
        asset_class: String,
        /// Stop loss percentage (e.g. 1.5 = 1.5% below/above entry).
        #[arg(long)]
        stop_loss: Option<f64>,
        /// Take profit percentage (e.g. 3.0 = 3% above/below entry).
        #[arg(long)]
        take_profit: Option<f64>,
        /// IBKR account ID (for P&L cutoff check).
        #[arg(long)]
        account: Option<String>,
        /// Portfolio value in USD (for P&L cutoff check).
        #[arg(long)]
        portfolio: Option<f64>,
        /// Maximum simultaneous positions allowed.
        #[arg(long, default_value_t = 3)]
        max_positions: usize,
        /// TWS/Gateway port.
        #[arg(long, default_value_t = 4002)]
        port: u16,
        /// TWS/Gateway host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    /// List open positions from IBKR.
    Positions {
        /// TWS/Gateway port.
        #[arg(long, default_value_t = 4002)]
        port: u16,
        /// TWS/Gateway host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    /// Get daily P&L for an IBKR account.
    Pnl {
        /// IBKR account ID (e.g. DU1234567).
        account: String,
        /// TWS/Gateway port.
        #[arg(long, default_value_t = 4002)]
        port: u16,
        /// TWS/Gateway host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    /// Close an open position.
    Close {
        /// Symbol (e.g. AAPL, EUR/USD, BTC).
        symbol: String,
        /// Asset class: stock, forex/fx, crypto.
        #[arg(long, default_value = "stock")]
        asset_class: String,
        /// Quantity to close (omit to close entire position).
        #[arg(long)]
        quantity: Option<f64>,
        /// TWS/Gateway port.
        #[arg(long, default_value_t = 4002)]
        port: u16,
        /// TWS/Gateway host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    /// List all open/pending orders.
    Orders {
        /// TWS/Gateway port.
        #[arg(long, default_value_t = 4002)]
        port: u16,
        /// TWS/Gateway host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
    /// Cancel an order by ID, or cancel all open orders.
    Cancel {
        /// Order ID to cancel (omit to cancel ALL open orders).
        #[arg(long)]
        order_id: Option<i32>,
        /// TWS/Gateway port.
        #[arg(long, default_value_t = 4002)]
        port: u16,
        /// TWS/Gateway host.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
    },
}

/// Print a JSON connectivity error and exit.
fn connectivity_error(host: &str, port: u16) -> ! {
    let err = serde_json::json!({
        "error": format!("IB Gateway not reachable at {host}:{port}"),
    });
    println!("{}", serde_json::to_string(&err).unwrap());
    std::process::exit(1);
}

/// Parse a side string into a `Side` enum.
fn parse_side(s: &str) -> anyhow::Result<Side> {
    match s.to_lowercase().as_str() {
        "buy" => Ok(Side::Buy),
        "sell" => Ok(Side::Sell),
        _ => anyhow::bail!("Invalid side '{s}'. Use 'buy' or 'sell'."),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Check { port, host } => {
            let config = IbkrConfig {
                host: host.clone(),
                port,
                client_id: 1,
            };
            let connected = omega_quant::market_data::check_connection(&config).await;
            let result = serde_json::json!({
                "connected": connected,
                "host": host,
                "port": port,
            });
            println!("{}", serde_json::to_string(&result)?);
        }

        Commands::Scan {
            scan_code,
            instrument,
            location,
            count,
            min_price,
            min_volume,
            port,
            host,
        } => {
            let config = IbkrConfig {
                host: host.clone(),
                port,
                client_id: 1,
            };
            if !omega_quant::market_data::check_connection(&config).await {
                connectivity_error(&host, port);
            }

            let results = omega_quant::market_data::run_scanner(
                &config,
                &scan_code,
                &instrument,
                &location,
                count,
                min_price,
                min_volume,
            )
            .await?;
            println!("{}", serde_json::to_string(&results)?);
        }

        Commands::Analyze {
            symbol,
            asset_class,
            portfolio,
            port,
            host,
            bars,
        } => {
            let config = IbkrConfig {
                host: host.clone(),
                port,
                client_id: 1,
            };
            if !omega_quant::market_data::check_connection(&config).await {
                connectivity_error(&host, port);
            }

            let parsed_class: AssetClass = asset_class.parse()?;
            let mut engine = omega_quant::QuantEngine::new(&symbol, portfolio);
            let mut rx = omega_quant::market_data::start_price_feed(&symbol, &config, parsed_class);
            let mut count: u32 = 0;

            // Timeout for first bar — if no data within 15s, the market is likely closed
            // or the data subscription is missing.
            let first_bar_timeout = std::time::Duration::from_secs(15);
            let first = tokio::time::timeout(first_bar_timeout, rx.recv()).await;
            match first {
                Ok(Ok(tick)) => {
                    let signal = engine.process_price(tick.price);
                    println!("{}", serde_json::to_string(&signal)?);
                    count += 1;
                }
                _ => {
                    let err = serde_json::json!({
                        "error": format!("No data received for {symbol} ({parsed_class}) within 15s — market may be closed or data subscription missing"),
                    });
                    println!("{}", serde_json::to_string(&err)?);
                    std::process::exit(1);
                }
            }

            while count < bars {
                match tokio::time::timeout(std::time::Duration::from_secs(10), rx.recv()).await {
                    Ok(Ok(tick)) => {
                        let signal = engine.process_price(tick.price);
                        println!("{}", serde_json::to_string(&signal)?);
                        count += 1;
                    }
                    _ => break,
                }
            }
        }

        Commands::Order {
            symbol,
            side,
            quantity,
            asset_class,
            stop_loss,
            take_profit,
            account,
            portfolio,
            max_positions,
            port,
            host,
        } => {
            let config = IbkrConfig {
                host: host.clone(),
                port,
                client_id: 1,
            };
            if !omega_quant::market_data::check_connection(&config).await {
                connectivity_error(&host, port);
            }

            let order_side = parse_side(&side)?;
            let parsed_class: AssetClass = asset_class.parse()?;
            let contract = build_contract(&symbol, parsed_class)?;

            // Safety: check position count.
            if let Ok(positions) = get_positions(&config).await {
                check_max_positions(positions.len(), max_positions)?;
            }

            // Safety: check daily P&L cutoff.
            if let (Some(acct), Some(port_val)) = (&account, portfolio) {
                if let Ok(pnl) = get_daily_pnl(&config, acct).await {
                    check_daily_pnl_cutoff(pnl.daily_pnl, port_val, 5.0)?;
                }
            }

            if let (Some(sl_pct), Some(tp_pct)) = (stop_loss, take_profit) {
                // Bracket order: fetch entry price, calculate SL/TP levels.
                let entry_price = get_ibkr_price(&config, &contract).await?;
                let (sl_price, tp_price) = match order_side {
                    Side::Buy => (
                        entry_price * (1.0 - sl_pct / 100.0),
                        entry_price * (1.0 + tp_pct / 100.0),
                    ),
                    Side::Sell => (
                        entry_price * (1.0 + sl_pct / 100.0),
                        entry_price * (1.0 - tp_pct / 100.0),
                    ),
                };

                let state = place_bracket_order(
                    &config, &contract, order_side, quantity, tp_price, sl_price,
                )
                .await?;

                let result = serde_json::json!({
                    "type": "bracket",
                    "status": format!("{:?}", state.status),
                    "entry_price": entry_price,
                    "stop_loss_price": sl_price,
                    "take_profit_price": tp_price,
                    "filled_qty": state.total_filled_qty,
                    "filled_usd": state.total_filled_usd,
                    "order_ids": state.order_ids,
                    "errors": state.errors,
                });
                println!("{}", serde_json::to_string(&result)?);
            } else {
                // Simple market order.
                let plan = omega_quant::execution::ExecutionPlan::Immediate(ImmediatePlan {
                    symbol: symbol.clone(),
                    side: order_side,
                    quantity,
                    estimated_price: 0.0,
                    estimated_usd: 0.0,
                });

                let circuit_breaker = CircuitBreaker::default();
                let daily_limits = DailyLimits::new(10, 50_000.0, 5);
                let mut executor = Executor::new(config, circuit_breaker, daily_limits);
                let state = executor.execute(&plan).await;

                let result = serde_json::json!({
                    "type": "market",
                    "status": format!("{:?}", state.status),
                    "filled_qty": state.total_filled_qty,
                    "avg_price": if state.total_filled_qty > 0.0 {
                        state.total_filled_usd / state.total_filled_qty
                    } else {
                        0.0
                    },
                    "filled_usd": state.total_filled_usd,
                    "errors": state.errors,
                    "abort_reason": state.abort_reason,
                });
                println!("{}", serde_json::to_string(&result)?);
            }
        }

        Commands::Positions { port, host } => {
            let config = IbkrConfig {
                host: host.clone(),
                port,
                client_id: 1,
            };
            if !omega_quant::market_data::check_connection(&config).await {
                connectivity_error(&host, port);
            }

            let positions = get_positions(&config).await?;
            println!("{}", serde_json::to_string(&positions)?);
        }

        Commands::Pnl {
            account,
            port,
            host,
        } => {
            let config = IbkrConfig {
                host: host.clone(),
                port,
                client_id: 1,
            };
            if !omega_quant::market_data::check_connection(&config).await {
                connectivity_error(&host, port);
            }

            let pnl = get_daily_pnl(&config, &account).await?;
            println!("{}", serde_json::to_string(&pnl)?);
        }

        Commands::Close {
            symbol,
            asset_class,
            quantity,
            port,
            host,
        } => {
            let config = IbkrConfig {
                host: host.clone(),
                port,
                client_id: 1,
            };
            if !omega_quant::market_data::check_connection(&config).await {
                connectivity_error(&host, port);
            }

            let parsed_class: AssetClass = asset_class.parse()?;
            let contract = build_contract(&symbol, parsed_class)?;

            // Determine side and quantity from current position.
            let positions = get_positions(&config).await?;
            let match_symbol = match parsed_class {
                AssetClass::Forex => symbol.split('/').next().unwrap_or(&symbol).to_string(),
                _ => symbol.clone(),
            };
            let pos = positions
                .iter()
                .find(|p| p.symbol == match_symbol)
                .ok_or_else(|| anyhow::anyhow!("No open position found for {symbol}"))?;

            let close_qty = quantity.unwrap_or(pos.quantity.abs());
            let close_side = if pos.quantity > 0.0 {
                Side::Sell
            } else {
                Side::Buy
            };

            let state = close_position(&config, &contract, close_qty, close_side).await?;

            let result = serde_json::json!({
                "status": format!("{:?}", state.status),
                "side": format!("{close_side}"),
                "closed_qty": state.total_filled_qty,
                "filled_usd": state.total_filled_usd,
                "errors": state.errors,
            });
            println!("{}", serde_json::to_string(&result)?);
        }

        Commands::Orders { port, host } => {
            let config = IbkrConfig {
                host: host.clone(),
                port,
                client_id: 1,
            };
            if !omega_quant::market_data::check_connection(&config).await {
                connectivity_error(&host, port);
            }

            let orders = get_open_orders(&config).await?;
            println!("{}", serde_json::to_string(&orders)?);
        }

        Commands::Cancel {
            order_id,
            port,
            host,
        } => {
            let config = IbkrConfig {
                host: host.clone(),
                port,
                client_id: 1,
            };
            if !omega_quant::market_data::check_connection(&config).await {
                connectivity_error(&host, port);
            }

            if let Some(id) = order_id {
                let status = cancel_order_by_id(&config, id).await?;
                let result = serde_json::json!({
                    "cancelled": id,
                    "status": status,
                });
                println!("{}", serde_json::to_string(&result)?);
            } else {
                cancel_all_orders(&config).await?;
                let result = serde_json::json!({
                    "cancelled": "all",
                    "status": "global_cancel_sent",
                });
                println!("{}", serde_json::to_string(&result)?);
            }
        }
    }

    Ok(())
}
