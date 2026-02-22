//! Order management: open orders, cancel, bracket orders, close position.

use super::{ExecutionState, ExecutionStatus};
use crate::execution::Side;
use crate::market_data::IbkrConfig;
use anyhow::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tracing::info;

/// Info about an open order from IBKR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenOrderInfo {
    /// IBKR order ID.
    pub order_id: i32,
    /// Instrument symbol.
    pub symbol: String,
    /// Side: "BUY" or "SELL".
    pub action: String,
    /// Total order quantity.
    pub quantity: f64,
    /// Order type (e.g. "MKT", "LMT", "STP").
    pub order_type: String,
    /// Limit price (if applicable).
    pub limit_price: Option<f64>,
    /// Stop price (if applicable).
    pub stop_price: Option<f64>,
    /// Order status (e.g. "Submitted", "PreSubmitted", "Filled").
    pub status: String,
    /// Quantity already filled.
    pub filled: f64,
    /// Quantity remaining.
    pub remaining: f64,
    /// Parent order ID (0 if no parent).
    pub parent_id: i32,
}

/// Get all open orders from IBKR.
pub async fn get_open_orders(config: &IbkrConfig) -> Result<Vec<OpenOrderInfo>> {
    use ibapi::orders::Orders;
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id + 600)
        .await
        .map_err(|e| anyhow::anyhow!("IBKR connection failed: {e}"))?;

    let mut subscription = client
        .all_open_orders()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to request open orders: {e}"))?;

    let mut orders = Vec::new();
    let timeout_dur = std::time::Duration::from_secs(10);

    while let Ok(Some(Ok(item))) = tokio::time::timeout(timeout_dur, subscription.next()).await {
        match item {
            Orders::OrderData(data) => {
                orders.push(OpenOrderInfo {
                    order_id: data.order_id,
                    symbol: data.contract.symbol.to_string(),
                    action: format!("{:?}", data.order.action),
                    quantity: data.order.total_quantity,
                    order_type: data.order.order_type.clone(),
                    limit_price: data.order.limit_price,
                    stop_price: data.order.aux_price,
                    status: data.order_state.status.clone(),
                    filled: data.order.filled_quantity,
                    remaining: data.order.total_quantity - data.order.filled_quantity,
                    parent_id: data.order.parent_id,
                });
            }
            Orders::OrderStatus(_) | Orders::Notice(_) => {}
        }
    }

    Ok(orders)
}

/// Cancel a specific order by ID.
pub async fn cancel_order_by_id(config: &IbkrConfig, order_id: i32) -> Result<String> {
    use ibapi::orders::CancelOrder;
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id + 700)
        .await
        .map_err(|e| anyhow::anyhow!("IBKR connection failed: {e}"))?;

    let mut subscription = client
        .cancel_order(order_id, "")
        .await
        .map_err(|e| anyhow::anyhow!("Failed to cancel order {order_id}: {e}"))?;

    let timeout_dur = std::time::Duration::from_secs(10);
    if let Ok(Some(Ok(item))) = tokio::time::timeout(timeout_dur, subscription.next()).await {
        match item {
            CancelOrder::OrderStatus(status) => return Ok(status.status),
            CancelOrder::Notice(notice) => return Ok(format!("{notice:?}")),
        }
    }

    Ok("cancel_sent".to_string())
}

/// Cancel all open orders globally.
pub async fn cancel_all_orders(config: &IbkrConfig) -> Result<()> {
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id + 800)
        .await
        .map_err(|e| anyhow::anyhow!("IBKR connection failed: {e}"))?;

    client
        .global_cancel()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to cancel all orders: {e}"))?;

    Ok(())
}

