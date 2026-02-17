# Configuration Specification: config.example.toml

## Overview

**File Path:** `config.example.toml` (committed to repository; actual config is `config.toml`, which is gitignored)

**Purpose:** Template configuration file for Omega personal AI agent infrastructure. Defines bot identity, provider backends, messaging channels, memory storage, and security policies.

**Usage:** Copy to `config.toml` and customize for your deployment. The actual `config.toml` file containing sensitive data (API keys, bot tokens) must never be committed to version control.

## File Structure

The configuration uses TOML format with six main sections:

```toml
[omega]           # Core bot identity and logging
[auth]            # Authentication and authorization
[provider]        # AI backend provider selection and configuration
[channel.*]       # Messaging platform integrations
[memory]          # Conversation storage backend
[scheduler]       # Scheduled task delivery
[heartbeat]       # Periodic AI health check-in
[sandbox]         # Command execution security
```

---

## Section: `[omega]`

Core bot configuration and global settings.

### Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `name` | String | `"Ω OMEGA"` | Display name for the bot. Used in logs, responses, and service identification. |
| `data_dir` | Path | `"~/.omega"` | Directory for persistent data (database, logs, cache). Supports `~` expansion for home directory. Must be writable by the process owner. |
| `log_level` | String | `"info"` | Logging verbosity. Valid values: `"trace"`, `"debug"`, `"info"`, `"warn"`, `"error"`. Controls `tracing` output detail. |

### Example

```toml
[omega]
name = "Ω OMEGA"
data_dir = "~/.omega"
log_level = "info"
```

### Notes

- The `data_dir` path is created automatically if it does not exist (with appropriate permissions).
- Log files are written to `data_dir/omega.log`.
- Database is stored in `data_dir/memory.db` (if using SQLite backend).

---

## Section: `[auth]`

Authentication and authorization controls.

### Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `true` | Global auth enforcement flag. When `true`, requires each channel to define `allowed_users`. When `false`, all users are allowed (not recommended for production). |
| `deny_message` | String | `"Access denied. You are not authorized to use this agent."` | Message returned to unauthorized users attempting to interact with the bot. Customize for user-friendly feedback. |

### Example

```toml
[auth]
enabled = true
deny_message = "Access denied. You are not authorized to use this agent."
```

### Notes

- **Security Sensitive:** Auth controls prevent unauthorized access.
- If `enabled = true`, each channel (`[channel.telegram]`, `[channel.whatsapp]`) must explicitly define `allowed_users` with a list of authorized identifiers.
- Empty `allowed_users` in a channel allows all users if `auth.enabled = false`, but is interpreted as "no users allowed" if `auth.enabled = true`.

---

## Section: `[provider]`

AI backend configuration. Omega supports multiple providers; exactly one must be set as `default`.

### Global Provider Keys

| Key | Type | Description |
|-----|------|-------------|
| `default` | String | The primary provider to use. Must match the name of a subsection (e.g., `"claude-code"`, `"anthropic"`, `"openai"`). |

### Subsections: Provider Backends

Each backend is configured in its own subsection: `[provider.<name>]`

#### `[provider.claude-code]`

Local Claude Code CLI integration (zero-config, recommended default).

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `true` | Enable/disable this provider. When disabled, queries will not use this backend. |
| `max_turns` | Integer | `10` | Maximum conversation turns (exchanges) before forcing a summary. Prevents runaway context growth. |
| `allowed_tools` | Array[String] | `["Bash", "Read", "Write", "Edit"]` | Whitelist of tools the Claude Code provider can invoke. Restricts what operations are permitted. |
| `timeout_secs` | Integer | `600` | Max wait time in seconds for CLI response (10-minute ceiling). Tunable per deployment. |

**Authentication:**
- No API key required. Uses local `claude` CLI authentication already configured on the system.
- Internally removes `CLAUDECODE` env var to avoid nested session conflicts.

**Example:**
```toml
[provider.claude-code]
enabled = true
max_turns = 10
allowed_tools = ["Bash", "Read", "Write", "Edit"]
timeout_secs = 600
```

#### `[provider.anthropic]`

