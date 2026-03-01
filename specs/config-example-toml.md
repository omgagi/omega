# Configuration Specification: config.example.toml

## Overview

**File Path:** `backend/config.example.toml` (committed to repository; actual config is `config.toml`, which is gitignored)

**Purpose:** Template configuration file for Omega personal AI agent infrastructure. Defines bot identity, 6 provider backends, 2 messaging channels, memory storage, scheduler, heartbeat, HTTP API, and security policies.

**Usage:** Copy to `config.toml` and customize for your deployment. The `omega init` wizard generates a `config.toml` automatically, but this file serves as the manual reference.

## File Structure

The configuration uses TOML format with 11 sections + 1 comment-only section:

```toml
[omega]              # Core bot identity and logging
[auth]               # Authentication and authorization
[provider]           # AI backend provider selection
[provider.claude-code]  # Claude Code CLI (default)
[provider.anthropic]    # Anthropic API
[provider.openai]       # OpenAI-compatible API
[provider.ollama]       # Ollama local
[provider.openrouter]   # OpenRouter aggregation
[provider.gemini]       # Google Gemini API
[channel.telegram]   # Telegram bot
[channel.whatsapp]   # WhatsApp via QR pairing
[memory]             # Conversation storage backend
[scheduler]          # Scheduled task delivery
[heartbeat]          # Periodic AI health check-in
[api]                # HTTP API for SaaS dashboard integration
# Security           # Comment-only (always active)
```

---

## Section: `[omega]`

Core bot configuration and global settings.

### Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `name` | String | `"OMEGA O"` | Display name for the bot. Used in logs, responses, and service identification. |
| `data_dir` | Path | `"~/.omega"` | Directory for persistent data (database, logs, prompts, skills, projects). Supports `~` expansion. |
| `log_level` | String | `"info"` | Logging verbosity. Valid values: `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"`. |

### Notes

- The `data_dir` path is created automatically if it does not exist.
- Log files are written to `{data_dir}/logs/omega.log` via `tracing-appender`.
- Database is stored at the path specified in `[memory].db_path` (default: `{data_dir}/data/memory.db`).
- Subdirectories created automatically: `data/`, `logs/`, `prompts/`, `skills/`, `projects/`, `workspace/`, `stores/`, `topologies/`.

---

## Section: `[auth]`

Authentication and authorization controls.

### Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `true` | Global auth enforcement flag. When `true`, requires each channel to define `allowed_users`. |
| `deny_message` | String | `"Access denied..."` | Message returned to unauthorized users. |

---

## Section: `[provider]`

AI backend configuration. Omega supports 6 providers; exactly one must be set as `default`.

### Global Provider Keys

| Key | Type | Description |
|-----|------|-------------|
| `default` | String | The active provider name. Must match a subsection name. |

### `[provider.claude-code]` -- Claude Code CLI (Default)

Local Claude Code CLI integration (zero-config, recommended default).

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `true` | Enable/disable this provider. |
| `max_turns` | Integer | `100` | Maximum conversation turns per invocation. |
| `allowed_tools` | Array[String] | `[]` | Tool allowlist. Empty = full tool access (all Claude Code tools available). |
| `timeout_secs` | Integer | `3600` | Max wait for CLI response (60-minute ceiling). |
| `max_resume_attempts` | Integer | `5` | Auto-resume when Claude hits max_turns (0 = disabled). |
| `model` | String | `"claude-sonnet-4-6"` | Fast model: classification + direct responses. |
| `model_complex` | String | `"claude-opus-4-6"` | Complex model: multi-step autonomous execution. |

**Authentication:** No API key required. Uses local `claude` CLI authentication.

### `[provider.anthropic]` -- Anthropic API

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable Anthropic API. |
| `api_key` | String | `""` | API key (or env: `ANTHROPIC_API_KEY`). |
| `model` | String | `"claude-sonnet-4-20250514"` | Model identifier. |

### `[provider.openai]` -- OpenAI-compatible

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable OpenAI API. |
| `api_key` | String | `""` | API key (or env: `OPENAI_API_KEY`). |
| `model` | String | `"gpt-4o"` | Model identifier. |
| `base_url` | String | `"https://api.openai.com/v1"` | API endpoint URL (customizable for compatible endpoints). |

### `[provider.ollama]` -- Ollama (local)

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable local Ollama. |
| `base_url` | String | `"http://localhost:11434"` | Ollama server endpoint. |
| `model` | String | `"llama3"` | Model name. |

