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
| `wacore` | Core types (Event, Jid, Message) |
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
}
```

The `client` field is set during the `Connected` event and cleared on disconnect/logout.

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
| `Message(msg, info)` | Extracts text from `conversation` or `extended_text_message.text`, applies auth filter, forwards to gateway |

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

### Outgoing (Gateway → WhatsApp)

Text is sent as `wa::Message { conversation: Some(text) }`. Messages over 4096 chars are chunked.

---

## Session Persistence

The `whatsapp-rust` crate's `SqliteStore` handles session persistence automatically. The session database at `{data_dir}/whatsapp_session/whatsapp.db` stores:
- Device identity keys
- Prekey bundles
- Session state
- App state

On restart, the bot reconnects using the persisted session without requiring a new QR scan.
