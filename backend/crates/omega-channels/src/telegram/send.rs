//! Message sending: text, photos, chat actions, and command registration.

use super::TelegramChannel;
use omega_core::error::OmegaError;
use tracing::{debug, info, warn};

impl TelegramChannel {
    /// Send a text message to a specific chat.
    pub(crate) async fn send_text(&self, chat_id: i64, text: &str) -> Result<(), OmegaError> {
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
    pub(crate) async fn send_photo_bytes(
        &self,
        chat_id: i64,
        image: &[u8],
        caption: &str,
    ) -> Result<(), OmegaError> {
        let url = format!("{}/sendPhoto", self.base_url);

        let part = reqwest::multipart::Part::bytes(image.to_vec())
            .file_name("photo.png")
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

    /// Register bot commands with Telegram so users see an autocomplete menu.
    /// Best-effort: logs failures but does not propagate errors.
    pub(crate) async fn register_commands(&self) {
        let commands = serde_json::json!({
            "commands": [
                { "command": "help", "description": "Show available commands" },
                { "command": "status", "description": "Uptime, provider, database info" },
                { "command": "memory", "description": "Your conversation and facts stats" },
                { "command": "history", "description": "Last 5 conversation summaries" },
                { "command": "facts", "description": "List known facts about you" },
                { "command": "forget", "description": "Clear current conversation" },
                { "command": "tasks", "description": "List your scheduled tasks" },
                { "command": "cancel", "description": "Cancel a task by ID" },
                { "command": "language", "description": "Show or set your language" },
                { "command": "personality", "description": "Show or set how I behave" },
                { "command": "skills", "description": "List available skills" },
                { "command": "projects", "description": "List available projects" },
                { "command": "project", "description": "Show, activate, or deactivate a project" },
                { "command": "purge", "description": "Delete all learned facts (clean slate)" },
                { "command": "whatsapp", "description": "Connect WhatsApp via QR code" },
                { "command": "heartbeat", "description": "Heartbeat status and watchlist" },
                { "command": "learning", "description": "Show what I've learned from you" },
            ]
        });

        let url = format!("{}/setMyCommands", self.base_url);
        match self.client.post(&url).json(&commands).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("registered Telegram bot commands");
            }
            Ok(resp) => {
                let body = resp.text().await.unwrap_or_default();
                warn!("failed to register Telegram bot commands: {body}");
            }
            Err(e) => {
                warn!("failed to register Telegram bot commands: {e}");
            }
        }
    }

    /// Send a chat action (e.g. "typing") to a chat.
    pub(crate) async fn send_chat_action(
        &self,
        chat_id: i64,
        action: &str,
    ) -> Result<(), OmegaError> {
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

/// Split a long message into chunks that respect Telegram's limit.
///
/// All slice boundaries are aligned to UTF-8 char boundaries to avoid panics
/// on multi-byte content (Cyrillic, CJK, emoji, etc.).
pub(crate) fn split_message(text: &str, max_len: usize) -> Vec<&str> {
    if text.len() <= max_len {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = text.floor_char_boundary((start + max_len).min(text.len()));
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