### `[provider.openrouter]` -- OpenRouter

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable OpenRouter. |
| `api_key` | String | `""` | API key (or env: `OPENROUTER_API_KEY`). |
| `model` | String | `"anthropic/claude-sonnet-4-20250514"` | Model identifier with namespace prefix. |

### `[provider.gemini]` -- Google Gemini

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable Gemini API. |
| `api_key` | String | `""` | API key (or env: `GEMINI_API_KEY`). |
| `model` | String | `"gemini-2.0-flash"` | Model identifier. |

### Provider Selection Logic

1. One provider must be marked `enabled = true`.
2. The `default` key specifies which enabled provider to use.
3. For Claude Code, `model` and `model_complex` produce different fast/complex model strings. For all other providers, both fast and complex map to the same single `model` value.

---

## Section: `[channel.*]`

Messaging platform integrations. Each channel is a subsection.

### `[channel.telegram]`

Telegram bot integration via long polling.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable Telegram. |
| `bot_token` | String | `""` | Bot token from @BotFather (or env: `TELEGRAM_BOT_TOKEN`). |
| `allowed_users` | Array[Integer] | `[]` | Telegram user IDs. Empty = allow all. |
| `whisper_api_key` | String | `""` | OpenAI key for voice transcription (or env: `OPENAI_API_KEY`). |

### `[channel.whatsapp]`

WhatsApp Web protocol via QR pairing (not a bridge/webhook service).

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable WhatsApp. |
| `allowed_users` | Array[String] | `[]` | Phone numbers (e.g. `["5511999887766"]`). Empty = allow all. |
| `whisper_api_key` | String | `""` | OpenAI key for voice transcription (or env: `OPENAI_API_KEY`). |

**Note:** WhatsApp does not use a token or bridge URL. Session is established via QR code pairing (`omega pair` or during `omega init`). Session data is stored in `~/.omega/whatsapp_session/`.

---

## Section: `[memory]`

Conversation memory and context storage.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `backend` | String | `"sqlite"` | Storage backend (only SQLite supported). |
| `db_path` | Path | `"~/.omega/data/memory.db"` | Database file path. Supports `~` expansion. Auto-created on first run. |
| `max_context_messages` | Integer | `50` | Max messages included in conversation context. |

---

## Section: `[scheduler]`

Scheduled task delivery configuration.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `true` | Enable/disable scheduler. Zero cost when no tasks exist. |
| `poll_interval_secs` | Integer | `60` | Seconds between task queue polls. |

---

## Section: `[heartbeat]`

Periodic AI health check-in configuration.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable heartbeat loop. |
| `interval_minutes` | Integer | `30` | Minutes between check-ins. |
| `active_start` | String | `"08:00"` | Active window start (empty = always active). |
| `active_end` | String | `"22:00"` | Active window end. |
| `channel` | String | `"telegram"` | Channel for alert delivery. |
| `reply_target` | String | `""` | Chat ID for delivery. |

---

## Section: `[api]`

HTTP API server for SaaS dashboard integration.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable API server. |
| `host` | String | `"127.0.0.1"` | Bind address (localhost only -- use reverse proxy for external). |
| `port` | Integer | `3000` | Listen port. |
| `api_key` | String | `""` | Bearer token auth. Empty = no auth (local-only use). |

---

## Security (Comment-Only Section)

No configuration fields. System protection is always active:
- **OS-level:** Blocks writes to `/System`, `/bin`, `/sbin`, `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/libexec`, `/private/etc`, `/Library`, and `~/.omega/data/`
- **Code-level:** Blocks writes to `memory.db` from HTTP provider tools
- **Domain data:** Use `~/.omega/stores/` for skill-specific databases

---

## Environment Variable Overrides

| Env Var | Config Key |
|---------|-----------|
| `ANTHROPIC_API_KEY` | `[provider.anthropic] api_key` |
| `OPENAI_API_KEY` | `[provider.openai] api_key` (also: `whisper_api_key` fallback) |
| `OPENROUTER_API_KEY` | `[provider.openrouter] api_key` |
| `GEMINI_API_KEY` | `[provider.gemini] api_key` |
| `TELEGRAM_BOT_TOKEN` | `[channel.telegram] bot_token` |

---

## Summary

- **6 providers:** claude-code, anthropic, openai, ollama, openrouter, gemini
- **2 channels:** telegram, whatsapp
- **11 config sections + 1 comment-only security section**
- **Background loops:** scheduler (enabled by default), heartbeat (disabled by default)
- **HTTP API:** disabled by default, axum-based, Bearer token auth
