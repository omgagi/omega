//! Message sending: text, photos, chat actions, and command registration.

use super::TelegramChannel;
use crate::utils::split_message;
use omega_core::error::OmegaError;
use tracing::{info, warn};

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
                    warn!("Markdown parse failed, retrying as plain text: {error_text}");
                    let plain_body = serde_json::json!({
                        "chat_id": chat_id,
                        "text": chunk,
                    });
                    let plain_resp = self
                        .client
                        .post(format!("{}/sendMessage", self.base_url))
                        .json(&plain_body)
                        .send()
                        .await
                        .map_err(|e| {
                            OmegaError::Channel(format!("telegram send (plain) failed: {e}"))
                        })?;
                    if !plain_resp.status().is_success() {
                        let plain_err = plain_resp.text().await.unwrap_or_default();
                        return Err(OmegaError::Channel(format!(
                            "telegram send (plain fallback) failed: {plain_err}"
                        )));
                    }
                } else {
                    return Err(OmegaError::Channel(format!(
                        "telegram send failed ({status}): {error_text}"
                    )));
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
            let status = resp.status();
            let error_text = resp.text().await.unwrap_or_default();
            return Err(OmegaError::Channel(format!(
                "telegram sendPhoto failed ({status}): {error_text}"
            )));
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
                { "command": "setup", "description": "Configure OMEGA \u{03a9} as a domain expert" },
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
