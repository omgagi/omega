# Omega Core Config -- Technical Specification

## Path

`crates/omega-core/src/config.rs`

## Purpose

Defines all configuration types for the Omega agent and provides a `load()` function that reads a TOML file, deserializes it into a strongly-typed `Config` tree, and falls back to sensible defaults when the file is absent. Every section of the config is optional (via `#[serde(default)]`) except the top-level `[omega]` table, which must be present in the TOML file when the file exists.

## Dependencies

| Crate | Use |
|-------|-----|
| `serde` (Serialize, Deserialize) | TOML deserialization and serialization of all config structs |
| `toml` | Parsing the TOML file contents |
| `std::path::Path` | File existence check |
| `crate::error::OmegaError` | Error propagation via `OmegaError::Config(String)` |
| `tracing` | Logging when the config file is missing |

---

## Struct Hierarchy

```
Config
  +-- omega: OmegaConfig
  +-- auth: AuthConfig
  +-- provider: ProviderConfig
  |     +-- claude_code: Option<ClaudeCodeConfig>
  |     +-- anthropic: Option<AnthropicConfig>
  |     +-- openai: Option<OpenAiConfig>
  |     +-- ollama: Option<OllamaConfig>
  |     +-- openrouter: Option<OpenRouterConfig>
  +-- channel: ChannelConfig
  |     +-- telegram: Option<TelegramConfig>
  |     +-- whatsapp: Option<WhatsAppConfig>
  +-- memory: MemoryConfig
  +-- sandbox: SandboxConfig
  +-- heartbeat: HeartbeatConfig
  +-- scheduler: SchedulerConfig
```

---

## Struct Definitions

### `Config` (top-level)

| Field | Type | `#[serde]` | Default |
|-------|------|------------|---------|
| `omega` | `OmegaConfig` | -- (required in TOML) | `OmegaConfig::default()` when file absent |
| `auth` | `AuthConfig` | `#[serde(default)]` | `AuthConfig::default()` |
| `provider` | `ProviderConfig` | `#[serde(default)]` | `ProviderConfig::default()` |
| `channel` | `ChannelConfig` | `#[serde(default)]` | `ChannelConfig::default()` |
| `memory` | `MemoryConfig` | `#[serde(default)]` | `MemoryConfig::default()` |
| `sandbox` | `SandboxConfig` | `#[serde(default)]` | `SandboxConfig::default()` |
| `heartbeat` | `HeartbeatConfig` | `#[serde(default)]` | `HeartbeatConfig::default()` |
| `scheduler` | `SchedulerConfig` | `#[serde(default)]` | `SchedulerConfig::default()` |

Derives: `Debug, Clone, Serialize, Deserialize`

### `OmegaConfig`

General agent identity and runtime settings.

| Field | Type | Default Function | Default Value |
|-------|------|-----------------|---------------|
| `name` | `String` | `default_name()` | `"Omega"` |
| `data_dir` | `String` | `default_data_dir()` | `"~/.omega"` |
| `log_level` | `String` | `default_log_level()` | `"info"` |

Derives: `Debug, Clone, Serialize, Deserialize`
Implements: `Default` (manual impl calling the default functions).

### `AuthConfig`

| Field | Type | Default Function | Default Value |
|-------|------|-----------------|---------------|
| `enabled` | `bool` | `default_true()` | `true` |
| `deny_message` | `String` | `default_deny_message()` | `"Access denied. You are not authorized to use this agent."` |

Derives: `Debug, Clone, Serialize, Deserialize, Default`

Note: The `Default` derive sets `enabled` to `false` and `deny_message` to `""` (Rust defaults). However, when deserialized from TOML with a missing field, serde uses the `#[serde(default = "...")]` functions, which produce `true` and the full deny message respectively. This means the serde defaults and the `Default` trait defaults differ -- serde defaults take precedence during deserialization.

### `ProviderConfig`

| Field | Type | `#[serde]` | Default Value |
|-------|------|------------|---------------|
| `default` | `String` | `default = "default_provider"` | `"claude-code"` |
| `claude_code` | `Option<ClaudeCodeConfig>` | `default, rename = "claude-code"` | `None` |
| `anthropic` | `Option<AnthropicConfig>` | -- | `None` |
| `openai` | `Option<OpenAiConfig>` | -- | `None` |
| `ollama` | `Option<OllamaConfig>` | -- | `None` |
| `openrouter` | `Option<OpenRouterConfig>` | -- | `None` |

Derives: `Debug, Clone, Serialize, Deserialize, Default`

