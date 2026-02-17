# Technical Specification: Telegram Channel

**File:** `crates/omega-channels/src/telegram.rs`
**Crate:** `omega-channels`
**Last updated:** 2026-02-16

## Purpose

Implements the `Channel` trait from `omega-core` for the Telegram Bot API. Uses HTTP long polling via `getUpdates` to receive messages and `sendMessage` / `sendChatAction` to send responses and typing indicators. This is the primary messaging channel for Omega.

---

## Dependencies

| Crate | Usage |
|-------|-------|
| `async_trait` | Enables async methods in the `Channel` trait implementation |
| `omega_core` | `TelegramConfig`, `OmegaError`, `IncomingMessage`, `OutgoingMessage`, `Channel` trait |
| `serde` | Deserialization of Telegram Bot API JSON responses |
| `reqwest` | HTTP client for Bot API requests |
| `tokio` | Async runtime, `mpsc` channels, `Mutex`, timed sleeps |
| `tracing` | Structured logging (`debug`, `error`, `info`, `warn`) |
| `uuid` | Generates unique IDs for incoming messages |
| `chrono` | Timestamps on incoming messages (`Utc::now()`) |
| `serde_json` | Constructing JSON request bodies |

---

## Structs

### `TelegramChannel`

The main public struct. Holds configuration, HTTP client, and polling state.

| Field | Type | Description |
|-------|------|-------------|
| `config` | `TelegramConfig` | Bot token and allowed user list |
| `client` | `reqwest::Client` | Reusable HTTP client for all API calls |
| `base_url` | `String` | Precomputed `https://api.telegram.org/bot{token}` |
| `last_update_id` | `Arc<Mutex<Option<i64>>>` | Tracks the most recent `update_id` to avoid reprocessing; shared with the polling task |

### `TelegramConfig` (from `omega-core`)

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `enabled` | `bool` | `false` | Whether the Telegram channel is active |
| `bot_token` | `String` | `""` | Bot API token from BotFather |
| `allowed_users` | `Vec<i64>` | `[]` | Telegram user IDs authorized to interact; empty means allow all |

### Telegram API Deserialization Types (private)

#### `TgResponse<T>`

| Field | Type | Description |
|-------|------|-------------|
| `ok` | `bool` | Whether the API call succeeded |
| `result` | `Option<T>` | The response payload (generic) |
| `description` | `Option<String>` | Error description when `ok` is `false` |

#### `TgUpdate`

| Field | Type | Description |
|-------|------|-------------|
| `update_id` | `i64` | Unique identifier for this update; used for offset tracking |
| `message` | `Option<TgMessage>` | The message payload, if this update contains one |

#### `TgMessage`

| Field | Type | Description |
|-------|------|-------------|
| `message_id` | `i64` | Unique message identifier within the chat |
| `from` | `Option<TgUser>` | Sender of the message |
| `chat` | `TgChat` | Chat the message belongs to |
| `text` | `Option<String>` | Text content of the message |

Note: `message_id` and `from` are marked `#[allow(dead_code)]` -- `message_id` is deserialized but not currently used; `from` is used for auth and sender name resolution.

#### `TgUser`

| Field | Type | Description |
|-------|------|-------------|
| `id` | `i64` | Unique Telegram user identifier |
| `first_name` | `String` | User's first name |
| `last_name` | `Option<String>` | User's last name |
| `username` | `Option<String>` | User's `@username` |

#### `TgChat`

| Field | Type | Description |
|-------|------|-------------|
| `id` | `i64` | Unique chat identifier; used as `reply_target` |
| `chat_type` | `String` | Chat type from the Telegram API: `"private"`, `"group"`, `"supergroup"`, or `"channel"`. Annotated with `#[serde(default, rename = "type")]` to map from the JSON `type` field and default to an empty string if missing. Used to set `is_group` on `IncomingMessage`. |

---

## Function Signatures

### Inherent Methods on `TelegramChannel`

```rust
pub fn new(config: TelegramConfig) -> Self
```

Constructs a new `TelegramChannel`. Precomputes `base_url` from the bot token, initializes the HTTP client, and sets `last_update_id` to `None`.

```rust
async fn send_message(&self, chat_id: i64, text: &str) -> Result<(), OmegaError>
```

