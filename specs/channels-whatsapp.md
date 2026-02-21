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
    pub whisper_api_key: Option<String>,  // OpenAI API key for voice transcription
}
```

TOML:
```toml
[channel.whatsapp]
enabled = false
allowed_users = []
whisper_api_key = ""   # Optional: OpenAI key for voice transcription
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
    qr_tx: Arc<Mutex<Option<mpsc::Sender<String>>>>,
    pair_done_tx: Arc<Mutex<Option<mpsc::Sender<bool>>>>,
    last_qr: Arc<Mutex<Option<String>>>,
    msg_tx: Arc<Mutex<Option<mpsc::Sender<IncomingMessage>>>>,
}
```

- `client` — set during the `Connected` event and cleared on disconnect/logout.
- `sent_ids` — tracks sent message IDs to prevent echo loops in self-chat.
- `qr_tx` / `pair_done_tx` — optional senders for forwarding QR/pairing events from the running bot to the gateway (set by `pairing_channels()`).
- `last_qr` — buffers the last QR code data so `pairing_channels()` can replay it even if the QR event fired before the gateway started listening.
- `msg_tx` — stored message sender from `start()`, reused by `restart_for_pairing()` to build a new bot on the same message channel.

---

## Channel Trait Implementation

| Method | Behavior |
|--------|----------|
| `name()` | Returns `"whatsapp"` |
| `start()` | Initializes `SqliteStore`, builds `Bot` with event handler, starts bot loop. Forwards `Event::Message` to mpsc as `IncomingMessage`. |
| `send()` | Sanitizes markdown to WhatsApp-native formatting, sends text via `retry_send()` with `wa::Message { conversation: Some(text) }`. Chunks at 4096 chars. |
| `send_typing()` | Sends composing presence via `client.chatstate().send_composing()`. |
| `send_photo()` | Uploads image via `client.upload(MediaType::Image)`, constructs `ImageMessage` from `UploadResponse` fields, sends via `retry_send()`. |
| `stop()` | Clears client reference, logs shutdown. |

---

## Event Handling

| Event | Action |
|-------|--------|
| `PairingQrCode { code, .. }` | Buffers in `last_qr`, forwards `code` to `qr_tx` if set |
| `PairSuccess` | Logs success, sends `true` to `pair_done_tx` if set |
| `Connected` | Stores `Arc<Client>` for sending, clears `last_qr`, sends `true` to `pair_done_tx` if set |
| `Disconnected` | Clears client reference |
| `LoggedOut` | Clears client reference, warns about invalidated session |
| `Message(msg, info)` | Delegates to `handle_whatsapp_message()` — group-aware filter, echo prevention, message unwrapping, text/image/voice extraction, auth check, forward to gateway |

### Message Processing Pipeline

1. **Group filter**: If `info.source.is_group`, drop immediately with debug log. Only self-chat is processed (`is_from_me` + `sender.user == chat.user`)
2. **Echo prevention**: Check `sent_ids` set — if message ID matches a sent message, skip it
3. **Auth check**: Verify sender phone is in `allowed_users` (or list is empty)
4. **Unwrap wrappers**: Extract inner message from `DeviceSentMessage`, `EphemeralMessage`, or `ViewOnceMessage` containers
5. **Text extraction**: Read from `conversation` or `extended_text_message.text`
6. **Image handling**: Check `inner.image_message`. If an image message is found:
   - Extract caption from `img.caption` (defaults to `"[Photo]"` if empty)
   - Acquire the client from `client_store` (lock, clone, drop)
   - Download image bytes via `wa_client.download(img.as_ref())` (`ImageMessage` implements `Downloadable`)
   - Derive file extension from `img.mimetype` (e.g., `"image/jpeg"` -> `"jpeg"`)
   - Build `Attachment { file_type: Image, data: Some(bytes), filename: Some("{uuid}.{ext}") }`
   - Set text to the caption
   - On download failure, log a warning and skip the message
7. **Voice handling**: If no text or image, check `inner.audio_message`. If present and `whisper_api_key` is configured:
   - Download audio bytes via `wa_client.download(audio.as_ref())`
   - Transcribe via `crate::whisper::transcribe_whisper()` (shared with Telegram)
   - Inject as `"[Voice message] {transcript}"`
   - Skip with debug log if no whisper key
8. **Empty guard**: If text is still empty and no image/voice was processed, skip the message
9. **Sender name**: Use `info.push_name` when available, fall back to phone number
10. **Forward**: Send `IncomingMessage` (with `is_group: false`, `attachments` if applicable) to gateway via mpsc channel

---

## QR Code Functions

| Function | Signature | Purpose |
|----------|-----------|---------|
| `generate_qr_terminal` | `fn(qr_data: &str) -> Result<String, OmegaError>` | Unicode QR for terminal display |
| `generate_qr_image` | `fn(qr_data: &str) -> Result<Vec<u8>, OmegaError>` | PNG bytes for sending as photo |
| `start_pairing` | `async fn(data_dir: &str) -> Result<(Receiver<String>, Receiver<bool>), OmegaError>` | Standalone pairing flow for CLI (`omega init`) — creates a separate bot. **Not used by gateway** (see `pairing_channels()`). |

### Instance Methods

| Method | Signature | Purpose |
|--------|-----------|---------|
| `is_connected` | `async fn(&self) -> bool` | Returns `true` if the WhatsApp client is currently connected. |
| `pairing_channels` | `async fn(&self) -> (Receiver<String>, Receiver<bool>)` | Creates fresh `(qr_rx, done_rx)` receivers that forward events from the running bot. Replays buffered `last_qr` if available. Calling this replaces any previous senders. |
| `restart_for_pairing` | `async fn(&self) -> Result<(), OmegaError>` | Deletes the stale session directory, clears the client, and builds+runs a fresh bot on the same message channel. Used when WhatsApp was unlinked from the phone — the library won't generate QR codes with invalidated session keys. |

### Internal Helper

| Method | Signature | Purpose |
|--------|-----------|---------|
| `build_and_run_bot` | `async fn(&self, tx: Sender<IncomingMessage>) -> Result<(), OmegaError>` | Builds the WhatsApp bot with event handler and runs it in background. Shared by `start()` and `restart_for_pairing()`. |

---

## Pairing Entry Points

1. **CLI (`omega init`)**: Uses `start_pairing()` (creates a separate bot). Terminal QR code, blocks until scan or timeout. No conflict since the gateway isn't running.
2. **Telegram `/whatsapp` command**: Gateway downcasts to `WhatsAppChannel`, calls `pairing_channels()` to get receivers from the running bot, sends QR image via `send_photo()`. No second bot created.
3. **Conversational trigger**: AI responds with `WHATSAPP_QR` marker, gateway intercepts and runs the same `pairing_channels()` flow.

---

## Gateway Integration

### Auth (`check_auth`)

WhatsApp uses `allowed_users: Vec<String>` (phone numbers). Empty = allow all.

### WHATSAPP_QR Marker

The gateway extracts `WHATSAPP_QR` lines from AI responses (like `SCHEDULE:` and `LANG_SWITCH:`). When detected:
1. Downcasts `channels["whatsapp"]` to `WhatsAppChannel` via `as_any()`
2. If already connected → tells user, returns
3. Calls `restart_for_pairing()` — deletes stale session + builds fresh bot (generates new QR codes)
4. Calls `pairing_channels()` to get receivers from the fresh bot
5. Waits for QR data (30s timeout)
6. Renders QR as PNG via `generate_qr_image()`
7. Sends image via `channel.send_photo()`
8. Waits for pairing confirmation (60s timeout)

No second bot is created — `restart_for_pairing()` replaces the current bot in-process. This handles both first-time pairing and re-pairing after unlinking from the phone.

---

## Message Types

### Incoming (WhatsApp → Gateway)

| Field | Value |
|-------|-------|
| `channel` | `"whatsapp"` |
| `sender_id` | Phone number (e.g. `"5511999887766"`) |
| `sender_name` | `push_name` from WhatsApp profile when available, falls back to phone number |
| `reply_target` | Chat JID (e.g. `"5511999887766@s.whatsapp.net"`) |
| `is_group` | Always `false` (group messages are dropped at channel level) |
| `attachments` | Empty for text-only messages; contains `Attachment { file_type: Image, data: Some(bytes), filename: Some("{uuid}.{ext}") }` for image messages |

### Outgoing (Gateway → WhatsApp)

Text is sanitized from Markdown to WhatsApp-native formatting (`sanitize_for_whatsapp()`), then sent as `wa::Message { conversation: Some(text) }`. Messages over 4096 chars are chunked. Photos are uploaded via `client.upload(MediaType::Image)` and sent as `ImageMessage`. All sends are retried up to 3 times with exponential backoff (500ms, 1s, 2s).

### Markdown Sanitization

| Markdown | WhatsApp |
|----------|----------|
| `## Header` | `*HEADER*` (bold uppercase) |
| `**bold**` | `*bold*` |
| `[text](url)` | `text (url)` |
| `\| col \| col \|` | `- col \| col` (bullets) |
| `---` | removed |
| `*bold*`, `_italic_`, `~strike~` | passthrough (native WhatsApp) |

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
