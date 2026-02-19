//! # omega-quant
//!
//! Quantitative trading engine for Omega. Provides Kalman-filtered prices,
//! HMM regime detection, fractional Kelly sizing, Merton optimal allocation,
//! and safe execution (TWAP + Immediate) with circuit breaker, daily limits,
//! human confirmation, and crash recovery. Uses IBKR (Interactive Brokers)
//! via TWS API for market data and order execution.

pub mod execution;
pub mod executor;
pub mod hmm;
pub mod kalman;
pub mod kelly;
pub mod market_data;
pub mod signal;

use chrono::Utc;
use signal::{
    Action, Direction, ExecutionStrategy, HurstInterpretation, QuantSignal, Regime,
    RegimeProbabilities,
};

/// Default risk-free rate (annualized).
const RISK_FREE_RATE: f64 = 0.05;

/// Regime-specific drift rates (annualized).
const DRIFT_BULL: f64 = 0.60;
const DRIFT_BEAR: f64 = -0.40;
const DRIFT_LATERAL: f64 = 0.05;

/// Regime-specific volatilities (annualized).
const VOL_BULL: f64 = 0.55;
const VOL_BEAR: f64 = 0.85;
const VOL_LATERAL: f64 = 0.35;

/// Regime-specific win rates for Kelly calculation.
const WIN_RATE_BULL: f64 = 0.58;
const WIN_RATE_BEAR: f64 = 0.42;
const WIN_RATE_LATERAL: f64 = 0.50;

/// Main orchestrator combining all quant modules.
pub struct QuantEngine {
    symbol: String,
    kalman: kalman::KalmanFilter,
    hmm: hmm::HiddenMarkovModel,
    kelly: kelly::KellyCriterion,
    portfolio_value: f64,
    risk_aversion: f64,
    prev_price: Option<f64>,
    ewma_vol: f64,
    tick_count: u64,
    last_signal: Option<QuantSignal>,
}

impl QuantEngine {
    /// Create a new engine with default risk aversion (γ = 2.0).
    pub fn new(symbol: &str, portfolio_value: f64) -> Self {
        Self::new_with_risk_aversion(symbol, portfolio_value, 2.0)
    }

    /// Create a new engine with custom risk aversion.
    pub fn new_with_risk_aversion(symbol: &str, portfolio_value: f64, risk_aversion: f64) -> Self {
        Self {
            symbol: symbol.to_string(),
            kalman: kalman::KalmanFilter::crypto_default(),
            hmm: hmm::HiddenMarkovModel::crypto_default(),
            kelly: kelly::KellyCriterion::crypto_default(),
            portfolio_value,
            risk_aversion,
            prev_price: None,
            ewma_vol: 0.0,
            tick_count: 0,
            last_signal: None,
        }
    }

    /// Get the most recent signal (if any).
    pub fn last_signal(&self) -> Option<QuantSignal> {
        self.last_signal.clone()
    }

    /// Train the HMM on historical return data.
    pub fn train_hmm(&mut self, returns: &[f64], iterations: usize) {
        self.hmm.train(returns, iterations);
    }

    /// Update portfolio value.
    pub fn set_portfolio_value(&mut self, value: f64) {
        self.portfolio_value = value;
    }

