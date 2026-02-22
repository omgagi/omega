//! Telegram Bot API channel.
//!
//! Uses long polling via `getUpdates` and `sendMessage` for responses.
//! Docs: <https://core.telegram.org/bots/api>

mod polling;
pub(crate) mod send;
pub(crate) mod types;

#[cfg(test)]
mod tests;

use omega_core::config::TelegramConfig;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Telegram channel using the Bot API with long polling.
pub struct TelegramChannel {
    config: TelegramConfig,
    client: reqwest::Client,
    base_url: String,
    /// Tracks the last update_id to avoid reprocessing.
    last_update_id: Arc<Mutex<Option<i64>>>,
}

impl TelegramChannel {
    /// Create a new Telegram channel from config.
    pub fn new(config: TelegramConfig) -> Self {
        let base_url = format!("https://api.telegram.org/bot{}", config.bot_token);
        Self {
            config,
            client: reqwest::Client::new(),
            base_url,
            last_update_id: Arc::new(Mutex::new(None)),
        }
    }
}
