//! WhatsApp channel — pure Rust implementation via `whatsapp-rust`.
//!
//! Uses the WhatsApp Web protocol (Noise handshake + Signal encryption).
//! Pairing is done by scanning a QR code, like WhatsApp Web.
//! Session is persisted to `{data_dir}/whatsapp_session/whatsapp.db`.

mod bot;
mod channel;
mod events;
mod qr;
mod send;

#[cfg(test)]
mod tests;

pub use qr::{generate_qr_image, generate_qr_terminal, start_pairing};

use omega_core::config::WhatsAppConfig;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

use omega_core::message::IncomingMessage;

/// WhatsApp channel using the WhatsApp Web protocol.
pub struct WhatsAppChannel {
    pub(super) config: WhatsAppConfig,
    pub(super) data_dir: String,
    /// Client handle for sending messages — set after `start()`.
    pub(super) client: Arc<Mutex<Option<Arc<whatsapp_rust::client::Client>>>>,
    /// Message IDs we sent — used to ignore our own echo in self-chat.
    pub(super) sent_ids: Arc<Mutex<HashSet<String>>>,
    /// Sender for QR code events from the running bot (for gateway-initiated pairing).
    pub(super) qr_tx: Arc<Mutex<Option<mpsc::Sender<String>>>>,
    /// Sender for pairing-done events from the running bot.
    pub(super) pair_done_tx: Arc<Mutex<Option<mpsc::Sender<bool>>>>,
    /// Last QR code data — buffered so `pairing_channels()` can replay it
    /// even if the QR event fired before the gateway started listening.
    pub(super) last_qr: Arc<Mutex<Option<String>>>,
    /// Message sender — stored so `restart_for_pairing()` can reuse it.
    pub(super) msg_tx: Arc<Mutex<Option<mpsc::Sender<IncomingMessage>>>>,
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

    /// Get the session database path.
    pub(super) fn session_db_path(&self) -> String {
        let dir = omega_core::config::shellexpand(&self.data_dir);
        let session_dir = format!("{dir}/whatsapp_session");
        // Ensure directory exists.
        let _ = std::fs::create_dir_all(&session_dir);
        format!("{session_dir}/whatsapp.db")
    }
}
