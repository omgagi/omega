# Developer Guide: omega-core

## Path
`/Users/isudoajl/ownCloud/Projects/omega/backend/crates/omega-core/src/lib.rs`

## What is omega-core?

`omega-core` is the foundational crate of the Omega workspace. Every other crate in the project depends on it. It provides the shared vocabulary -- types, traits, errors, and configuration -- that the rest of the system uses to communicate.

Think of it as the contract layer: providers, channels, memory, and the gateway all speak the same language because they all import from `omega-core`.

## What does it provide?

The crate is organized into six public modules:

| Module | What you get |
|--------|-------------|
| `config` | The entire configuration tree and a TOML loader |
| `context` | Conversation context passed to AI providers |
| `error` | A single error enum used across all Omega crates |
| `message` | Incoming and outgoing message types |
| `sanitize` | Prompt injection defense |
| `traits` | The `Provider` and `Channel` trait interfaces |

The crate root re-exports the `shellexpand` utility function for convenience. All other items are accessed through their module path:

```rust
use omega_core::config::Config;
use omega_core::error::OmegaError;
use omega_core::traits::Provider;
use omega_core::shellexpand; // Re-exported from config
```

---

## Configuration (`omega_core::config`)

This module maps directly to the `config.toml` file structure. Load a config with:

```rust
use omega_core::config;

let cfg = config::load("config.toml")?;
```

If the file does not exist, `load()` returns a fully populated `Config` with sensible defaults (Claude Code as the provider, SQLite for memory, auth enabled, etc.). It never panics.

### The Config struct

The top-level `Config` contains nested sections:

```rust
let cfg = config::load("config.toml")?;

// General settings
println!("Agent name: {}", cfg.omega.name);        // "OMEGA \u{03a9}"
println!("Data dir: {}", cfg.omega.data_dir);       // "~/.omega"

// Auth
println!("Auth enabled: {}", cfg.auth.enabled);     // true

// Provider selection
println!("Default provider: {}", cfg.provider.default); // "claude-code"

// Claude Code specifics
if let Some(cc) = &cfg.provider.claude_code {
    println!("Max turns: {}", cc.max_turns);         // 25
    println!("Tools: {:?}", cc.allowed_tools);       // [] (empty = full tool access)
}

// Channels
if let Some(tg) = &cfg.channel.telegram {
    println!("Telegram enabled: {}", tg.enabled);
}

// Memory
println!("DB path: {}", cfg.memory.db_path);        // "~/.omega/data/memory.db"
println!("Max context: {}", cfg.memory.max_context_messages); // 50
```

All sections except `omega` are optional in the TOML file -- they default gracefully.

### Provider configs

Six provider backends have dedicated config structs:

- **`ClaudeCodeConfig`** -- `enabled`, `max_turns`, `allowed_tools`, `timeout_secs`, `max_resume_attempts`, `model`, `model_complex`
- **`AnthropicConfig`** -- `enabled`, `api_key`, `model`, `max_tokens`
- **`OpenAiConfig`** -- `enabled`, `api_key`, `model`, `base_url`
- **`OllamaConfig`** -- `enabled`, `base_url`, `model`
- **`OpenRouterConfig`** -- `enabled`, `api_key`, `model`
- **`GeminiConfig`** -- `enabled`, `api_key`, `model`

Each is an `Option` inside `ProviderConfig`, so they only appear in the config file when needed.

### Channel configs

- **`TelegramConfig`** -- `enabled`, `bot_token`, `allowed_users` (list of Telegram user IDs), `whisper_api_key`
- **`WhatsAppConfig`** -- `enabled`, `allowed_users` (list of phone numbers), `whisper_api_key`

### Other sections

- **`MemoryConfig`** -- backend type (`"sqlite"`), database path, max context window size
- **`HeartbeatConfig`** -- periodic AI check-in settings (enabled, interval, active hours, channel, reply_target)
- **`SchedulerConfig`** -- task scheduler settings (enabled, poll_interval_secs)
- **`ApiConfig`** -- HTTP API server settings (enabled, host, port, api_key)
- **Filesystem protection** -- always-on via `omega_sandbox` crate (blocklist approach, no config needed)
- **`AuthConfig`** -- global auth toggle and the message shown to unauthorized users

---

## Conversation Context (`omega_core::context`)