The `claude_code` field uses `rename = "claude-code"` so the TOML key is `[provider.claude-code]`.

When the config file is absent, the `load()` function explicitly constructs a `ProviderConfig` with `claude_code: Some(ClaudeCodeConfig::default())` -- this ensures the Claude Code provider is available out of the box even without a config file.

### `ClaudeCodeConfig`

| Field | Type | Default Function | Default Value |
|-------|------|-----------------|---------------|
| `enabled` | `bool` | `default_true()` | `true` |
| `max_turns` | `u32` | `default_max_turns()` | `10` |
| `allowed_tools` | `Vec<String>` | `default_allowed_tools()` | `["Bash", "Read", "Write", "Edit"]` |
| `timeout_secs` | `u64` | `default_timeout_secs()` | `600` |

Derives: `Debug, Clone, Serialize, Deserialize`
Implements: `Default` (manual, sets `enabled` to `true`, `max_turns` to `default_max_turns()`, `allowed_tools` to `default_allowed_tools()`, `timeout_secs` to `default_timeout_secs()`).

### `AnthropicConfig`

| Field | Type | Default Function | Default Value |
|-------|------|-----------------|---------------|
| `enabled` | `bool` | serde default | `false` |
| `api_key` | `String` | serde default | `""` |
| `model` | `String` | `default_anthropic_model()` | `"claude-sonnet-4-20250514"` |

Derives: `Debug, Clone, Serialize, Deserialize`

### `OpenAiConfig`

| Field | Type | Default Function | Default Value |
|-------|------|-----------------|---------------|
| `enabled` | `bool` | serde default | `false` |
| `api_key` | `String` | serde default | `""` |
| `model` | `String` | `default_openai_model()` | `"gpt-4o"` |
| `base_url` | `String` | `default_openai_base_url()` | `"https://api.openai.com/v1"` |

Derives: `Debug, Clone, Serialize, Deserialize`

### `OllamaConfig`

| Field | Type | Default Function | Default Value |
|-------|------|-----------------|---------------|
| `enabled` | `bool` | serde default | `false` |
| `base_url` | `String` | `default_ollama_base_url()` | `"http://localhost:11434"` |
| `model` | `String` | `default_ollama_model()` | `"llama3"` |

Derives: `Debug, Clone, Serialize, Deserialize`

### `OpenRouterConfig`

| Field | Type | Default Function | Default Value |
|-------|------|-----------------|---------------|
| `enabled` | `bool` | serde default | `false` |
| `api_key` | `String` | serde default | `""` |
| `model` | `String` | serde default | `""` |

Derives: `Debug, Clone, Serialize, Deserialize`

### `ChannelConfig`

| Field | Type | Default Value |
|-------|------|---------------|
| `telegram` | `Option<TelegramConfig>` | `None` |
| `whatsapp` | `Option<WhatsAppConfig>` | `None` |

Derives: `Debug, Clone, Serialize, Deserialize, Default`

### `TelegramConfig`

| Field | Type | Default Value |
|-------|------|---------------|
| `enabled` | `bool` | `false` |
| `bot_token` | `String` | `""` |
| `allowed_users` | `Vec<i64>` | `[]` |

Derives: `Debug, Clone, Serialize, Deserialize`

Note: `allowed_users` contains Telegram numeric user IDs. An empty list means "allow all" (per gateway auth logic).

### `WhatsAppConfig`

| Field | Type | Default Value |
|-------|------|---------------|
| `enabled` | `bool` | `false` |
| `bridge_url` | `String` | `""` |
| `phone_number` | `String` | `""` |

Derives: `Debug, Clone, Serialize, Deserialize`

### `MemoryConfig`

| Field | Type | Default Function | Default Value |
|-------|------|-----------------|---------------|
| `backend` | `String` | `default_memory_backend()` | `"sqlite"` |
| `db_path` | `String` | `default_db_path()` | `"~/.omega/memory.db"` |
| `max_context_messages` | `usize` | `default_max_context()` | `50` |

Derives: `Debug, Clone, Serialize, Deserialize`
Implements: `Default` (manual).

### `SandboxMode` (enum)

Controls how the AI provider interacts with the filesystem and system resources. Used as the `mode` field of `SandboxConfig`.

```rust
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode {
    #[default]
    Sandbox,
    Rx,
    Rwx,
}
```

