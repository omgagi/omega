//! Incoming WhatsApp message handling — filtering, unwrapping, and forwarding.

use omega_core::message::{Attachment, AttachmentType, IncomingMessage};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, info, warn};
use uuid::Uuid;
use whatsapp_rust::client::Client;

/// Process an incoming WhatsApp message event.
///
/// Handles filtering (self-chat vs group, auth, echo prevention),
/// message unwrapping, image/voice downloads, and forwarding to the gateway.
pub(super) async fn handle_whatsapp_message(
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
