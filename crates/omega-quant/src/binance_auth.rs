//! Binance API authentication — HMAC-SHA256 signing, order placement, and ticker queries.

use crate::market_data::BinanceNetwork;
use anyhow::{Context, Result};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::info;

type HmacSha256 = Hmac<Sha256>;

/// Binance API credentials.
#[derive(Debug, Clone)]
pub struct BinanceCredentials {
    pub api_key: String,
    pub secret_key: String,
    pub network: BinanceNetwork,
}

impl BinanceCredentials {
    /// Load credentials from environment variables.
    ///
    /// Testnet: `BINANCE_TESTNET_API_KEY` / `BINANCE_TESTNET_SECRET_KEY`
    /// Mainnet: `BINANCE_API_KEY` / `BINANCE_SECRET_KEY`
    pub fn from_env(network: BinanceNetwork) -> Result<Self> {
        let (key_var, secret_var) = match network {
            BinanceNetwork::Testnet => ("BINANCE_TESTNET_API_KEY", "BINANCE_TESTNET_SECRET_KEY"),
            BinanceNetwork::Mainnet => ("BINANCE_API_KEY", "BINANCE_SECRET_KEY"),
        };

        let api_key =
            std::env::var(key_var).with_context(|| format!("missing env var {key_var}"))?;
        let secret_key =
            std::env::var(secret_var).with_context(|| format!("missing env var {secret_var}"))?;

        Ok(Self {
            api_key,
            secret_key,
            network,
        })
    }

    /// Create credentials from explicit values.
    pub fn new(api_key: &str, secret_key: &str, network: BinanceNetwork) -> Self {
        Self {
            api_key: api_key.to_string(),
            secret_key: secret_key.to_string(),
            network,
        }
    }

    /// Sign a query string with HMAC-SHA256.
    pub fn sign(&self, query_string: &str) -> String {
        let mut mac =
            HmacSha256::new_from_slice(self.secret_key.as_bytes()).expect("HMAC key length");
        mac.update(query_string.as_bytes());
        hex::encode(mac.finalize().into_bytes())
    }

    /// Add timestamp, recvWindow, and signature to a query string.
    pub fn sign_params(&self, params: &str) -> String {
        let timestamp = chrono::Utc::now().timestamp_millis();
        let full_params = if params.is_empty() {
            format!("timestamp={timestamp}&recvWindow=5000")
        } else {
            format!("{params}&timestamp={timestamp}&recvWindow=5000")
        };
        let signature = self.sign(&full_params);
        format!("{full_params}&signature={signature}")
    }
}

/// Order response from Binance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrderResponse {
    pub symbol: String,
    #[serde(rename = "orderId")]
    pub order_id: i64,
    pub status: String,
    #[serde(rename = "executedQty", default)]
    pub executed_qty: String,
    #[serde(rename = "cummulativeQuoteQty", default)]
    pub cumulative_quote_qty: String,
    #[serde(rename = "type", default)]
    pub order_type: String,
    pub side: String,
}

/// Ticker price data from Binance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickerData {
    pub symbol: String,
    pub price: String,
}

impl TickerData {
    /// Parse price as f64.
    pub fn price_f64(&self) -> f64 {
        self.price.parse().unwrap_or(0.0)
    }
}

/// Place an order on Binance.
pub async fn place_order(
    creds: &BinanceCredentials,
    client: &reqwest::Client,
    symbol: &str,
    side: &str,
    qty: &str,
    order_type: &str,
    price: Option<&str>,
) -> Result<OrderResponse> {
    let mut params = format!("symbol={symbol}&side={side}&type={order_type}&quantity={qty}");
    if let Some(p) = price {
        params.push_str(&format!("&price={p}&timeInForce=GTC"));
    }

    let signed = creds.sign_params(&params);
    let url = format!("{}/order?{signed}", creds.network.rest_base());

    info!("quant: placing order: {side} {qty} {symbol} @ {order_type}");
    let resp = client
        .post(&url)
        .header("X-MBX-APIKEY", &creds.api_key)
        .send()
        .await
        .context("failed to place order")?;

    let status = resp.status();
    let body = resp.text().await.context("failed to read order response")?;

    if !status.is_success() {
        anyhow::bail!("order failed (HTTP {status}): {body}");
    }

    serde_json::from_str(&body).context("failed to parse order response")
}

/// Get current ticker price.
pub async fn get_ticker(
    creds: &BinanceCredentials,
    client: &reqwest::Client,
    symbol: &str,
) -> Result<TickerData> {
    let url = format!("{}/ticker/price?symbol={symbol}", creds.network.rest_base());
    let resp = client
        .get(&url)
        .header("X-MBX-APIKEY", &creds.api_key)
        .send()
        .await
        .context("failed to fetch ticker")?;

    resp.json().await.context("failed to parse ticker response")
}

/// Cancel an open order.
pub async fn cancel_order(
    creds: &BinanceCredentials,
    client: &reqwest::Client,
    symbol: &str,
    order_id: i64,
) -> Result<()> {
    let params = format!("symbol={symbol}&orderId={order_id}");
    let signed = creds.sign_params(&params);
    let url = format!("{}/order?{signed}", creds.network.rest_base());

    let resp = client
        .delete(&url)
        .header("X-MBX-APIKEY", &creds.api_key)
        .send()
        .await
        .context("failed to cancel order")?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("cancel failed: {body}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_verification() {
        let creds = BinanceCredentials::new("test_key", "test_secret", BinanceNetwork::Testnet);
        let sig1 = creds.sign("symbol=BTCUSDT&side=BUY");
        let sig2 = creds.sign("symbol=BTCUSDT&side=BUY");
        assert_eq!(sig1, sig2, "Same input should produce same signature");

        let sig3 = creds.sign("symbol=BTCUSDT&side=SELL");
        assert_ne!(
            sig1, sig3,
            "Different input should produce different signature"
        );
    }

    #[test]
    fn test_sign_params_includes_timestamp() {
        let creds = BinanceCredentials::new("key", "secret", BinanceNetwork::Testnet);
        let signed = creds.sign_params("symbol=BTCUSDT");
        assert!(signed.contains("timestamp="));
        assert!(signed.contains("signature="));
        assert!(signed.contains("recvWindow=5000"));
    }

    #[test]
    fn test_order_response_deser() {
        let json = r#"{
            "symbol": "BTCUSDT",
            "orderId": 12345,
            "status": "FILLED",
            "executedQty": "0.001",
            "cummulativeQuoteQty": "50.0",
            "type": "MARKET",
            "side": "BUY"
        }"#;
        let resp: OrderResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.symbol, "BTCUSDT");
        assert_eq!(resp.order_id, 12345);
        assert_eq!(resp.status, "FILLED");
        assert_eq!(resp.side, "BUY");
    }

    #[test]
    fn test_ticker_data_price() {
        let ticker = TickerData {
            symbol: "BTCUSDT".into(),
            price: "50123.45".into(),
        };
        assert!((ticker.price_f64() - 50123.45).abs() < 0.01);
    }

    #[test]
    fn test_testnet_default() {
        let creds = BinanceCredentials::new("k", "s", BinanceNetwork::Testnet);
        assert_eq!(creds.network, BinanceNetwork::Testnet);
    }
}