| Variant | Serialized Value | Description |
|---------|-----------------|-------------|
| `Sandbox` | `"sandbox"` | Default. Full sandbox isolation. Provider receives system prompt instructions restricting it to the `~/.omega/workspace/` directory. |
| `Rx` | `"rx"` | Read-only mode. Provider receives system prompt instructions allowing read access but forbidding writes, deletes, and command execution. |
| `Rwx` | `"rwx"` | Unrestricted mode. No sandbox prompt constraint is injected. Provider operates without filesystem restrictions. |

Derives: `Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize`

The `#[serde(rename_all = "lowercase")]` attribute ensures that the TOML values are lowercase strings (`"sandbox"`, `"rx"`, `"rwx"`).

#### `SandboxMode` Methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `prompt_constraint` | `fn prompt_constraint(&self, workspace_path: &str) -> Option<String>` | Returns the system prompt instructions for the mode. `Sandbox` returns SANDBOX mode text referencing the workspace path. `Rx` returns READ-ONLY mode text. `Rwx` returns `None` (no constraint). |
| `display_name` | `fn display_name(&self) -> &str` | Returns the human-readable name: `"sandbox"`, `"rx"`, or `"rwx"`. |

### `SandboxConfig`

| Field | Type | `#[serde]` | Default Value |
|-------|------|------------|---------------|
| `mode` | `SandboxMode` | `#[serde(default)]` | `SandboxMode::Sandbox` |

Derives: `Debug, Clone, Default, Serialize, Deserialize`

### `HeartbeatConfig`

Periodic AI check-in configuration.

| Field | Type | Default Function | Default Value |
|-------|------|-----------------|---------------|
| `enabled` | `bool` | serde default | `false` |
| `interval_minutes` | `u64` | `default_heartbeat_interval()` | `30` |
| `active_start` | `String` | serde default | `""` |
| `active_end` | `String` | serde default | `""` |
| `channel` | `String` | serde default | `""` |
| `reply_target` | `String` | serde default | `""` |

Derives: `Debug, Clone, Serialize, Deserialize`
Implements: `Default` (manual -- sets `enabled` to `false`, `interval_minutes` to `default_heartbeat_interval()`, all strings to empty).

Note: `active_start` and `active_end` are `"HH:MM"` formatted strings (e.g., `"08:00"`, `"22:00"`). When both are empty, the heartbeat is always active. `channel` specifies which messaging channel to deliver alerts on (e.g., `"telegram"`). `reply_target` is the platform-specific delivery target (e.g., a Telegram chat ID).

### `SchedulerConfig`

User-scheduled reminders and recurring task configuration.

| Field | Type | Default Function | Default Value |
|-------|------|-----------------|---------------|
| `enabled` | `bool` | `default_true()` | `true` |
| `poll_interval_secs` | `u64` | `default_poll_interval()` | `60` |

Derives: `Debug, Clone, Serialize, Deserialize`
Implements: `Default` (manual -- sets `enabled` to `true`, `poll_interval_secs` to `default_poll_interval()`).

Note: The scheduler is enabled by default because it has zero cost when no tasks exist. The poll interval controls how often the scheduler checks for due tasks.

---

## Default Value Functions

All private functions in the module that supply serde defaults:

| Function | Return Type | Value |
|----------|------------|-------|
| `default_name()` | `String` | `"Omega"` |
| `default_data_dir()` | `String` | `"~/.omega"` |
| `default_log_level()` | `String` | `"info"` |
| `default_provider()` | `String` | `"claude-code"` |
| `default_true()` | `bool` | `true` |
| `default_deny_message()` | `String` | `"Access denied. You are not authorized to use this agent."` |
| `default_max_turns()` | `u32` | `10` |
| `default_allowed_tools()` | `Vec<String>` | `["Bash", "Read", "Write", "Edit"]` |
| `default_anthropic_model()` | `String` | `"claude-sonnet-4-20250514"` |
| `default_openai_model()` | `String` | `"gpt-4o"` |
| `default_openai_base_url()` | `String` | `"https://api.openai.com/v1"` |
| `default_ollama_base_url()` | `String` | `"http://localhost:11434"` |
| `default_ollama_model()` | `String` | `"llama3"` |
| `default_memory_backend()` | `String` | `"sqlite"` |
| `default_db_path()` | `String` | `"~/.omega/memory.db"` |
| `default_max_context()` | `usize` | `50` |
| `default_timeout_secs()` | `u64` | `600` |
| `default_heartbeat_interval()` | `u64` | `30` |
| `default_poll_interval()` | `u64` | `60` |

---

## `load()` Function

```rust
pub fn load(path: &str) -> Result<Config, OmegaError>
```

### Behavior

