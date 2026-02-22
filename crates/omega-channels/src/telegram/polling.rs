//! Long-polling update loop and Channel trait implementation.

use super::types::{TgFile, TgResponse, TgUpdate};
use super::TelegramChannel;
use async_trait::async_trait;
use omega_core::{
    error::OmegaError,
    message::{Attachment, AttachmentType, IncomingMessage, OutgoingMessage},
    traits::Channel,
};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

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

                // Successful poll -- reset backoff.
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
                                        match crate::whisper::transcribe_whisper(
                                            &client, key, &bytes,
                                        )
                                        .await
                                        {
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

                    // Drop group messages -- OMEGA only interacts person-to-person.
                    let is_group = matches!(msg.chat.chat_type.as_str(), "group" | "supergroup");
                    if is_group {
                        debug!("telegram: ignoring group message from chat {}", msg.chat.id);
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
                        attachments,
                        reply_target: Some(msg.chat.id.to_string()),
                        is_group: false,
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

        self.send_text(chat_id, &message.text).await
    }

    async fn stop(&self) -> Result<(), OmegaError> {
        info!("Telegram channel stopped");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
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
