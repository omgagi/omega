# WhatsApp Channel -- Developer Documentation

## Overview

The WhatsApp channel connects Omega to WhatsApp using a pure Rust implementation of the WhatsApp Web protocol. No external bridge, no Meta Business account needed. Users pair by scanning a QR code, just like WhatsApp Web.

**Crate:** `whatsapp-rust` (via `whatsapp-rust` crate on crates.io)

**Important:** This is an unofficial implementation. Using custom WhatsApp clients may violate Meta's Terms of Service. Use at your own risk.

---

## Quick Setup

### Via CLI (`omega init`)

```
$ omega init

  WhatsApp Setup
  --------------
  Would you like to connect WhatsApp? [y/N]: y

  Starting WhatsApp pairing...
  Open WhatsApp on your phone > Linked Devices > Link a Device

  Scan this QR code with WhatsApp:

  ██████████████████████
  ██ ▄▄▄▄▄ █ ...
  ...

  Waiting for scan... Connected!
  WhatsApp linked successfully.
```

### Via Telegram (`/whatsapp`)

Send `/whatsapp` to your Omega bot on Telegram. The bot sends a QR code image. Scan it with your phone.

### Via Conversation

Ask Omega to "connect WhatsApp" or "set up WhatsApp" in a Telegram chat. The AI responds with a `WHATSAPP_QR` marker, which the gateway intercepts and triggers the QR flow automatically.

---

## Configuration

```toml
[channel.whatsapp]
enabled = false
allowed_users = []          # Phone numbers. Empty = allow all.
whisper_api_key = ""        # Optional: OpenAI key for voice transcription
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | `false` | Whether the WhatsApp channel is active |
| `allowed_users` | `Vec<String>` | `[]` | Allowed phone numbers. Empty = allow all. |
| `whisper_api_key` | `Option<String>` | `None` | OpenAI API key for Whisper voice transcription. Presence = voice enabled. |

---

## Session Persistence

Session data is stored at `~/.omega/whatsapp_session/whatsapp.db`. This SQLite database contains device identity keys, prekey bundles, and session state.

On restart, Omega reconnects using the persisted session — no re-scan needed. If the session is invalidated (e.g., logged out from phone), a new QR scan is required.

---

## How It Works

WhatsApp operates in **self-chat mode only** — only messages you send to yourself are processed. Group messages are dropped immediately at the channel level and never reach the gateway.

1. **Pairing**: The `whatsapp-rust` library initiates a WebSocket connection to WhatsApp servers and generates QR codes. The user scans one with their phone (WhatsApp > Linked Devices > Link a Device).

2. **Receiving messages**: The bot's event handler receives `Event::Message` events. Only self-chat messages are processed (`is_from_me` + sender matches chat JID). Group messages are dropped at the channel level with a debug log. The sender's `push_name` is used for display when available.

3. **Message unwrapping**: Messages are often wrapped in `DeviceSentMessage`, `EphemeralMessage`, or `ViewOnceMessage` containers. The handler unwraps these before extracting text from `conversation` or `extended_text_message.text`.

4. **Image handling**: If no text is found, the handler checks for an `image_message`. Image messages are downloaded via the WhatsApp client (`ImageMessage` implements the `Downloadable` trait), and the image bytes are passed through as an `Attachment` with the caption as text (defaults to `"[Photo]"`).

5. **Voice transcription**: If no text or image, the handler checks for an `audio_message`. When a `whisper_api_key` is configured, voice messages are downloaded and transcribed via OpenAI Whisper (shared module with Telegram), injected as `"[Voice message] {transcript}"`.

6. **Echo prevention**: Sent message IDs are tracked in a `HashSet`. When the bot sends a reply, the message ID is recorded. When the echo arrives back as an incoming event, the ID is matched and the message is skipped, preventing infinite loops.

7. **Sending messages**: Text is sanitized from Markdown to WhatsApp-native formatting (headers become bold uppercase, `**bold**` becomes `*bold*`, links are expanded, tables become bullets, horizontal rules are removed). Messages over 4096 characters are automatically chunked. All sends use retry with exponential backoff (3 attempts: 500ms, 1s, 2s).

8. **Sending photos**: Images are uploaded via `client.upload(MediaType::Image)` and sent as `ImageMessage` with retry backoff.

9. **Typing indicators**: The `send_typing()` method sends "composing" presence via `client.chatstate().send_composing()`.

---

## Authentication

WhatsApp auth uses phone numbers. Configure `allowed_users` with phone numbers (digits only, no `+` prefix):

```toml
[channel.whatsapp]
enabled = true
allowed_users = ["5511999887766", "5521888776655"]
```

Leave `allowed_users = []` to allow all incoming messages.

---

## Channel Trait Methods

| Method | Description |
|--------|-------------|
| `name()` | Returns `"whatsapp"` |
| `start()` | Initializes session store, builds bot, starts event loop |
| `send()` | Sanitizes markdown, sends text message to the chat JID with retry |
| `send_typing()` | Sends "composing" presence indicator |
| `send_photo()` | Uploads and sends image via WhatsApp media upload with retry |
| `stop()` | Disconnects and cleans up |

---

## QR Code Utilities

The `whatsapp` module exports public functions for QR code generation:

- `generate_qr_terminal(data)` — Unicode string for terminal display
- `generate_qr_image(data)` — PNG bytes for sending as an image
- `start_pairing(data_dir)` — Standalone pairing flow for CLI (`omega init`), creates a separate bot
- `pairing_channels()` — Instance method: returns receivers from the running bot for in-process pairing (used by gateway)
- `restart_for_pairing()` — Instance method: deletes stale session and rebuilds the bot so it generates fresh QR codes (used by gateway when re-pairing after unlinking)

---

## Troubleshooting

### Session expired / Re-pairing after unlinking
If WhatsApp logs out the device (from phone settings), simply send `/whatsapp` again via Telegram. The gateway automatically deletes the stale session and generates a fresh QR code — no manual cleanup or service restart needed.

For manual cleanup:
```bash
rm -rf ~/.omega/whatsapp_session/
omega init  # or /whatsapp from Telegram
```

### QR code not appearing
Check that the `whatsapp-rust` dependencies are correctly installed. The library needs network access to WhatsApp servers.

### Messages not received after pairing
If you paired WhatsApp via `/whatsapp` but messages don't flow:
- The gateway uses the running bot's event stream — no restart should be needed
- Check logs for "WhatsApp connected" after scanning the QR code
- If the session was previously invalidated, send `/whatsapp` again — the gateway automatically deletes the stale session and rebuilds the bot

### Messages not received (general)
- WhatsApp only processes self-chat messages (messages you send to yourself). Group messages are dropped at the channel level.
- Text, image, and voice messages are supported in self-chat.
- Voice transcription requires `whisper_api_key` in config. Without it, voice messages are silently skipped.
- If an image or audio download fails (personal chat), the message is skipped — check logs for download warnings.
- Check `allowed_users` in config — your phone number must be listed (or leave empty for all)
- Verify the session is still valid (check logs for "WhatsApp connected" or "logged out")

---

## Reference

- Implementation: `crates/omega-channels/src/whatsapp.rs`
- Config struct: `crates/omega-core/src/config.rs` (`WhatsAppConfig`)
- Channel trait: `crates/omega-core/src/traits.rs`
- Gateway integration: `src/gateway.rs` (auth + WHATSAPP_QR marker)
- Commands: `src/commands.rs` (`/whatsapp`)
- Init wizard: `src/init.rs` (WhatsApp QR step)
- whatsapp-rust crate: https://crates.io/crates/whatsapp-rust
