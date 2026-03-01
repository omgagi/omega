# Omega Configuration System

## Path

`backend/crates/omega-core/src/config.rs`

## Overview

Omega uses a single TOML file (`config.toml`) to control every aspect of the agent: identity, AI providers, messaging channels, memory, auth, and sandboxing. The config system is designed around a zero-config philosophy -- if you delete the file entirely, Omega will start with sensible defaults and use the Claude Code CLI as its provider.

All configuration types live in `omega-core` so that every other crate in the workspace can depend on them without circular imports.

## Quick Start

Copy the example config and edit it:

```bash
cp config.example.toml config.toml
```

Or run the interactive setup wizard:

```bash
cargo run -- init
```

If you do nothing at all, `omega ask "hello"` will work out of the box using the Claude Code CLI (assuming `claude` is installed and authenticated).

## File Location

Omega looks for `config.toml` in the current working directory by default. You can override this with the `--config` flag:

```bash
omega --config /path/to/my-config.toml start
```

The config file is gitignored because it may contain secrets (API keys, bot tokens). The committed `config.example.toml` serves as a reference template.

## Full TOML Structure

```toml
[omega]
name = "OMEGA \u{03a9}"
data_dir = "~/.omega"
log_level = "info"

[auth]
enabled = true
deny_message = "Access denied. You are not authorized to use this agent."

[provider]
default = "claude-code"

[provider.claude-code]
enabled = true
max_turns = 25
allowed_tools = []  # empty = full tool access
timeout_secs = 3600
max_resume_attempts = 5
model = "claude-sonnet-4-6"
model_complex = "claude-opus-4-6"

[provider.anthropic]
enabled = false
api_key = ""
model = "claude-sonnet-4-20250514"
max_tokens = 8192

[provider.openai]
enabled = false
api_key = ""
model = "gpt-4o"
base_url = "https://api.openai.com/v1"

[provider.ollama]
enabled = false
base_url = "http://localhost:11434"
model = "llama3"

[provider.openrouter]
enabled = false
api_key = ""
model = "anthropic/claude-sonnet-4-20250514"

[provider.gemini]
enabled = false
api_key = ""
model = "gemini-2.0-flash"

[channel.telegram]
enabled = false
bot_token = ""
allowed_users = []

[channel.whatsapp]
enabled = false
allowed_users = []

[memory]
backend = "sqlite"
db_path = "~/.omega/data/memory.db"
max_context_messages = 50

[scheduler]
enabled = true
poll_interval_secs = 60

[heartbeat]
enabled = false
interval_minutes = 30
active_start = "08:00"
active_end = "22:00"
channel = "telegram"
reply_target = ""

[api]
enabled = false
host = "127.0.0.1"
port = 3000
api_key = ""
```

Every section except `[omega]` can be omitted entirely and Omega will use defaults.

> **Note:** There is no `[sandbox]` config section. Filesystem protection is always-on via `omega_sandbox`'s blocklist approach (Seatbelt on macOS, Landlock on Linux).

## Section-by-Section Guide

### `[omega]` -- Agent Identity

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `name` | string | `"OMEGA \u{03a9}"` | Display name for the agent. Used in system prompts and logs. |
| `data_dir` | string | `"~/.omega"` | Directory for databases, logs, and runtime files. The `~` is expanded to your home directory at runtime. |
| `log_level` | string | `"info"` | Tracing level. Can also be overridden by the `RUST_LOG` environment variable. |

### `[auth]` -- Access Control

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `true` | When true, every incoming message is checked against the channel's `allowed_users` list. |
| `deny_message` | string | `"Access denied..."` | The message sent back to unauthorized users. |

When auth is enabled and a channel's `allowed_users` list is empty, all users on that channel are allowed (this is a convenience for development). When `allowed_users` contains specific IDs, only those users can interact with the agent.

### `[provider]` -- AI Backends

The `default` key selects which provider handles messages. Currently supported values: `"claude-code"`.

#### `[provider.claude-code]` -- Claude Code CLI

This is the primary, zero-config provider. It shells out to the `claude` CLI tool.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `true` | Whether this provider is available for selection. |
| `max_turns` | integer | `25` | Maximum number of agentic turns Claude Code can take per request. |
| `allowed_tools` | array of strings | `[]` (empty = full tool access) | Which Claude Code tools the agent is allowed to use. Empty array grants access to all tools. |
| `timeout_secs` | integer | `3600` | Max seconds to wait for CLI response. 60-minute ceiling. |
| `max_resume_attempts` | integer | `5` | Max auto-resume attempts when CLI hits max turns with a session ID. |
| `model` | string | `"claude-sonnet-4-6"` | Default model for fast classification and direct responses. Passed as `--model` to the CLI. |
| `model_complex` | string | `"claude-opus-4-6"` | Model used for complex multi-step task execution. Used when the classifier routes to step-by-step processing. |

No API key is needed -- Claude Code uses the local CLI's authentication.

