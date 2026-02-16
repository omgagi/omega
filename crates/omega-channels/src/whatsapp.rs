//! WhatsApp channel — pure Rust implementation via `whatsapp-rust`.
//!
//! Uses the WhatsApp Web protocol (Noise handshake + Signal encryption).
//! Pairing is done by scanning a QR code, like WhatsApp Web.
//! Session is persisted to `{data_dir}/whatsapp_session/whatsapp.db`.

use async_trait::async_trait;
use omega_core::{
    config::WhatsAppConfig,
    error::OmegaError,
    message::{IncomingMessage, OutgoingMessage},
    traits::Channel,
};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};
use uuid::Uuid;
use wacore::types::events::Event;
use wacore_binary::jid::Jid;
use whatsapp_rust::bot::Bot;
use whatsapp_rust::client::Client;
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

use crate::whatsapp_store::SqlxWhatsAppStore;

/// WhatsApp channel using the WhatsApp Web protocol.
pub struct WhatsAppChannel {
    config: WhatsAppConfig,
    data_dir: String,
    /// Client handle for sending messages — set after `start()`.
    client: Arc<Mutex<Option<Arc<Client>>>>,
    /// Message IDs we sent — used to ignore our own echo in self-chat.
    sent_ids: Arc<Mutex<HashSet<String>>>,
}