When a message flows through the gateway, it gets wrapped in a `Context` before reaching the AI provider. The context carries the system prompt, conversation history, and the current user message.

### Creating a context

For simple use (no history):

```rust
use omega_core::context::Context;

let ctx = Context::new("What is the capital of France?");
// ctx.system_prompt = "You are OMEGA Ω, a personal AI assistant..."
// ctx.history = [] (empty)
// ctx.current_message = "What is the capital of France?"
```

### Adding history

The gateway populates `ctx.history` from the memory store before calling the provider:

```rust
use omega_core::context::{Context, ContextEntry};

let mut ctx = Context::new("Follow-up question");
ctx.history.push(ContextEntry {
    role: "user".to_string(),
    content: "Previous question".to_string(),
});
ctx.history.push(ContextEntry {
    role: "assistant".to_string(),
    content: "Previous answer".to_string(),
});
```

### Flattening to a prompt string

Some providers (like Claude Code CLI) accept a single text input rather than structured messages. Use `to_prompt_string()` to flatten the context:

```rust
let prompt = ctx.to_prompt_string();
// Output:
// [System]
// You are OMEGA Ω, a personal AI assistant...
//
// [User]
// Previous question
//
// [Assistant]
// Previous answer
//
// [User]
// Follow-up question
```

---

## Error Handling (`omega_core::error`)

All Omega crates use a single error type:

```rust
use omega_core::error::OmegaError;
```

`OmegaError` is a `thiserror`-derived enum with seven variants:

- **`Provider(String)`** -- An AI backend failed (timeout, bad response, rate limit)
- **`Channel(String)`** -- A messaging platform failed (network error, auth rejected)
- **`Config(String)`** -- Configuration could not be loaded or parsed
- **`Memory(String)`** -- Database operation failed
- **`Sandbox(String)`** -- Command execution failed
- **`Io(std::io::Error)`** -- Standard I/O error (automatic conversion via `#[from]`)
- **`Serialization(serde_json::Error)`** -- JSON parse/serialize error (automatic conversion via `#[from]`)

The `Io` and `Serialization` variants support the `?` operator directly -- any `std::io::Error` or `serde_json::Error` is automatically converted:

```rust
fn read_something() -> Result<String, OmegaError> {
    let data = std::fs::read_to_string("file.txt")?; // io::Error -> OmegaError::Io
    Ok(data)
}
```

---

## Message Types (`omega_core::message`)

Messages are the currency of the gateway pipeline.

### Incoming messages

An `IncomingMessage` arrives from a channel (Telegram, WhatsApp, etc.):

```rust
use omega_core::message::IncomingMessage;
```

Key fields:
- `id: Uuid` -- unique identifier
- `channel: String` -- which channel it came from (e.g. `"telegram"`)
- `sender_id: String` -- platform-specific user ID
- `sender_name: Option<String>` -- display name, if known
- `text: String` -- the message content
- `timestamp: DateTime<Utc>` -- when it was sent
- `reply_target: Option<String>` -- where to route the response (e.g. a Telegram chat_id)
- `attachments: Vec<Attachment>` -- any attached files

### Outgoing messages

An `OutgoingMessage` is the response that gets sent back to a channel:

```rust
use omega_core::message::{OutgoingMessage, MessageMetadata};
```

Key fields:
- `text: String` -- the response text
- `metadata: MessageMetadata` -- which provider was used, how long it took, token count
- `reply_target: Option<String>` -- routing target from the incoming message

### Attachments

File attachments use a simple model:

```rust
use omega_core::message::{Attachment, AttachmentType};
```

`AttachmentType` variants: `Image`, `Document`, `Audio`, `Video`, `Other`.

Each `Attachment` can carry a `url` (remote), `data` (raw bytes), or both.

---

## Prompt Sanitization (`omega_core::sanitize`)

Before user input reaches an AI provider, it passes through the sanitization layer. This defends against prompt injection attacks without blocking the message.

### Usage

```rust
use omega_core::sanitize;

let result = sanitize::sanitize("Tell me about [System] prompts");

if result.was_modified {
    for warning in &result.warnings {
        tracing::warn!("sanitize: {}", warning);
    }
}

// Use result.text (cleaned version) downstream
```

### What it catches

1. **Role impersonation tags** -- Tags like `[System]`, `<|im_start|>`, and `<<SYS>>` are neutralized by inserting invisible zero-width spaces. The text looks the same to humans but breaks the tag pattern for LLMs.

