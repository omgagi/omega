//! IBKR market data — real-time price feed via TWS API.

use anyhow::{Context, Result};
use tokio::sync::broadcast;
use tracing::{error, info, warn};

/// IBKR connection configuration.
#[derive(Debug, Clone)]
pub struct IbkrConfig {
    /// TWS/Gateway host (default: "127.0.0.1").
    pub host: String,
    /// TWS/Gateway port (paper: 4002, live: 4001).
    pub port: u16,
    /// Unique client ID per connection.
    pub client_id: i32,
}

impl IbkrConfig {
    /// Paper trading configuration (port 4002).
    pub fn paper() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 4002,
            client_id: 1,
        }
    }

    /// Live trading configuration (port 4001).
    pub fn live() -> Self {
        Self {
            host: "127.0.0.1".into(),
            port: 4001,
            client_id: 1,
        }
    }

    /// Connection URL in `host:port` format.
    pub fn connection_url(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// A single price tick from IBKR.
#[derive(Debug, Clone)]
pub struct PriceTick {
    /// Symbol (e.g. "AAPL").
    pub symbol: String,
    /// Last/close price.
    pub price: f64,
    /// Tick timestamp (epoch millis).
    pub timestamp: i64,
}

/// Start a real-time price feed for a symbol via TWS API.
///
/// Returns a broadcast receiver that emits `PriceTick` events. The feed runs in
/// a background task and reconnects automatically on disconnection.
pub fn start_price_feed(symbol: &str, config: &IbkrConfig) -> broadcast::Receiver<PriceTick> {
    let (tx, rx) = broadcast::channel(256);
    let symbol = symbol.to_string();
    let config = config.clone();

    tokio::spawn(async move {
        loop {
            info!(
                "quant: connecting to IB Gateway at {}",
                config.connection_url()
            );

            match connect_and_stream(&symbol, &config, &tx).await {
                Ok(()) => {
                    info!("quant: IBKR feed ended normally for {symbol}");
                }
                Err(e) => {
                    error!("quant: IBKR connection failed: {e}");
                }
            }

            if tx.receiver_count() == 0 {
                info!("quant: all receivers dropped, stopping feed");
                return;
            }

            warn!("quant: IBKR disconnected, reconnecting in 5s...");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    rx
}

/// Connect to IBKR and stream real-time bars, sending ticks on the broadcast channel.
async fn connect_and_stream(
    symbol: &str,
    config: &IbkrConfig,
    tx: &broadcast::Sender<PriceTick>,
) -> Result<()> {
    use ibapi::contracts::Contract;
    use ibapi::market_data::realtime::{BarSize, WhatToShow};
    use ibapi::market_data::TradingHours;
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id)
        .await
        .context("failed to connect to IB Gateway")?;

    info!("quant: connected to IB Gateway, subscribing to {symbol}");

    let contract = Contract::stock(symbol).build();

    let mut subscription = client
        .realtime_bars(
            &contract,
            BarSize::Sec5,
            WhatToShow::Trades,
            TradingHours::Extended,
        )
        .await
        .context("failed to subscribe to realtime bars")?;

    while let Some(result) = subscription.next().await {
        match result {
            Ok(bar) => {
                let tick = PriceTick {
                    symbol: symbol.to_string(),
                    price: bar.close,
                    timestamp: chrono::Utc::now().timestamp_millis(),
                };

                if tx.send(tick).is_err() {
                    info!("quant: all receivers dropped, stopping feed");
                    return Ok(());
                }
            }
            Err(e) => {
                warn!("quant: error receiving bar: {e}");
                break;
            }
        }
    }

    Ok(())
}

/// Check if IB Gateway is reachable at the given config.
pub async fn check_connection(config: &IbkrConfig) -> bool {
    use tokio::net::TcpStream;

    matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            TcpStream::connect(config.connection_url()),
        )
        .await,
        Ok(Ok(_))
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_paper_config() {
        let cfg = IbkrConfig::paper();
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 4002);
        assert_eq!(cfg.client_id, 1);
        assert_eq!(cfg.connection_url(), "127.0.0.1:4002");
    }

    #[test]
    fn test_live_config() {
        let cfg = IbkrConfig::live();
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.port, 4001);
        assert_eq!(cfg.client_id, 1);
        assert_eq!(cfg.connection_url(), "127.0.0.1:4001");
    }

    #[test]
    fn test_configs_differ() {
        assert_ne!(IbkrConfig::paper().port, IbkrConfig::live().port);
    }

    #[test]
    fn test_price_tick_clone() {
        let tick = PriceTick {
            symbol: "AAPL".into(),
            price: 150.25,
            timestamp: 1234567890,
        };
        let cloned = tick.clone();
        assert_eq!(cloned.symbol, "AAPL");
        assert!((cloned.price - 150.25).abs() < f64::EPSILON);
        assert_eq!(cloned.timestamp, 1234567890);
    }

    #[tokio::test]
    async fn test_check_connection_unreachable() {
        // Port 19999 should not have IB Gateway running.
        let cfg = IbkrConfig {
            host: "127.0.0.1".into(),
            port: 19999,
            client_id: 99,
        };
        assert!(!check_connection(&cfg).await);
    }
}
