# Technical Specification: backend/crates/omega-channels/src/lib.rs

## File

| Field | Value |
|-------|-------|
| **Path** | `backend/crates/omega-channels/src/lib.rs` |
| **Crate** | `omega-channels` |
| **Role** | Crate root -- declares submodules and controls public API surface |

## Purpose

This file is the entry point for the `omega-channels` crate. It serves two purposes:

1. Declare the submodules that implement individual messaging platform integrations.
2. Control which modules are publicly re-exported to downstream crates (`omega-core`, the gateway, the binary).

## Module Declarations

| Module | Visibility | Status | Description |
|--------|-----------|--------|-------------|
| `telegram` | `pub mod` | Implemented | Telegram Bot API channel using long-polling (`getUpdates`), `sendMessage`, `sendPhoto`, and `sendChatAction`. Directory module with 5 files: `mod.rs`, `polling.rs`, `send.rs`, `types.rs`, `tests.rs`. |
| `whatsapp` | `pub mod` | Implemented | WhatsApp Web protocol channel via `whatsapp-rust`. Directory module with 6 files: `mod.rs`, `bot.rs`, `channel.rs`, `events.rs`, `qr.rs`, `send.rs`, `tests.rs`. Supports text, image, voice (Whisper), self-chat mode, markdown sanitization, send retry with backoff, and QR-based pairing. |
| `whatsapp_store` | `pub mod` | Implemented | SQLite-based session persistence for WhatsApp (Signal protocol keys, device identity). Directory module with 5 files. |
| `whisper` | `pub mod` | Implemented | Shared OpenAI Whisper transcription module used by both Telegram and WhatsApp channels. |
| `utils` | `pub mod` | Implemented | Shared utilities: `split_message()` for platform-agnostic message chunking with UTF-8 safety. |

## Re-exports

The file does **not** contain any explicit `pub use` re-exports. Public access to channel implementations is provided solely through module visibility:

| Symbol | Access Path | Notes |
|--------|-------------|-------|
| `TelegramChannel` | `omega_channels::telegram::TelegramChannel` | Public struct; implements `omega_core::traits::Channel`. |
| `WhatsAppChannel` | `omega_channels::whatsapp::WhatsAppChannel` | Public struct; implements `omega_core::traits::Channel`. |
| `generate_qr_terminal` | `omega_channels::whatsapp::generate_qr_terminal` | Public function; generates terminal-friendly QR code. |
| `generate_qr_image` | `omega_channels::whatsapp::generate_qr_image` | Public function; generates QR code as PNG bytes. |
| `start_pairing` | `omega_channels::whatsapp::start_pairing` | Public async function; starts standalone WhatsApp pairing flow. |
| `split_message` | `omega_channels::utils::split_message` | Public function; splits messages respecting char boundaries. |

All modules are `pub mod`, making their public types accessible from outside the crate.

## Feature Gates

There are **no** feature gates defined in `lib.rs` or in the crate's `Cargo.toml`. All modules are compiled unconditionally.

## Dependencies (from Cargo.toml)

| Dependency | Usage |
|------------|-------|
| `omega-core` | Provides `Channel` trait, `IncomingMessage`, `OutgoingMessage`, `OmegaError`, `TelegramConfig`, `WhatsAppConfig`, `shellexpand` |
| `tokio` | Async runtime, `mpsc` channels, `Mutex`, `sleep` |
| `serde` / `serde_json` | Deserializing Telegram/WhatsApp API JSON responses, building JSON request bodies |
| `tracing` | Structured logging (`info!`, `warn!`, `error!`, `debug!`) |
| `thiserror` | Declared as dependency but error types come from `omega-core` |
| `anyhow` | Declared as dependency; not directly used in current code |
| `async-trait` | `#[async_trait]` on the `Channel` impl for both Telegram and WhatsApp |
| `reqwest` | HTTP client for Telegram Bot API and OpenAI Whisper API |
| `uuid` | Generating unique `IncomingMessage.id` values and filenames |
| `chrono` | Timestamping incoming messages with `Utc::now()` |
| `whatsapp-rust` | WhatsApp Web protocol client (`Bot`, `Client`) |
| `whatsapp-rust-tokio-transport` | Tokio-based WebSocket transport for WhatsApp |
| `whatsapp-rust-ureq-http-client` | HTTP client adapter for WhatsApp media uploads |
| `wacore` / `wacore-binary` / `waproto` | WhatsApp protocol types, JID handling, message structures |
| `sqlx` | SQLite-backed WhatsApp session store |
| `qrcode` | QR code generation for WhatsApp pairing |
| `image` | PNG encoding for QR code images |

