# Specification: backend/crates/omega-core/src/lib.rs

## File Path
`/Users/isudoajl/ownCloud/Projects/omega/backend/crates/omega-core/src/lib.rs`

## Purpose
Crate root for `omega-core`. Declares and re-exports six public submodules that together form the foundational type system, trait interfaces, configuration layer, error handling, message model, prompt sanitization, and conversation context structures used by every other crate in the Omega workspace. Also exports the `shellexpand` utility function at the crate root.

## Dependencies (Cargo.toml)
- **tokio** -- Async runtime (used in trait definitions via `async_trait`)
- **serde / serde_json** -- Serialization and deserialization for all public structs
- **toml** -- TOML config file parsing (in `config` module)
- **tracing** -- Structured logging (in `config::load`, `config::migrate_layout`, etc.)
- **thiserror** -- Derive macro for `OmegaError` variants
- **anyhow** -- Available for downstream error handling
- **uuid** -- Unique message identifiers (`IncomingMessage.id`)
- **chrono** -- UTC timestamps on messages (`IncomingMessage.timestamp`)
- **async_trait** -- Enables async methods in trait definitions (`Provider`, `Channel`)

---

## Module Declarations and Re-exports

The crate root (`lib.rs`) contains six `pub mod` declarations and one re-export.

```rust
pub mod config;
pub mod context;
pub mod error;
pub mod message;
pub mod sanitize;
pub mod traits;

pub use config::shellexpand;
```

### Module Doc Comment
```rust
//! # omega-core
//!
//! Core types, traits, configuration, and error handling for the Omega agent.
```

---

## Module Hierarchy

```
omega_core
 +-- config/       Configuration types, default value functions, TOML loader, prompt system,
 |                  layout migration, heartbeat interval patching, bundled prompt deployment
 |   +-- mod.rs       Top-level Config struct, AuthConfig, OmegaConfig, MemoryConfig,
 |   |                HeartbeatConfig, SchedulerConfig, ApiConfig, SYSTEM_FACT_KEYS,
 |   |                shellexpand(), migrate_layout(), patch_heartbeat_interval(), load()
 |   +-- providers.rs ProviderConfig, ClaudeCodeConfig, AnthropicConfig, OpenAiConfig,
 |   |                OllamaConfig, OpenRouterConfig, GeminiConfig
 |   +-- channels.rs  ChannelConfig, TelegramConfig, WhatsAppConfig
 |   +-- prompts.rs   Prompts struct, install_bundled_prompts(), bundled_workspace_claude(),
 |   |                BUNDLED_SYSTEM_PROMPT, BUNDLED_WELCOME_TOML, BUNDLED_WORKSPACE_CLAUDE
 |   +-- defaults.rs  All default_*() value functions
 |   +-- tests.rs     Unit tests for config deserialization and prompt parsing
 +-- context       ContextNeeds, ContextEntry, McpServer, Context, ApiMessage, prompt flattening
 +-- error         OmegaError enum (thiserror)
 +-- message       IncomingMessage, OutgoingMessage, MessageMetadata, Attachment, AttachmentType
 +-- sanitize      SanitizeResult, sanitize() function, injection pattern tests
 +-- traits        Provider trait, Channel trait (async_trait)
```

---

## Module Details

### `config`

**Directory:** `backend/crates/omega-core/src/config/` (6-file directory module: mod.rs, providers.rs, channels.rs, prompts.rs, defaults.rs, tests.rs)

**Purpose:** Defines the full configuration tree that maps 1:1 to `config.toml`. Provides a `load()` function that reads TOML from disk and falls back to sensible defaults. Also provides layout migration, heartbeat interval patching, prompt loading, and bundled prompt deployment.

**Public Structs (via mod.rs):**

| Struct | Derives | Purpose |
|--------|---------|---------|
| `Config` | Debug, Clone, Serialize, Deserialize | Top-level configuration container |
| `OmegaConfig` | Debug, Clone, Serialize, Deserialize | General agent settings (name, data_dir, log_level) |
| `AuthConfig` | Debug, Clone, Serialize, Deserialize, Default | Auth enforcement toggle and deny message |
| `MemoryConfig` | Debug, Clone, Serialize, Deserialize | Memory backend settings (backend, db_path, max_context_messages) |
| `HeartbeatConfig` | Debug, Clone, Serialize, Deserialize | Periodic AI check-in settings (enabled, interval, active hours, channel, reply_target) |
| `SchedulerConfig` | Debug, Clone, Serialize, Deserialize | Task scheduler settings (enabled, poll_interval_secs) |
| `ApiConfig` | Debug, Clone, Serialize, Deserialize | HTTP API server settings (enabled, host, port, api_key) |

