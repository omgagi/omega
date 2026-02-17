# Messages in Omega

## Path
`crates/omega-core/src/message.rs`

## Overview

Messages are the fundamental data unit in Omega. Every interaction -- a user typing something on Telegram, the AI responding, an attachment being sent -- is modeled as a message struct. This module defines the types that all other crates depend on for communication.

There are two core message types:

- **IncomingMessage** -- Represents something a user said. Created by a channel (Telegram, WhatsApp) and consumed by the gateway.
- **OutgoingMessage** -- Represents Omega's response. Created by a provider (Claude Code, OpenAI) and delivered back through the channel.

These two types flow through the entire system: channels produce incoming messages, the gateway orchestrates processing, providers produce outgoing messages, memory stores both, and the audit system logs them.

## What a Message Looks Like

### IncomingMessage

An incoming message captures everything Omega needs to know about what a user said and where to send the reply.

```rust
pub struct IncomingMessage {
    pub id: Uuid,                        // Unique identifier
    pub channel: String,                 // "telegram", "whatsapp", etc.
    pub sender_id: String,               // Platform user ID
    pub sender_name: Option<String>,     // Human-readable name
    pub text: String,                    // What the user said
    pub timestamp: DateTime<Utc>,        // When they said it
    pub reply_to: Option<Uuid>,          // If replying to an earlier message
    pub attachments: Vec<Attachment>,     // Files, images, etc.
    pub reply_target: Option<String>,    // Where to send the response
    pub is_group: bool,                  // Group chat flag
}
```

Most fields are self-explanatory, but a few deserve special attention:

- **`sender_id`** is a `String` rather than an integer because different platforms use different ID formats. Telegram uses numeric IDs, but other platforms might use alphanumeric strings. Keeping it as a `String` means every platform can be accommodated without conversion.

- **`reply_target`** is the key to response routing. On Telegram, this is the chat ID. When Omega sends a response, it reads this field to know which chat to deliver it to. The gateway copies this value from the incoming message to the outgoing message, so the provider never needs to know about platform-specific routing.

- **`attachments`** is always present (never `None`) but is typically an empty `Vec`. Attachment processing is planned but not yet implemented.

- **`is_group`** indicates whether the message came from a group chat. Telegram sets this to `true` for groups and supergroups; WhatsApp currently always sets it to `false` (self-chat only). The gateway uses this flag to inject group-specific behavior rules and suppress `SILENT` responses.

### OutgoingMessage

An outgoing message is simpler. It contains the response text, metadata about how the response was generated, and a routing target.

```rust
pub struct OutgoingMessage {
    pub text: String,                    // The AI's response
    pub metadata: MessageMetadata,       // How it was generated
    pub reply_target: Option<String>,    // Where to deliver it
}
```

The `metadata` field is interesting -- it tells you which provider answered, which model was used, and how long it took:

```rust
pub struct MessageMetadata {
    pub provider_used: String,           // "claude-code", "openai", etc.
    pub tokens_used: Option<u64>,        // Token count (if reported)
    pub processing_time_ms: u64,         // Wall-clock time in ms
    pub model: Option<String>,           // "claude-opus-4-6", etc.
}
```

This metadata is stored in SQLite alongside the response and logged in the audit trail, making it easy to answer questions like "which model answered this?" or "how long did that take?".

## The Lifecycle of a Message

Understanding how messages flow through Omega is key to understanding the entire architecture. Here is the full journey.

### Step 1: A User Sends a Message

A user types something on Telegram (or another platform). The Telegram channel listener receives the update and creates an `IncomingMessage`:

```rust
let incoming = IncomingMessage {
    id: Uuid::new_v4(),
    channel: "telegram".to_string(),
    sender_id: user.id.to_string(),
    sender_name: Some("Alice".to_string()),
    text: "What's the weather like?".to_string(),
    timestamp: chrono::Utc::now(),
    reply_to: None,
    attachments: Vec::new(),
    reply_target: Some(chat_id.to_string()),
    is_group: false,
};
```