2. **Instruction override phrases** -- Phrases like "ignore all previous instructions" and "you are now" are detected. The message is then wrapped with a boundary marker so the provider understands it is untrusted user input.

3. **Suspicious code blocks** -- If a code block contains role tags, a warning is logged (but the code block is preserved, since users legitimately send code).

The function never blocks a message. It modifies or annotates, then lets it through.

---

## Trait Interfaces (`omega_core::traits`)

These are the two trait definitions that the rest of the workspace builds on. Both use `async_trait` and require `Send + Sync` so they work inside `Arc` for thread-safe sharing across async tasks.

### Provider trait

Every AI backend implements `Provider`:

```rust
use omega_core::traits::Provider;
use omega_core::context::Context;
use omega_core::message::OutgoingMessage;
use omega_core::error::OmegaError;

// Implementing a new provider:
#[async_trait::async_trait]
impl Provider for MyProvider {
    fn name(&self) -> &str { "my-provider" }

    fn requires_api_key(&self) -> bool { true }

    async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError> {
        // Send context to your backend, return a response
        todo!()
    }

    async fn is_available(&self) -> bool {
        // Check if the backend is reachable
        true
    }
}
```

Four methods:
- `name()` -- Human-readable identifier
- `requires_api_key()` -- Whether the provider needs an API key
- `complete()` -- The core method: take a conversation context, return a response
- `is_available()` -- Health check before startup

### Channel trait

Every messaging platform implements `Channel`:

```rust
use omega_core::traits::Channel;
use omega_core::message::{IncomingMessage, OutgoingMessage};
use omega_core::error::OmegaError;

#[async_trait::async_trait]
impl Channel for MyChannel {
    fn name(&self) -> &str { "my-channel" }

    async fn start(&self) -> Result<tokio::sync::mpsc::Receiver<IncomingMessage>, OmegaError> {
        // Begin listening for messages, return a receiver
        todo!()
    }

    async fn send(&self, message: OutgoingMessage) -> Result<(), OmegaError> {
        // Send the response to the platform
        todo!()
    }

    // send_typing() has a default no-op implementation -- override if your platform supports it
    async fn send_typing(&self, target: &str) -> Result<(), OmegaError> {
        Ok(())
    }

    // send_photo() has a default no-op implementation
    async fn send_photo(&self, target: &str, image: &[u8], caption: &str) -> Result<(), OmegaError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), OmegaError> {
        // Graceful shutdown
        Ok(())
    }

    // Required: enables the gateway to downcast to channel-specific types
    fn as_any(&self) -> &dyn std::any::Any;
}
```

Seven methods:
- `name()` -- Human-readable identifier
- `start()` -- Begin listening; returns a `tokio::sync::mpsc::Receiver` that yields incoming messages
- `send()` -- Send a response back through the channel
- `send_typing()` -- Optional typing indicator (defaults to no-op)
- `send_photo()` -- Optional photo sending (defaults to no-op)
- `stop()` -- Graceful shutdown
- `as_any()` -- Downcast support for channel-specific methods

---

## How everything fits together

Here is the flow through `omega-core` types in a typical message lifecycle:

```
Telegram sends a message
    |
    v
IncomingMessage (message module)
    |
    v
sanitize::sanitize(text) -> SanitizeResult (sanitize module)
    |
    v
Context { system_prompt, history, current_message } (context module)
    |
    v
provider.complete(&context) -> OutgoingMessage (traits + message modules)
    |
    v
channel.send(outgoing) (traits module)
```

Configuration (`config::Config`) drives which providers and channels are instantiated. Errors at any step are expressed as `OmegaError` variants, giving the gateway a uniform error handling path.

---

## Quick reference

| You want to... | Import this |
|----------------|-------------|
| Load configuration | `omega_core::config::load` |
| Access config types | `omega_core::config::Config`, `omega_core::config::TelegramConfig`, etc. |
| Build a conversation context | `omega_core::context::Context::new("message")` |
| Handle errors | `omega_core::error::OmegaError` |
| Work with messages | `omega_core::message::IncomingMessage`, `omega_core::message::OutgoingMessage` |
| Sanitize user input | `omega_core::sanitize::sanitize(input)` |
| Implement a provider | `omega_core::traits::Provider` |
| Implement a channel | `omega_core::traits::Channel` |
