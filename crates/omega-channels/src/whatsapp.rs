//! WhatsApp channel — pure Rust implementation via `whatsapp-rust`.
//!
//! Uses the WhatsApp Web protocol (Noise handshake + Signal encryption).
//! Pairing is done by scanning a QR code, like WhatsApp Web.
//! Session is persisted to `{data_dir}/whatsapp_session/whatsapp.db`.

use async_trait::async_trait;
use omega_core::{
    config::WhatsAppConfig,
    error::OmegaError,
    message::{Attachment, AttachmentType, IncomingMessage, OutgoingMessage},
    traits::Channel,
};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};
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
    /// Sender for QR code events from the running bot (for gateway-initiated pairing).
    qr_tx: Arc<Mutex<Option<mpsc::Sender<String>>>>,
    /// Sender for pairing-done events from the running bot.
    pair_done_tx: Arc<Mutex<Option<mpsc::Sender<bool>>>>,
    /// Last QR code data — buffered so `pairing_channels()` can replay it
    /// even if the QR event fired before the gateway started listening.
    last_qr: Arc<Mutex<Option<String>>>,
    /// Message sender — stored so `restart_for_pairing()` can reuse it.
    msg_tx: Arc<Mutex<Option<mpsc::Sender<IncomingMessage>>>>,
}

impl WhatsAppChannel {
    /// Create a new WhatsApp channel from config.
    pub fn new(config: WhatsAppConfig, data_dir: &str) -> Self {
        Self {
            config,
            data_dir: data_dir.to_string(),
            client: Arc::new(Mutex::new(None)),
            sent_ids: Arc::new(Mutex::new(HashSet::new())),
            qr_tx: Arc::new(Mutex::new(None)),
            pair_done_tx: Arc::new(Mutex::new(None)),
            last_qr: Arc::new(Mutex::new(None)),
            msg_tx: Arc::new(Mutex::new(None)),
        }
    }

    /// Check if the WhatsApp client is currently connected.
    pub async fn is_connected(&self) -> bool {
        self.client.lock().await.is_some()
    }

    /// Create fresh pairing channels. Returns `(qr_rx, done_rx)` receivers
    /// that forward QR code and pairing-done events from the running bot.
    ///
    /// If a QR code was already generated before this call (e.g., during startup),
    /// it is immediately replayed into the `qr_rx` channel.
    ///
    /// Calling this replaces any previous senders (stale receivers get dropped).
    pub async fn pairing_channels(&self) -> (mpsc::Receiver<String>, mpsc::Receiver<bool>) {
        let (qr_tx, qr_rx) = mpsc::channel::<String>(4);
        let (done_tx, done_rx) = mpsc::channel::<bool>(1);

        // Replay the last buffered QR code if one exists.
        if let Some(ref qr) = *self.last_qr.lock().await {
            let _ = qr_tx.send(qr.clone()).await;
        }

        *self.qr_tx.lock().await = Some(qr_tx);
        *self.pair_done_tx.lock().await = Some(done_tx);
        (qr_rx, done_rx)
    }

    /// Delete the stale session, build a fresh bot, and run it.
    ///
    /// Used when WhatsApp was unlinked from the phone and the session is
    /// invalidated — the library won't generate new QR codes with stale keys.
    /// Deletes `{data_dir}/whatsapp_session/`, creates a fresh backend + bot,
    /// and runs it. New QR codes flow via the shared `qr_tx` / `pair_done_tx`.
    pub async fn restart_for_pairing(&self) -> Result<(), OmegaError> {
        // Delete stale session so the library starts fresh (generates QR codes).
        let dir = omega_core::config::shellexpand(&self.data_dir);
        let session_dir = format!("{dir}/whatsapp_session");
        if std::path::Path::new(&session_dir).exists() {
            info!("deleting stale WhatsApp session at {session_dir}");
            let _ = std::fs::remove_dir_all(&session_dir);
        }

        // Clear client — old bot is now orphaned.
        *self.client.lock().await = None;
        // Clear buffered QR — stale.
        *self.last_qr.lock().await = None;

        let tx = self
            .msg_tx
            .lock()
            .await
            .clone()
            .ok_or_else(|| OmegaError::Channel("WhatsApp not started yet".into()))?;

        self.build_and_run_bot(tx).await
    }