**Public Structs (via providers.rs, re-exported from config):**

| Struct | Derives | Purpose |
|--------|---------|---------|
| `ProviderConfig` | Debug, Clone, Serialize, Deserialize, Default | Provider selection and per-provider sub-configs |
| `ClaudeCodeConfig` | Debug, Clone, Serialize, Deserialize | Claude Code CLI settings (enabled, max_turns, allowed_tools, timeout_secs, max_resume_attempts, model, model_complex) |
| `AnthropicConfig` | Debug, Clone, Serialize, Deserialize | Anthropic API settings (enabled, api_key, model, max_tokens) |
| `OpenAiConfig` | Debug, Clone, Serialize, Deserialize | OpenAI-compatible settings (enabled, api_key, model, base_url) |
| `OllamaConfig` | Debug, Clone, Serialize, Deserialize | Ollama local settings (enabled, base_url, model) |
| `OpenRouterConfig` | Debug, Clone, Serialize, Deserialize | OpenRouter proxy settings (enabled, api_key, model) |
| `GeminiConfig` | Debug, Clone, Serialize, Deserialize | Google Gemini settings (enabled, api_key, model) |

**Public Structs (via channels.rs, re-exported from config):**

| Struct | Derives | Purpose |
|--------|---------|---------|
| `ChannelConfig` | Debug, Clone, Serialize, Deserialize, Default | Channel selection container |
| `TelegramConfig` | Debug, Clone, Serialize, Deserialize | Telegram bot settings (enabled, bot_token, allowed_users, whisper_api_key) |
| `WhatsAppConfig` | Debug, Clone, Serialize, Deserialize | WhatsApp native settings (enabled, allowed_users, whisper_api_key) |

**Public Structs (via prompts.rs, re-exported from config):**

| Struct | Derives | Purpose |
|--------|---------|---------|
| `Prompts` | Debug, Clone | Externalized prompts and welcome messages, loaded from disk at startup |

**Public Constants (in mod.rs):**

| Constant | Type | Purpose |
|----------|------|---------|
| `SYSTEM_FACT_KEYS` | `&[&str]` | System-managed fact keys that only bot commands may write. 8 entries: `welcomed`, `preferred_language`, `active_project`, `personality`, `onboarding_stage`, `pending_build_request`, `pending_discovery`, `pending_setup`. |

**Public Functions (in mod.rs):**

| Function | Signature | Purpose |
|----------|-----------|---------|
| `shellexpand` | `pub fn shellexpand(path: &str) -> String` | Expand `~` to home directory in path strings |
| `migrate_layout` | `pub fn migrate_layout(data_dir: &str, config_path: &str)` | Migrate flat `~/.omega/` layout to structured subdirectories (data/, logs/, prompts/) |
| `patch_heartbeat_interval` | `pub fn patch_heartbeat_interval(config_path: &str, minutes: u64)` | Text-based patching of `interval_minutes` in config.toml's `[heartbeat]` section |
| `load` | `pub fn load(path: &str) -> Result<Config, OmegaError>` | Read and parse TOML config; return defaults if file missing |

**Public Functions (in prompts.rs, re-exported from config):**

| Function | Signature | Purpose |
|----------|-----------|---------|
| `install_bundled_prompts` | `pub fn install_bundled_prompts(data_dir: &str)` | Deploy bundled SYSTEM_PROMPT.md and WELCOME.toml to data_dir/prompts/ (never overwrites existing) |
| `bundled_workspace_claude` | `pub fn bundled_workspace_claude() -> &'static str` | Return the bundled WORKSPACE_CLAUDE.md template string |

**`Prompts` Struct Fields:**

| Field | Type | Description |
|-------|------|-------------|
| `identity` | `String` | Agent identity prompt |
| `soul` | `String` | Agent personality and values |
| `system` | `String` | Core behavioral rules (always injected) |
| `scheduling` | `String` | Scheduling rules (conditionally injected) |
| `projects_rules` | `String` | Project management rules (conditionally injected) |
| `builds` | `String` | Build rules (conditionally injected) |
| `meta` | `String` | Meta rules (conditionally injected) |
| `summarize` | `String` | Conversation summarization instruction |
| `facts` | `String` | Facts extraction instruction |
| `heartbeat` | `String` | Heartbeat prompt (no checklist) |
| `heartbeat_checklist` | `String` | Heartbeat prompt with `{checklist}` placeholder |
| `welcome` | `HashMap<String, String>` | Welcome messages keyed by language name |

**`Prompts` Methods:**

| Method | Signature | Purpose |
|--------|-----------|---------|
| `load` | `pub fn load(data_dir: &str) -> Self` | Load prompts from `{data_dir}/prompts/SYSTEM_PROMPT.md` and `WELCOME.toml`, falling back to defaults |

