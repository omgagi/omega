//! Live order executor with circuit breaker, daily limits, and crash recovery.

use crate::binance_auth::{self, BinanceCredentials};
use crate::execution::{ExecutionPlan, ImmediatePlan, Side, TwapPlan};
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

/// Live executor with safety guardrails.
pub struct Executor {
    creds: BinanceCredentials,
    client: reqwest::Client,
    circuit_breaker: CircuitBreaker,
    daily_limits: DailyLimits,
}

impl Executor {
    /// Create a new executor.
    pub fn new(
        creds: BinanceCredentials,
        circuit_breaker: CircuitBreaker,
        daily_limits: DailyLimits,
    ) -> Self {
        Self {
            creds,
            client: reqwest::Client::new(),
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
                // Check daily limits
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

    /// Execute a single immediate order.
    async fn execute_immediate(&mut self, plan: &ImmediatePlan, state: &mut ExecutionState) {
        let side = match plan.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };

        match binance_auth::place_order(
            &self.creds,
            &self.client,
            &plan.symbol,
            side,
            &format!("{:.6}", plan.quantity),
            "MARKET",
            None,
        )
        .await
        {
            Ok(resp) => {
                state.order_ids.push(resp.order_id);
                let filled_qty: f64 = resp.executed_qty.parse().unwrap_or(0.0);
                let filled_usd: f64 = resp.cumulative_quote_qty.parse().unwrap_or(0.0);
                state.total_filled_qty += filled_qty;
                state.total_filled_usd += filled_usd;
                state.slices_completed = 1;
                state.status = ExecutionStatus::Completed;
                self.daily_limits.record_trade(filled_usd);
                info!(
                    "quant: immediate order filled: {:.6} {} (${:.2})",
                    filled_qty, plan.symbol, filled_usd
                );
            }
            Err(e) => {
                state.status = ExecutionStatus::Failed;
                state.errors.push(e.to_string());
                error!("quant: immediate order failed: {e}");
            }
        }
    }

    /// Execute a TWAP plan slice by slice.
    async fn execute_twap(&mut self, plan: &TwapPlan, state: &mut ExecutionState) {
        let side = match plan.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };
        let entry_price = plan.estimated_price;
        let mut consecutive_failures: u32 = 0;

        for (i, slice) in plan.slices.iter().enumerate() {
            // Check circuit breaker: price deviation
            match binance_auth::get_ticker(&self.creds, &self.client, &plan.symbol).await {
                Ok(ticker) => {
                    let current_price = ticker.price_f64();
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
                    warn!("quant: failed to check ticker for circuit breaker: {e}");
                }
            }

            // Execute slice
            match binance_auth::place_order(
                &self.creds,
                &self.client,
                &plan.symbol,
                side,
                &format!("{:.6}", slice.quantity),
                "MARKET",
                None,
            )
            .await
            {
                Ok(resp) => {
                    state.order_ids.push(resp.order_id);
                    let filled_qty: f64 = resp.executed_qty.parse().unwrap_or(0.0);
                    let filled_usd: f64 = resp.cumulative_quote_qty.parse().unwrap_or(0.0);
                    state.total_filled_qty += filled_qty;
                    state.total_filled_usd += filled_usd;
                    state.slices_completed += 1;
                    consecutive_failures = 0;
                    info!(
                        "quant: TWAP slice {}/{}: {:.6} filled (${:.2})",
                        i + 1,
                        plan.slices.len(),
                        filled_qty,
                        filled_usd
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

            // Progress update every 5 slices
            if (i + 1) % 5 == 0 {
                info!(
                    "quant: TWAP progress: {}/{} slices, {:.6} filled",
                    state.slices_completed,
                    plan.slices.len(),
                    state.total_filled_qty,
                );
            }

            // Wait between slices (except last)
            if i + 1 < plan.slices.len() {
                tokio::time::sleep(std::time::Duration::from_secs(plan.interval_secs)).await;
            }
        }

        // Determine final status
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
            symbol: "BTCUSDT".into(),
            side: Side::Buy,
            quantity: 0.01,
            estimated_price: 50_000.0,
            estimated_usd: 500.0,
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
}