#### `[provider.anthropic]` -- Anthropic API

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Enable this provider. |
| `api_key` | string | `""` | Your Anthropic API key. Can also be set via `ANTHROPIC_API_KEY` env var. |
| `model` | string | `"claude-sonnet-4-20250514"` | Model identifier. |
| `max_tokens` | integer | `8192` | Maximum tokens for the response. |

#### `[provider.openai]` -- OpenAI-Compatible API

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Enable this provider. |
| `api_key` | string | `""` | Your API key. Can also be set via `OPENAI_API_KEY` env var. |
| `model` | string | `"gpt-4o"` | Model identifier. |
| `base_url` | string | `"https://api.openai.com/v1"` | API base URL. Change this to point at any OpenAI-compatible endpoint. |

#### `[provider.ollama]` -- Ollama (Local)

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Enable this provider. |
| `base_url` | string | `"http://localhost:11434"` | Ollama server URL. |
| `model` | string | `"llama3"` | Model to use. |

No API key needed -- Ollama runs locally.

#### `[provider.openrouter]` -- OpenRouter Proxy

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Enable this provider. |
| `api_key` | string | `""` | Your OpenRouter API key. Can also be set via `OPENROUTER_API_KEY` env var. |
| `model` | string | `""` | Model identifier (e.g., `"anthropic/claude-sonnet-4-20250514"`). |

#### `[provider.gemini]` -- Google Gemini API

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Enable this provider. |
| `api_key` | string | `""` | Your Gemini API key. |
| `model` | string | `"gemini-2.0-flash"` | Model identifier. |

### `[channel.telegram]` -- Telegram Bot

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Enable the Telegram channel. |
| `bot_token` | string | `""` | Bot token from @BotFather. Can also be set via `TELEGRAM_BOT_TOKEN` env var. |
| `allowed_users` | array of integers | `[]` | Telegram user IDs allowed to interact. Empty means allow all. |

### `[channel.whatsapp]` -- WhatsApp (Native)

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Enable the WhatsApp channel. |
| `allowed_users` | array of strings | `[]` | Phone numbers allowed to interact (e.g., `["5511999887766"]`). Empty means allow all. |
| `whisper_api_key` | string | `null` | OpenAI API key for Whisper voice transcription. When present, voice messages are transcribed. |

Session data is stored at `{data_dir}/whatsapp_session/`. Pairing is done by scanning a QR code (like WhatsApp Web).

### `[memory]` -- Conversation Storage

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `backend` | string | `"sqlite"` | Storage backend. Currently only `"sqlite"` is supported. |
| `db_path` | string | `"~/.omega/data/memory.db"` | Path to the SQLite database. `~` is expanded at runtime. |
| `max_context_messages` | integer | `50` | How many recent messages to include when building context for the provider. |

### `[scheduler]` -- Task Queue

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `true` | Whether the scheduler background loop runs. Safe to leave enabled -- it has zero cost when no tasks exist. |
| `poll_interval_secs` | integer | `60` | How often (in seconds) the scheduler checks for due tasks. Lower values mean faster delivery but slightly more database polling. |

The scheduler delivers reminders and recurring tasks that users create through natural language (e.g., "remind me to call John at 3pm"). Tasks are stored in the `scheduled_tasks` SQLite table and delivered via the channel that created them.

### `[heartbeat]` -- Periodic Check-in

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Whether the heartbeat background loop runs. Disabled by default because it requires channel and reply_target to be configured. |
| `interval_minutes` | integer | `30` | How often (in minutes) the heartbeat fires. |
| `active_start` | string | `""` | Start of the active hours window in `HH:MM` format (e.g., `"08:00"`). Empty means always active. |
| `active_end` | string | `""` | End of the active hours window in `HH:MM` format (e.g., `"22:00"`). Empty means always active. |
| `channel` | string | `""` | Which channel to deliver heartbeat alerts on (e.g., `"telegram"`). |
| `reply_target` | string | `""` | Platform-specific target for delivery (e.g., a Telegram chat ID). |

The heartbeat calls the AI provider periodically to perform a health check. If the provider responds with `HEARTBEAT_OK`, the result is suppressed (log only). Otherwise, the response is sent as an alert to the configured channel and reply target. An optional `~/.omega/prompts/HEARTBEAT.md` file can contain a checklist for the AI to evaluate.

### `[api]` -- HTTP API Server

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Whether the HTTP API server is started. Disabled by default. |
| `host` | string | `"127.0.0.1"` | IP address to bind to. Use `"0.0.0.0"` for external access. |
| `port` | integer | `3000` | Port to listen on. |
| `api_key` | string | `""` | Bearer token for API authentication. Empty = no auth (suitable for local-only use). |

When enabled, serves health check and WhatsApp QR pairing endpoints for SaaS dashboard integration.

### Filesystem Protection (Always-On)

There is no `[sandbox]` config section. Filesystem protection is always active via the `omega_sandbox` crate's blocklist approach. The workspace directory `~/.omega/workspace/` is created automatically on startup and serves as the AI's working directory.