## Public API Surface Summary

### Telegram Channel

| Item | Kind | Module | Description |
|------|------|--------|-------------|
| `telegram` | module | crate root | Public module containing the Telegram integration |
| `TelegramChannel` | struct | `telegram` | Implements `Channel` trait for Telegram Bot API |
| `TelegramChannel::new(config: TelegramConfig) -> Self` | constructor | `telegram` | Constructor |

### Telegram -- Channel Trait Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `name` | `fn name(&self) -> &str` | Returns `"telegram"` |
| `start` | `async fn start(&self) -> Result<mpsc::Receiver<IncomingMessage>, OmegaError>` | Spawns long-polling task, returns message receiver |
| `send` | `async fn send(&self, message: OutgoingMessage) -> Result<(), OmegaError>` | Sends an outgoing message to the target chat |
| `send_typing` | `async fn send_typing(&self, target: &str) -> Result<(), OmegaError>` | Sends a "typing" chat action indicator |
| `send_photo` | `async fn send_photo(&self, target: &str, image: &[u8], caption: &str) -> Result<(), OmegaError>` | Sends a photo with caption via `sendPhoto` API |
| `stop` | `async fn stop(&self) -> Result<(), OmegaError>` | Logs shutdown; no-op cleanup |
| `as_any` | `fn as_any(&self) -> &dyn std::any::Any` | Returns self for downcasting |

### WhatsApp Channel

| Item | Kind | Module | Description |
|------|------|--------|-------------|
| `whatsapp` | module | crate root | Public module containing the WhatsApp integration |
| `WhatsAppChannel` | struct | `whatsapp` | Implements `Channel` trait for WhatsApp Web protocol |
| `WhatsAppChannel::new(config: WhatsAppConfig, data_dir: &str) -> Self` | constructor | `whatsapp` | Constructor; takes config and data directory for session storage |
| `WhatsAppChannel::is_connected(&self) -> bool` | method | `whatsapp` | Check if WhatsApp client is currently connected |
| `WhatsAppChannel::pairing_channels(&self)` | method | `whatsapp` | Create pairing event channels; returns `(qr_rx, done_rx)` for QR and pairing-done events |
| `WhatsAppChannel::restart_for_pairing(&self)` | method | `whatsapp` | Delete stale session and restart bot for fresh pairing |

### WhatsApp -- Channel Trait Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `name` | `fn name(&self) -> &str` | Returns `"whatsapp"` |
| `start` | `async fn start(&self) -> Result<mpsc::Receiver<IncomingMessage>, OmegaError>` | Builds and runs WhatsApp bot, returns message receiver |
| `send` | `async fn send(&self, message: OutgoingMessage) -> Result<(), OmegaError>` | Sends text message to a JID with markdown sanitization and retry |
| `send_typing` | `async fn send_typing(&self, target: &str) -> Result<(), OmegaError>` | Sends composing chatstate to a JID |
| `send_photo` | `async fn send_photo(&self, target: &str, image: &[u8], caption: &str) -> Result<(), OmegaError>` | Uploads and sends image with caption to a JID |
| `stop` | `async fn stop(&self) -> Result<(), OmegaError>` | Clears client reference; logs shutdown |
| `as_any` | `fn as_any(&self) -> &dyn std::any::Any` | Returns self for downcasting |

