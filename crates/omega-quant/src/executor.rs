//! Live order executor with circuit breaker, daily limits, bracket orders, and crash recovery.
//!
//! Uses IBKR TWS API via `ibapi` for order placement. Authentication is handled
//! by IB Gateway — no API keys needed in code.

use crate::execution::{ExecutionPlan, ImmediatePlan, Side, TwapPlan};
use crate::market_data::IbkrConfig;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{error, info, warn};

/// Circuit breaker configuration.
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    /// Max price deviation from entry before aborting (default: 2%).
    pub max_deviation_pct: f64,
    /// Max consecutive slice failures before aborting (default: 3).
    pub max_consecutive_failures: u32,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self {
            max_deviation_pct: 0.02,
            max_consecutive_failures: 3,
        }
    }
}

/// Safety limits for daily trading.
#[derive(Debug, Clone)]
pub struct DailyLimits {
    /// Maximum number of trades per day.
    pub max_trades: u32,
    /// Maximum total USD per day.
    pub max_usd: f64,
    /// Minimum cooldown between trades in minutes.
    pub cooldown_minutes: u32,
    /// Trades executed today.
    pub trades_today: u32,
    /// USD traded today.
    pub usd_today: f64,
    /// Last trade timestamp.
    pub last_trade_time: Option<DateTime<Utc>>,
}

impl DailyLimits {
    /// Create new limits from config values.
    pub fn new(max_trades: u32, max_usd: f64, cooldown_minutes: u32) -> Self {
        Self {
            max_trades,
            max_usd,
            cooldown_minutes,
            trades_today: 0,
            usd_today: 0.0,
            last_trade_time: None,
        }
    }

    /// Check if a trade is allowed. Returns `Err` with reason if blocked.
    pub fn check(&self, trade_usd: f64) -> Result<()> {
        if self.trades_today >= self.max_trades {
            anyhow::bail!(
                "Daily trade limit reached ({}/{})",
                self.trades_today,
                self.max_trades
            );
        }

        if self.usd_today + trade_usd > self.max_usd {
            anyhow::bail!(
                "Daily USD limit would be exceeded (${:.0} + ${:.0} > ${:.0})",
                self.usd_today,
                trade_usd,
                self.max_usd
            );
        }

        if let Some(last) = self.last_trade_time {
            let elapsed = Utc::now() - last;
            let cooldown = chrono::Duration::minutes(self.cooldown_minutes as i64);
            if elapsed < cooldown {
                let remaining = cooldown - elapsed;
                anyhow::bail!("Cooldown active: {}s remaining", remaining.num_seconds());
            }
        }

        Ok(())
    }

    /// Record a completed trade.
    pub fn record_trade(&mut self, usd: f64) {
        self.trades_today += 1;
        self.usd_today += usd;
        self.last_trade_time = Some(Utc::now());
    }
}

/// Execution status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Pending,
    Confirmed,
    Running,
    Completed,
    PartialFill,
    Aborted,
    Failed,
}