The channel sends this message through an async mpsc channel to the gateway.

### Step 2: The Gateway Processes the Message

The gateway receives the `IncomingMessage` and runs it through its pipeline:

1. **Auth check** -- Is `sender_id` in the allowed list for this `channel`?
2. **Sanitization** -- Clean `text` to prevent injection attacks. The sanitized version replaces the original `text`.
3. **Command check** -- Is `text` a bot command like `/help`? If so, handle it directly and skip the provider.
4. **Context building** -- Use `channel`, `sender_id`, and `text` to build a rich context with conversation history and user facts from memory.
5. **Provider call** -- Send the context to the AI provider.

### Step 3: The Provider Responds

The provider processes the context and returns an `OutgoingMessage`:

```rust
OutgoingMessage {
    text: "I don't have access to real-time weather data...".to_string(),
    metadata: MessageMetadata {
        provider_used: "claude-code".to_string(),
        tokens_used: None,
        processing_time_ms: 3200,
        model: Some("claude-opus-4-6".to_string()),
    },
    reply_target: None,  // Provider doesn't know about platform routing
}
```

Notice that `reply_target` is `None` here. The provider is platform-agnostic -- it does not know or care whether the user is on Telegram or WhatsApp. The gateway handles routing.

### Step 4: The Gateway Routes the Response

The gateway copies the `reply_target` from the incoming message to the outgoing message:

```rust
response.reply_target = incoming.reply_target.clone();
```

This is a small but important step. It connects the platform-agnostic provider output to the platform-specific channel routing.

### Step 5: Storage and Delivery

The gateway then:

1. **Stores the exchange** in SQLite -- both the user's text (from `IncomingMessage`) and the AI's response (from `OutgoingMessage`) are saved as conversation history.
2. **Logs the audit entry** -- The full interaction, including metadata, is written to the audit table.
3. **Sends the response** -- The channel reads `reply_target` to determine the destination and delivers `text` to the user.

## How to Construct Messages

### Creating an IncomingMessage (Channel Implementors)

If you are writing a new channel (e.g., WhatsApp, Discord), you need to create `IncomingMessage` instances from platform events. Here is the pattern:

```rust
use omega_core::message::{IncomingMessage, Attachment};
use chrono::Utc;
use uuid::Uuid;

let incoming = IncomingMessage {
    id: Uuid::new_v4(),
    channel: "your-channel-name".to_string(),
    sender_id: platform_user_id.to_string(),
    sender_name: Some(display_name),
    text: message_text,
    timestamp: Utc::now(),
    reply_to: None,              // Set if this is a reply to another message
    attachments: Vec::new(),     // Populate when attachment handling is implemented
    reply_target: Some(platform_chat_id.to_string()),
    is_group: false,             // Set to true for group/supergroup chats
};
```

Key rules:
- Always generate a fresh `Uuid::new_v4()` for `id`.
- Always use `Utc::now()` for `timestamp`.
- Convert platform IDs to `String` regardless of their native type.
- Set `reply_target` to whatever identifier the channel needs to route the response back.

### Creating an OutgoingMessage (Provider Implementors)

If you are writing a new provider, you return an `OutgoingMessage` from your `complete()` implementation:

```rust
use omega_core::message::{OutgoingMessage, MessageMetadata};

Ok(OutgoingMessage {
    text: response_text,
    metadata: MessageMetadata {
        provider_used: "your-provider-name".to_string(),
        tokens_used: Some(token_count),  // or None if not available
        processing_time_ms: elapsed_ms,
        model: Some("model-name".to_string()),
    },
    reply_target: None,  // Always None -- the gateway sets this
})
```

Key rules:
- Always set `reply_target` to `None`. The gateway is responsible for routing.
- Measure `processing_time_ms` as accurately as possible (wall-clock, not CPU time).
- Set `provider_used` to a stable identifier for your provider (used in audit logs and metrics).

