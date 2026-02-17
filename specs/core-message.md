# Specification: omega-core/src/message.rs

## Path
`crates/omega-core/src/message.rs`

## Purpose
Defines the canonical message types used throughout the Omega pipeline. Every message that enters the system (from a channel) is represented as an `IncomingMessage`, and every response that leaves the system (from a provider) is represented as an `OutgoingMessage`. These two structs, together with `MessageMetadata`, `Attachment`, and `AttachmentType`, form the core data contract between channels, the gateway, memory, providers, and the audit system.

## Module Location
The module is declared in `omega-core/src/lib.rs` as `pub mod message` and re-exported as `omega_core::message`. All other crates in the workspace import message types through `omega_core::message::*`.

## Dependencies

### External Crates
| Crate | Usage |
|-------|-------|
| `chrono` | `DateTime<Utc>` for message timestamps |
| `serde` | `Serialize`, `Deserialize` derives for all types |
| `uuid` | `Uuid` for unique message identifiers |

## Data Structures

### IncomingMessage

```rust
/// An incoming message from a channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncomingMessage {
    pub id: Uuid,
    pub channel: String,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub text: String,
    pub timestamp: DateTime<Utc>,
    pub reply_to: Option<Uuid>,
    pub attachments: Vec<Attachment>,
    #[serde(default)]
    pub reply_target: Option<String>,
    /// Whether this message comes from a group chat.
    #[serde(default)]
    pub is_group: bool,
}
```

**Derive Macros:** `Debug`, `Clone`, `Serialize`, `Deserialize`

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | `Uuid` | Yes | Unique identifier for this message, generated at the channel level via `Uuid::new_v4()`. |
| `channel` | `String` | Yes | Name of the originating channel (e.g. `"telegram"`, `"whatsapp"`). Used for routing responses and auth checks. |
| `sender_id` | `String` | Yes | Platform-specific user identifier. Stored as `String` to accommodate different platform ID formats (Telegram uses numeric i64, others may use alphanumeric). |
| `sender_name` | `Option<String>` | No | Human-readable sender name. Populated from platform data (e.g. Telegram first_name + last_name). Used in audit logs and context building. |
| `text` | `String` | Yes | The message text content. This is the raw user input before sanitization. |
| `timestamp` | `DateTime<Utc>` | Yes | UTC timestamp of when the message was received. Generated at the channel level via `chrono::Utc::now()`. |
| `reply_to` | `Option<Uuid>` | No | If this message is a reply to a previous message, the UUID of that original message. Currently unused by channels but reserved for threading support. |
| `attachments` | `Vec<Attachment>` | Yes (empty default) | List of file attachments. Currently channels send `Vec::new()` as attachment handling is not yet implemented. |
| `reply_target` | `Option<String>` | No | Platform-specific routing target for sending the response back. For Telegram, this is the `chat_id` as a string. Annotated with `#[serde(default)]` to default to `None` during deserialization. |
| `is_group` | `bool` | Yes (default `false`) | Whether this message comes from a group chat (e.g., Telegram group/supergroup). Set by the channel during message construction. Used by the gateway to inject group-chat rules and suppress `SILENT` responses. Annotated with `#[serde(default)]` to default to `false`. |

**Serde Annotations:**
- `reply_target` uses `#[serde(default)]` to handle missing field during deserialization.
- `is_group` uses `#[serde(default)]` to default to `false` during deserialization.

---

### OutgoingMessage

```rust
/// An outgoing message to send back through a channel.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OutgoingMessage {
    pub text: String,
    pub metadata: MessageMetadata,
    #[serde(default)]
    pub reply_target: Option<String>,
}
```

**Derive Macros:** `Debug`, `Clone`, `Default`, `Serialize`, `Deserialize`

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `text` | `String` | Yes | The response text to send to the user. Populated by the provider or by command handlers. |
| `metadata` | `MessageMetadata` | Yes | Metadata about how this response was generated (provider name, model, timing, tokens). Defaults to empty metadata via `MessageMetadata::default()`. |
| `reply_target` | `Option<String>` | No | Platform-specific routing target, copied from `IncomingMessage.reply_target` by the gateway after the provider returns. Annotated with `#[serde(default)]`. |

**Serde Annotations:**
- `reply_target` uses `#[serde(default)]` to handle missing field during deserialization.

**Default Implementation:**
The struct derives `Default`, producing:
- `text`: `""` (empty string)
- `metadata`: `MessageMetadata::default()`
- `reply_target`: `None`

---

### MessageMetadata

```rust
/// Metadata about how a message was generated.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MessageMetadata {
    pub provider_used: String,
    pub tokens_used: Option<u64>,
    pub processing_time_ms: u64,
    pub model: Option<String>,
}
```

**Derive Macros:** `Debug`, `Clone`, `Serialize`, `Deserialize`, `Default`

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `provider_used` | `String` | Yes | Identifier of the provider that generated the response (e.g. `"claude-code"`). Defaults to `""`. |
| `tokens_used` | `Option<u64>` | No | Token count consumed by the provider, if the provider reports it. Claude Code CLI does not currently report tokens, so this is typically `None`. |
| `processing_time_ms` | `u64` | Yes | Wall-clock time in milliseconds from sending the request to receiving the response. Measured by the provider implementation. Defaults to `0`. |
| `model` | `Option<String>` | No | Model identifier used by the provider (e.g. `"claude-opus-4-6"`). `None` if the provider does not report a model name. |

**Default Implementation:**
- `provider_used`: `""` (empty string)
- `tokens_used`: `None`
- `processing_time_ms`: `0`
- `model`: `None`

---

### Attachment

