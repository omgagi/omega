//! Channel trait implementation for WhatsApp.

use super::send::{retry_send, sanitize_for_whatsapp, split_message};
use super::WhatsAppChannel;
use async_trait::async_trait;
use omega_core::{
    error::OmegaError,
    message::{IncomingMessage, OutgoingMessage},
    traits::Channel,
};
use tracing::info;
use wacore_binary::jid::Jid;

impl WhatsAppChannel {
    /// Send a photo (image bytes) with a caption to a JID.
    async fn send_photo_impl(
        &self,
        jid_str: &str,
        image: &[u8],
        caption: &str,
    ) -> Result<(), OmegaError> {
        let client_guard = self.client.lock().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| OmegaError::Channel("whatsapp client not connected".into()))?;

        let jid: Jid = jid_str
            .parse()
            .map_err(|e| OmegaError::Channel(format!("invalid whatsapp JID '{jid_str}': {e}")))?;

        let upload = client
            .upload(image.to_vec(), whatsapp_rust::download::MediaType::Image)
            .await
            .map_err(|e| OmegaError::Channel(format!("whatsapp image upload failed: {e}")))?;

        let msg = waproto::whatsapp::Message {
            image_message: Some(Box::new(waproto::whatsapp::message::ImageMessage {
                mimetype: Some("image/png".to_string()),
                caption: Some(caption.to_string()),
                url: Some(upload.url),
                direct_path: Some(upload.direct_path),
                media_key: Some(upload.media_key),
                file_enc_sha256: Some(upload.file_enc_sha256),
                file_sha256: Some(upload.file_sha256),
                file_length: Some(upload.file_length),
                ..Default::default()
            })),
            ..Default::default()
        };

        let msg_id = retry_send(client, &jid, msg).await?;
        self.sent_ids.lock().await.insert(msg_id);

        Ok(())
    }

    /// Send a text message to a JID string (phone@s.whatsapp.net).
    async fn send_text(&self, jid_str: &str, text: &str) -> Result<(), OmegaError> {
        let client_guard = self.client.lock().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| OmegaError::Channel("whatsapp client not connected".into()))?;

        let jid: Jid = jid_str
            .parse()
            .map_err(|e| OmegaError::Channel(format!("invalid whatsapp JID '{jid_str}': {e}")))?;

        let sanitized = sanitize_for_whatsapp(text);
        let chunks = split_message(&sanitized, 4096);
        for chunk in chunks {
            let msg = waproto::whatsapp::Message {
                conversation: Some(chunk.to_string()),
                ..Default::default()
            };
            let msg_id = retry_send(client, &jid, msg).await?;
            // Track sent message ID to ignore our own echo.
            self.sent_ids.lock().await.insert(msg_id);
        }

        Ok(())
    }
}

#[async_trait]
impl Channel for WhatsAppChannel {
    fn name(&self) -> &str {
        "whatsapp"
    }

    async fn start(&self) -> Result<mpsc::Receiver<IncomingMessage>, OmegaError> {
        let (tx, rx) = mpsc::channel(64);
        *self.msg_tx.lock().await = Some(tx.clone());
        self.build_and_run_bot(tx).await?;
        info!("WhatsApp channel started");
        Ok(rx)
    }

    async fn send_typing(&self, target: &str) -> Result<(), OmegaError> {
        let client_guard = self.client.lock().await;
        if let Some(ref client) = *client_guard {
            let jid: Jid = target.parse().map_err(|e| {
                OmegaError::Channel(format!("invalid whatsapp JID '{target}': {e}"))
            })?;
            let _ = client.chatstate().send_composing(&jid).await;
        }
        Ok(())
    }

    async fn send_photo(
        &self,
        target: &str,
        image: &[u8],
        caption: &str,
    ) -> Result<(), OmegaError> {
        self.send_photo_impl(target, image, caption).await
    }

    async fn send(&self, message: OutgoingMessage) -> Result<(), OmegaError> {
        let target = message
            .reply_target
            .as_deref()
            .ok_or_else(|| OmegaError::Channel("no reply_target on outgoing message".into()))?;

        self.send_text(target, &message.text).await
    }

    async fn stop(&self) -> Result<(), OmegaError> {
        info!("WhatsApp channel stopped");
        *self.client.lock().await = None;
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

use tokio::sync::mpsc;
