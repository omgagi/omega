//! IBKR TWS API operations: order placement, price fetching, positions, P&L.

use crate::execution::Side;
use crate::market_data::IbkrConfig;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

/// Fill result from an IBKR order.
pub(super) struct OrderFill {
    pub order_id: i32,
    pub filled_qty: f64,
    pub filled_usd: f64,
}

/// Place a market order via IBKR TWS API.
pub(super) async fn place_ibkr_order(
    config: &IbkrConfig,
    contract: &ibapi::contracts::Contract,
    side: Side,
    quantity: f64,
) -> Result<OrderFill> {
    use ibapi::orders::{order_builder, Action, PlaceOrder};
    use ibapi::Client;

    let symbol = contract.symbol.to_string();

    let client = Client::connect(&config.connection_url(), config.client_id + 100)
        .await
        .map_err(|e| anyhow::anyhow!("IBKR connection failed: {e}"))?;

    let action = match side {
        Side::Buy => Action::Buy,
        Side::Sell => Action::Sell,
    };
    let order = order_builder::market_order(action, quantity);

    let order_id = client
        .next_valid_order_id()
        .await
        .map_err(|e| anyhow::anyhow!("failed to get order ID: {e}"))?;

    let mut notifications = client
        .place_order(order_id, contract, &order)
        .await
        .map_err(|e| anyhow::anyhow!("IBKR order placement failed: {e}"))?;

    // Read execution results.
    let mut filled_qty = 0.0;
    let mut avg_price = 0.0;

    while let Some(result) = notifications.next().await {
        match result {
            Ok(PlaceOrder::ExecutionData(exec)) => {
                filled_qty = exec.execution.cumulative_quantity;
                avg_price = exec.execution.average_price;
            }
            Ok(PlaceOrder::CommissionReport(_)) => {
                break; // Commission report signals order completion.
            }
            Ok(_) => {} // Ignore other notifications (OrderStatus, OpenOrder, etc.)
            Err(e) => {
                warn!("quant: order notification error: {e}");
                break;
            }
        }
    }

    let filled_usd = filled_qty * avg_price;

    info!(
        "quant: IBKR order filled: {side:?} {filled_qty:.6} {symbol} @ ${avg_price:.2} = ${filled_usd:.2}"
    );

    Ok(OrderFill {
        order_id,
        filled_qty,
        filled_usd,
    })
}

/// Get current price for a contract via IBKR market data snapshot.
///
/// Uses the snapshot API (reqMktData) which works reliably across all asset classes,
/// unlike `realtime_bars` which doesn't support forex (CASH) contracts.
pub async fn get_ibkr_price(
    config: &IbkrConfig,
    contract: &ibapi::contracts::Contract,
) -> Result<f64> {
    use ibapi::contracts::tick_types::TickType;
    use ibapi::market_data::realtime::TickTypes;
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id + 200)
        .await
        .map_err(|e| anyhow::anyhow!("IBKR connection failed for price check: {e}"))?;

    let mut subscription = client
        .market_data(contract)
        .snapshot()
        .subscribe()
        .await
        .map_err(|e| anyhow::anyhow!("IBKR snapshot request failed: {e}"))?;

    let timeout_dur = std::time::Duration::from_secs(10);
    let mut best_price: Option<f64> = None;

    // Read ticks until SnapshotEnd or timeout. Prefer last price, fall back to bid/ask mid.
    let mut bid: Option<f64> = None;
    let mut ask: Option<f64> = None;

    while let Ok(Some(Ok(tick))) = tokio::time::timeout(timeout_dur, subscription.next()).await {
        match tick {
            TickTypes::Price(p) if p.price > 0.0 => match p.tick_type {
                TickType::Last
                | TickType::Close
                | TickType::DelayedLast
                | TickType::DelayedClose => {
                    best_price = Some(p.price);
                }
                TickType::Bid | TickType::DelayedBid => bid = Some(p.price),
                TickType::Ask | TickType::DelayedAsk => ask = Some(p.price),
                _ => {}
            },
            TickTypes::PriceSize(ps) if ps.price > 0.0 => match ps.price_tick_type {
                TickType::Last
                | TickType::Close
                | TickType::DelayedLast
                | TickType::DelayedClose => {
                    best_price = Some(ps.price);
                }
                TickType::Bid | TickType::DelayedBid => bid = Some(ps.price),
                TickType::Ask | TickType::DelayedAsk => ask = Some(ps.price),
                _ => {}
            },
            TickTypes::SnapshotEnd => break,
            _ => {}
        }
    }

    // Use last/close if available, otherwise midpoint of bid/ask.
    if let Some(price) = best_price {
        return Ok(price);
    }
    if let (Some(b), Some(a)) = (bid, ask) {
        return Ok((b + a) / 2.0);
    }

    anyhow::bail!("No price data received from IBKR snapshot")
}

/// Position information from IBKR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionInfo {
    /// Account ID.
    pub account: String,
    /// Instrument symbol.
    pub symbol: String,
    /// Security type (e.g. "STK", "CRYPTO", "CASH").
    pub security_type: String,
    /// Position quantity (positive = long, negative = short).
    pub quantity: f64,
    /// Average cost per unit.
    pub avg_cost: f64,
}

/// Daily P&L for an account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyPnl {
    /// Daily profit/loss.
    pub daily_pnl: f64,
    /// Unrealized P&L (open positions).
    pub unrealized_pnl: Option<f64>,
    /// Realized P&L (closed positions).
    pub realized_pnl: Option<f64>,
}

/// Get all open positions from IBKR.
pub async fn get_positions(config: &IbkrConfig) -> Result<Vec<PositionInfo>> {
    use ibapi::accounts::PositionUpdate;
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id + 400)
        .await
        .map_err(|e| anyhow::anyhow!("IBKR connection failed: {e}"))?;

    let mut subscription = client
        .positions()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to request positions: {e}"))?;

    let mut positions = Vec::new();

    while let Some(result) = subscription.next().await {
        match result {
            Ok(PositionUpdate::Position(pos)) => {
                if pos.position.abs() > f64::EPSILON {
                    positions.push(PositionInfo {
                        account: pos.account.clone(),
                        symbol: pos.contract.symbol.to_string(),
                        security_type: pos.contract.security_type.to_string(),
                        quantity: pos.position,
                        avg_cost: pos.average_cost,
                    });
                }
            }
            Ok(PositionUpdate::PositionEnd) => break,
            Err(e) => {
                warn!("quant: position error: {e}");
                break;
            }
        }
    }

    Ok(positions)
}

/// Get daily P&L for an account.
pub async fn get_daily_pnl(config: &IbkrConfig, account_id: &str) -> Result<DailyPnl> {
    use ibapi::accounts::types::AccountId;
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id + 500)
        .await
        .map_err(|e| anyhow::anyhow!("IBKR connection failed: {e}"))?;

    let account = AccountId(account_id.to_string());
    let mut subscription = client
        .pnl(&account, None)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to request P&L: {e}"))?;

    let timeout_dur = std::time::Duration::from_secs(10);
    match tokio::time::timeout(timeout_dur, subscription.next()).await {
        Ok(Some(Ok(pnl))) => Ok(DailyPnl {
            daily_pnl: pnl.daily_pnl,
            unrealized_pnl: pnl.unrealized_pnl,
            realized_pnl: pnl.realized_pnl,
        }),
        Ok(Some(Err(e))) => anyhow::bail!("P&L error: {e}"),
        Ok(None) => anyhow::bail!("No P&L data received"),
        Err(_) => anyhow::bail!("P&L request timed out"),
    }
}
