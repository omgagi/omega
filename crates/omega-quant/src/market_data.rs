//! IBKR market data — real-time price feed, scanner, and multi-asset contracts.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
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

/// Asset class for multi-instrument support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AssetClass {
    Stock,
    Forex,
    Crypto,
}

impl std::str::FromStr for AssetClass {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "stock" | "stk" => Ok(Self::Stock),
            "forex" | "fx" | "cash" => Ok(Self::Forex),
            "crypto" => Ok(Self::Crypto),
            _ => anyhow::bail!("Unknown asset class '{s}'. Use: stock, forex/fx, crypto"),
        }
    }
}

impl std::fmt::Display for AssetClass {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stock => write!(f, "stock"),
            Self::Forex => write!(f, "forex"),
            Self::Crypto => write!(f, "crypto"),
        }
    }
}

/// Build an IBKR contract for the given symbol and asset class.
///
/// - Stock: symbol = `"AAPL"`
/// - Forex: symbol = `"EUR/USD"` (split on `/`)
/// - Crypto: symbol = `"BTC"`
pub fn build_contract(symbol: &str, asset_class: AssetClass) -> Result<ibapi::contracts::Contract> {
    use ibapi::contracts::Contract;

    match asset_class {
        AssetClass::Stock => Ok(Contract::stock(symbol).build()),
        AssetClass::Forex => {
            let parts: Vec<&str> = symbol.split('/').collect();
            if parts.len() != 2 {
                anyhow::bail!(
                    "Forex symbol must be in BASE/QUOTE format (e.g. EUR/USD), got: {symbol}"
                );
            }
            Ok(Contract::forex(parts[0], parts[1]).build())
        }
        AssetClass::Crypto => Ok(Contract::crypto(symbol).build()),
    }
}

/// A single price tick from IBKR.
#[derive(Debug, Clone)]
pub struct PriceTick {
    /// Symbol (e.g. "AAPL").
    pub symbol: String,
    /// Last/close price.
    pub price: f64,
    /// Bar volume.
    pub volume: f64,
    /// Tick timestamp (epoch millis).
    pub timestamp: i64,
}

/// Scanner result from IBKR market scanner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    /// Rank in scanner results (0-based).
    pub rank: i32,
    /// Instrument symbol.
    pub symbol: String,
    /// Security type (e.g. "STK", "CRYPTO", "CASH").
    pub security_type: String,
    /// Exchange.
    pub exchange: String,
    /// Currency.
    pub currency: String,
}