### Creating a Quick Text Response (Gateway Helpers)

The gateway has a `send_text()` helper for simple responses (errors, command output). It builds an `OutgoingMessage` with default metadata:

```rust
let msg = OutgoingMessage {
    text: "Some response text".to_string(),
    metadata: MessageMetadata::default(),
    reply_target: incoming.reply_target.clone(),
};
```

`MessageMetadata::default()` produces empty provider name, zero tokens, zero processing time, and no model. This is appropriate for locally generated responses that did not involve a provider call.

## How to Inspect Messages

### Reading Metadata

After a provider call, inspect the response metadata to understand what happened:

```rust
let response: OutgoingMessage = provider.complete(&context).await?;

println!("Provider: {}", response.metadata.provider_used);
println!("Time: {}ms", response.metadata.processing_time_ms);

if let Some(model) = &response.metadata.model {
    println!("Model: {}", model);
}

if let Some(tokens) = response.metadata.tokens_used {
    println!("Tokens: {}", tokens);
}
```

### Checking Reply Routing

To verify that a message will be routed correctly:

```rust
if let Some(target) = &outgoing.reply_target {
    // Message will be sent to this platform-specific target
    tracing::info!("Routing response to target: {}", target);
} else {
    // No target -- this message cannot be delivered to a channel
    tracing::warn!("OutgoingMessage has no reply_target");
}
```

## Attachments (Future)

The `Attachment` and `AttachmentType` types are defined but not yet active in the pipeline. They are designed to support file sharing in messages:

```rust
pub struct Attachment {
    pub file_type: AttachmentType,       // Image, Document, Audio, Video, Other
    pub url: Option<String>,             // Remote URL for the file
    pub data: Option<Vec<u8>>,           // Inline binary data
    pub filename: Option<String>,        // Original filename
}
```

The dual `url` / `data` design supports two common patterns:
- **URL-based** -- The platform hosts the file and provides a download URL. Omega stores the URL and fetches data on demand.
- **Inline** -- The file data is embedded directly in the message. Useful for small files or when the platform does not provide persistent URLs.

When attachment support is implemented, channels will populate the `attachments` field on `IncomingMessage`, and providers will need to handle non-text content in their `complete()` implementations.

## Design Notes

### Why Strings for IDs?

Platform user IDs (`sender_id`) and routing targets (`reply_target`) are stored as `String` rather than typed identifiers. This is intentional:

- Telegram uses `i64` user IDs and chat IDs.
- WhatsApp uses phone-number-based string IDs.
- Discord uses `u64` snowflake IDs.
- Future platforms may use UUIDs, alphanumeric tokens, or other formats.

By using `String`, the message types remain platform-agnostic. Each channel is responsible for parsing and formatting its own ID types.

### Why is reply_target on Both Structs?

The `reply_target` field appears on both `IncomingMessage` and `OutgoingMessage`. This might seem redundant, but it serves a clean separation of concerns:

- On `IncomingMessage`, it records where the message came from (set by the channel).
- On `OutgoingMessage`, it specifies where the response should go (set by the gateway).
- The provider never touches `reply_target` -- it creates responses with `reply_target: None`.
- The gateway bridges the two by copying `incoming.reply_target` to `outgoing.reply_target`.

This means providers are completely decoupled from platform routing. A provider can be tested in isolation without any channel infrastructure.

### Why Derive Default on OutgoingMessage but Not IncomingMessage?

`OutgoingMessage` derives `Default` because the gateway sometimes needs to create quick responses (error messages, command output) without a provider call. `MessageMetadata::default()` provides sensible zero values for these cases.

`IncomingMessage` does not derive `Default` because every incoming message must have a valid `id`, `channel`, `sender_id`, `text`, and `timestamp`. There is no meaningful "empty" incoming message, so the type system prevents accidental construction of one.
