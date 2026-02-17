use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An incoming message from a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub id: Uuid,
    /// Channel name (e.g. "telegram", "whatsapp").
    pub channel: String,
    /// Platform-specific user ID.
    pub sender_id: String,
    /// Human-readable sender name.
    pub sender_name: Option<String>,
    /// Message text content.
    pub text: String,
    pub timestamp: DateTime<Utc>,
    /// If this is a reply, the ID of the original message.
    pub reply_to: Option<Uuid>,
    pub attachments: Vec<Attachment>,
    /// Platform-specific target for routing the response (e.g. Telegram chat_id).
    #[serde(default)]
    pub reply_target: Option<String>,
    /// Whether this message comes from a group chat.
    #[serde(default)]
    pub is_group: bool,
}

/// An outgoing message to send back through a channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutgoingMessage {
    pub text: String,
    pub metadata: MessageMetadata,
    /// Platform-specific target for routing (e.g. Telegram chat_id).
    #[serde(default)]
    pub reply_target: Option<String>,
}

/// Metadata about how a message was generated.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageMetadata {
    /// Which provider produced this response.
    pub provider_used: String,
    /// Token count (if available from the provider).
    pub tokens_used: Option<u64>,
    /// Wall-clock processing time in milliseconds.
    pub processing_time_ms: u64,
    /// Model identifier (if applicable).
    pub model: Option<String>,
}

/// A file attachment on a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub file_type: AttachmentType,
    pub url: Option<String>,
    pub data: Option<Vec<u8>>,
    pub filename: Option<String>,
}

/// Supported attachment types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttachmentType {
    Image,
    Document,
    Audio,
    Video,
    Other,
}