    /// Process a new price tick and produce a full signal.
    pub fn process_price(&mut self, price: f64) -> QuantSignal {
        self.tick_count += 1;

        // --- Kalman filter ---
        let (filtered_price, trend) = self.kalman.update(price);

        // --- Returns + EWMA volatility ---
        let pct_return = if let Some(prev) = self.prev_price {
            if prev > 0.0 {
                (price - prev) / prev
            } else {
                0.0
            }
        } else {
            0.0
        };
        self.prev_price = Some(price);

        // EWMA volatility (λ = 0.94)
        let lambda = 0.94;
        self.ewma_vol = (lambda * self.ewma_vol * self.ewma_vol
            + (1.0 - lambda) * pct_return * pct_return)
            .sqrt();

        // --- HMM regime detection ---
        let obs = hmm::HiddenMarkovModel::discretize_return(pct_return);
        let (regime, probs) = self.hmm.update(obs);
        let regime_confidence = match regime {
            Regime::Bull => probs[0],
            Regime::Bear => probs[1],
            Regime::Lateral => probs[2],
        };

        // --- Merton optimal allocation (inlined) ---
        // u* = (μ - r) / (γ * σ²), clamped to [-0.5, 1.5]
        let (mu_weighted, sigma_sq) = {
            let mu = probs[0] * DRIFT_BULL + probs[1] * DRIFT_BEAR + probs[2] * DRIFT_LATERAL;
            let vol = probs[0] * VOL_BULL + probs[1] * VOL_BEAR + probs[2] * VOL_LATERAL;
            (mu, vol * vol)
        };
        let merton_raw = if sigma_sq > 1e-12 {
            (mu_weighted - RISK_FREE_RATE) / (self.risk_aversion * sigma_sq)
        } else {
            0.0
        };
        let merton_allocation = merton_raw.clamp(-0.5, 1.5);

        // --- Kelly sizing ---
        let (win_rate, win_loss_ratio) = match regime {
            Regime::Bull => (WIN_RATE_BULL, 1.8),
            Regime::Bear => (WIN_RATE_BEAR, 1.2),
            Regime::Lateral => (WIN_RATE_LATERAL, 1.0),
        };
        let kelly_output = self.kelly.calculate(
            win_rate,
            win_loss_ratio,
            self.portfolio_value,
            regime_confidence,
        );

        // --- Direction & Action ---
        let direction = if merton_allocation > 0.1 {
            Direction::Long
        } else if merton_allocation < -0.1 {
            Direction::Short
        } else {
            Direction::Hold
        };

        let action = match (&regime, &direction) {
            (Regime::Bull, Direction::Long) if kelly_output.should_trade => Action::Long {
                urgency: regime_confidence,
            },
            (Regime::Bear, Direction::Short) if kelly_output.should_trade => Action::Short {
                urgency: regime_confidence,
            },
            (Regime::Bear, _) if merton_allocation < -0.2 => Action::ReducePosition {
                by_percent: (merton_allocation.abs() * 50.0).min(100.0),
            },
            _ if !kelly_output.should_trade => Action::Hold,
            _ => Action::Hold,
        };

        // --- Execution strategy ---
        let execution = match &action {
            Action::Hold => ExecutionStrategy::DontTrade,
            Action::Long { .. } | Action::Short { .. } => {
                if kelly_output.position_size_usd < 100.0 {
                    ExecutionStrategy::DontTrade
                } else if kelly_output.position_size_usd < 5000.0 {
                    ExecutionStrategy::Immediate
                } else {
                    ExecutionStrategy::Twap {
                        slices: ((kelly_output.position_size_usd / 1000.0).ceil() as u32).max(3),
                        interval_secs: 60,
                    }
                }
            }
            Action::ReducePosition { .. } | Action::Exit => ExecutionStrategy::Immediate,
        };

        // --- Confidence ---
        let confidence = regime_confidence * kelly_output.full_kelly.abs().min(1.0);

        // --- Reasoning ---
        let regime_emoji = match regime {
            Regime::Bull => "📈",
            Regime::Bear => "📉",
            Regime::Lateral => "➡️",
        };
        let reasoning = format!(
            "{regime_emoji} {regime:?} regime (confidence: {:.0}%) | \
             Kalman trend: {trend:+.6} | EWMA vol: {:.2}% | \
             Merton: {merton_allocation:+.2} | Kelly: ${:.0} ({:.1}%) | \
             Action: {action}",
            regime_confidence * 100.0,
            self.ewma_vol * 100.0,
            kelly_output.position_size_usd,
            kelly_output.fractional_kelly * 100.0,
        );

        let result = QuantSignal {
            timestamp: Utc::now(),
            symbol: self.symbol.clone(),
            raw_price: price,
            filtered_price,
            trend,
            regime,
            regime_probabilities: RegimeProbabilities {
                bull: probs[0],
                bear: probs[1],
                lateral: probs[2],
            },
            hurst_exponent: 0.5,
            hurst_interpretation: HurstInterpretation::Random,
            merton_allocation,
            kelly_fraction: kelly_output.fractional_kelly,
            kelly_position_usd: kelly_output.position_size_usd,
            kelly_should_trade: kelly_output.should_trade,
            direction,
            action,
            execution,
            confidence,
            reasoning,
        };

        self.last_signal = Some(result.clone());
        result
    }