Anthropic Sonnet API backend (requires API key).

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable Anthropic API. |
| `api_key` | String | `""` (empty) | **SECURITY SENSITIVE:** Anthropic API key. Set via config or env var `ANTHROPIC_API_KEY`. Never commit non-empty values. |
| `model` | String | `"claude-sonnet-4-20250514"` | Model identifier. See [Anthropic models documentation](https://docs.anthropic.com). |

**Example:**
```toml
[provider.anthropic]
enabled = false
api_key = ""  # Or env: ANTHROPIC_API_KEY
model = "claude-sonnet-4-20250514"
```

#### `[provider.openai]`

OpenAI API backend (requires API key). Supports OpenAI-compatible endpoints.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable OpenAI API. |
| `api_key` | String | `""` (empty) | **SECURITY SENSITIVE:** OpenAI API key. Set via config or env var `OPENAI_API_KEY`. Never commit non-empty values. |
| `model` | String | `"gpt-4o"` | Model identifier. Examples: `"gpt-4o"`, `"gpt-4-turbo"`, etc. |
| `base_url` | String | `"https://api.openai.com/v1"` | API endpoint URL. Supports drop-in OpenAI-compatible services. |

**Example:**
```toml
[provider.openai]
enabled = false
api_key = ""  # Or env: OPENAI_API_KEY
model = "gpt-4o"
base_url = "https://api.openai.com/v1"
```

#### `[provider.ollama]`

Local Ollama LLM backend (no API key, requires local service).

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable local Ollama. |
| `base_url` | String | `"http://localhost:11434"` | Ollama service endpoint. Must be running locally or on accessible network. |
| `model` | String | `"llama3"` | Model name to use. Must be pulled/available in Ollama. |

**Example:**
```toml
[provider.ollama]
enabled = false
base_url = "http://localhost:11434"
model = "llama3"
```

**Prerequisites:**
- Ollama must be installed and running on the specified endpoint.
- The specified model must be available: `ollama pull <model>`.

#### `[provider.openrouter]`

OpenRouter aggregation API (requires API key). Supports hundreds of models.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable OpenRouter. |
| `api_key` | String | `""` (empty) | **SECURITY SENSITIVE:** OpenRouter API key. Set via config or env var `OPENROUTER_API_KEY`. Never commit non-empty values. |
| `model` | String | `"anthropic/claude-sonnet-4-20250514"` | Model identifier using OpenRouter's namespace format (e.g., `"anthropic/..."`, `"openai/..."`, `"meta-llama/..."`). |

**Example:**
```toml
[provider.openrouter]
enabled = false
api_key = ""  # Or env: OPENROUTER_API_KEY
model = "anthropic/claude-sonnet-4-20250514"
```

### Provider Selection Logic

1. One provider must be marked `enabled = true`.
2. The `default` key specifies which enabled provider to use.
3. If multiple providers are enabled, the `default` determines priority.
4. If the default provider is disabled, startup will fail.

---

## Section: `[channel.*]`

Messaging platform integrations. Each channel is a subsection: `[channel.telegram]`, `[channel.whatsapp]`, etc.

### `[channel.telegram]`

Telegram bot integration.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable Telegram bot polling/updates. |
| `bot_token` | String | `""` (empty) | **SECURITY SENSITIVE:** Telegram Bot API token from [@BotFather](https://t.me/botfather). Set via config or env var `TELEGRAM_BOT_TOKEN`. Never commit non-empty values. |
| `allowed_users` | Array[Integer] | `[]` (empty) | List of Telegram user IDs permitted to use the bot. If empty and `auth.enabled = true`, no users are allowed. If empty and `auth.enabled = false`, all users are allowed. User IDs are numeric identifiers, not usernames. |

**Example:**
```toml
[channel.telegram]
enabled = false
bot_token = ""              # Or env: TELEGRAM_BOT_TOKEN
allowed_users = []          # Empty = allow all (if auth disabled) or deny all (if auth enabled)
```

**Finding Your Telegram User ID:**
1. Message the bot or any Telegram account with `/start`.
2. Check the bot logs or Telegram webhook for your numeric ID.

### `[channel.whatsapp]`

WhatsApp integration via bridge service.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable WhatsApp bridge. |
| `bridge_url` | String | `"http://localhost:3000"` | URL of the WhatsApp bridge service (usually a local or remote webhook/proxy). |
| `phone_number` | String | `""` (empty) | **SECURITY SENSITIVE:** Phone number associated with the WhatsApp account (E.164 format, e.g., `"+14155552671"`). |

**Example:**
```toml
[channel.whatsapp]
enabled = false
bridge_url = "http://localhost:3000"
phone_number = ""
```

**Prerequisites:**
- A WhatsApp bridge service must be running (e.g., Twilio, WhatsApp Cloud API, or custom bridge).
- The `bridge_url` must be reachable and configured to forward messages to Omega.

### Channel Logic

- **Only enabled channels** are activated on startup.
- Each enabled channel requires authentication to be properly configured (if `auth.enabled = true`).
- Channels can run simultaneously; a message to one channel does not affect others.

---

## Section: `[memory]`

Conversation memory and context storage.

### Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `backend` | String | `"sqlite"` | Memory backend type. Currently only `"sqlite"` is supported. |
| `db_path` | Path | `"~/.omega/memory.db"` | Path to SQLite database file. Supports `~` expansion. Auto-created on first run. |
| `max_context_messages` | Integer | `50` | Maximum number of recent messages to include in context when querying the provider. Prevents context explosion while retaining conversation continuity. |

**Example:**
```toml
[memory]
backend = "sqlite"
db_path = "~/.omega/memory.db"
max_context_messages = 50
```

### Notes

- **SQLite:** Fully embedded, no external database server required.
- **Context Window:** When a user sends a message, Omega retrieves up to `max_context_messages` prior messages to provide context to the AI. Older messages are retrieved on-demand or via summaries.
- **Audit Trail:** All interactions (user message, provider response, metadata) are logged in the same database for accountability and debugging.
- The database file should be backed up regularly if conversations are important.

---

## Section: `[scheduler]`

Scheduled task delivery configuration. Controls the background loop that delivers due reminders and recurring tasks to users.

### Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `true` | Enable/disable the scheduler background loop. When `true`, Omega periodically checks for due tasks and delivers them. Zero cost when no tasks exist. |
| `poll_interval_secs` | Integer | `60` | How often (in seconds) the scheduler checks for due tasks. Lower values mean faster delivery at the cost of more database queries. |

### Example

```toml
[scheduler]
enabled = true                  # Zero cost when no tasks exist
poll_interval_secs = 60
```

### Notes

- The scheduler is enabled by default because it has no overhead when no scheduled tasks exist in the database.
- Tasks are created automatically when the AI provider includes a `SCHEDULE:` marker in its response (triggered by user requests like "remind me to...").
- Users can view pending tasks with `/tasks` and cancel them with `/cancel <id>`.
- Supported repeat patterns: `once` (one-shot), `daily`, `weekly`, `monthly`, `weekdays` (Mon-Fri).

---

## Section: `[heartbeat]`

Periodic AI health check-in configuration. When enabled, Omega periodically invokes the AI provider with a heartbeat prompt. If the provider reports all is well (`HEARTBEAT_OK`), the response is suppressed. If the provider reports an issue, an alert is delivered to the configured channel.

### Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | Boolean | `false` | Enable/disable the heartbeat background loop. Disabled by default because it requires channel and reply_target configuration. |
| `interval_minutes` | Integer | `30` | Minutes between heartbeat check-ins. |
| `active_start` | String | `""` (empty) | Start of active hours window in `"HH:MM"` format (e.g., `"08:00"`). When both start and end are empty, heartbeat is always active. |
| `active_end` | String | `""` (empty) | End of active hours window in `"HH:MM"` format (e.g., `"22:00"`). Midnight wrapping is supported (e.g., `"22:00"` to `"06:00"`). |
| `channel` | String | `""` (empty) | Channel name for alert delivery (e.g., `"telegram"`). Must match a configured and enabled channel. |
| `reply_target` | String | `""` (empty) | Platform-specific delivery target (e.g., Telegram chat ID). Required for the heartbeat to deliver alerts. |

### Example

```toml
[heartbeat]
enabled = false
interval_minutes = 30
active_start = "08:00"          # Empty = always active
active_end = "22:00"
channel = "telegram"
reply_target = ""               # Chat ID for delivery
```

### Notes

- **Heartbeat File:** If `~/.omega/HEARTBEAT.md` exists and contains content, it is included in the heartbeat prompt as a checklist for the provider to review.
- **Suppression:** When the provider responds with text containing `HEARTBEAT_OK`, no message is sent to the user. Only non-OK responses are delivered as alerts.
- **Active Hours:** Heartbeat checks are skipped outside the configured active hours window. Useful to avoid late-night alerts.
- **Prerequisites:** The `channel` must be configured and enabled, and `reply_target` must be set to a valid chat/conversation ID for the target channel.

---

## Section: `[sandbox]`

Sandbox mode configuration. Controls how the AI provider interacts with the filesystem and system resources via system prompt constraints and working directory confinement.

### Keys

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `mode` | String | `"sandbox"` | Sandbox operating mode. Valid values: `"sandbox"`, `"rx"`, `"rwx"`. Controls both the working directory and the system prompt constraints injected for the provider. |

**Mode Values:**

| Mode | Working Directory | System Prompt Constraint | Description |
|------|------------------|-------------------------|-------------|
| `"sandbox"` | `~/.omega/workspace/` | SANDBOX mode instructions (confine to workspace) | Default. Provider is confined to the workspace directory. System prompt instructs the provider to only operate within the workspace path. |
| `"rx"` | `~/.omega/workspace/` | READ-ONLY mode instructions (no writes, no deletes, no commands) | Read-only mode. Provider receives system prompt instructions allowing read access but forbidding writes, deletes, and command execution. |
| `"rwx"` | None (no confinement) | None (no constraint injected) | Unrestricted mode. No sandbox prompt constraint is injected and no working directory is set on the provider subprocess. |

**Example:**
```toml
[sandbox]
mode = "sandbox"   # "sandbox" | "rx" | "rwx"
```

### Notes

- **Security Sensitive:** Sandbox mode controls the level of filesystem access granted to the AI provider.
- **Workspace Directory:** In `sandbox` and `rx` modes, the provider subprocess `current_dir` is set to `~/.omega/workspace/`, which is automatically created at startup.
- **System Prompt Injection:** The sandbox constraint text is prepended to the system prompt before context building, giving the provider explicit instructions about its operating boundaries.
- **Root Check:** Omega explicitly refuses to run as root (uid 0) to prevent privilege escalation risks.
- **Default Safety:** The default mode is `"sandbox"`, ensuring new installations start with filesystem confinement enabled.

---

## Environment Variable Overrides

Sensitive values can be provided via environment variables instead of hardcoding in `config.toml`. This is the recommended approach for API keys and tokens.

### Supported Overrides

| Env Var | Config Key | Example |
|---------|-----------|---------|
| `ANTHROPIC_API_KEY` | `[provider.anthropic] api_key` | `sk-ant-...` |
| `OPENAI_API_KEY` | `[provider.openai] api_key` | `sk-...` |
| `OPENROUTER_API_KEY` | `[provider.openrouter] api_key` | `sk-or-...` |
| `TELEGRAM_BOT_TOKEN` | `[channel.telegram] bot_token` | `123456789:ABCDEFGHIJKLMNopqrstuvwxyz...` |

### Usage

```bash
export ANTHROPIC_API_KEY="sk-ant-..."
export TELEGRAM_BOT_TOKEN="123456789:ABC..."
omega ask "What is 2+2?"
```

The config file should contain empty string values (`api_key = ""`) for these fields, and the application will read from environment variables at runtime.

---

## Security Considerations

### Critical

1. **Never commit `config.toml`** — it contains sensitive data. Use `config.example.toml` as a template.
2. **API Keys:** Always store in environment variables or a secure secrets manager, never in config files.
3. **Bot Tokens:** Treat as private secrets. Rotate if compromised.
4. **File Permissions:** Ensure `config.toml` is readable only by the Omega process owner: `chmod 600 config.toml`.
5. **Root Check:** Omega will not run as root (uid 0). Always run under a regular user.

### Recommendations

1. Use a secrets manager (e.g., 1Password, Bitwarden, HashiCorp Vault) to store keys.
2. Inject secrets via environment variables at runtime.
3. Enable `auth.enabled = true` and populate `allowed_users` to restrict access.
4. Keep `sandbox.mode = "sandbox"` (the default) unless you explicitly need broader access.
5. Monitor `~/.omega/omega.log` for suspicious activity.
6. Review `~/.omega/memory.db` audit trail periodically.

---

## Quick Start

1. **Copy the template:**
   ```bash
   cp config.example.toml config.toml
   ```

2. **Edit `config.toml`:**
   - Set `[omega] name`, `data_dir`, `log_level`.
   - Choose a provider (Claude Code recommended; enable and set as `default`).
   - Enable desired channels and set tokens/credentials via environment variables.
   - Configure `[auth]` with `allowed_users` if desired.
   - Adjust `[sandbox]` if needed (keep restrictive by default).

3. **Set environment variables:**
   ```bash
   export TELEGRAM_BOT_TOKEN="your_token_here"
   export ANTHROPIC_API_KEY="your_key_here"
   ```

4. **Validate config:**
   ```bash
   cargo run -- check-config
   # (if such a command exists; otherwise, errors appear on startup)
   ```

5. **Start Omega:**
   ```bash
   cargo run --release
   ```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| **"config.toml not found"** | Copy `config.example.toml` to `config.toml` in the project root. |
| **"provider not enabled"** | Verify `[provider.<name>] enabled = true` and that it's set as `default`. |
| **"API key not found"** | Check env var is set: `echo $ANTHROPIC_API_KEY`. Ensure it's exported, not just set. |
| **"Access denied"** | If `auth.enabled = true`, add your user ID to `allowed_users` in the channel config. |
| **Provider restricted** | Check `sandbox.mode` — use `"rwx"` for unrestricted access, or `"rx"` for read-only. Default `"sandbox"` confines to workspace. |
| **Database locked** | Ensure no other Omega instance is running. Delete `memory.db` if corrupted. |

---

## Related Documentation

- **Project README:** See project root for overview and architecture.
- **CLAUDE.md:** Design rules and constraints.
- **Provider Implementation:** `omega-providers/src/`
- **Channel Implementation:** `omega-channels/src/`
- **Memory Layer:** `omega-memory/src/`
- **Sandbox Security:** `omega-sandbox/src/` (planned)

