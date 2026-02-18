# Technical Specification: WhatsApp Channel

**File:** `crates/omega-channels/src/whatsapp.rs`
**Crate:** `omega-channels`
**Module:** `whatsapp` (public)
**Status:** Implemented — pure Rust via `whatsapp-rust` crate

---

## Overview

The WhatsApp channel connects Omega to WhatsApp using the WhatsApp Web protocol (Noise handshake + Signal encryption), implemented purely in Rust via the `whatsapp-rust` crate. No external bridge process is needed. Users pair by scanning a QR code, identical to the WhatsApp Web experience.

---

## Dependencies

| Crate | Purpose |
|-------|---------|
| `whatsapp-rust` | WhatsApp Web protocol client |
| `whatsapp-rust-tokio-transport` | Tokio-based WebSocket transport |
| `whatsapp-rust-ureq-http-client` | HTTP client for WhatsApp API calls |
| `wacore` | Core types (Event, MessageSource, store traits) |
| `wacore-binary` | Jid type (`wacore_binary::jid::Jid`) |
| `waproto` | Protobuf message types |
| `sqlx` | SQLite session storage backend |
| `bincode` | Binary serialization for Device struct |
| `qrcode` | QR code generation (terminal + image) |
| `image` | PNG encoding for QR images |

---

## Configuration

```rust
pub struct WhatsAppConfig {
    pub enabled: bool,
    pub allowed_users: Vec<String>,  // phone numbers, empty = allow all
}
```

TOML:
```toml
[channel.whatsapp]
enabled = false
allowed_users = []
```

Session data stored at `{data_dir}/whatsapp_session/whatsapp.db`.

---

## Core Struct

```rust
pub struct WhatsAppChannel {
    config: WhatsAppConfig,
    data_dir: String,
    client: Arc<Mutex<Option<Arc<Client>>>>,
    sent_ids: Arc<Mutex<HashSet<String>>>,
}
```

- `client` — set during the `Connected` event and cleared on disconnect/logout.
- `sent_ids` — tracks sent message IDs to prevent echo loops in self-chat.

---

## Channel Trait Implementation

| Method | Behavior |
|--------|----------|
| `name()` | Returns `"whatsapp"` |
| `start()` | Initializes `SqliteStore`, builds `Bot` with event handler, starts bot loop. Forwards `Event::Message` to mpsc as `IncomingMessage`. |
| `send()` | Sends text via `client.send_message()` with `wa::Message { conversation: Some(text) }`. Chunks at 4096 chars. |
| `send_typing()` | Sends composing presence via `client.chatstate().send_composing()`. |
| `stop()` | Clears client reference, logs shutdown. |

---

## Event Handling

| Event | Action |
|-------|--------|
| `PairingQrCode { code, .. }` | Logs QR availability |
| `PairSuccess` | Logs success |
| `Connected` | Stores `Arc<Client>` for sending |
| `Disconnected` | Clears client reference |
| `LoggedOut` | Clears client reference, warns about invalidated session |
| `Message(msg, info)` | Self-chat filter, echo prevention, message unwrapping, text extraction, auth check, forward to gateway |

### Message Processing Pipeline

1. **Self-chat filter**: Skip if `!info.source.is_from_me` or `sender.user != chat.user`
2. **Echo prevention**: Check `sent_ids` set — if message ID matches a sent message, skip it
3. **Auth check**: Verify sender phone is in `allowed_users` (or list is empty)
4. **Unwrap wrappers**: Extract inner message from `DeviceSentMessage`, `EphemeralMessage`, or `ViewOnceMessage` containers
5. **Text extraction**: Read from `conversation` or `extended_text_message.text`
6. **Image handling**: If no text was extracted, check `inner.image_message`. If an image message is found:
   - Extract caption from `img.caption` (defaults to `"[Photo]"` if empty)
   - Acquire the client from `client_store` (lock, clone, drop)
   - Download image bytes via `wa_client.download(img.as_ref())` (`ImageMessage` implements `Downloadable`)
   - Derive file extension from `img.mimetype` (e.g., `"image/jpeg"` -> `"jpeg"`)
   - Build `Attachment { file_type: Image, data: Some(bytes), filename: Some("{uuid}.{ext}") }`
   - Set text to the caption
   - On download failure, log a warning and skip the message