    /// Get the session database path.
    fn session_db_path(&self) -> String {
        let dir = omega_core::config::shellexpand(&self.data_dir);
        let session_dir = format!("{dir}/whatsapp_session");
        // Ensure directory exists.
        let _ = std::fs::create_dir_all(&session_dir);
        format!("{session_dir}/whatsapp.db")
    }

    /// Build a WhatsApp bot with the event handler and run it in the background.
    ///
    /// Shared by `start()` and `restart_for_pairing()`. The event handler
    /// updates the same `Arc`-wrapped fields regardless of which bot is running.
    async fn build_and_run_bot(&self, tx: mpsc::Sender<IncomingMessage>) -> Result<(), OmegaError> {
        let db_path = self.session_db_path();
        let allowed_users = self.config.allowed_users.clone();
        let client_handle = self.client.clone();

        info!("WhatsApp bot building (session: {db_path})...");

        let backend = Arc::new(
            SqlxWhatsAppStore::new(&db_path)
                .await
                .map_err(|e| OmegaError::Channel(format!("whatsapp store init failed: {e}")))?,
        );

        let tx_events = tx;
        let client_for_event = client_handle.clone();
        let sent_ids_for_event = self.sent_ids.clone();
        let whisper_api_key = self.config.whisper_api_key.clone();
        let qr_tx_handle = self.qr_tx.clone();
        let pair_done_tx_handle = self.pair_done_tx.clone();
        let last_qr_handle = self.last_qr.clone();

        let mut bot = Bot::builder()
            .with_backend(backend)
            .with_transport_factory(TokioWebSocketTransportFactory::new())
            .with_http_client(UreqHttpClient::new())
            .with_device_props(
                Some("OMEGA".to_string()),
                None,
                Some(waproto::whatsapp::device_props::PlatformType::Desktop),
            )
            .on_event(move |event, client| {
                let tx = tx_events.clone();
                let allowed = allowed_users.clone();
                let client_store = client_for_event.clone();
                let sent_ids = sent_ids_for_event.clone();
                let whisper_key = whisper_api_key.clone();
                let qr_fwd = qr_tx_handle.clone();
                let pair_done_fwd = pair_done_tx_handle.clone();
                let last_qr_buf = last_qr_handle.clone();
                async move {
                    match event {
                        Event::PairingQrCode { code, .. } => {
                            info!("WhatsApp QR code generated (scan to pair)");
                            debug!("QR data: {code}");
                            // Always buffer the latest QR code for replay.
                            *last_qr_buf.lock().await = Some(code.clone());
                            // Forward to gateway if it's listening for pairing.
                            if let Some(sender) = qr_fwd.lock().await.as_ref() {
                                let _ = sender.send(code).await;
                            }
                        }
                        Event::PairSuccess(_) => {
                            info!("WhatsApp pairing successful!");
                            // Notify gateway that pairing succeeded.
                            if let Some(sender) = pair_done_fwd.lock().await.as_ref() {
                                let _ = sender.send(true).await;
                            }
                        }
                        Event::Connected(_) => {
                            info!("WhatsApp connected");
                            // Store client reference for sending.
                            *client_store.lock().await = Some(client);
                            // Clear QR buffer — session is valid, no more QR needed.
                            *last_qr_buf.lock().await = None;
                            // Also notify gateway — Connected fires after PairSuccess.
                            if let Some(sender) = pair_done_fwd.lock().await.as_ref() {
                                let _ = sender.send(true).await;
                            }
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
                            handle_whatsapp_message(
                                *msg,
                                info,
                                &tx,
                                &allowed,
                                &client_store,
                                &sent_ids,
                                &whisper_key,
                            )
                            .await;
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

        info!("WhatsApp bot started");
        Ok(())
    }

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

// --- Message handling (extracted for reuse across bot instances) ---

/// Process an incoming WhatsApp message event.
///
/// Handles filtering (self-chat vs group, auth, echo prevention),
/// message unwrapping, image/voice downloads, and forwarding to the gateway.
async fn handle_whatsapp_message(
    msg: waproto::whatsapp::Message,
    info: wacore::types::message::MessageInfo,
    tx: &mpsc::Sender<IncomingMessage>,
    allowed: &[String],
    client_store: &Arc<Mutex<Option<Arc<Client>>>>,
    sent_ids: &Arc<Mutex<HashSet<String>>>,
    whisper_key: &Option<String>,
) {
    let is_group = info.source.is_group;

    debug!(
        "WA msg: is_group={}, is_from_me={}, sender={}, chat={}",
        is_group, info.source.is_from_me, info.source.sender.user, info.source.chat.user,
    );

    // Only process self-chat (personal messages to yourself). Drop all group messages.
    if is_group {
        debug!("WA filtered: ignoring group message");
        return;
    }
    if !info.source.is_from_me {
        return;
    }
    if info.source.sender.user != info.source.chat.user {
        debug!(
            "WA filtered: sender '{}' != chat '{}'",
            info.source.sender.user, info.source.chat.user
        );
        return;
    }

    let msg_id = info.id.clone();
    let phone = info.source.sender.user.clone();

    if sent_ids.lock().await.remove(&msg_id) {
        debug!("skipping own echo: {msg_id}");
        return;
    }

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

    let (text, attachments) = {
        if let Some(ref img) = inner.image_message {
            let caption = img.caption.as_deref().unwrap_or("[Photo]").to_string();
            let wa_client = { client_store.lock().await.clone() };
            if let Some(wa_client) = wa_client {
                match wa_client.download(img.as_ref()).await {
                    Ok(bytes) => {
                        let ext = img
                            .mimetype
                            .as_deref()
                            .and_then(|m| m.split('/').nth(1))
                            .unwrap_or("jpg");
                        let filename = format!("{}.{ext}", Uuid::new_v4());
                        let attachment = Attachment {
                            file_type: AttachmentType::Image,
                            url: None,
                            data: Some(bytes),
                            filename: Some(filename),
                        };
                        info!("downloaded whatsapp image");
                        (caption, vec![attachment])
                    }
                    Err(e) => {
                        warn!("whatsapp image download failed: {e}");
                        return;
                    }
                }
            } else {
                warn!("whatsapp client not available for image download");
                return;
            }
        } else if let Some(ref audio) = inner.audio_message {
            match whisper_key.as_deref() {
                Some(key) if !key.is_empty() => {
                    let wa_client = { client_store.lock().await.clone() };
                    if let Some(wa_client) = wa_client {
                        match wa_client.download(audio.as_ref()).await {
                            Ok(bytes) => {
                                let http = reqwest::Client::new();
                                match crate::whisper::transcribe_whisper(&http, key, &bytes).await {
                                    Ok(transcript) => {
                                        let secs = audio.seconds.unwrap_or(0);
                                        info!("transcribed whatsapp voice ({secs}s)");
                                        (format!("[Voice message] {transcript}"), Vec::new())
                                    }
                                    Err(e) => {
                                        warn!("whatsapp voice transcription failed: {e}");
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("whatsapp audio download failed: {e}");
                                return;
                            }
                        }
                    } else {
                        warn!("whatsapp client not available for audio download");
                        return;
                    }
                }
                _ => {
                    debug!("skipping whatsapp voice (no whisper key)");
                    return;
                }
            }
        } else if text.is_empty() {
            return;
        } else {
            (text, Vec::new())
        }
    };

    let chat_jid = info.source.chat.to_string();
    let sender_name = if info.push_name.is_empty() {
        phone.clone()
    } else {
        info.push_name.clone()
    };

    let incoming = IncomingMessage {
        id: Uuid::new_v4(),
        channel: "whatsapp".to_string(),
        sender_id: phone.clone(),
        sender_name: Some(sender_name),
        text,
        timestamp: chrono::Utc::now(),
        reply_to: None,
        attachments,
        reply_target: Some(chat_jid),
        is_group: false,
    };

    if tx.send(incoming).await.is_err() {
        info!("whatsapp channel receiver dropped");
    }
}

// --- QR Code generation utilities ---

/// Generate a compact QR code for terminal display using Unicode half-block characters.
///
/// Packs two rows of modules into one line of text using `▀`, `▄`, `█`, and space.
/// This produces a QR code roughly half the height of a naive renderer.
pub fn generate_qr_terminal(qr_data: &str) -> Result<String, OmegaError> {
    use qrcode::{Color, EcLevel, QrCode};

    let code = QrCode::with_error_correction_level(qr_data.as_bytes(), EcLevel::L)
        .map_err(|e| OmegaError::Channel(format!("QR generation failed: {e}")))?;

    let width = code.width();
    let colors: Vec<Color> = code.into_colors();
    let is_dark = |row: usize, col: usize| -> bool {
        if row < width && col < width {
            colors[row * width + col] == Color::Dark
        } else {
            false
        }
    };

    let mut out = String::new();
    // Process two rows at a time.
    let mut row = 0;
    while row < width {
        for col in 0..width {
            let top = is_dark(row, col);
            let bottom = if row + 1 < width {
                is_dark(row + 1, col)
            } else {
                false
            };
            out.push(match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        out.push('\n');
        row += 2;
    }

    Ok(out)
}

/// Generate a QR code as PNG image bytes (for sending as a photo).
pub fn generate_qr_image(qr_data: &str) -> Result<Vec<u8>, OmegaError> {
    use image::{ImageBuffer, Luma};
    use qrcode::{EcLevel, QrCode};

    let code = QrCode::with_error_correction_level(qr_data.as_bytes(), EcLevel::L)
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
        .with_device_props(
            Some("OMEGA".to_string()),
            None,
            Some(waproto::whatsapp::device_props::PlatformType::Desktop),
        )
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

/// Retry delays for exponential backoff: 500ms, 1s, 2s.
const RETRY_DELAYS_MS: [u64; 3] = [500, 1000, 2000];

/// Send a WhatsApp message with retry and exponential backoff.
///
/// Attempts up to 3 times with delays of 500ms, 1s, 2s between retries.
/// Clones the message for each retry attempt.
async fn retry_send(
    client: &Client,
    jid: &Jid,
    msg: waproto::whatsapp::Message,
) -> Result<String, OmegaError> {
    let mut last_err = None;

    for (attempt, delay_ms) in RETRY_DELAYS_MS.iter().enumerate() {
        match client.send_message(jid.clone(), msg.clone()).await {
            Ok(msg_id) => return Ok(msg_id),
            Err(e) => {
                let attempt_num = attempt + 1;
                if attempt_num < RETRY_DELAYS_MS.len() {
                    warn!(
                        "whatsapp send attempt {attempt_num}/{} failed: {e}, retrying in {delay_ms}ms",
                        RETRY_DELAYS_MS.len()
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
                } else {
                    error!(
                        "whatsapp send attempt {attempt_num}/{} failed: {e}, giving up",
                        RETRY_DELAYS_MS.len()
                    );
                }
                last_err = Some(e);
            }
        }
    }

    Err(OmegaError::Channel(format!(
        "whatsapp send failed after {} attempts: {}",
        RETRY_DELAYS_MS.len(),
        last_err.map(|e| e.to_string()).unwrap_or_default()
    )))
}

/// Convert Markdown formatting to WhatsApp-native formatting.
///
/// - `## Header` → `*HEADER*` (bold uppercase)
/// - `**bold**` → `*bold*`
/// - `[text](url)` → `text (url)`
/// - `| col | col |` table rows → `- col | col` bullets
/// - `---` horizontal rules → removed
fn sanitize_for_whatsapp(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for line in text.lines() {
        let trimmed = line.trim();

        // Remove horizontal rules.
        if trimmed.chars().all(|c| c == '-' || c == ' ') && trimmed.matches('-').count() >= 3 {
            continue;
        }

        // Convert markdown headers to bold uppercase.
        if let Some(header) = trimmed.strip_prefix("### ") {
            out.push_str(&format!("*{}*", header.trim().to_uppercase()));
            out.push('\n');
            continue;
        }
        if let Some(header) = trimmed.strip_prefix("## ") {
            out.push_str(&format!("*{}*", header.trim().to_uppercase()));
            out.push('\n');
            continue;
        }
        if let Some(header) = trimmed.strip_prefix("# ") {
            out.push_str(&format!("*{}*", header.trim().to_uppercase()));
            out.push('\n');
            continue;
        }

        // Convert table rows (skip separator rows like |---|---|).
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let inner = &trimmed[1..trimmed.len() - 1];
            // Skip separator rows.
            if inner
                .chars()
                .all(|c| c == '-' || c == '|' || c == ' ' || c == ':')
            {
                continue;
            }
            let cols: Vec<&str> = inner.split('|').map(|s| s.trim()).collect();
            out.push_str("- ");
            out.push_str(&cols.join(" | "));
            out.push('\n');
            continue;
        }

        let mut result = line.to_string();

        // Convert markdown links: [text](url) → text (url)
        while let Some(start_bracket) = result.find('[') {
            if let Some(end_bracket) = result[start_bracket..].find("](") {
                let abs_end_bracket = start_bracket + end_bracket;
                if let Some(end_paren) = result[abs_end_bracket + 2..].find(')') {
                    let abs_end_paren = abs_end_bracket + 2 + end_paren;
                    let link_text = &result[start_bracket + 1..abs_end_bracket];
                    let url = &result[abs_end_bracket + 2..abs_end_paren];
                    let replacement = format!("{link_text} ({url})");
                    result.replace_range(start_bracket..=abs_end_paren, &replacement);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Convert **bold** to *bold* (WhatsApp native).
        while let Some(start_pos) = result.find("**") {
            if let Some(end_pos) = result[start_pos + 2..].find("**") {
                let abs_end = start_pos + 2 + end_pos;
                let inner_text = result[start_pos + 2..abs_end].to_string();
                result.replace_range(start_pos..abs_end + 2, &format!("*{inner_text}*"));
            } else {
                break;
            }
        }

        out.push_str(&result);
        out.push('\n');
    }

    // Remove trailing newline if the original didn't have one.
    if !text.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    out
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
    use wacore_binary::jid::JidExt;

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
    fn test_jid_group_detection() {
        // Group JIDs use @g.us server.
        let group_jid: Jid = "120363001234567890@g.us".parse().unwrap();
        assert!(group_jid.is_group(), "g.us JID should be detected as group");

        // Personal JIDs use @s.whatsapp.net server.
        let personal_jid: Jid = "5511999887766@s.whatsapp.net".parse().unwrap();
        assert!(
            !personal_jid.is_group(),
            "s.whatsapp.net JID should not be group"
        );
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

    #[test]
    fn test_sanitize_headers() {
        assert_eq!(sanitize_for_whatsapp("## Hello World"), "*HELLO WORLD*");
        assert_eq!(sanitize_for_whatsapp("# Big Title"), "*BIG TITLE*");
        assert_eq!(sanitize_for_whatsapp("### Small"), "*SMALL*");
    }

    #[test]
    fn test_sanitize_bold() {
        assert_eq!(
            sanitize_for_whatsapp("this is **bold** text"),
            "this is *bold* text"
        );
    }

    #[test]
    fn test_sanitize_links() {
        assert_eq!(
            sanitize_for_whatsapp("check [Google](https://google.com) out"),
            "check Google (https://google.com) out"
        );
    }

    #[test]
    fn test_sanitize_tables() {
        let input = "| Name | Age |\n|------|-----|\n| Alice | 30 |";
        let result = sanitize_for_whatsapp(input);
        assert!(result.contains("- Name | Age"), "should convert header row");
        assert!(result.contains("- Alice | 30"), "should convert data row");
        assert!(!result.contains("------"), "should remove separator row");
    }

    #[test]
    fn test_sanitize_horizontal_rules() {
        let input = "above\n---\nbelow";
        let result = sanitize_for_whatsapp(input);
        assert_eq!(result, "above\nbelow");
    }

    #[test]
    fn test_sanitize_passthrough() {
        // Native WhatsApp formatting should pass through unchanged.
        assert_eq!(sanitize_for_whatsapp("*bold*"), "*bold*");
        assert_eq!(sanitize_for_whatsapp("_italic_"), "_italic_");
        assert_eq!(sanitize_for_whatsapp("~strike~"), "~strike~");
        assert_eq!(sanitize_for_whatsapp("```code```"), "```code```");
    }

    #[test]
    fn test_sanitize_preserves_plain_text() {
        let plain = "Hello, how are you doing today?";
        assert_eq!(sanitize_for_whatsapp(plain), plain);
    }

    #[test]
    fn test_retry_delays_exponential() {
        assert_eq!(RETRY_DELAYS_MS.len(), 3, "should have 3 retry attempts");
        assert_eq!(RETRY_DELAYS_MS[0], 500, "first delay 500ms");
        assert_eq!(RETRY_DELAYS_MS[1], 1000, "second delay 1s");
        assert_eq!(RETRY_DELAYS_MS[2], 2000, "third delay 2s");
        // Verify exponential pattern: each delay is 2x the previous.
        assert_eq!(RETRY_DELAYS_MS[1], RETRY_DELAYS_MS[0] * 2);
        assert_eq!(RETRY_DELAYS_MS[2], RETRY_DELAYS_MS[1] * 2);
    }
}