**Default Values (private helper functions in defaults.rs):**

| Function | Returns | Value |
|----------|---------|-------|
| `default_name()` | `String` | `"OMEGA \u{03a9}"` |
| `default_data_dir()` | `String` | `"~/.omega"` |
| `default_log_level()` | `String` | `"info"` |
| `default_provider()` | `String` | `"claude-code"` |
| `default_true()` | `bool` | `true` |
| `default_deny_message()` | `String` | `"Access denied. You are not authorized to use this agent."` |
| `default_max_turns()` | `u32` | `25` |
| `default_allowed_tools()` | `Vec<String>` | `[]` (empty) |
| `default_anthropic_model()` | `String` | `"claude-sonnet-4-20250514"` |
| `default_openai_model()` | `String` | `"gpt-4o"` |
| `default_openai_base_url()` | `String` | `"https://api.openai.com/v1"` |
| `default_ollama_base_url()` | `String` | `"http://localhost:11434"` |
| `default_ollama_model()` | `String` | `"llama3"` |
| `default_gemini_model()` | `String` | `"gemini-2.0-flash"` |
| `default_memory_backend()` | `String` | `"sqlite"` |
| `default_db_path()` | `String` | `"~/.omega/data/memory.db"` |
| `default_max_context()` | `usize` | `50` |
| `default_heartbeat_interval()` | `u64` | `30` |
| `default_poll_interval()` | `u64` | `60` |
| `default_api_host()` | `String` | `"127.0.0.1"` |
| `default_api_port()` | `u16` | `3000` |
| `default_timeout_secs()` | `u64` | `3600` |
| `default_max_resume_attempts()` | `u32` | `5` |
| `default_model()` | `String` | `"claude-sonnet-4-6"` |
| `default_model_complex()` | `String` | `"claude-opus-4-6"` |

---

### `context`

**File:** `backend/crates/omega-core/src/context.rs`

**Purpose:** Represents the conversation context that is passed to AI providers. Carries a system prompt, conversation history, and the current user message. Also defines `ContextNeeds` for selective context loading and `ApiMessage` for structured API providers.

**Public Structs:**

| Struct | Derives | Purpose |
|--------|---------|---------|
| `ContextNeeds` | (none) | Controls which optional context blocks are loaded and injected |
| `ContextEntry` | Debug, Clone, Serialize, Deserialize | Single history entry with `role` ("user"/"assistant") and `content` |
| `McpServer` | Debug, Clone, Serialize, Deserialize, Default | MCP server declared by a skill |
| `Context` | Debug, Clone, Serialize, Deserialize | Full conversation context: system prompt + history + current message + overrides |
| `ApiMessage` | Debug, Clone, Serialize, Deserialize | Structured message for API-based providers |

**Public Methods on `Context`:**

| Method | Signature | Purpose |
|--------|-----------|---------|
| `new` | `pub fn new(message: &str) -> Self` | Create a context with the default system prompt, empty history, and a given message |
| `to_prompt_string` | `pub fn to_prompt_string(&self) -> String` | Flatten context into a single delimited string for single-input providers |
| `to_api_messages` | `pub fn to_api_messages(&self) -> (String, Vec<ApiMessage>)` | Convert context to structured API messages (system_prompt, messages) |

---

### `error`

**File:** `backend/crates/omega-core/src/error.rs`

**Purpose:** Defines the unified error enum used across the entire Omega workspace.

**Public Enum:** `OmegaError` (7 variants, 2 with `#[from]` for automatic conversion).

---

### `message`

**File:** `backend/crates/omega-core/src/message.rs`

**Purpose:** Defines the message types that flow through the gateway pipeline: incoming messages from channels, outgoing responses to channels, response metadata, and file attachments.

**Public Structs:** `IncomingMessage`, `OutgoingMessage`, `MessageMetadata`, `Attachment`

**Public Enum:** `AttachmentType` (5 variants: `Image`, `Document`, `Audio`, `Video`, `Other`)

---

### `sanitize`

**File:** `backend/crates/omega-core/src/sanitize.rs`

**Purpose:** Prompt injection defense. Neutralizes role impersonation tags, instruction override phrases, and suspicious code blocks before user input reaches the AI provider.

**Public Struct:** `SanitizeResult` (text, was_modified, warnings)

**Public Function:** `sanitize(input: &str) -> SanitizeResult`

---

### `traits`

**File:** `backend/crates/omega-core/src/traits.rs`

**Purpose:** Defines the two core async trait interfaces that all providers and channels must implement.

**Traits:**

#### `Provider` (async_trait)