### WhatsApp QR/Pairing (re-exported from `whatsapp::qr`)

| Function | Signature | Description |
|----------|-----------|-------------|
| `generate_qr_terminal` | `pub fn generate_qr_terminal(qr_data: &str) -> Result<String, OmegaError>` | Generate a compact QR code for terminal display using Unicode half-block characters |
| `generate_qr_image` | `pub fn generate_qr_image(qr_data: &str) -> Result<Vec<u8>, OmegaError>` | Generate a QR code as PNG image bytes |
| `start_pairing` | `pub async fn start_pairing(data_dir: &str) -> Result<(mpsc::Receiver<String>, mpsc::Receiver<bool>), OmegaError>` | Start standalone WhatsApp pairing flow; returns QR and done receivers |

### Shared Utilities

| Function | Signature | Module | Description |
|----------|-----------|--------|-------------|
| `split_message` | `pub fn split_message(text: &str, max_len: usize) -> Vec<&str>` | `utils` | Splits long messages respecting platform char limits and UTF-8 boundaries |
| `transcribe_whisper` | `pub async fn transcribe_whisper(client: &reqwest::Client, api_key: &str, audio_bytes: &[u8]) -> Result<String, OmegaError>` | `whisper` | Transcribes audio via OpenAI Whisper API |

## Tests

### Telegram Tests (`telegram/tests.rs`)

| Test | Description |
|------|-------------|
| `test_split_short_message` | Asserts a short string returns a single chunk |
| `test_split_long_message` | Asserts a 6000-char string is split into chunks each <= 4096 |
| `test_tg_chat_group_detection` | Verifies group/supergroup/private chat type detection |
| `test_tg_chat_type_defaults_when_missing` | Verifies missing type defaults to empty string |
| `test_tg_message_with_voice` | Verifies voice message deserialization |
| `test_tg_message_text_only` | Verifies text-only message deserialization |
| `test_tg_message_with_photo` | Verifies photo message with caption deserialization |
| `test_tg_message_with_photo_no_caption` | Verifies photo message without caption |
| `test_split_message_multibyte` | Tests splitting Cyrillic text at non-char boundaries |
| `test_split_message_emoji_boundary` | Tests splitting emoji text at non-char boundaries |

### WhatsApp Tests (`whatsapp/tests.rs`)

| Test | Description |
|------|-------------|
| `test_split_short_message` | Asserts a short string returns a single chunk |
| `test_split_long_message` | Asserts a 6000-char string is split into chunks each <= 4096 |
| `test_jid_group_detection` | Verifies `@g.us` (group) vs `@s.whatsapp.net` (personal) JID detection |
| `test_generate_qr_terminal` | Verifies QR terminal generation succeeds and is non-empty |
| `test_generate_qr_image` | Verifies QR image generation produces valid PNG (magic bytes) |
| `test_sanitize_headers` | Verifies `## Header` -> `*HEADER*` conversion |
| `test_sanitize_bold` | Verifies `**bold**` -> `*bold*` conversion |
| `test_sanitize_links` | Verifies `[text](url)` -> `text (url)` conversion |
| `test_sanitize_tables` | Verifies table row conversion to bullet points |
| `test_sanitize_horizontal_rules` | Verifies `---` removal |
| `test_sanitize_passthrough` | Verifies native WhatsApp formatting passes through unchanged |
| `test_sanitize_preserves_plain_text` | Verifies plain text is not modified |
| `test_split_message_multibyte` | Tests splitting Cyrillic text at non-char boundaries |
| `test_split_message_emoji_boundary` | Tests splitting emoji text at non-char boundaries |
| `test_retry_delays_exponential` | Verifies 3 retry delays with exponential backoff (500ms, 1s, 2s) |