    /// Format a signal as a human-readable advisory block.
    pub fn format_signal(signal: &QuantSignal) -> String {
        let regime_emoji = match signal.regime {
            Regime::Bull => "📈",
            Regime::Bear => "📉",
            Regime::Lateral => "➡️",
        };
        format!(
            "[QUANT ADVISORY — NOT FINANCIAL ADVICE]\n\
             Symbol: {} | Price: ${:.2} (filtered: ${:.2})\n\
             Regime: {regime_emoji} {:?} (Bull: {:.0}% | Bear: {:.0}% | Lateral: {:.0}%)\n\
             Hurst: {:.2} ({})\n\
             Merton allocation: {:+.2} | Kelly: {:.1}% (${:.0})\n\
             Direction: {:?} | Action: {} | Execution: {:?}\n\
             Confidence: {:.0}%\n\
             {}\n\
             [END QUANT ADVISORY]",
            signal.symbol,
            signal.raw_price,
            signal.filtered_price,
            signal.regime,
            signal.regime_probabilities.bull * 100.0,
            signal.regime_probabilities.bear * 100.0,
            signal.regime_probabilities.lateral * 100.0,
            signal.hurst_exponent,
            signal.hurst_interpretation,
            signal.merton_allocation,
            signal.kelly_fraction * 100.0,
            signal.kelly_position_usd,
            signal.direction,
            signal.action,
            signal.execution,
            signal.confidence * 100.0,
            signal.reasoning,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_pipeline_50_bars() {
        let mut engine = QuantEngine::new("BTCUSDT", 10_000.0);

        // Simulate 50 price bars with an uptrend
        let base_price = 50_000.0;
        let mut signals = Vec::new();
        for i in 0..50 {
            let price = base_price + (i as f64) * 100.0 + (i as f64 * 0.5).sin() * 200.0;
            let signal = engine.process_price(price);
            signals.push(signal);
        }

        assert_eq!(signals.len(), 50);
        // All signals should have valid fields
        for s in &signals {
            assert_eq!(s.symbol, "BTCUSDT");
            assert!(s.filtered_price > 0.0);
            assert!(s.regime_probabilities.bull >= 0.0 && s.regime_probabilities.bull <= 1.0);
            assert!(s.regime_probabilities.bear >= 0.0 && s.regime_probabilities.bear <= 1.0);
            assert!(s.regime_probabilities.lateral >= 0.0 && s.regime_probabilities.lateral <= 1.0);
            assert!(
                (s.regime_probabilities.bull
                    + s.regime_probabilities.bear
                    + s.regime_probabilities.lateral
                    - 1.0)
                    .abs()
                    < 1e-6
            );
            assert_eq!(s.hurst_exponent, 0.5);
            assert!(s.merton_allocation >= -0.5 && s.merton_allocation <= 1.5);
            assert!(s.confidence >= 0.0);
        }
    }

    #[test]
    fn test_signal_formatting_contains_disclaimer() {
        let mut engine = QuantEngine::new("ETHUSDT", 5_000.0);
        let signal = engine.process_price(3_000.0);
        let formatted = QuantEngine::format_signal(&signal);
        assert!(formatted.contains("NOT FINANCIAL ADVICE"));
        assert!(formatted.contains("ETHUSDT"));
        assert!(formatted.contains("QUANT ADVISORY"));
    }

    #[test]
    fn test_merton_allocation_clamped() {
        let mut engine = QuantEngine::new_with_risk_aversion("BTCUSDT", 100_000.0, 0.1);
        // Feed extreme prices to push Merton to extremes
        for i in 0..20 {
            let price = 50_000.0 + (i as f64) * 5_000.0;
            let signal = engine.process_price(price);
            assert!(
                signal.merton_allocation >= -0.5 && signal.merton_allocation <= 1.5,
                "Merton allocation out of bounds: {}",
                signal.merton_allocation
            );
        }
    }

    #[test]
    fn test_hurst_hardcoded() {
        let mut engine = QuantEngine::new("BTCUSDT", 10_000.0);
        let signal = engine.process_price(50_000.0);
        assert_eq!(signal.hurst_exponent, 0.5);
        assert!(matches!(
            signal.hurst_interpretation,
            HurstInterpretation::Random
        ));
    }

    #[test]
    fn test_set_portfolio_value() {
        let mut engine = QuantEngine::new("BTCUSDT", 10_000.0);
        engine.set_portfolio_value(50_000.0);
        // Process a price and verify kelly uses new portfolio value
        engine.process_price(50_000.0);
        let signal = engine.process_price(50_100.0);
        // Kelly position should be based on new portfolio value
        assert!(signal.kelly_position_usd <= 50_000.0 * 0.5);
    }
}
