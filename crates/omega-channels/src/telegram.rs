//! Telegram Bot API channel.
//!
//! Uses long polling via `getUpdates` and `sendMessage` for responses.
//! Docs: <https://core.telegram.org/bots/api>

use async_trait::async_trait;
use omega_core::{
    config::TelegramConfig,
    error::OmegaError,
    message::{IncomingMessage, OutgoingMessage},
    traits::Channel,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Telegram channel using the Bot API with long polling.
pub struct TelegramChannel {
    config: TelegramConfig,
    client: reqwest::Client,
    base_url: String,
    /// Tracks the last update_id to avoid reprocessing.
    last_update_id: Arc<Mutex<Option<i64>>>,
}

// --- Telegram API types ---

#[derive(Debug, Deserialize)]
struct TgResponse<T> {
    ok: bool,
    result: Option<T>,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TgUpdate {
    update_id: i64,
    message: Option<TgMessage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TgMessage {
    message_id: i64,
    from: Option<TgUser>,
    chat: TgChat,
    text: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TgUser {
    id: i64,
    first_name: String,
    last_name: Option<String>,
    username: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TgChat {
    id: i64,
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

    /// Send a text message to a specific chat.
    async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), OmegaError> {
        let chunks = split_message(text, 4096);

        for chunk in chunks {
            let url = format!("{}/sendMessage", self.base_url);
            let body = serde_json::json!({
                "chat_id": chat_id,
                "text": chunk,
                "parse_mode": "Markdown",
            });

            let resp = self
                .client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(|e| OmegaError::Channel(format!("telegram send failed: {e}")))?;

            let status = resp.status();
            if !status.is_success() {
                let error_text = resp.text().await.unwrap_or_default();
                if error_text.contains("can't parse entities") {
                    debug!("Markdown parse failed, retrying as plain text");
                    let plain_body = serde_json::json!({
                        "chat_id": chat_id,
                        "text": chunk,
                    });
                    self.client
                        .post(format!("{}/sendMessage", self.base_url))
                        .json(&plain_body)
                        .send()
                        .await
                        .map_err(|e| {
                            OmegaError::Channel(format!("telegram send (plain) failed: {e}"))
                        })?;
                } else {
                    warn!("telegram send got {status}: {error_text}");
                }
            }
        }

        Ok(())
    }

    /// Send a photo (PNG bytes) with a caption to a chat.
    async fn send_photo_bytes(
        &self,
        chat_id: i64,
        image: &[u8],
        caption: &str,
    ) -> Result<(), OmegaError> {
        let url = format!("{}/sendPhoto", self.base_url);

        let part = reqwest::multipart::Part::bytes(image.to_vec())
            .file_name("qr.png")
            .mime_str("image/png")
            .map_err(|e| OmegaError::Channel(format!("mime error: {e}")))?;

        let form = reqwest::multipart::Form::new()
            .text("chat_id", chat_id.to_string())
            .text("caption", caption.to_string())
            .part("photo", part);

        let resp = self
            .client
            .post(&url)
            .multipart(form)
            .send()
            .await
            .map_err(|e| OmegaError::Channel(format!("telegram sendPhoto failed: {e}")))?;

        if !resp.status().is_success() {
            let error_text = resp.text().await.unwrap_or_default();
            warn!("telegram sendPhoto error: {error_text}");
        }

        Ok(())
    }

    /// Send a chat action (e.g. "typing") to a chat.
    async fn send_chat_action(&self, chat_id: i64, action: &str) -> Result<(), OmegaError> {
        let url = format!("{}/sendChatAction", self.base_url);
        let body = serde_json::json!({
            "chat_id": chat_id,
            "action": action,
        });

        self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| OmegaError::Channel(format!("telegram sendChatAction failed: {e}")))?;

        Ok(())
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    fn name(&self) -> &str {
        "telegram"
    }

    async fn start(&self) -> Result<mpsc::Receiver<IncomingMessage>, OmegaError> {
        let (tx, rx) = mpsc::channel(64);
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let allowed_users = self.config.allowed_users.clone();
        let last_update_id = self.last_update_id.clone();

        info!("Telegram channel starting long polling...");

        tokio::spawn(async move {
            let mut backoff_secs: u64 = 1;

            loop {
                let last = last_update_id.lock().await;
                let offset = last.map(|id| id + 1);
                drop(last);

                let mut url = format!("{base_url}/getUpdates?timeout=30");
                if let Some(off) = offset {
                    url.push_str(&format!("&offset={off}"));
                }

                let resp = match client
                    .get(&url)
                    .timeout(std::time::Duration::from_secs(35))
                    .send()
                    .await
                {
                    Ok(r) => r,
                    Err(e) => {
                        error!("telegram poll error (retry in {backoff_secs}s): {e}");
                        tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
                        backoff_secs = (backoff_secs * 2).min(60);
                        continue;
                    }
                };

                let body: TgResponse<Vec<TgUpdate>> = match resp.json().await {
                    Ok(b) => b,
                    Err(e) => {
                        error!("telegram parse error (retry in {backoff_secs}s): {e}");
                        tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
                        backoff_secs = (backoff_secs * 2).min(60);
                        continue;
                    }
                };

                if !body.ok {
                    error!(
                        "telegram API error (retry in {backoff_secs}s): {}",
                        body.description.unwrap_or_default()
                    );
                    tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
                    backoff_secs = (backoff_secs * 2).min(60);
                    continue;
                }

                // Successful poll — reset backoff.
                backoff_secs = 1;

                let updates = body.result.unwrap_or_default();

                if let Some(last_update) = updates.last() {
                    *last_update_id.lock().await = Some(last_update.update_id);
                }

                for update in updates {
                    let msg = match update.message {
                        Some(m) => m,
                        None => continue,
                    };

                    let text = match msg.text {
                        Some(t) => t,
                        None => continue,
                    };

                    let user = match msg.from {
                        Some(u) => u,
                        None => continue,
                    };

                    // Auth check.
                    if !allowed_users.is_empty() && !allowed_users.contains(&user.id) {
                        warn!("ignoring message from unauthorized user {}", user.id);
                        continue;
                    }

                    let sender_name = if let Some(ref un) = user.username {
                        format!("@{un}")
                    } else if let Some(ref ln) = user.last_name {
                        format!("{} {ln}", user.first_name)
                    } else {
                        user.first_name.clone()
                    };

                    let incoming = IncomingMessage {
                        id: Uuid::new_v4(),
                        channel: "telegram".to_string(),
                        sender_id: user.id.to_string(),
                        sender_name: Some(sender_name),
                        text,
                        timestamp: chrono::Utc::now(),
                        reply_to: None,
                        attachments: Vec::new(),
                        reply_target: Some(msg.chat.id.to_string()),
                    };

                    if tx.send(incoming).await.is_err() {
                        info!("telegram channel receiver dropped, stopping poll");
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn send_typing(&self, target: &str) -> Result<(), OmegaError> {
        let chat_id: i64 = target.parse().map_err(|e| {
            OmegaError::Channel(format!("invalid telegram chat_id '{target}': {e}"))
        })?;
        self.send_chat_action(chat_id, "typing").await
    }

    async fn send_photo(
        &self,
        target: &str,
        image: &[u8],
        caption: &str,
    ) -> Result<(), OmegaError> {
        let chat_id: i64 = target.parse().map_err(|e| {
            OmegaError::Channel(format!("invalid telegram chat_id '{target}': {e}"))
        })?;
        self.send_photo_bytes(chat_id, image, caption).await
    }

    async fn send(&self, message: OutgoingMessage) -> Result<(), OmegaError> {
        let chat_id_str = message
            .reply_target
            .as_deref()
            .ok_or_else(|| OmegaError::Channel("no reply_target on outgoing message".into()))?;

        let chat_id: i64 = chat_id_str.parse().map_err(|e| {
            OmegaError::Channel(format!("invalid telegram chat_id '{chat_id_str}': {e}"))
        })?;

        self.send_message(chat_id, &message.text).await
    }

    async fn stop(&self) -> Result<(), OmegaError> {
        info!("Telegram channel stopped");
        Ok(())
    }
}

/// Split a long message into chunks that respect Telegram's limit.
fn split_message(text: &str, max_len: usize) -> Vec<&str> {
    if text.len() <= max_len {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = (start + max_len).min(text.len());
        let break_at = if end < text.len() {
            text[start..end]
                .rfind('\n')
                .map(|i| start + i + 1)
                .unwrap_or(end)
        } else {
            end
        };
        chunks.push(&text[start..break_at]);
        start = break_at;
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_short_message() {
        let chunks = split_message("hello", 4096);
        assert_eq!(chunks, vec!["hello"]);
    }

    #[test]
    fn test_split_long_message() {
        let text = "a\n".repeat(3000);
        let chunks = split_message(&text, 4096);
        assert!(chunks.len() >= 2);
        for chunk in &chunks {
            assert!(chunk.len() <= 4096);
        }
    }
}