impl WhatsAppChannel {
    /// Create a new WhatsApp channel from config.
    pub fn new(config: WhatsAppConfig, data_dir: &str) -> Self {
        Self {
            config,
            data_dir: data_dir.to_string(),
            client: Arc::new(Mutex::new(None)),
            sent_ids: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Get the session database path.
    fn session_db_path(&self) -> String {
        let dir = omega_core::config::shellexpand(&self.data_dir);
        let session_dir = format!("{dir}/whatsapp_session");
        // Ensure directory exists.
        let _ = std::fs::create_dir_all(&session_dir);
        format!("{session_dir}/whatsapp.db")
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

        let chunks = split_message(text, 4096);
        for chunk in chunks {
            let msg = waproto::whatsapp::Message {
                conversation: Some(chunk.to_string()),
                ..Default::default()
            };
            let msg_id = client
                .send_message(jid.clone(), msg)
                .await
                .map_err(|e| OmegaError::Channel(format!("whatsapp send failed: {e}")))?;
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
        let db_path = self.session_db_path();
        let allowed_users = self.config.allowed_users.clone();
        let client_handle = self.client.clone();

        info!("WhatsApp channel starting (session: {db_path})...");

        let backend = Arc::new(
            SqlxWhatsAppStore::new(&db_path)
                .await
                .map_err(|e| OmegaError::Channel(format!("whatsapp store init failed: {e}")))?,
        );

        let tx_events = tx.clone();
        let client_for_event = client_handle.clone();
        let sent_ids_for_event = self.sent_ids.clone();

        let mut bot = Bot::builder()
            .with_backend(backend)
            .with_transport_factory(TokioWebSocketTransportFactory::new())
            .with_http_client(UreqHttpClient::new())
            .on_event(move |event, client| {
                let tx = tx_events.clone();
                let allowed = allowed_users.clone();
                let client_store = client_for_event.clone();
                let sent_ids = sent_ids_for_event.clone();
                async move {
                    match event {
                        Event::PairingQrCode { code, .. } => {
                            info!("WhatsApp QR code generated (scan to pair)");
                            debug!("QR data: {code}");
                        }
                        Event::PairSuccess(_) => {
                            info!("WhatsApp pairing successful!");
                        }
                        Event::Connected(_) => {
                            info!("WhatsApp connected");
                            // Store client reference for sending.
                            *client_store.lock().await = Some(client);
                        }
                        Event::Disconnected(_) => {
                            warn!("WhatsApp disconnected");
                            *client_store.lock().await = None;
                        }
                        Event::LoggedOut(_) => {
                            warn!("WhatsApp logged out — session invalidated");
                            *client_store.lock().await = None;
                        }
                        Event::Message(msg, info) => {
                            // Only process self-chat (personal channel).
                            if !info.source.is_from_me {
                                return;
                            }
                            if info.source.sender.user != info.source.chat.user {
                                return;
                            }

                            let msg_id = info.id.clone();
                            let phone = info.source.sender.user.clone();

                            // Skip messages we sent (echo prevention).
                            if sent_ids.lock().await.remove(&msg_id) {
                                debug!("skipping own echo: {msg_id}");
                                return;
                            }

                            // Auth check.
                            if !allowed.is_empty() && !allowed.contains(&phone) {
                                warn!("ignoring whatsapp message from unauthorized {phone}");
                                return;
                            }

                            // Unwrap nested wrappers (device_sent, ephemeral, view_once).
                            let inner = msg
                                .device_sent_message
                                .as_ref()
                                .and_then(|d| d.message.as_deref())
                                .or_else(|| {
                                    msg.ephemeral_message
                                        .as_ref()
                                        .and_then(|e| e.message.as_deref())
                                })
                                .or_else(|| {
                                    msg.view_once_message
                                        .as_ref()
                                        .and_then(|v| v.message.as_deref())
                                })
                                .unwrap_or(&msg);

                            // Extract text from the (possibly unwrapped) message.
                            let text = inner
                                .conversation
                                .as_deref()
                                .or_else(|| {
                                    inner
                                        .extended_text_message
                                        .as_ref()
                                        .and_then(|e| e.text.as_deref())
                                })
                                .unwrap_or("")
                                .to_string();

                            if text.is_empty() {
                                return;
                            }

                            let chat_jid = info.source.chat.to_string();

                            let incoming = IncomingMessage {
                                id: Uuid::new_v4(),
                                channel: "whatsapp".to_string(),
                                sender_id: phone.clone(),
                                sender_name: Some(phone.clone()),
                                text,
                                timestamp: chrono::Utc::now(),
                                reply_to: None,
                                attachments: Vec::new(),
                                reply_target: Some(chat_jid),
                            };

                            if tx.send(incoming).await.is_err() {
                                info!("whatsapp channel receiver dropped");
                            }
                        }
                        _ => {}
                    }
                }
            })
            .build()
            .await
            .map_err(|e| OmegaError::Channel(format!("whatsapp bot build failed: {e}")))?;

        // Store client reference immediately if already connected.
        *client_handle.lock().await = Some(bot.client());

        // Run bot in background.
        let _handle = bot
            .run()
            .await
            .map_err(|e| OmegaError::Channel(format!("whatsapp bot run failed: {e}")))?;

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
}

// --- QR Code generation utilities ---

/// Generate a QR code as a unicode string for terminal display.
pub fn generate_qr_terminal(qr_data: &str) -> Result<String, OmegaError> {
    use qrcode::QrCode;

    let code = QrCode::new(qr_data.as_bytes())
        .map_err(|e| OmegaError::Channel(format!("QR generation failed: {e}")))?;

    let string = code
        .render::<char>()
        .quiet_zone(false)
        .module_dimensions(2, 1)
        .build();

    Ok(string)
}

/// Generate a QR code as PNG image bytes (for sending as a photo).
pub fn generate_qr_image(qr_data: &str) -> Result<Vec<u8>, OmegaError> {
    use image::{ImageBuffer, Luma};
    use qrcode::QrCode;

    let code = QrCode::new(qr_data.as_bytes())
        .map_err(|e| OmegaError::Channel(format!("QR generation failed: {e}")))?;

    let module_size: u32 = 10;
    let quiet_zone: u32 = 2;
    let modules = code.width() as u32;
    let img_size = (modules + quiet_zone * 2) * module_size;

    let img = ImageBuffer::from_fn(img_size, img_size, |x, y| {
        let mx = (x / module_size).saturating_sub(quiet_zone);
        let my = (y / module_size).saturating_sub(quiet_zone);

        if x / module_size < quiet_zone
            || y / module_size < quiet_zone
            || mx >= modules
            || my >= modules
        {
            Luma([255u8]) // White border
        } else {
            use qrcode::Color;
            match code[(mx as usize, my as usize)] {
                Color::Dark => Luma([0u8]),
                Color::Light => Luma([255u8]),
            }
        }
    });

    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png)
        .map_err(|e| OmegaError::Channel(format!("PNG encoding failed: {e}")))?;

    Ok(buf.into_inner())
}

/// Start the pairing flow: returns an mpsc receiver that yields QR data strings
/// as WhatsApp rotates QR codes periodically.
pub async fn start_pairing(
    data_dir: &str,
) -> Result<(mpsc::Receiver<String>, mpsc::Receiver<bool>), OmegaError> {
    let (qr_tx, qr_rx) = mpsc::channel::<String>(4);
    let (done_tx, done_rx) = mpsc::channel::<bool>(1);

    let dir = omega_core::config::shellexpand(data_dir);
    let session_dir = format!("{dir}/whatsapp_session");
    let _ = std::fs::create_dir_all(&session_dir);
    let db_path = format!("{session_dir}/whatsapp.db");

    let backend = Arc::new(
        SqlxWhatsAppStore::new(&db_path)
            .await
            .map_err(|e| OmegaError::Channel(format!("whatsapp store init failed: {e}")))?,
    );

    let mut bot = Bot::builder()
        .with_backend(backend)
        .with_transport_factory(TokioWebSocketTransportFactory::new())
        .with_http_client(UreqHttpClient::new())
        .on_event(move |event, _client| {
            let qr_tx = qr_tx.clone();
            let done_tx = done_tx.clone();
            async move {
                match event {
                    Event::PairingQrCode { code, .. } => {
                        let _ = qr_tx.send(code).await;
                    }
                    Event::PairSuccess(_) | Event::Connected(_) => {
                        let _ = done_tx.send(true).await;
                    }
                    _ => {}
                }
            }
        })
        .build()
        .await
        .map_err(|e| OmegaError::Channel(format!("whatsapp pairing build failed: {e}")))?;

    let _handle = bot
        .run()
        .await
        .map_err(|e| OmegaError::Channel(format!("whatsapp pairing run failed: {e}")))?;

    Ok((qr_rx, done_rx))
}

/// Split a long message into chunks that respect WhatsApp's 4096-char limit.
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
    fn test_generate_qr_terminal() {
        let result = generate_qr_terminal("test-data");
        assert!(result.is_ok());
        let qr = result.unwrap();
        assert!(!qr.is_empty());
    }

    #[test]
    fn test_generate_qr_image() {
        let result = generate_qr_image("test-data");
        assert!(result.is_ok());
        let png = result.unwrap();
        // PNG magic bytes.
        assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
    }
}
