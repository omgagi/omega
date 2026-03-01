//! Bot lifecycle — building, running, and restarting the WhatsApp bot.

use super::events::handle_whatsapp_message;
use super::WhatsAppChannel;
use crate::whatsapp_store::SqlxWhatsAppStore;
use omega_core::{error::OmegaError, message::IncomingMessage};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{info, warn};
use wacore::types::events::Event;
use whatsapp_rust::bot::Bot;
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;

impl WhatsAppChannel {
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

    /// Build a WhatsApp bot with the event handler and run it in the background.
    ///
    /// Shared by `start()` and `restart_for_pairing()`. The event handler
    /// updates the same `Arc`-wrapped fields regardless of which bot is running.
    pub(super) async fn build_and_run_bot(
        &self,
        tx: mpsc::Sender<IncomingMessage>,
    ) -> Result<(), OmegaError> {
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
        let http_client = reqwest::Client::new();
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
                let http = http_client.clone();
                let qr_fwd = qr_tx_handle.clone();
                let pair_done_fwd = pair_done_tx_handle.clone();
                let last_qr_buf = last_qr_handle.clone();
                async move {
                    match event {
                        Event::PairingQrCode { code, .. } => {
                            info!("WhatsApp QR code generated (scan to pair)");
                            tracing::debug!("QR data: {code}");
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
                                &http,
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
}