```rust
/// A file attachment on a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attachment {
    pub file_type: AttachmentType,
    pub url: Option<String>,
    pub data: Option<Vec<u8>>,
    pub filename: Option<String>,
}
```

**Derive Macros:** `Debug`, `Clone`, `Serialize`, `Deserialize`

**Fields:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `file_type` | `AttachmentType` | Yes | The type of the attachment (image, document, audio, video, or other). |
| `url` | `Option<String>` | No | A URL where the attachment can be downloaded. Used for platform-hosted files. |
| `data` | `Option<Vec<u8>>` | No | Raw binary data of the attachment. Used when the file is embedded rather than hosted. |
| `filename` | `Option<String>` | No | Original filename of the attachment, if known. |

**Current Status:** Attachments are defined but not yet used by any channel. Telegram and WhatsApp channels currently send `Vec::new()` for the attachments field.

---

### AttachmentType

```rust
/// Supported attachment types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AttachmentType {
    Image,
    Document,
    Audio,
    Video,
    Other,
}
```

**Derive Macros:** `Debug`, `Clone`, `Serialize`, `Deserialize`

**Variants:**

| Variant | Description |
|---------|-------------|
| `Image` | Image files (PNG, JPEG, GIF, etc.) |
| `Document` | Document files (PDF, DOCX, TXT, etc.) |
| `Audio` | Audio files (MP3, OGG, WAV, etc.) |
| `Video` | Video files (MP4, AVI, etc.) |
| `Other` | Any file type not covered above |

## Message Flow Through the System

### Incoming Message Flow

```
[Platform User] → [Channel (e.g. Telegram)]
                         ↓
                  IncomingMessage created
                  (id=Uuid::new_v4(), channel="telegram",
                   sender_id=user.id, text=msg.text,
                   timestamp=Utc::now(), reply_target=chat_id)
                         ↓
                  [Channel mpsc::Sender] → [Gateway mpsc::Receiver]
                         ↓
                  [Gateway::handle_message()]
                         ↓
          ┌──────────────┼──────────────┐
          ↓              ↓              ↓
     [Auth Check]   [Sanitize]    [Command?]
     uses sender_id  replaces text  reads text
     + channel
          ↓              ↓
     [Memory::build_context()]
     uses channel, sender_id, text
          ↓
     [Provider::complete()]
     receives Context built from IncomingMessage
          ↓
     [Memory::store_exchange()]
     persists incoming.text as "user" role
```

### Outgoing Message Flow

```
[Provider::complete()]
         ↓
  OutgoingMessage created
  (text=response, metadata={provider, model, time},
   reply_target=None)
         ↓
  [Gateway] sets reply_target from incoming.reply_target
         ↓
  [Memory::store_exchange()]
  persists response.text as "assistant" role
  serializes response.metadata as JSON
         ↓
  [AuditLogger::log()]
  records metadata.provider_used, metadata.model,
  metadata.processing_time_ms
         ↓
  [Channel::send()]
  reads reply_target to route the message
  reads text to send to the user
```

### Reply Target Routing

The `reply_target` field is the mechanism for routing responses back to the correct chat/conversation on the platform:

1. **Channel creates IncomingMessage** with `reply_target = Some(chat_id)`.
2. **Provider creates OutgoingMessage** with `reply_target = None` (provider is platform-agnostic).
3. **Gateway copies** `incoming.reply_target` into `outgoing.reply_target` after the provider returns.
4. **Channel reads** `outgoing.reply_target` to determine where to send the response.

For Telegram, `reply_target` is the chat ID as a string (parsed back to `i64` by the Telegram channel).

## Cross-Crate Usage

### Consumers

| Crate | Module | Usage |
|-------|--------|-------|
| `omega-channels` | `telegram.rs` | Creates `IncomingMessage` from Telegram updates; consumes `OutgoingMessage` in `send()`. |
| `omega-providers` | `claude_code.rs` | Creates `OutgoingMessage` (with `MessageMetadata`) from provider response. |
| `omega-memory` | `store.rs` | Reads `IncomingMessage` in `build_context()` and `store_exchange()`. Reads `OutgoingMessage` in `store_exchange()`. |
| `omega` (root) | `gateway.rs` | Receives `IncomingMessage` from channels, passes to pipeline, sends `OutgoingMessage` to channels. Creates `OutgoingMessage` for error and command responses via `send_text()`. |

### Trait Integration

The message types are embedded in the core traits:

```rust
// Provider trait — produces OutgoingMessage
async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError>;

// Channel trait — produces IncomingMessage, consumes OutgoingMessage
async fn start(&self) -> Result<mpsc::Receiver<IncomingMessage>, OmegaError>;
async fn send(&self, message: OutgoingMessage) -> Result<(), OmegaError>;
```

## Serialization

All types implement `Serialize` and `Deserialize` via serde derives. This enables:

1. **MPSC transport** -- Messages are cloned across async task boundaries (using `Clone`, not serde).
2. **Metadata persistence** -- `MessageMetadata` is serialized to JSON via `serde_json::to_string()` and stored in SQLite `metadata_json` column.
3. **Potential future use** -- HTTP API transport, file-based persistence, or inter-process communication.

## Invariants

1. Every `IncomingMessage` has a unique `id` (UUID v4).
2. Every `IncomingMessage` has a non-empty `channel` and `sender_id`.
3. `OutgoingMessage.reply_target` is `None` when created by a provider; the gateway is responsible for setting it from the corresponding `IncomingMessage.reply_target`.
4. `MessageMetadata.provider_used` is set by the provider implementation, never by the gateway.
5. `attachments` is always present (may be empty) on `IncomingMessage`.
6. All timestamp values are UTC.
7. `is_group` defaults to `false` and is only set to `true` by channels that support group chat detection (currently Telegram).
