//! Telegram Bot API channel.
//!
//! Uses long polling via `getUpdates` and `sendMessage` for responses.
//! Docs: <https://core.telegram.org/bots/api>

use async_trait::async_trait;
use omega_core::{
    config::TelegramConfig,
    error::OmegaError,
    message::{Attachment, AttachmentType, IncomingMessage, OutgoingMessage},
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
    voice: Option<TgVoice>,
    photo: Option<Vec<TgPhotoSize>>,
    caption: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TgVoice {
    file_id: String,
    duration: i64,
    mime_type: Option<String>,
    file_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct TgFile {
    file_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TgPhotoSize {
    file_id: String,
    width: i64,
    height: i64,
    file_size: Option<i64>,
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
    /// Chat type: "private", "group", "supergroup", or "channel".
    #[serde(default, rename = "type")]
    chat_type: String,
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
    async fn register_commands(&self) {
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
        self.register_commands().await;

        let (tx, rx) = mpsc::channel(64);
        let client = self.client.clone();
        let base_url = self.base_url.clone();
        let bot_token = self.config.bot_token.clone();
        let allowed_users = self.config.allowed_users.clone();
        let whisper_api_key = self.config.whisper_api_key.clone();
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

                    let (text, attachments) = if let Some(t) = msg.text {
                        (t, Vec::new())
                    } else if let Some(ref voice) = msg.voice {
                        match whisper_api_key.as_deref() {
                            Some(key) if !key.is_empty() => {
                                match download_telegram_file(
                                    &client,
                                    &base_url,
                                    &bot_token,
                                    &voice.file_id,
                                )
                                .await
                                {
                                    Ok(bytes) => {
                                        match transcribe_whisper(&client, key, &bytes).await {
                                            Ok(transcript) => {
                                                info!(
                                                    "transcribed voice message ({}s)",
                                                    voice.duration
                                                );
                                                (
                                                    format!("[Voice message] {transcript}"),
                                                    Vec::new(),
                                                )
                                            }
                                            Err(e) => {
                                                warn!("voice transcription failed: {e}");
                                                continue;
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("voice download failed: {e}");
                                        continue;
                                    }
                                }
                            }
                            _ => {
                                debug!("skipping voice (no whisper key)");
                                continue;
                            }
                        }
                    } else if let Some(ref photos) = msg.photo {
                        // Telegram sends multiple sizes; the last is the largest.
                        if let Some(largest) = photos.last() {
                            match download_telegram_file(
                                &client,
                                &base_url,
                                &bot_token,
                                &largest.file_id,
                            )
                            .await
                            {
                                Ok(bytes) => {
                                    let filename = format!("{}.jpg", Uuid::new_v4());
                                    let attachment = Attachment {
                                        file_type: AttachmentType::Image,
                                        url: None,
                                        data: Some(bytes),
                                        filename: Some(filename),
                                    };
                                    let text = msg
                                        .caption
                                        .clone()
                                        .unwrap_or_else(|| "[Photo]".to_string());
                                    info!(
                                        "downloaded photo ({}x{})",
                                        largest.width, largest.height
                                    );
                                    (text, vec![attachment])
                                }
                                Err(e) => {
                                    warn!("photo download failed: {e}");
                                    continue;
                                }
                            }
                        } else {
                            continue;
                        }
                    } else {
                        continue;
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

                    let is_group = matches!(msg.chat.chat_type.as_str(), "group" | "supergroup");

                    let incoming = IncomingMessage {
                        id: Uuid::new_v4(),
                        channel: "telegram".to_string(),
                        sender_id: user.id.to_string(),
                        sender_name: Some(sender_name),
                        text,
                        timestamp: chrono::Utc::now(),
                        reply_to: None,
                        attachments,
                        reply_target: Some(msg.chat.id.to_string()),
                        is_group,
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

/// Download a file from Telegram servers by file_id.
async fn download_telegram_file(
    client: &reqwest::Client,
    base_url: &str,
    bot_token: &str,
    file_id: &str,
) -> Result<Vec<u8>, OmegaError> {
    // Step 1: getFile to obtain file_path.
    let url = format!("{base_url}/getFile?file_id={file_id}");
    let resp: TgResponse<TgFile> = client
        .get(&url)
        .send()
        .await
        .map_err(|e| OmegaError::Channel(format!("telegram getFile failed: {e}")))?
        .json()
        .await
        .map_err(|e| OmegaError::Channel(format!("telegram getFile parse failed: {e}")))?;

    let file_path = resp
        .result
        .and_then(|f| f.file_path)
        .ok_or_else(|| OmegaError::Channel("telegram getFile returned no file_path".into()))?;

    // Step 2: Download the actual file bytes.
    let download_url = format!("https://api.telegram.org/file/bot{bot_token}/{file_path}");
    let bytes = client
        .get(&download_url)
        .send()
        .await
        .map_err(|e| OmegaError::Channel(format!("telegram file download failed: {e}")))?
        .bytes()
        .await
        .map_err(|e| OmegaError::Channel(format!("telegram file read failed: {e}")))?;

    Ok(bytes.to_vec())
}

/// Transcribe audio bytes via OpenAI Whisper API.
async fn transcribe_whisper(
    client: &reqwest::Client,
    api_key: &str,
    audio_bytes: &[u8],
) -> Result<String, OmegaError> {
    let part = reqwest::multipart::Part::bytes(audio_bytes.to_vec())
        .file_name("voice.ogg")
        .mime_str("audio/ogg")
        .map_err(|e| OmegaError::Channel(format!("whisper mime error: {e}")))?;

    let form = reqwest::multipart::Form::new()
        .text("model", "whisper-1")
        .part("file", part);

    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| OmegaError::Channel(format!("whisper request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(OmegaError::Channel(format!(
            "whisper API error {status}: {body}"
        )));
    }

    #[derive(Deserialize)]
    struct WhisperResponse {
        text: String,
    }

    let result: WhisperResponse = resp
        .json()
        .await
        .map_err(|e| OmegaError::Channel(format!("whisper response parse failed: {e}")))?;

    Ok(result.text)
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

    #[test]
    fn test_tg_chat_group_detection() {
        let group: TgChat = serde_json::from_str(r#"{"id": -100123, "type": "group"}"#).unwrap();
        assert_eq!(group.chat_type, "group");

        let supergroup: TgChat =
            serde_json::from_str(r#"{"id": -100456, "type": "supergroup"}"#).unwrap();
        assert_eq!(supergroup.chat_type, "supergroup");

        let private: TgChat = serde_json::from_str(r#"{"id": 789, "type": "private"}"#).unwrap();
        assert_eq!(private.chat_type, "private");

        // is_group check
        assert!(matches!(group.chat_type.as_str(), "group" | "supergroup"));
        assert!(matches!(
            supergroup.chat_type.as_str(),
            "group" | "supergroup"
        ));
        assert!(!matches!(
            private.chat_type.as_str(),
            "group" | "supergroup"
        ));
    }

    #[test]
    fn test_tg_chat_type_defaults_when_missing() {
        let chat: TgChat = serde_json::from_str(r#"{"id": 123}"#).unwrap();
        assert_eq!(chat.chat_type, "");
        // Missing type should not be detected as group.
        assert!(!matches!(chat.chat_type.as_str(), "group" | "supergroup"));
    }

    #[test]
    fn test_tg_message_with_voice() {
        let json = r#"{
            "message_id": 1,
            "chat": {"id": 100, "type": "private"},
            "voice": {
                "file_id": "abc123",
                "duration": 5,
                "mime_type": "audio/ogg",
                "file_size": 12345
            }
        }"#;
        let msg: TgMessage = serde_json::from_str(json).unwrap();
        assert!(msg.text.is_none());
        assert!(msg.voice.is_some());
        let voice = msg.voice.unwrap();
        assert_eq!(voice.file_id, "abc123");
        assert_eq!(voice.duration, 5);
        assert_eq!(voice.mime_type.as_deref(), Some("audio/ogg"));
    }

    #[test]
    fn test_tg_message_with_photo() {
        let json = r#"{
            "message_id": 3,
            "chat": {"id": 100, "type": "private"},
            "photo": [
                {"file_id": "small", "width": 90, "height": 90, "file_size": 1000},
                {"file_id": "medium", "width": 320, "height": 320, "file_size": 5000},
                {"file_id": "large", "width": 800, "height": 800, "file_size": 20000}
            ],
            "caption": "Check this out"
        }"#;
        let msg: TgMessage = serde_json::from_str(json).unwrap();
        assert!(msg.text.is_none());
        assert!(msg.voice.is_none());
        let photos = msg.photo.unwrap();
        assert_eq!(photos.len(), 3);
        assert_eq!(photos.last().unwrap().file_id, "large");
        assert_eq!(photos.last().unwrap().width, 800);
        assert_eq!(msg.caption.as_deref(), Some("Check this out"));
    }

    #[test]
    fn test_tg_message_with_photo_no_caption() {
        let json = r#"{
            "message_id": 4,
            "chat": {"id": 100, "type": "private"},
            "photo": [
                {"file_id": "only", "width": 640, "height": 480}
            ]
        }"#;
        let msg: TgMessage = serde_json::from_str(json).unwrap();
        assert!(msg.photo.is_some());
        assert!(msg.caption.is_none());
        let photos = msg.photo.unwrap();
        assert_eq!(photos.len(), 1);
        assert_eq!(photos[0].file_id, "only");
        assert!(photos[0].file_size.is_none());
    }

    #[test]
    fn test_tg_message_text_only() {
        let json = r#"{
            "message_id": 2,
            "chat": {"id": 100, "type": "private"},
            "text": "hello"
        }"#;
        let msg: TgMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.text.as_deref(), Some("hello"));
        assert!(msg.voice.is_none());
    }
}
