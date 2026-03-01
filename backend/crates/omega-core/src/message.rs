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
    /// Origin identifier for webhook-injected messages.
    /// None for channel-originated messages, Some("source_name") for webhooks.
    #[serde(default)]
    pub source: Option<String>,
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
    /// Session ID returned by the provider (Claude Code CLI only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that `#[serde(default)]` fields get their defaults when omitted from JSON.
    #[test]
    fn test_incoming_message_serde_defaults() {
        let json = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000000",
            "channel": "telegram",
            "sender_id": "123",
            "sender_name": null,
            "text": "hello",
            "timestamp": "2026-01-01T00:00:00Z",
            "reply_to": null,
            "attachments": []
        });
        let msg: IncomingMessage = serde_json::from_value(json).unwrap();
        assert_eq!(msg.channel, "telegram");
        assert_eq!(msg.sender_id, "123");
        assert_eq!(msg.text, "hello");
        // serde(default) fields should be None / false when omitted
        assert!(
            msg.reply_target.is_none(),
            "reply_target should default to None"
        );
        assert!(!msg.is_group, "is_group should default to false");
        assert!(msg.source.is_none(), "source should default to None");
    }

    #[test]
    fn test_outgoing_message_construction() {
        let msg = OutgoingMessage {
            text: "response".to_string(),
            metadata: MessageMetadata {
                provider_used: "claude-code".to_string(),
                tokens_used: Some(42),
                processing_time_ms: 150,
                model: Some("sonnet".to_string()),
                session_id: None,
            },
            reply_target: Some("chat_123".to_string()),
        };
        assert_eq!(msg.text, "response");
        assert_eq!(msg.metadata.provider_used, "claude-code");
        assert_eq!(msg.metadata.tokens_used, Some(42));
        assert_eq!(msg.metadata.processing_time_ms, 150);
        assert_eq!(msg.metadata.model.as_deref(), Some("sonnet"));
        assert!(msg.metadata.session_id.is_none());
        assert_eq!(msg.reply_target.as_deref(), Some("chat_123"));
    }

    #[test]
    fn test_message_metadata_defaults() {
        let meta = MessageMetadata::default();
        assert_eq!(meta.provider_used, "");
        assert!(meta.tokens_used.is_none());
        assert_eq!(meta.processing_time_ms, 0);
        assert!(meta.model.is_none());
        assert!(meta.session_id.is_none());
    }
}
