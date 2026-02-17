# Telegram Channel

The Telegram channel connects Omega to Telegram via the [Bot API](https://core.telegram.org/bots/api). It uses long polling to receive messages and HTTP POST requests to send responses. This is the primary messaging channel for Omega and was introduced in Phase 2.

**Source file:** `crates/omega-channels/src/telegram.rs`

---

## How It Works

At a high level, the Telegram channel does three things:

1. **Listens** for incoming messages using long polling (`getUpdates`).
2. **Sends** responses back to the user (`sendMessage`).
3. **Shows typing indicators** while the AI provider is thinking (`sendChatAction`).

When the gateway calls `start()`, the channel spawns a background task that continuously polls the Telegram API for new messages. Each valid message is converted into an `IncomingMessage` and sent through a Tokio `mpsc` channel back to the gateway for processing.

---

## Bot API Integration

All communication goes through the Telegram Bot API at:

```
https://api.telegram.org/bot{YOUR_BOT_TOKEN}/
```

The channel uses three endpoints:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/getUpdates` | GET | Long polling for new messages |
| `/sendMessage` | POST | Sending text responses |
| `/sendChatAction` | POST | Sending typing indicators |

The bot token is read from your `config.toml` under `[telegram]`. You get this token from [@BotFather](https://t.me/BotFather) when you create a new bot.

---

## Long Polling

Rather than setting up a webhook (which requires a public HTTPS endpoint), Omega uses long polling. This is simpler to set up and works behind firewalls and NATs.

Here is what happens in each poll cycle:

1. The channel sends a `getUpdates` request with a **30-second server-side timeout**. This means Telegram holds the connection open for up to 30 seconds, returning immediately if new messages arrive.
2. The HTTP client uses a **35-second client-side timeout** (5 extra seconds to account for network latency).
3. When updates arrive, the channel records the highest `update_id` and uses `offset = update_id + 1` on the next request. This tells Telegram to only return newer updates, avoiding duplicates.
4. The loop runs continuously until the receiver side of the channel is dropped (i.e., the gateway shuts down).

### Exponential Backoff

If a poll fails (network error, parse error, or API error), the channel waits before retrying:

- Starts at **1 second**.
- Doubles after each consecutive failure: 1s, 2s, 4s, 8s, 16s, 32s, 60s.
- Caps at **60 seconds**.
- Resets back to 1 second after any successful poll.

This prevents hammering the API during outages while recovering quickly once the issue is resolved.

---

## Message Parsing and Authorization

When a message arrives from Telegram, the channel applies several filters before forwarding it to the gateway:

1. **Must be a message** -- non-message updates (edited messages, channel posts, callbacks) are skipped.
2. **Must have text** -- photos, stickers, voice messages, and other non-text content are currently ignored.
3. **Must have a sender** -- anonymous messages are skipped.
4. **Must be authorized** -- if `allowed_users` is configured (non-empty), the sender's Telegram user ID must be in the list. Unauthorized messages are logged and silently dropped.

The sender's display name is resolved in this priority order:
- `@username` if they have one
- `FirstName LastName` if both are available
- `FirstName` as a fallback

The `chat.id` is stored as `reply_target` on the `IncomingMessage`, so the gateway knows where to send the response.

---

## Typing Indicators

When the gateway receives a message and starts processing it, it calls `send_typing()` with the chat ID. This sends a `"typing"` action to Telegram, which shows a "typing..." animation in the user's chat client.

Telegram automatically cancels the typing indicator after about 5 seconds or when the bot sends a message, whichever comes first. For long-running provider calls, the gateway may call `send_typing()` multiple times to keep the indicator alive.

---

## Sending Messages

### Markdown Support

All messages are sent with `parse_mode: "Markdown"` by default. This lets AI responses include formatting like bold, italic, code blocks, and links.

If Telegram rejects the Markdown (common when the AI produces slightly malformed syntax), the channel automatically retries the same message as plain text. This fallback ensures the user always gets a response, even if formatting is lost.

### Message Chunking

Telegram has a **4096 character limit** per message. If the AI response exceeds this, the channel splits it into multiple messages:

- It prefers splitting at **newline boundaries** to avoid cutting sentences or code blocks in half.
- If no newline is found within the 4096-character window, it does a hard split at the limit.
- Each chunk is sent as a separate `sendMessage` call.

---

## Configuration

Add this section to your `config.toml`:

```toml
[telegram]
enabled = true
bot_token = "123456789:ABCdefGHIjklMNOpqrsTUVwxyz"
allowed_users = [123456789]  # Your Telegram user ID
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `enabled` | bool | No | Defaults to `false`. Set to `true` to activate. |
| `bot_token` | string | Yes | The token you get from @BotFather. |
| `allowed_users` | list of integers | No | Telegram user IDs that may interact with the bot. Leave empty to allow everyone (not recommended). |

### Finding Your Telegram User ID

Send a message to your bot, then check the Omega logs. If your ID is not in `allowed_users`, you will see a log line like:

```
WARN ignoring message from unauthorized user 123456789
```

That number is your Telegram user ID. Add it to `allowed_users` and restart Omega.

Alternatively, you can send `/start` to [@userinfobot](https://t.me/userinfobot) on Telegram to get your ID.

---

## Error Handling

The channel handles errors at two levels:

### Polling Errors (background task)

These are handled internally with exponential backoff. The channel logs the error and retries automatically. You will see messages like:

```
ERROR telegram poll error (retry in 2s): connection refused
ERROR telegram parse error (retry in 4s): expected value at line 1 column 1
ERROR telegram API error (retry in 1s): Unauthorized
```

The polling task never panics and never terminates on its own (except when the gateway shuts down).

### Sending Errors (returned to gateway)

These are returned as `OmegaError::Channel` to the caller:

- **Missing reply target:** The outgoing message did not have a `reply_target` set.
- **Invalid chat ID:** The `reply_target` could not be parsed as an integer.
- **Network failure:** The HTTP request to Telegram failed.

---

## Debugging Tips

### Bot is not receiving messages

1. Check that `enabled = true` in your config.
2. Verify the `bot_token` is correct (try `curl https://api.telegram.org/bot{TOKEN}/getMe`).
3. Make sure your user ID is in `allowed_users` (or that the list is empty).
4. Look for polling errors in the logs at `~/.omega/omega.log`.

### Messages are arriving but responses fail

1. Check the logs for `telegram send failed` or `telegram send got` errors.
2. Verify the bot has permission to send messages in the chat.
3. If you see `"can't parse entities"` in the logs followed by a plain-text retry, the AI produced malformed Markdown. This is handled automatically, but you can check if the fallback succeeded.

### Typing indicator not showing

1. The typing indicator only lasts ~5 seconds. If the provider responds very quickly, you may not see it.
2. Check the logs for `telegram sendChatAction failed` errors.

### Duplicate messages

If Omega restarts, `last_update_id` resets to `None`, which could theoretically cause a small batch of recent messages to be reprocessed. In practice, Telegram marks updates as confirmed once you use an offset, so this is rarely an issue. If it is, the gateway's deduplication (via the `IncomingMessage.id` UUID) will catch most cases.

---

## Group Chat Awareness

The Telegram channel detects group chats using the `type` field from the Telegram Bot API's `Chat` object. When the chat type is `"group"` or `"supergroup"`, the `is_group` flag on `IncomingMessage` is set to `true`.

When `is_group` is true, the gateway:

1. **Injects group-specific rules** into the system prompt, instructing the AI to only respond when directly mentioned, asked a question, or when it can add genuine value.
2. **Suppresses SILENT responses** -- the AI can reply with exactly `SILENT` to indicate it should stay quiet, and the gateway silently drops the message instead of sending it.
3. **Prevents personal fact leakage** -- the group-chat rules instruct the AI not to reveal facts learned in private conversations.

## Limitations

- **Text only.** Photos, documents, voice messages, stickers, and other media types are silently skipped.
- **No inline keyboards or buttons.** Responses are plain text (with optional Markdown formatting).
- **No webhook mode.** Only long polling is supported. This is simpler but slightly higher latency than webhooks.
- **Message chunking is byte-based.** The 4096-byte split operates on byte offsets, not Unicode grapheme clusters. In practice this is fine because Telegram's own limit is also byte-based.