1. Converts `path` to a `std::path::Path`.
2. If the file does not exist:
   - Logs an info-level message via `tracing`.
   - Returns a fully-defaulted `Config` with `claude_code` explicitly set to `Some(ClaudeCodeConfig::default())`, and `heartbeat` and `scheduler` set to their defaults.
3. If the file exists:
   - Reads the entire file into a `String` (`std::fs::read_to_string`).
   - On read failure: returns `OmegaError::Config` with the I/O error message.
   - Deserializes the string via `toml::from_str::<Config>`.
   - On parse failure: returns `OmegaError::Config` with the parse error message.
4. Returns `Ok(config)`.

### File-absent default differs from serde Default

When the file is absent, `load()` explicitly constructs:

```rust
ProviderConfig {
    default: default_provider(),           // "claude-code"
    claude_code: Some(ClaudeCodeConfig::default()),
    ..Default::default()
}
```

This is different from `ProviderConfig::default()` which would set `claude_code` to `None`. The explicit construction ensures that the zero-config experience works: a user can run `omega ask "hello"` without any config file and the Claude Code provider will be selected.

---

## Environment Variable Overrides

The config module itself does **not** implement env-var overrides. However, several values are documented in `config.example.toml` as having env-var alternatives:

| Config Field | Env Var (documented) | Override Location |
|-------------|---------------------|-------------------|
| `provider.anthropic.api_key` | `ANTHROPIC_API_KEY` | Provider crate (at usage time) |
| `provider.openai.api_key` | `OPENAI_API_KEY` | Provider crate (at usage time) |
| `provider.openrouter.api_key` | `OPENROUTER_API_KEY` | Provider crate (at usage time) |
| `channel.telegram.bot_token` | `TELEGRAM_BOT_TOKEN` | Channel crate (at usage time) |

The tracing subscriber in `main.rs` reads `RUST_LOG` via `EnvFilter::try_from_default_env()`, which overrides `log_level` from config.

The `~` in paths like `~/.omega` is expanded at usage time (in `omega-memory` and `init`) by resolving `std::env::var_os("HOME")`, not in the config module.

---

## Validation

The config module performs **no semantic validation**. It only checks:

1. Whether the file exists (returns defaults if not).
2. Whether the file can be read (I/O error).
3. Whether the TOML is structurally valid and matches the expected types (parse error).

Semantic validation happens downstream:

- `main.rs` checks that the selected provider is available (`provider.is_available().await`).
- `main.rs` checks that at least one channel is enabled.
- `main.rs` checks that Telegram's `bot_token` is not empty when Telegram is enabled.
- The `selfcheck` module performs a broader health check before starting the gateway.

---

## Serde Patterns

### `#[serde(default)]` on struct fields

Used on every field of `Config` except `omega`. Means the entire section can be omitted from the TOML file and the struct's `Default` impl (or type default) will be used.

### `#[serde(default = "function_name")]` on individual fields

Points to a private function that returns the default value for that field when it is missing from the TOML.

### `#[serde(rename = "claude-code")]`

Maps the Rust field `claude_code` to the TOML key `claude-code`, allowing idiomatic naming on both sides.

### `Option<T>` for optional subsections

Channel and provider sub-configs use `Option<T>` rather than `#[serde(default)]` with `Default`. This means the absence of a `[channel.telegram]` section results in `None` (the section was never mentioned) rather than a default-constructed `TelegramConfig` (the section was mentioned but all fields were omitted). This distinction is meaningful -- `None` means "not configured at all."

---

## TOML-to-Rust Key Mapping

| TOML Key | Rust Path |
|----------|-----------|
| `[omega]` | `Config.omega` |
| `[auth]` | `Config.auth` |
| `[provider]` | `Config.provider` |
| `[provider.claude-code]` | `Config.provider.claude_code` |
| `[provider.anthropic]` | `Config.provider.anthropic` |
| `[provider.openai]` | `Config.provider.openai` |
| `[provider.ollama]` | `Config.provider.ollama` |
| `[provider.openrouter]` | `Config.provider.openrouter` |
| `[channel.telegram]` | `Config.channel.telegram` |
| `[channel.whatsapp]` | `Config.channel.whatsapp` |
| `[memory]` | `Config.memory` |
| `[sandbox]` | `Config.sandbox` |
| `[heartbeat]` | `Config.heartbeat` |
| `[scheduler]` | `Config.scheduler` |

---

## Bundled Prompt Deployment

### Constants