Sends a text message to the specified `chat_id`. Splits the text into chunks of up to 4096 characters (Telegram's limit) using `split_message()`. Each chunk is sent as a separate `sendMessage` request with `parse_mode: "Markdown"`. If Telegram rejects the Markdown (response contains `"can't parse entities"`), retries the chunk as plain text without `parse_mode`.

```rust
async fn send_chat_action(&self, chat_id: i64, action: &str) -> Result<(), OmegaError>
```

Sends a chat action (e.g., `"typing"`) to the specified chat via the `sendChatAction` API endpoint.

### Free Functions

```rust
fn split_message(text: &str, max_len: usize) -> Vec<&str>
```

Splits a string into chunks no longer than `max_len` bytes. Prefers splitting at newline boundaries (`\n`) when possible to avoid breaking mid-line. Returns a `Vec` of string slices.

---

## Trait Implementation: `Channel`

The `TelegramChannel` implements the `Channel` trait defined in `omega-core::traits`.

### `fn name(&self) -> &str`

Returns `"telegram"`.

### `async fn start(&self) -> Result<mpsc::Receiver<IncomingMessage>, OmegaError>`

Spawns a background `tokio::spawn` task that performs long polling against the Telegram Bot API. Returns an `mpsc::Receiver<IncomingMessage>` with a buffer size of 64.

**Long Polling Mechanism (detailed below).**

### `async fn send_typing(&self, target: &str) -> Result<(), OmegaError>`

Parses `target` as an `i64` chat ID and calls `send_chat_action(chat_id, "typing")`. Returns a `Channel` error if the target cannot be parsed.

### `async fn send(&self, message: OutgoingMessage) -> Result<(), OmegaError>`

Extracts `reply_target` from the outgoing message (required; errors if absent), parses it as an `i64` chat ID, and delegates to `send_message()`.

### `async fn stop(&self) -> Result<(), OmegaError>`

Logs `"Telegram channel stopped"` and returns `Ok(())`. The background polling task will terminate naturally when the `mpsc::Sender` is dropped (i.e., when the receiver side is dropped and the next `tx.send()` fails).

---

## Long Polling Mechanism

The polling loop runs inside a `tokio::spawn` task created in `start()`. It shares `last_update_id` and `base_url` with the parent struct via `Arc`/`Clone`.

### Poll Cycle

1. **Read offset:** Lock `last_update_id`. Compute `offset = last_update_id + 1` (or omit if `None`).
2. **Request:** `GET {base_url}/getUpdates?timeout=30[&offset={offset}]`
   - Client-side timeout: **35 seconds** (5s margin over the server-side 30s long-poll timeout).
3. **Error handling:** On HTTP or parse error, log and sleep for `backoff_secs`, then double backoff (capped at 60s).
4. **API error check:** If `body.ok == false`, log the description and apply backoff.
5. **Reset backoff:** On a successful poll, reset `backoff_secs` to 1.
6. **Update offset:** Set `last_update_id` to the `update_id` of the last update in the batch.
7. **Process updates:** Iterate through each update (see Message Parsing below).
8. **Channel closed detection:** If `tx.send(incoming).await.is_err()`, the receiver was dropped -- log and return (terminates the task).

### Timing

| Parameter | Value |
|-----------|-------|
| Server-side long-poll timeout | 30 seconds |
| Client-side HTTP timeout | 35 seconds |
| Initial backoff on error | 1 second |
| Backoff multiplier | 2x |
| Maximum backoff | 60 seconds |
| Backoff reset | On any successful poll |

---

## Message Parsing

For each `TgUpdate` in the response:

1. **Skip non-message updates:** If `update.message` is `None`, skip.
2. **Skip non-text messages:** If `message.text` is `None`, skip (photos, stickers, etc. are ignored).
3. **Skip anonymous messages:** If `message.from` is `None`, skip.
4. **Authorization check:** If `allowed_users` is non-empty and the sender's `user.id` is not in the list, log a warning and skip.
5. **Resolve sender name:** Priority order:
   - `@username` if present
   - `first_name last_name` if `last_name` is present
   - `first_name` alone as fallback
6. **Construct `IncomingMessage`:**

| Field | Value |
|-------|-------|
| `id` | `Uuid::new_v4()` |
| `channel` | `"telegram"` |
| `sender_id` | `user.id.to_string()` |
| `sender_name` | Resolved name (see above) |
| `text` | `message.text` |
| `timestamp` | `chrono::Utc::now()` |
| `reply_to` | `None` |
| `attachments` | `Vec::new()` (empty) |
| `reply_target` | `Some(chat.id.to_string())` |
| `is_group` | `true` if `chat.chat_type` is `"group"` or `"supergroup"`, `false` otherwise |

---

## Typing Indicator Logic

The `send_typing` method:

1. Receives a `target` string (the `reply_target` from an `IncomingMessage`, which is a stringified Telegram chat ID).
2. Parses it to `i64`. Returns `OmegaError::Channel` on parse failure.
3. Calls `sendChatAction` with `action: "typing"`.

Telegram typing indicators auto-expire after approximately 5 seconds or when the bot sends a message, whichever comes first. The gateway is responsible for calling `send_typing` repeatedly if processing takes longer.

---

## Message Sending and Chunking

### `send_message` Flow

1. Split the text into chunks of at most **4096 bytes** using `split_message()`.
2. For each chunk:
   a. POST to `{base_url}/sendMessage` with `chat_id`, `text`, and `parse_mode: "Markdown"`.
   b. If the response status is not success:
      - If the error body contains `"can't parse entities"`: retry the same chunk as **plain text** (no `parse_mode`).
      - Otherwise: log a warning with the status and error text. Do **not** retry.
3. Return `Ok(())` after all chunks are sent.

### `split_message` Algorithm

```
Input: text (str), max_len (usize)
Output: Vec<&str> of chunks, each <= max_len bytes

if text.len() <= max_len:
    return [text]

start = 0
while start < text.len():
    end = min(start + max_len, text.len())
    if end < text.len():
        break_at = last '\n' in text[start..end]
        if found: break_at = start + position + 1
        else: break_at = end
    else:
        break_at = end
    push text[start..break_at]
    start = break_at
```

Key behavior:
- Prefers splitting at newline boundaries to preserve formatting.
- Falls back to hard split at `max_len` if no newline is found.
- Operates on byte lengths (not grapheme clusters), which matches Telegram's limit semantics.

---

## Error Handling

All errors are wrapped in `OmegaError::Channel(String)`.

| Scenario | Behavior |
|----------|----------|
| HTTP request failure (poll) | Log error, sleep with exponential backoff, retry |
| JSON parse failure (poll) | Log error, sleep with exponential backoff, retry |
| API returns `ok: false` (poll) | Log description, sleep with exponential backoff, retry |
| HTTP request failure (send) | Return `OmegaError::Channel` |
| Markdown parse failure (send) | Retry as plain text; if plain text also fails, return error |
| Non-success status (send, non-Markdown) | Log warning, continue to next chunk |
| Missing `reply_target` (send) | Return `OmegaError::Channel` |
| Invalid `chat_id` parse | Return `OmegaError::Channel` |
| Receiver dropped (poll) | Log info, terminate polling task |
| Unauthorized user | Log warning, skip message |

---

## Tests

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_split_short_message()
    // Asserts a message under 4096 chars returns a single chunk.

    #[test]
    fn test_split_long_message()
    // Creates a 6000-char string of "a\n" repeated, splits at 4096.
    // Asserts at least 2 chunks, each <= 4096 bytes.

    #[test]
    fn test_tg_chat_group_detection()
    // Deserializes a TgChat with `"type": "supergroup"` and verifies
    // `chat_type` is `"supergroup"`. Also tests `matches!` logic for
    // group/supergroup → `is_group = true`.

    #[test]
    fn test_tg_chat_type_defaults_when_missing()
    // Deserializes a TgChat JSON object without a `type` field.
    // Verifies `chat_type` defaults to an empty string, resulting
    // in `is_group = false`.
}
```

---

## Data Flow Diagram

```
                         Telegram Bot API
                              |
                     getUpdates (long poll)
                              |
                              v
               +-----------------------------+
               |    TelegramChannel::start   |
               |    (tokio::spawn loop)      |
               +-----------------------------+
                              |
                   mpsc::channel(64)
                              |
                              v
                        Gateway Loop
                              |
                    +---------+---------+
                    |                   |
              send_typing()         send()
                    |                   |
             sendChatAction       sendMessage
              (typing)          (Markdown/plain)
                    |                   |
                    v                   v
                     Telegram Bot API
```
