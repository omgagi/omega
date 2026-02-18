//! Binance market data — WebSocket kline feed and REST historical data.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

/// Binance network selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinanceNetwork {
    Testnet,
    Mainnet,
}

impl BinanceNetwork {
    /// WebSocket base URL.
    pub fn ws_base(&self) -> &str {
        match self {
            Self::Testnet => "wss://testnet.binance.vision/ws",
            Self::Mainnet => "wss://stream.binance.com:9443/ws",
        }
    }

    /// REST API base URL.
    pub fn rest_base(&self) -> &str {
        match self {
            Self::Testnet => "https://testnet.binance.vision/api/v3",
            Self::Mainnet => "https://api.binance.com/api/v3",
        }
    }
}

/// A single kline (candlestick) bar.
#[derive(Debug, Clone)]
pub struct Kline {
    pub symbol: String,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub close_time: i64,
    pub is_closed: bool,
}

/// Raw WebSocket kline event from Binance.
#[derive(Debug, Deserialize)]
struct WsKlineEvent {
    #[serde(rename = "k")]
    kline: WsKlineData,
}

#[derive(Debug, Deserialize)]
struct WsKlineData {
    #[serde(rename = "s")]
    symbol: String,
    #[serde(rename = "o")]
    open: String,
    #[serde(rename = "h")]
    high: String,
    #[serde(rename = "l")]
    low: String,
    #[serde(rename = "c")]
    close: String,
    #[serde(rename = "v")]
    volume: String,
    #[serde(rename = "T")]
    close_time: i64,
    #[serde(rename = "x")]
    is_closed: bool,
}

impl WsKlineData {
    fn to_kline(&self) -> Kline {
        Kline {
            symbol: self.symbol.clone(),
            open: self.open.parse().unwrap_or(0.0),
            high: self.high.parse().unwrap_or(0.0),
            low: self.low.parse().unwrap_or(0.0),
            close: self.close.parse().unwrap_or(0.0),
            volume: self.volume.parse().unwrap_or(0.0),
            close_time: self.close_time,
            is_closed: self.is_closed,
        }
    }
}

/// Start a WebSocket kline feed with auto-reconnect.
///
/// Returns a broadcast receiver that emits `Kline` events. The feed runs in
/// a background task and reconnects automatically on disconnection.
pub fn start_kline_feed(
    symbol: &str,
    interval: &str,
    network: BinanceNetwork,
) -> broadcast::Receiver<Kline> {
    let (tx, rx) = broadcast::channel(256);
    let url = format!(
        "{}/{}@kline_{}",
        network.ws_base(),
        symbol.to_lowercase(),
        interval
    );

    tokio::spawn(async move {
        loop {
            info!("quant: connecting to Binance WebSocket: {url}");
            match tokio_tungstenite::connect_async(&url).await {
                Ok((ws_stream, _)) => {
                    info!("quant: WebSocket connected");
                    let (_, mut read) = ws_stream.split();

                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                match serde_json::from_str::<WsKlineEvent>(&text) {
                                    Ok(event) => {
                                        let kline = event.kline.to_kline();
                                        if tx.send(kline).is_err() {
                                            info!("quant: all receivers dropped, stopping feed");
                                            return;
                                        }
                                    }
                                    Err(e) => {
                                        warn!("quant: failed to parse kline: {e}");
                                    }
                                }
                            }
                            Ok(tokio_tungstenite::tungstenite::Message::Ping(data)) => {
                                // Ping is handled at protocol level by tungstenite
                                tracing::trace!("quant: ping received ({} bytes)", data.len());
                            }
                            Err(e) => {
                                warn!("quant: WebSocket error: {e}");
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    error!("quant: WebSocket connection failed: {e}");
                }
            }

            // Reconnect after 5 seconds
            warn!("quant: WebSocket disconnected, reconnecting in 5s...");
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        }
    });

    rx
}

/// Raw REST kline response from Binance (array of arrays).
type RestKlineRow = Vec<serde_json::Value>;

/// Fetch historical klines via REST API.
pub async fn fetch_historical_klines(
    symbol: &str,
    interval: &str,
    limit: u32,
    network: BinanceNetwork,
) -> Result<Vec<Kline>> {
    let url = format!(
        "{}/klines?symbol={}&interval={}&limit={}",
        network.rest_base(),
        symbol,
        interval,
        limit
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .send()
        .await
        .context("failed to fetch historical klines")?;

    let rows: Vec<RestKlineRow> = resp
        .json()
        .await
        .context("failed to parse kline response")?;

    let mut klines = Vec::with_capacity(rows.len());
    for row in &rows {
        if row.len() < 12 {
            continue;
        }
        klines.push(Kline {
            symbol: symbol.to_string(),
            open: parse_json_f64(&row[1]),
            high: parse_json_f64(&row[2]),
            low: parse_json_f64(&row[3]),
            close: parse_json_f64(&row[4]),
            volume: parse_json_f64(&row[5]),
            close_time: row[6].as_i64().unwrap_or(0),
            is_closed: true,
        });
    }

    Ok(klines)
}

/// Parse a JSON value (string or number) as f64.
fn parse_json_f64(val: &serde_json::Value) -> f64 {
    match val {
        serde_json::Value::String(s) => s.parse().unwrap_or(0.0),
        serde_json::Value::Number(n) => n.as_f64().unwrap_or(0.0),
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_construction() {
        let testnet = BinanceNetwork::Testnet;
        assert!(testnet.ws_base().contains("testnet"));
        assert!(testnet.rest_base().contains("testnet"));

        let mainnet = BinanceNetwork::Mainnet;
        assert!(mainnet.ws_base().contains("binance.com"));
        assert!(mainnet.rest_base().contains("binance.com"));
    }

    #[test]
    fn test_kline_parsing() {
        let json = r#"{"e":"kline","E":1234567890,"s":"BTCUSDT","k":{"t":1234567800,"T":1234567899,"s":"BTCUSDT","i":"1m","f":0,"L":0,"o":"50000.00","c":"50100.00","h":"50200.00","l":"49900.00","v":"10.5","n":100,"x":true,"q":"525000.00","V":"5.0","Q":"250000.00","B":"0"}}"#;
        let event: WsKlineEvent = serde_json::from_str(json).unwrap();
        let kline = event.kline.to_kline();
        assert_eq!(kline.symbol, "BTCUSDT");
        assert_eq!(kline.open, 50_000.0);
        assert_eq!(kline.close, 50_100.0);
        assert_eq!(kline.high, 50_200.0);
        assert_eq!(kline.low, 49_900.0);
        assert!(kline.is_closed);
    }

    #[test]
    fn test_network_urls_differ() {
        assert_ne!(
            BinanceNetwork::Testnet.ws_base(),
            BinanceNetwork::Mainnet.ws_base()
        );
        assert_ne!(
            BinanceNetwork::Testnet.rest_base(),
            BinanceNetwork::Mainnet.rest_base()
        );
    }

    #[test]
    fn test_parse_json_f64() {
        assert_eq!(
            parse_json_f64(&serde_json::Value::String("42.5".into())),
            42.5
        );
        assert_eq!(parse_json_f64(&serde_json::json!(42.5)), 42.5);
        assert_eq!(parse_json_f64(&serde_json::Value::Null), 0.0);
    }
}