/// Start a real-time price feed for a symbol via TWS API.
///
/// Returns a broadcast receiver that emits `PriceTick` events. The feed runs in
/// a background task and reconnects automatically on disconnection.
pub fn start_price_feed(
    symbol: &str,
    config: &IbkrConfig,
    asset_class: AssetClass,
) -> broadcast::Receiver<PriceTick> {
    let (tx, rx) = broadcast::channel(256);
    let symbol = symbol.to_string();
    let config = config.clone();

    tokio::spawn(async move {
        loop {
            info!(
                "quant: connecting to IB Gateway at {}",
                config.connection_url()
            );

            match connect_and_stream(&symbol, &config, asset_class, &tx).await {
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

/// Connect to IBKR and stream price data, sending ticks on the broadcast channel.
///
/// For stocks/crypto: uses `realtime_bars` (5-second OHLCV bars).
/// For forex: uses `tick_by_tick_midpoint` (realtime_bars not supported for CASH contracts).
async fn connect_and_stream(
    symbol: &str,
    config: &IbkrConfig,
    asset_class: AssetClass,
    tx: &broadcast::Sender<PriceTick>,
) -> Result<()> {
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id)
        .await
        .context("failed to connect to IB Gateway")?;

    info!("quant: connected to IB Gateway, subscribing to {symbol}");

    let contract = build_contract(symbol, asset_class)?;

    match asset_class {
        AssetClass::Forex => stream_tick_by_tick(symbol, &client, &contract, tx).await,
        _ => stream_realtime_bars(symbol, &client, &contract, tx).await,
    }
}

/// Stream using `realtime_bars` (5-second bars) — works for stocks and crypto.
async fn stream_realtime_bars(
    symbol: &str,
    client: &ibapi::Client,
    contract: &ibapi::contracts::Contract,
    tx: &broadcast::Sender<PriceTick>,
) -> Result<()> {
    use ibapi::market_data::realtime::{BarSize, WhatToShow};
    use ibapi::market_data::TradingHours;

    let mut subscription = client
        .realtime_bars(
            contract,
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
                    volume: bar.volume,
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

/// Stream using `tick_by_tick_midpoint` — works for forex (CASH contracts on IDEALPRO).
async fn stream_tick_by_tick(
    symbol: &str,
    client: &ibapi::Client,
    contract: &ibapi::contracts::Contract,
    tx: &broadcast::Sender<PriceTick>,
) -> Result<()> {
    let mut subscription = client
        .tick_by_tick_midpoint(contract, 0, false)
        .await
        .context("failed to subscribe to tick-by-tick midpoint")?;

    while let Some(result) = subscription.next().await {
        match result {
            Ok(midpoint) => {
                let tick = PriceTick {
                    symbol: symbol.to_string(),
                    price: midpoint.mid_point,
                    volume: 0.0, // tick-by-tick doesn't include volume
                    timestamp: chrono::Utc::now().timestamp_millis(),
                };

                if tx.send(tick).is_err() {
                    info!("quant: all receivers dropped, stopping feed");
                    return Ok(());
                }
            }
            Err(e) => {
                warn!("quant: error receiving midpoint tick: {e}");
                break;
            }
        }
    }

    Ok(())
}

/// Run an IBKR market scanner to find instruments by criteria.
pub async fn run_scanner(
    config: &IbkrConfig,
    scan_code: &str,
    instrument: &str,
    location: &str,
    count: i32,
    min_price: Option<f64>,
    min_volume: Option<i32>,
) -> Result<Vec<ScanResult>> {
    use ibapi::scanner::ScannerSubscription;
    use ibapi::Client;

    let client = Client::connect(&config.connection_url(), config.client_id + 300)
        .await
        .context("failed to connect to IB Gateway for scanner")?;

    let params = ScannerSubscription {
        number_of_rows: count,
        instrument: Some(instrument.to_string()),
        location_code: Some(location.to_string()),
        scan_code: Some(scan_code.to_string()),
        above_price: min_price,
        above_volume: min_volume,
        ..ScannerSubscription::default()
    };

    let filter = Vec::new();
    let mut subscription = client
        .scanner_subscription(&params, &filter)
        .await
        .context("failed to start scanner subscription")?;

    let mut results = Vec::new();
    let timeout_dur = std::time::Duration::from_secs(10);

    // Scanner yields Vec<ScannerData> batches — read until timeout.
    while let Ok(Some(Ok(batch))) = tokio::time::timeout(timeout_dur, subscription.next()).await {
        for data in &batch {
            let contract = &data.contract_details.contract;
            results.push(ScanResult {
                rank: data.rank,
                symbol: contract.symbol.to_string(),
                security_type: contract.security_type.to_string(),
                exchange: contract.exchange.to_string(),
                currency: contract.currency.to_string(),
            });
        }
    }

    Ok(results)
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
            volume: 1000.0,
            timestamp: 1234567890,
        };
        let cloned = tick.clone();
        assert_eq!(cloned.symbol, "AAPL");
        assert!((cloned.price - 150.25).abs() < f64::EPSILON);
        assert!((cloned.volume - 1000.0).abs() < f64::EPSILON);
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

    #[test]
    fn test_build_contract_stock() {
        let contract = build_contract("AAPL", AssetClass::Stock).unwrap();
        assert_eq!(contract.symbol.to_string(), "AAPL");
    }

    #[test]
    fn test_build_contract_forex() {
        let contract = build_contract("EUR/USD", AssetClass::Forex).unwrap();
        assert_eq!(contract.symbol.to_string(), "EUR");
        assert_eq!(contract.currency.to_string(), "USD");
    }

    #[test]
    fn test_build_contract_forex_invalid() {
        assert!(build_contract("EURUSD", AssetClass::Forex).is_err());
    }

    #[test]
    fn test_build_contract_crypto() {
        let contract = build_contract("BTC", AssetClass::Crypto).unwrap();
        assert_eq!(contract.symbol.to_string(), "BTC");
    }

    #[test]
    fn test_asset_class_parse() {
        assert_eq!("stock".parse::<AssetClass>().unwrap(), AssetClass::Stock);
        assert_eq!("stk".parse::<AssetClass>().unwrap(), AssetClass::Stock);
        assert_eq!("forex".parse::<AssetClass>().unwrap(), AssetClass::Forex);
        assert_eq!("fx".parse::<AssetClass>().unwrap(), AssetClass::Forex);
        assert_eq!("cash".parse::<AssetClass>().unwrap(), AssetClass::Forex);
        assert_eq!("crypto".parse::<AssetClass>().unwrap(), AssetClass::Crypto);
        assert!("invalid".parse::<AssetClass>().is_err());
    }

    #[test]
    fn test_asset_class_display() {
        assert_eq!(AssetClass::Stock.to_string(), "stock");
        assert_eq!(AssetClass::Forex.to_string(), "forex");
        assert_eq!(AssetClass::Crypto.to_string(), "crypto");
    }

    #[test]
    fn test_scan_result_serde() {
        let result = ScanResult {
            rank: 1,
            symbol: "AAPL".into(),
            security_type: "STK".into(),
            exchange: "NASDAQ".into(),
            currency: "USD".into(),
        };
        let json = serde_json::to_string(&result).unwrap();
        let recovered: ScanResult = serde_json::from_str(&json).unwrap();
        assert_eq!(recovered.rank, 1);
        assert_eq!(recovered.symbol, "AAPL");
        assert_eq!(recovered.security_type, "STK");
    }
}