/// Persistent execution state for crash recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionState {
    pub plan_json: String,
    pub slices_completed: u32,
    pub total_slices: u32,
    pub total_filled_qty: f64,
    pub total_filled_usd: f64,
    pub status: ExecutionStatus,
    pub order_ids: Vec<i64>,
    pub errors: Vec<String>,
    pub abort_reason: Option<String>,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl ExecutionState {
    /// Create initial state for a plan.
    pub fn new(plan: &ExecutionPlan) -> Self {
        let total_slices = match plan {
            ExecutionPlan::Immediate(_) => 1,
            ExecutionPlan::Twap(t) => t.slices.len() as u32,
            ExecutionPlan::NoTrade { .. } => 0,
        };
        let plan_json = serde_json::to_string(plan).unwrap_or_default();

        Self {
            plan_json,
            slices_completed: 0,
            total_slices,
            total_filled_qty: 0.0,
            total_filled_usd: 0.0,
            status: ExecutionStatus::Pending,
            order_ids: Vec::new(),
            errors: Vec::new(),
            abort_reason: None,
            started_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }
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

/// Live executor with safety guardrails, using IBKR TWS API.
pub struct Executor {
    config: IbkrConfig,
    circuit_breaker: CircuitBreaker,
    daily_limits: DailyLimits,
}

impl Executor {
    /// Create a new executor with IBKR config.
    pub fn new(
        config: IbkrConfig,
        circuit_breaker: CircuitBreaker,
        daily_limits: DailyLimits,
    ) -> Self {
        Self {
            config,
            circuit_breaker,
            daily_limits,
        }
    }

    /// Execute a plan. Returns the final execution state.
    pub async fn execute(&mut self, plan: &ExecutionPlan) -> ExecutionState {
        let mut state = ExecutionState::new(plan);

        match plan {
            ExecutionPlan::NoTrade { reason } => {
                state.status = ExecutionStatus::Completed;
                state.abort_reason = Some(reason.clone());
                return state;
            }
            ExecutionPlan::Immediate(p) => {
                if let Err(e) = self.daily_limits.check(p.estimated_usd) {
                    state.status = ExecutionStatus::Aborted;
                    state.abort_reason = Some(e.to_string());
                    return state;
                }
                state.status = ExecutionStatus::Running;
                self.execute_immediate(p, &mut state).await;
            }
            ExecutionPlan::Twap(p) => {
                if let Err(e) = self.daily_limits.check(p.estimated_total_usd) {
                    state.status = ExecutionStatus::Aborted;
                    state.abort_reason = Some(e.to_string());
                    return state;
                }
                state.status = ExecutionStatus::Running;
                self.execute_twap(p, &mut state).await;
            }
        }

        state.updated_at = Utc::now();
        state
    }

    /// Execute a single immediate order via IBKR.
    async fn execute_immediate(&mut self, plan: &ImmediatePlan, state: &mut ExecutionState) {
        let contract = ibapi::contracts::Contract::stock(&plan.symbol).build();
        match place_ibkr_order(&self.config, &contract, plan.side, plan.quantity).await {
            Ok(fill) => {
                state.order_ids.push(fill.order_id as i64);
                state.total_filled_qty += fill.filled_qty;
                state.total_filled_usd += fill.filled_usd;
                state.slices_completed = 1;
                state.status = ExecutionStatus::Completed;
                self.daily_limits.record_trade(fill.filled_usd);
                info!(
                    "quant: immediate order filled: {:.6} {} (${:.2})",
                    fill.filled_qty, plan.symbol, fill.filled_usd
                );
            }
            Err(e) => {
                state.status = ExecutionStatus::Failed;
                state.errors.push(e.to_string());
                error!("quant: immediate order failed: {e}");
            }
        }
    }

    /// Execute a TWAP plan slice by slice via IBKR.
    async fn execute_twap(&mut self, plan: &TwapPlan, state: &mut ExecutionState) {
        let contract = ibapi::contracts::Contract::stock(&plan.symbol).build();
        let entry_price = plan.estimated_price;
        let mut consecutive_failures: u32 = 0;

        for (i, slice) in plan.slices.iter().enumerate() {
            // Circuit breaker: check price deviation.
            match get_ibkr_price(&self.config, &contract).await {
                Ok(current_price) => {
                    if entry_price > 0.0 {
                        let deviation = (current_price - entry_price).abs() / entry_price;
                        if deviation > self.circuit_breaker.max_deviation_pct {
                            state.status = ExecutionStatus::Aborted;
                            state.abort_reason = Some(format!(
                                "Circuit breaker: price deviated {:.2}% (max {:.2}%)",
                                deviation * 100.0,
                                self.circuit_breaker.max_deviation_pct * 100.0
                            ));
                            warn!("quant: {}", state.abort_reason.as_ref().unwrap());
                            return;
                        }
                    }
                }
                Err(e) => {
                    warn!("quant: failed to check price for circuit breaker: {e}");
                }
            }

            // Execute slice.
            match place_ibkr_order(&self.config, &contract, plan.side, slice.quantity).await {
                Ok(fill) => {
                    state.order_ids.push(fill.order_id as i64);
                    state.total_filled_qty += fill.filled_qty;
                    state.total_filled_usd += fill.filled_usd;
                    state.slices_completed += 1;
                    consecutive_failures = 0;
                    info!(
                        "quant: TWAP slice {}/{}: {:.6} filled (${:.2})",
                        i + 1,
                        plan.slices.len(),
                        fill.filled_qty,
                        fill.filled_usd
                    );
                }
                Err(e) => {
                    consecutive_failures += 1;
                    state.errors.push(format!("Slice {}: {e}", i + 1));
                    error!("quant: TWAP slice {} failed: {e}", i + 1);

                    if consecutive_failures >= self.circuit_breaker.max_consecutive_failures {
                        state.status = ExecutionStatus::Aborted;
                        state.abort_reason = Some(format!(
                            "Circuit breaker: {consecutive_failures} consecutive failures"
                        ));
                        warn!("quant: {}", state.abort_reason.as_ref().unwrap());
                        return;
                    }
                }
            }

            // Progress update every 5 slices.
            if (i + 1) % 5 == 0 {
                info!(
                    "quant: TWAP progress: {}/{} slices, {:.6} filled",
                    state.slices_completed,
                    plan.slices.len(),
                    state.total_filled_qty,
                );
            }

            // Wait between slices (except last).
            if i + 1 < plan.slices.len() {
                tokio::time::sleep(std::time::Duration::from_secs(plan.interval_secs)).await;
            }
        }

        // Determine final status.
        if state.slices_completed == plan.slices.len() as u32 {
            state.status = ExecutionStatus::Completed;
            self.daily_limits.record_trade(state.total_filled_usd);
        } else if state.slices_completed > 0 {
            state.status = ExecutionStatus::PartialFill;
            self.daily_limits.record_trade(state.total_filled_usd);
        } else {
            state.status = ExecutionStatus::Failed;
        }
    }
}

/// Fill result from an IBKR order.
struct OrderFill {
    order_id: i32,
    filled_qty: f64,
    filled_usd: f64,
}

/// Place a market order via IBKR TWS API.
async fn place_ibkr_order(
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
/// Creates 3 linked orders: parent MKT → TP LMT → SL STP.
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

/// Check if adding a position would exceed the maximum position count.
pub fn check_max_positions(current: usize, max: usize) -> Result<()> {
    if current >= max {
        anyhow::bail!(
            "Max positions limit reached ({current}/{max}). Close a position before opening new ones."
        );
    }
    Ok(())
}

/// Check if daily P&L has breached the cutoff threshold.
pub fn check_daily_pnl_cutoff(daily_pnl: f64, portfolio: f64, cutoff_pct: f64) -> Result<()> {
    if portfolio > 0.0 {
        let pnl_pct = (daily_pnl / portfolio) * 100.0;
        if pnl_pct <= -cutoff_pct.abs() {
            anyhow::bail!(
                "Daily P&L cutoff breached: {pnl_pct:.2}% (limit: -{:.1}%). Trading halted.",
                cutoff_pct.abs()
            );
        }
    }
    Ok(())
}

/// Serialize execution state to JSON for crash recovery.
pub fn persist_state(state: &ExecutionState) -> Result<String> {
    Ok(serde_json::to_string_pretty(state)?)
}

/// Recover execution state from JSON.
pub fn recover_state(json: &str) -> Result<ExecutionState> {
    Ok(serde_json::from_str(json)?)
}

/// Format a final execution report.
pub fn format_final_report(state: &ExecutionState) -> String {
    format!(
        "Execution Report\n\
         ─────────────────\n\
         Status: {:?}\n\
         Slices: {}/{}\n\
         Filled: {:.6} (${:.2})\n\
         Orders: {}\n\
         Errors: {}\n\
         {}Started: {}\n\
         Duration: {}s",
        state.status,
        state.slices_completed,
        state.total_slices,
        state.total_filled_qty,
        state.total_filled_usd,
        state.order_ids.len(),
        state.errors.len(),
        state
            .abort_reason
            .as_ref()
            .map(|r| format!("Abort reason: {r}\n"))
            .unwrap_or_default(),
        state.started_at.format("%H:%M:%S"),
        (state.updated_at - state.started_at).num_seconds(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_circuit_breaker_defaults() {
        let cb = CircuitBreaker::default();
        assert!((cb.max_deviation_pct - 0.02).abs() < 1e-6);
        assert_eq!(cb.max_consecutive_failures, 3);
    }

    #[test]
    fn test_state_persistence_roundtrip() {
        let plan = ExecutionPlan::Immediate(crate::execution::ImmediatePlan {
            symbol: "AAPL".into(),
            side: Side::Buy,
            quantity: 10.0,
            estimated_price: 150.0,
            estimated_usd: 1500.0,
        });
        let state = ExecutionState::new(&plan);
        let json = persist_state(&state).unwrap();
        let recovered = recover_state(&json).unwrap();
        assert_eq!(recovered.status, ExecutionStatus::Pending);
        assert_eq!(recovered.total_slices, 1);
    }

    #[test]
    fn test_daily_limits_check() {
        let limits = DailyLimits::new(10, 10_000.0, 5);
        assert!(limits.check(500.0).is_ok());
    }

    #[test]
    fn test_daily_limits_exceeded_trades() {
        let mut limits = DailyLimits::new(2, 100_000.0, 0);
        limits.trades_today = 2;
        assert!(limits.check(100.0).is_err());
    }

    #[test]
    fn test_daily_limits_exceeded_usd() {
        let mut limits = DailyLimits::new(100, 1_000.0, 0);
        limits.usd_today = 900.0;
        assert!(limits.check(200.0).is_err());
    }

    #[test]
    fn test_daily_limits_cooldown() {
        let mut limits = DailyLimits::new(100, 100_000.0, 5);
        limits.last_trade_time = Some(Utc::now());
        assert!(limits.check(100.0).is_err());
    }

    #[test]
    fn test_format_final_report() {
        let plan = ExecutionPlan::NoTrade {
            reason: "test".into(),
        };
        let state = ExecutionState::new(&plan);
        let report = format_final_report(&state);
        assert!(report.contains("Execution Report"));
        assert!(report.contains("Status"));
    }

    #[test]
    fn test_daily_limits_record_trade() {
        let mut limits = DailyLimits::new(10, 10_000.0, 0);
        limits.record_trade(500.0);
        assert_eq!(limits.trades_today, 1);
        assert!((limits.usd_today - 500.0).abs() < 1e-6);
        assert!(limits.last_trade_time.is_some());
    }

    #[test]
    fn test_position_info_serde() {
        let pos = PositionInfo {
            account: "DU1234567".into(),
            symbol: "AAPL".into(),
            security_type: "STK".into(),
            quantity: 100.0,
            avg_cost: 150.50,
        };
        let json = serde_json::to_string(&pos).unwrap();
        let recovered: PositionInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.account, "DU1234567");
        assert_eq!(recovered.symbol, "AAPL");
        assert!((recovered.quantity - 100.0).abs() < f64::EPSILON);
        assert!((recovered.avg_cost - 150.50).abs() < f64::EPSILON);
    }

    #[test]
    fn test_daily_pnl_serde() {
        let pnl = DailyPnl {
            daily_pnl: -250.50,
            unrealized_pnl: Some(-100.0),
            realized_pnl: Some(-150.50),
        };
        let json = serde_json::to_string(&pnl).unwrap();
        let recovered: DailyPnl = serde_json::from_str(&json).unwrap();
        assert!((recovered.daily_pnl - (-250.50)).abs() < f64::EPSILON);
        assert_eq!(recovered.unrealized_pnl, Some(-100.0));
        assert_eq!(recovered.realized_pnl, Some(-150.50));
    }

    #[test]
    fn test_bracket_price_calc_buy() {
        let entry: f64 = 100.0;
        let sl_pct: f64 = 1.5;
        let tp_pct: f64 = 3.0;
        let sl_price = entry * (1.0 - sl_pct / 100.0);
        let tp_price = entry * (1.0 + tp_pct / 100.0);
        assert!((sl_price - 98.5).abs() < 1e-6);
        assert!((tp_price - 103.0).abs() < 1e-6);
    }

    #[test]
    fn test_bracket_price_calc_sell() {
        let entry: f64 = 100.0;
        let sl_pct: f64 = 1.5;
        let tp_pct: f64 = 3.0;
        let sl_price = entry * (1.0 + sl_pct / 100.0);
        let tp_price = entry * (1.0 - tp_pct / 100.0);
        assert!((sl_price - 101.5).abs() < 1e-6);
        assert!((tp_price - 97.0).abs() < 1e-6);
    }

    #[test]
    fn test_max_positions_check() {
        assert!(check_max_positions(2, 3).is_ok());
        assert!(check_max_positions(3, 3).is_err());
        assert!(check_max_positions(5, 3).is_err());
    }

    #[test]
    fn test_pnl_cutoff_check() {
        // -3% loss on $10k portfolio = -$300, cutoff at 5% → OK
        assert!(check_daily_pnl_cutoff(-300.0, 10_000.0, 5.0).is_ok());
        // -6% loss on $10k portfolio = -$600, cutoff at 5% → blocked
        assert!(check_daily_pnl_cutoff(-600.0, 10_000.0, 5.0).is_err());
        // Zero portfolio → always OK (avoids division by zero)
        assert!(check_daily_pnl_cutoff(-1000.0, 0.0, 5.0).is_ok());
    }

    #[test]
    fn test_open_order_info_serde() {
        let order = OpenOrderInfo {
            order_id: 42,
            symbol: "AAPL".into(),
            action: "Buy".into(),
            quantity: 100.0,
            order_type: "LMT".into(),
            limit_price: Some(185.50),
            stop_price: None,
            status: "Submitted".into(),
            filled: 0.0,
            remaining: 100.0,
            parent_id: 0,
        };
        let json = serde_json::to_string(&order).unwrap();
        let recovered: OpenOrderInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.order_id, 42);
        assert_eq!(recovered.symbol, "AAPL");
        assert_eq!(recovered.status, "Submitted");
        assert!((recovered.remaining - 100.0).abs() < f64::EPSILON);
    }
}