| Constant | Source | Description |
|----------|--------|-------------|
| `BUNDLED_SYSTEM_PROMPT` | `include_str!("../../../prompts/SYSTEM_PROMPT.md")` | Default system prompt with 3 sections: `## Identity`, `## Soul`, `## System` |
| `BUNDLED_WELCOME_TOML` | `include_str!("../../../prompts/WELCOME.toml")` | Default welcome messages (8 languages) in TOML format |

### `Prompts` Struct — `identity` and `soul` Fields

The `Prompts` struct has three prompt fields parsed from `SYSTEM_PROMPT.md`:

| Field | Type | Section Header | Description |
|-------|------|----------------|-------------|
| `identity` | `String` | `## Identity` | Agent identity description (who the agent is). |
| `soul` | `String` | `## Soul` | Agent personality and values. |
| `system` | `String` | `## System` | Operational rules and constraints. |

**`Prompts::load()`** parses the file by splitting on `## Identity`, `## Soul`, and `## System` section headers. Each section's content (between its header and the next header) becomes the corresponding field value.

**Backward compatibility:** If the user's `SYSTEM_PROMPT.md` only contains `## System` (no `## Identity` or `## Soul` sections), the `identity` and `soul` fields retain their compiled-in defaults from `Default` impl. Only the `system` field is overwritten from the file.

**`Default` impl:** Provides hardcoded defaults for all three fields (`identity`, `soul`, `system`), ensuring the agent has a complete prompt even without any file on disk.

### `install_bundled_prompts(data_dir: &str)`

```rust
pub fn install_bundled_prompts(data_dir: &str)
```

Deploys `SYSTEM_PROMPT.md` and `WELCOME.toml` from compile-time embedded templates to `data_dir`. Creates the directory if needed. **Never overwrites existing files** so user edits are preserved. The deployed `SYSTEM_PROMPT.md` contains all three sections (`## Identity`, `## Soul`, `## System`).

Called from `main.rs` before `Prompts::load()` so first-run users get editable files instead of falling back to hardcoded defaults.

Follows the same pattern as `omega_skills::install_bundled_skills()`.

---

## Tests

### `test_timeout_config_default`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `ClaudeCodeConfig::default()` sets `timeout_secs` to `600`.

### `test_timeout_config_from_toml`

**Type:** Synchronous unit test (`#[test]`)

Verifies that a TOML config with an explicit `timeout_secs` value (e.g., `300`) is correctly deserialized into `ClaudeCodeConfig.timeout_secs`.

### `test_timeout_config_default_when_missing`

**Type:** Synchronous unit test (`#[test]`)

Verifies that when `timeout_secs` is omitted from the TOML, the serde default function `default_timeout_secs()` supplies `600`.

### `test_parse_identity_soul_system_sections`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `Prompts::load()` correctly parses all three sections (`## Identity`, `## Soul`, `## System`) from a `SYSTEM_PROMPT.md` file into the corresponding `identity`, `soul`, and `system` fields.

### `test_backward_compat_system_only`

**Type:** Synchronous unit test (`#[test]`)

Verifies backward compatibility: when the user's `SYSTEM_PROMPT.md` only contains `## System` (no `## Identity` or `## Soul`), the `identity` and `soul` fields retain their compiled-in defaults while `system` is loaded from the file.

### `test_prompts_default_has_identity_soul`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `Prompts::default()` provides non-empty hardcoded values for `identity` and `soul` fields.

### `test_install_bundled_prompts_creates_files`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `install_bundled_prompts()` deploys `SYSTEM_PROMPT.md` and `WELCOME.toml` to a temporary directory, that the deployed files contain expected markers (`## System`, `[messages]`, `English`), and that a second invocation does not overwrite files that were modified by the user.

### `test_sandbox_mode_default`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `SandboxMode::default()` is `SandboxMode::Sandbox`.

### `test_sandbox_mode_display_names`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `display_name()` returns `"sandbox"`, `"rx"`, and `"rwx"` for each respective variant.

### `test_sandbox_mode_prompt_constraint`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `prompt_constraint()` returns `Some(...)` with workspace path for `Sandbox`, `Some(...)` for `Rx`, and `None` for `Rwx`.

### `test_sandbox_config_from_toml`

**Type:** Synchronous unit test (`#[test]`)

Verifies that a TOML config with `mode = "rx"` is correctly deserialized into `SandboxConfig { mode: SandboxMode::Rx }`.

### `test_sandbox_config_default_when_missing`

**Type:** Synchronous unit test (`#[test]`)

Verifies that when the `[sandbox]` section is absent from TOML, the default `SandboxConfig` has `mode: SandboxMode::Sandbox`.