7. **Empty guard**: If text is still empty and no image attachment was built, skip the message
8. **Forward**: Send `IncomingMessage` (with `attachments` populated from step 6 if applicable) to gateway via mpsc channel

---

## QR Code Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `generate_qr_terminal` | `fn(qr_data: &str) -> Result<String, OmegaError>` | Unicode QR for terminal display |
| `generate_qr_image` | `fn(qr_data: &str) -> Result<Vec<u8>, OmegaError>` | PNG bytes for sending as photo |
| `start_pairing` | `async fn(data_dir: &str) -> Result<(Receiver<String>, Receiver<bool>), OmegaError>` | Pairing flow: yields QR data strings + completion signal |

---

## Pairing Entry Points

1. **CLI (`omega init`)**: Terminal QR code, blocks until scan or timeout.
2. **Telegram `/whatsapp` command**: QR sent as image via `send_photo()`.
3. **Conversational trigger**: AI responds with `WHATSAPP_QR` marker, gateway intercepts and runs the same flow.

---

## Gateway Integration

### Auth (`check_auth`)

WhatsApp uses `allowed_users: Vec<String>` (phone numbers). Empty = allow all.

### WHATSAPP_QR Marker

The gateway extracts `WHATSAPP_QR` lines from AI responses (like `SCHEDULE:` and `LANG_SWITCH:`). When detected:
1. Calls `whatsapp::start_pairing(data_dir)`
2. Waits for QR data
3. Renders QR as PNG via `generate_qr_image()`
4. Sends image via `channel.send_photo()`
5. Waits for pairing confirmation (60s timeout)

---

## Message Types

### Incoming (WhatsApp → Gateway)

| Field | Value |
|-------|-------|
| `channel` | `"whatsapp"` |
| `sender_id` | Phone number (e.g. `"5511999887766"`) |
| `sender_name` | Phone number (profile name not always available) |
| `reply_target` | Chat JID (e.g. `"5511999887766@s.whatsapp.net"`) |
| `is_group` | `false` (WhatsApp is currently self-chat only) |
| `attachments` | Empty for text-only messages; contains `Attachment { file_type: Image, data: Some(bytes), filename: Some("{uuid}.{ext}") }` for image messages |

### Outgoing (Gateway → WhatsApp)

Text is sent as `wa::Message { conversation: Some(text) }`. Messages over 4096 chars are chunked.

---

## Session Persistence

A custom `SqlxWhatsAppStore` (`crates/omega-channels/src/whatsapp_store.rs`) implements the wacore store traits (`SignalStore`, `AppSyncStore`, `ProtocolStore`, `DeviceStore`). The session database at `{data_dir}/whatsapp_session/whatsapp.db` stores:
- Device identity (serialized with `bincode`, stored as BLOB)
- Signal protocol keys (identity, prekeys, sessions, sender keys)
- App state (hash state as JSON, sync mutations as BLOB)
- LID mappings and device lists

On restart, the bot reconnects using the persisted session without requiring a new QR scan.

### Image Download Error Handling

When an incoming image message is detected but the download fails (`wa_client.download()` returns an error), the handler logs a warning with the error details and skips the message entirely. The message is not forwarded to the gateway. This prevents partial or broken attachments from reaching the provider.

### Store Notes
- `Device` struct uses custom serde (`key_pair_serde`, `BigArray`) requiring binary serialization — `serde_json` cannot handle it; `bincode` is used instead.
- `HashState` and `DeviceListRecord` are stored as JSON TEXT (standard serde works for these).
- The `create()` method returns `Ok(1)` without inserting data — the device is populated during pairing via `save()`.