- `omega_sandbox::protected_command()` wraps subprocess execution with OS-level protection (Seatbelt on macOS, Landlock on Linux), blocking writes to dangerous system directories and OMEGA's core database.
- `omega_sandbox::is_write_blocked()` checks paths at the tool level, denying writes to protected locations.
- The workspace (`~/.omega/workspace/`) and `/tmp` are always writable.

## Environment Variables

The config file is the primary source of truth, but several values can be supplied or overridden via environment variables:

| Env Var | Overrides | Notes |
|---------|-----------|-------|
| `RUST_LOG` | `omega.log_level` | Standard Rust tracing filter. Takes full precedence over the config value. Example: `RUST_LOG=debug omega start` |
| `ANTHROPIC_API_KEY` | `provider.anthropic.api_key` | Read by the Anthropic provider at runtime. |
| `OPENAI_API_KEY` | `provider.openai.api_key` | Read by the OpenAI provider at runtime. |
| `OPENROUTER_API_KEY` | `provider.openrouter.api_key` | Read by the OpenRouter provider at runtime. |
| `TELEGRAM_BOT_TOKEN` | `channel.telegram.bot_token` | Read by the Telegram channel at runtime. |

These overrides happen in the individual provider and channel crates, not in the config module itself. The config module performs pure file-based deserialization.

## How Config Loading Works

The `config::load(path)` function follows this sequence:

1. Check if the file at `path` exists.
2. If it does not exist, log a message and return a fully-defaulted config. The default config has Claude Code pre-enabled so the agent works without any file.
3. If it exists, read the file contents and parse them as TOML.
4. Any missing sections or fields are filled in with serde defaults (the `#[serde(default)]` and `#[serde(default = "function")]` annotations).
5. Return the parsed config or an error.

There is no validation beyond TOML parsing. Semantic checks (is the provider available? is the bot token non-empty?) happen later in `main.rs` and the self-check module.

## How to Add a New Config Option

### Adding a field to an existing section

1. Open `backend/crates/omega-core/src/config.rs`.
2. Add your field to the appropriate struct. Use `#[serde(default)]` for a type-default or `#[serde(default = "your_default_fn")]` for a custom default:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_memory_backend")]
    pub backend: String,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_max_context")]
    pub max_context_messages: usize,
    // NEW:
    #[serde(default = "default_pruning_enabled")]
    pub pruning_enabled: bool,
}
```

3. If you used a custom default function, add it to the defaults section at the bottom of the file:

```rust
fn default_pruning_enabled() -> bool {
    true
}
```

4. If the struct has a manual `Default` impl, update it to include your new field:

```rust
impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            backend: default_memory_backend(),
            db_path: default_db_path(),
            max_context_messages: default_max_context(),
            pruning_enabled: default_pruning_enabled(),  // NEW
        }
    }
}
```

5. Update `config.example.toml` with the new key and a comment explaining it.
6. Run `cargo check --workspace` to make sure everything compiles.

### Adding a new section

1. Define a new struct with `#[derive(Debug, Clone, Serialize, Deserialize)]`.
2. Add it as a field on `Config` with `#[serde(default)]`.
3. Implement `Default` for your struct (either derive it or write a manual impl).
4. If needed, add it to the file-absent fallback in `load()`.

### Naming conventions

- Rust fields use `snake_case`.
- TOML keys use `kebab-case` for multi-word names (use `#[serde(rename = "kebab-case")]`).
- Default functions are named `default_<field>()` and are private to the module.
- Use `Option<T>` for truly optional subsections (where "not present" is meaningfully different from "present with defaults").
- Use `#[serde(default)]` for sections that should always exist with sensible defaults.

## Bundled Prompts (Auto-Deployed)

On first startup, Omega automatically deploys two template files to `data_dir` (default `~/.omega/`):

- **`SYSTEM_PROMPT.md`** — The system prompt with three main sections (`## Identity`, `## Soul`, `## System`) plus auxiliary sections (Summarize, Facts, Heartbeat, Heartbeat Checklist). Identity defines the agent as an autonomous executor with concrete behavioral examples. Soul defines personality, context-aware tone (precise for code, patient for personal), and explicit boundaries (privacy, half-baked replies, human-judgment escalation). System defines operational rules including reward-based learning awareness and marker instructions. Edit this file to customize the AI's behavior.
- **`WELCOME.toml`** — Privacy-focused welcome messages in 8 languages (English, Spanish, Portuguese, French, German, Italian, Dutch, Russian). Edit this file to customize the greeting users receive.

These files are embedded in the binary at compile time from the `prompts/` directory in the repository. On startup, `install_bundled_prompts()` writes them to `data_dir` only if they don't already exist — **user edits are never overwritten**.

This follows the same pattern as bundled skills (`~/.omega/skills/*.md`).

## Security Notes

- **Never commit `config.toml`** -- it is gitignored for a reason. It may contain API keys and bot tokens.
- The `config.example.toml` file is committed and should contain empty strings for secrets.
- Prefer environment variables for secrets in production or CI environments.
- Auth is enabled by default. Disabling it (`enabled = false`) allows anyone who can reach your channels to use the agent.