| Method | Signature | Purpose |
|--------|-----------|---------|
| `name` | `fn name(&self) -> &str` | Human-readable provider name |
| `requires_api_key` | `fn requires_api_key(&self) -> bool` | Whether an API key is needed |
| `complete` | `async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError>` | Send context, get response |
| `is_available` | `async fn is_available(&self) -> bool` | Readiness check |

**Supertraits:** `Send + Sync` (required for `Arc<dyn Provider>` sharing across async tasks).

#### `Channel` (async_trait)

| Method | Signature | Default | Purpose |
|--------|-----------|---------|---------|
| `name` | `fn name(&self) -> &str` | -- | Human-readable channel name |
| `start` | `async fn start(&self) -> Result<Receiver<IncomingMessage>, OmegaError>` | -- | Begin listening, return message receiver |
| `send` | `async fn send(&self, message: OutgoingMessage) -> Result<(), OmegaError>` | -- | Send response to channel |
| `send_typing` | `async fn send_typing(&self, _target: &str) -> Result<(), OmegaError>` | `Ok(())` | Send typing indicator (optional) |
| `send_photo` | `async fn send_photo(&self, _target: &str, _image: &[u8], _caption: &str) -> Result<(), OmegaError>` | `Ok(())` | Send a photo with caption (optional) |
| `stop` | `async fn stop(&self) -> Result<(), OmegaError>` | -- | Graceful shutdown |
| `as_any` | `fn as_any(&self) -> &dyn Any` | -- | Downcast support for channel-specific methods |

**Supertraits:** `Send + Sync`.

---

## Public API Surface (by access path)

| Access Path | Kind |
|-------------|------|
| `omega_core::shellexpand` | Function (re-exported from config) |
| `omega_core::config::Config` | Struct |
| `omega_core::config::OmegaConfig` | Struct |
| `omega_core::config::AuthConfig` | Struct |
| `omega_core::config::ProviderConfig` | Struct |
| `omega_core::config::ClaudeCodeConfig` | Struct |
| `omega_core::config::AnthropicConfig` | Struct |
| `omega_core::config::OpenAiConfig` | Struct |
| `omega_core::config::OllamaConfig` | Struct |
| `omega_core::config::OpenRouterConfig` | Struct |
| `omega_core::config::GeminiConfig` | Struct |
| `omega_core::config::ChannelConfig` | Struct |
| `omega_core::config::TelegramConfig` | Struct |
| `omega_core::config::WhatsAppConfig` | Struct |
| `omega_core::config::MemoryConfig` | Struct |
| `omega_core::config::HeartbeatConfig` | Struct |
| `omega_core::config::SchedulerConfig` | Struct |
| `omega_core::config::ApiConfig` | Struct |
| `omega_core::config::Prompts` | Struct |
| `omega_core::config::SYSTEM_FACT_KEYS` | Constant |
| `omega_core::config::shellexpand` | Function |
| `omega_core::config::migrate_layout` | Function |
| `omega_core::config::patch_heartbeat_interval` | Function |
| `omega_core::config::load` | Function |
| `omega_core::config::install_bundled_prompts` | Function |
| `omega_core::config::bundled_workspace_claude` | Function |
| `omega_core::context::ContextNeeds` | Struct |
| `omega_core::context::ContextEntry` | Struct |
| `omega_core::context::McpServer` | Struct |
| `omega_core::context::Context` | Struct |
| `omega_core::context::ApiMessage` | Struct |
| `omega_core::error::OmegaError` | Enum |
| `omega_core::message::IncomingMessage` | Struct |
| `omega_core::message::OutgoingMessage` | Struct |
| `omega_core::message::MessageMetadata` | Struct |
| `omega_core::message::Attachment` | Struct |
| `omega_core::message::AttachmentType` | Enum |
| `omega_core::sanitize::SanitizeResult` | Struct |
| `omega_core::sanitize::sanitize` | Function |
| `omega_core::traits::Provider` | Trait |
| `omega_core::traits::Channel` | Trait |

---

## Summary Table

| Component | File(s) | Kind | Count |
|-----------|---------|------|-------|
| `lib.rs` | lib.rs | Crate root | 6 module declarations, 1 re-export |
| `config` | config/ (6 files) | Directory module | 18 public structs, 1 constant, 6 public functions, 24+ default helpers |
| `context` | context.rs | Module | 5 public structs, 3 public methods, 1 private function |
| `error` | error.rs | Module | 1 public enum (7 variants, 2 with `#[from]`) |
| `message` | message.rs | Module | 4 public structs, 1 public enum (5 variants) |
| `sanitize` | sanitize.rs | Module | 1 public struct, 1 public function, 5 tests |
| `traits` | traits.rs | Module | 2 public traits (4 + 7 methods) |