/// Place a bracket order (market entry + take profit + stop loss).
///
/// Creates 3 linked orders: parent MKT -> TP LMT -> SL STP.
/// The stop loss order has `transmit=true` which triggers all three.
pub async fn place_bracket_order(
    config: &IbkrConfig,
    contract: &ibapi::contracts::Contract,
    side: Side,
    quantity: f64,
    take_profit_price: f64,
    stop_loss_price: f64,
) -> Result<ExecutionState> {
    use ibapi::orders::{order_builder, Action, PlaceOrder};
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id + 100)
        .await
        .map_err(|e| anyhow::anyhow!("IBKR connection failed: {e}"))?;

    let parent_id = client
        .next_valid_order_id()
        .await
        .map_err(|e| anyhow::anyhow!("failed to get order ID: {e}"))?;

    let action = match side {
        Side::Buy => Action::Buy,
        Side::Sell => Action::Sell,
    };
    let reverse_action = match side {
        Side::Buy => Action::Sell,
        Side::Sell => Action::Buy,
    };

    // Parent: market order (transmit=false).
    let mut parent = order_builder::market_order(action, quantity);
    parent.order_id = parent_id;
    parent.transmit = false;

    // Take profit: limit order in opposite direction (transmit=false).
    let mut take_profit = order_builder::limit_order(reverse_action, quantity, take_profit_price);
    take_profit.order_id = parent_id + 1;
    take_profit.parent_id = parent_id;
    take_profit.transmit = false;

    // Stop loss: stop order in opposite direction (transmit=true triggers all).
    let mut stop_loss = order_builder::stop(reverse_action, quantity, stop_loss_price);
    stop_loss.order_id = parent_id + 2;
    stop_loss.parent_id = parent_id;
    stop_loss.transmit = true;

    let mut state = ExecutionState {
        plan_json: String::new(),
        slices_completed: 0,
        total_slices: 3,
        total_filled_qty: 0.0,
        total_filled_usd: 0.0,
        status: ExecutionStatus::Running,
        order_ids: vec![
            parent_id as i64,
            (parent_id + 1) as i64,
            (parent_id + 2) as i64,
        ],
        errors: Vec::new(),
        abort_reason: None,
        started_at: Utc::now(),
        updated_at: Utc::now(),
    };

    // Place parent order (holds notifications for fills).
    let mut parent_notifications = client
        .place_order(parent_id, contract, &parent)
        .await
        .map_err(|e| anyhow::anyhow!("Bracket parent order failed: {e}"))?;

    // Place take profit.
    let _tp = client
        .place_order(parent_id + 1, contract, &take_profit)
        .await
        .map_err(|e| anyhow::anyhow!("Bracket take-profit order failed: {e}"))?;

    // Place stop loss (transmit=true triggers all orders).
    let _sl = client
        .place_order(parent_id + 2, contract, &stop_loss)
        .await
        .map_err(|e| anyhow::anyhow!("Bracket stop-loss order failed: {e}"))?;

    // Read parent order fill.
    while let Some(result) = parent_notifications.next().await {
        match result {
            Ok(PlaceOrder::ExecutionData(exec)) => {
                state.total_filled_qty = exec.execution.cumulative_quantity;
                let avg_price = exec.execution.average_price;
                state.total_filled_usd = state.total_filled_qty * avg_price;
            }
            Ok(PlaceOrder::CommissionReport(_)) => break,
            Ok(_) => {}
            Err(e) => {
                state.errors.push(e.to_string());
                break;
            }
        }
    }

    state.slices_completed = 3;
    state.status = ExecutionStatus::Completed;
    state.updated_at = Utc::now();

    info!(
        "quant: bracket order placed: parent={}, TP={}, SL={}, filled={:.6}",
        parent_id,
        parent_id + 1,
        parent_id + 2,
        state.total_filled_qty
    );

    Ok(state)
}

/// Close a position with a market order.
pub async fn close_position(
    config: &IbkrConfig,
    contract: &ibapi::contracts::Contract,
    quantity: f64,
    side: Side,
) -> Result<ExecutionState> {
    use ibapi::orders::{order_builder, Action, PlaceOrder};
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id + 600)
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
        .map_err(|e| anyhow::anyhow!("Close order failed: {e}"))?;

    let mut state = ExecutionState {
        plan_json: String::new(),
        slices_completed: 0,
        total_slices: 1,
        total_filled_qty: 0.0,
        total_filled_usd: 0.0,
        status: ExecutionStatus::Running,
        order_ids: vec![order_id as i64],
        errors: Vec::new(),
        abort_reason: None,
        started_at: Utc::now(),
        updated_at: Utc::now(),
    };

    while let Some(result) = notifications.next().await {
        match result {
            Ok(PlaceOrder::ExecutionData(exec)) => {
                state.total_filled_qty = exec.execution.cumulative_quantity;
                let avg_price = exec.execution.average_price;
                state.total_filled_usd = state.total_filled_qty * avg_price;
            }
            Ok(PlaceOrder::CommissionReport(_)) => break,
            Ok(_) => {}
            Err(e) => {
                state.errors.push(e.to_string());
                break;
            }
        }
    }

    state.slices_completed = 1;
    state.status = ExecutionStatus::Completed;
    state.updated_at = Utc::now();

    info!(
        "quant: position closed: {side:?} {:.6} (${:.2})",
        state.total_filled_qty, state.total_filled_usd
    );

    Ok(state)
}
