# Technical Specification: Claude Code CLI Provider

**Directory:** `backend/crates/omega-providers/src/claude_code/` (5-file directory module: mod.rs, command.rs, provider.rs, response.rs, mcp.rs, tests.rs)

**Module:** `omega_providers::claude_code`

**Purpose:** Implements the `Provider` trait from `omega-core` by invoking the locally installed `claude` CLI as an async subprocess. This is the default, zero-config AI backend for Omega -- it requires no API keys and relies on the user's existing Claude Code authentication.

---

## Module Structure

| File | Purpose |
|------|---------|
| `mod.rs` | Module root: struct definitions (`ClaudeCodeProvider`, `ClaudeCliResponse`), `new()`, `from_config()`, `check_cli()`, `Default` impl, re-export of `mcp_tool_patterns` |
| `command.rs` | CLI command building and subprocess execution: `build_run_cli_args()`, `run_cli()`, `run_cli_with_session()`, `base_command()`, `execute_with_timeout()` |
| `provider.rs` | `Provider` trait implementation with auto-resume logic: `complete()`, `is_available()`, `auto_resume()` |
| `response.rs` | JSON response parsing and diagnostic logging: `parse_response()` |
| `mcp.rs` | MCP server settings management: `write_mcp_settings()`, `cleanup_mcp_settings()`, `mcp_tool_patterns()` |
| `tests.rs` | All unit tests for the module |

---

## Dependencies

| Crate | Items Used |
|-------|-----------|
| `async_trait` | `async_trait` macro |
| `omega_core::context` | `Context`, `McpServer` |
| `omega_core::error` | `OmegaError` |
| `omega_core::message` | `MessageMetadata`, `OutgoingMessage` |
| `omega_core::traits` | `Provider` trait |
| `omega_sandbox` | `protected_command()` for filesystem protection |
| `serde` | `Deserialize` |
| `std::time` | `Duration`, `Instant` |
| `tokio::process` | `Command` |
| `tracing` | `debug`, `info`, `warn`, `error` |
| `std::path` | `Path`, `PathBuf` |

---

## Structs

### `ClaudeCodeProvider`

Public struct. The provider that wraps the Claude Code CLI. Session continuity is managed per-request via `Context.session_id`, not stored on the provider.

| Field | Type | Visibility | Description |
|-------|------|------------|-------------|
| `max_turns` | `u32` | Private | Maximum number of agentic turns the CLI is allowed per single invocation. Default: `25`. |
| `allowed_tools` | `Vec<String>` | Private | List of tool names the CLI is permitted to use. Default: `[]` (empty = full tool access, `--dangerously-skip-permissions` passed to bypass all permission prompts since the OS-level system protection provides the real security boundary). When non-empty, each tool is passed as a separate `--allowedTools` argument. |
| `timeout` | `Duration` | Private | Maximum time to wait for the CLI subprocess to complete. Constructed from `Duration::from_secs(timeout_secs)`. Default: `3600` seconds (60 minutes). |
| `working_dir` | `Option<PathBuf>` | Private | Optional working directory for the CLI subprocess. When `Some`, sets the `current_dir` on the subprocess `Command`. Always set to `~/.omega/workspace/` in normal operation. Default: `None`. |
| `max_resume_attempts` | `u32` | Private | Maximum number of auto-resume attempts when the CLI hits `error_max_turns` with a `session_id`. Default: `5`. |
| `model` | `String` | Private | Default model to pass to the CLI via `--model`. Can be overridden per-request by `Context.model`. Default: `""` (empty, no `--model` flag passed). |

### `ClaudeCliResponse`

Private struct (in mod.rs). Deserializes the JSON output from `claude -p --output-format json`.

| Field | Type | Serde Attributes | Description |
|-------|------|-------------------|-------------|
| `response_type` | `Option<String>` | `#[serde(default, rename = "type")]` | The top-level type field. Expected value: `"result"`. |
| `subtype` | `Option<String>` | `#[serde(default)]` | Result subtype. Known values: `"success"`, `"error_max_turns"`. |
| `result` | `Option<String>` | `#[serde(default)]` | The actual text content of the response. |
| `is_error` | `bool` | `#[serde(default)]` | Whether the response is an error. Defaults to `false`. |
| `session_id` | `Option<String>` | `#[serde(default)]` | Session ID returned by the CLI. |
| `model` | `Option<String>` | `#[serde(default)]` | Model identifier used (e.g., `"claude-sonnet-4-20250514"`). |
| `num_turns` | `Option<u32>` | `#[serde(default)]` | Number of agentic turns consumed. |

**Note:** The `cost_usd` and `total_cost_usd` fields from the CLI JSON output are not deserialized -- the struct only captures fields that are actively used.

---

## Functions and Methods

### `ClaudeCodeProvider::new() -> Self`

**Visibility:** Public

Constructs a new `ClaudeCodeProvider` with default settings:

- `max_turns`: `25`
- `allowed_tools`: `[]` (full tool access)
- `timeout`: `Duration::from_secs(3600)` (60 minutes)
- `working_dir`: `None`
- `max_resume_attempts`: `5`
- `model`: `""` (empty, no `--model` flag passed)

### `ClaudeCodeProvider::from_config(max_turns, allowed_tools, timeout_secs, working_dir, max_resume_attempts, model) -> Self`

**Visibility:** Public

Constructs a `ClaudeCodeProvider` from explicit configuration values.

### `ClaudeCodeProvider::check_cli() -> bool` (async)

**Visibility:** Public (associated function, no `&self`)

Checks whether the `claude` CLI binary is installed and accessible by running `claude --version`. Returns `true` if the process exits successfully, `false` otherwise.

### `Default for ClaudeCodeProvider`

Delegates to `Self::new()`.

### `mcp_tool_patterns(servers: &[McpServer]) -> Vec<String>`

**Visibility:** Public (free function in `mcp.rs`, re-exported from mod.rs)

Generates `--allowedTools` wildcard patterns from MCP servers. Each server produces a pattern of the form `mcp__<name>__*`.

### `write_mcp_settings(workspace: &Path, servers: &[McpServer]) -> Result<PathBuf, OmegaError>`

**Visibility:** `pub(super)` (in `mcp.rs`)

Creates `{workspace}/.claude/settings.local.json` with MCP server configuration.

### `cleanup_mcp_settings(path: &Path)`

**Visibility:** `pub(super)` (in `mcp.rs`)

Removes the temporary MCP settings file. Logs a warning on failure instead of panicking.

### `build_run_cli_args(...)` (in `command.rs`)

**Visibility:** `pub(super)`

Pure function that constructs the CLI argument vector for `run_cli()`. Extracted for testability. Takes prompt, extra_allowed_tools, max_turns, allowed_tools, model, context_disabled_tools, session_id, and agent_name. Returns `Vec<String>`.

Agent name validation: rejects names containing `/`, `\`, or `..` to prevent path traversal attacks via the `--agent` flag.

### `run_cli(...)` (in `command.rs`)

**Visibility:** `pub(super)`

Assembles and executes the `claude` CLI subprocess with timeout. Uses `base_command()` for working directory and sandbox setup, then applies arguments from `build_run_cli_args()`.

### `run_cli_with_session(...)` (in `command.rs`)

**Visibility:** `pub(super)`

Executes the `claude` CLI subprocess with an explicit `--resume` argument. Called by the auto-resume loop.

### `base_command()` (in `command.rs`)

**Visibility:** Private

Builds the base `Command` with working directory and system protection via `omega_sandbox::protected_command()`. Removes the `CLAUDECODE` environment variable.

### `execute_with_timeout(...)` (in `command.rs`)

**Visibility:** Private

Executes a command with the configured timeout and standard error handling (non-zero exit, timeout).

### `parse_response(...)` (in `response.rs`)

**Visibility:** `pub(super)`

Parses the JSON response from Claude Code CLI with diagnostic logging. Returns `(text, model)` tuple.

### `auto_resume(...)` (in `provider.rs`)

**Visibility:** Private

Auto-resume loop when Claude hits max_turns. Retries up to `max_resume_attempts` times with exponential backoff (2s, 4s, 8s, ...), accumulating results. Skipped when `context.max_turns` is explicitly set by the caller.

---

## `Provider` Trait Implementation

### `fn name(&self) -> &str`

Returns the string literal `"claude-code"`.

### `fn requires_api_key(&self) -> bool`

Returns `false`.

### `async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError>`

The core method. Full execution flow:

1. **Prompt construction:** Calls `context.to_prompt_string()`.
2. **Effective overrides resolution:** Resolves `effective_max_turns`, `effective_tools`, and `effective_model` from context overrides falling back to provider defaults.
3. **MCP setup:** If `context.mcp_servers` is non-empty, writes settings file and generates tool patterns.
4. **CLI execution:** Calls `run_cli()` with all resolved parameters.
5. **MCP cleanup:** Always cleans up MCP settings (regardless of success/failure).
6. **Response parsing:** Calls `parse_response()`. Falls back to effective_model if CLI doesn't echo it.
7. **Auto-resume:** If `subtype == "error_max_turns"` with `session_id` and no explicit `max_turns` override, enters `auto_resume()` loop.
8. **Return:** Constructs `OutgoingMessage` with `session_id` from CLI response in metadata.

### `async fn is_available(&self) -> bool`

Delegates to `Self::check_cli()`.

---

## CLI Invocation Detail

Permission logic in `build_run_cli_args()` has 4 branches:

1. **Agent mode** (`agent_name` is `Some` and valid): passes `--dangerously-skip-permissions`, skips `--resume`.
2. **Tools disabled** (`context_disabled_tools = true`): passes `--allowedTools ""` to disable all tool use.
3. **Full access** (`allowed_tools` empty, no agent): passes `--dangerously-skip-permissions`. MCP patterns appended via `--allowedTools`.
4. **Explicit whitelist** (`allowed_tools` non-empty): passes `--allowedTools` for each tool plus MCP patterns.

| Argument | Value | Always Present |
|----------|-------|:--------------:|
| `--agent` | `agent_name` from Context | Only if `agent_name` is `Some`, non-empty, and passes validation |
| `-p` | The flattened prompt string | Yes |
| `--output-format` | `json` | Yes |
| `--max-turns` | Effective max_turns value | Yes |
| `--model` | Effective model string | Only if model is non-empty |
| `--resume` | `session_id` from Context | Only if `session_id` is `Some` AND `agent_name` is `None` |
| `--dangerously-skip-permissions` | (flag, no value) | When agent mode OR `allowed_tools` empty |
| `--allowedTools` | Tool names/patterns | When explicit whitelist or MCP patterns |

**Environment modifications:**

| Variable | Action | Reason |
|----------|--------|--------|
| `CLAUDECODE` | Removed | Prevents the CLI from detecting a nested Claude Code session and refusing to run. |

---

## Error Handling

| Scenario | Error Type | Behavior |
|----------|-----------|----------|
| CLI binary not found / spawn failure | `OmegaError::Provider` | `"failed to run claude CLI: {io_error}"` |
| CLI exits with non-zero status | `OmegaError::Provider` | `"claude CLI exited with {status}: {stderr}"` |
| CLI times out | `OmegaError::Provider` | `"claude CLI timed out after {N}s"` |
| JSON parse failure | No error (graceful degradation) | Warning logged, raw stdout used (or user-friendly fallback if empty) |
| `error_max_turns` with `session_id` | No error (auto-resume) | Auto-resume loop with exponential backoff |
| Empty result + `is_error == true` | No error (fallback text) | `"Error from Claude: {subtype}"` |
| Empty result + no error | No error (fallback text) | `"I received your message but wasn't able to generate a response. Please try again."` |

---

## Tests

| Test | Description |
|------|-------------|
| `test_default_provider` | Verifies defaults: `max_turns=25`, `allowed_tools=[]`, `timeout=3600s`, `working_dir=None`, `max_resume_attempts=5`, `model=""` |
| `test_from_config_with_timeout` | Verifies `from_config()` sets `timeout`, `max_turns`, `max_resume_attempts`, `model` correctly |
| `test_from_config_with_working_dir` | Verifies `from_config()` sets `working_dir` when provided |
| `test_parse_response_max_turns_with_session` | Verifies `parse_response()` extracts text and model from `error_max_turns` response |
| `test_parse_response_success` | Verifies `parse_response()` extracts text and model from success response |
| `test_mcp_tool_patterns_empty` | Empty input produces empty output |
| `test_mcp_tool_patterns` | Generates correct `mcp__<name>__*` patterns |
| `test_write_and_cleanup_mcp_settings` | Writes valid JSON structure and cleanup removes the file |
| `test_cleanup_mcp_settings_nonexistent` | Does not panic on non-existent file |
| `test_build_run_cli_args_no_agent_name` | Without agent_name, `--agent` is absent; `-p`, `--output-format`, `--max-turns`, `--model` present |
| `test_build_run_cli_args_with_agent_name` | With agent_name, `--agent <name>` appears before `-p` |
| `test_build_run_cli_args_agent_with_model_override` | `--model` still applied with `--agent` |
| `test_build_run_cli_args_agent_with_skip_permissions` | `--dangerously-skip-permissions` applied in agent mode |
| `test_build_run_cli_args_agent_with_max_turns` | `--max-turns` present with correct value in agent mode |
| `test_build_run_cli_args_agent_name_empty_string` | Empty agent_name does not emit `--agent` |
| `test_build_run_cli_args_agent_name_with_session_id` | agent_name takes priority over session_id (`--agent` present, `--resume` absent) |
| `test_build_run_cli_args_agent_name_path_traversal` | Path traversal in agent_name rejected (no `--agent` emitted) |
| `test_build_run_cli_args_agent_name_with_slash` | Forward slash in agent_name rejected |
| `test_build_run_cli_args_agent_name_with_backslash` | Backslash in agent_name rejected |
| `test_build_run_cli_args_explicit_allowed_tools_no_agent` | Explicit tools produce `--allowedTools`, no `--dangerously-skip-permissions` |
| `test_build_run_cli_args_disabled_tools` | `context_disabled_tools=true` produces `--allowedTools ""` |

---

## Expected JSON Response Format

```json
{
  "type": "result",
  "subtype": "success",
  "result": "The response text from Claude.",
  "is_error": false,
  "session_id": "abc123",
  "model": "claude-sonnet-4-20250514",
  "num_turns": 1
}
```

On max turns exceeded:

```json
{
  "type": "result",
  "subtype": "error_max_turns",
  "result": "Partial response text...",
  "is_error": false,
  "session_id": "abc123",
  "model": "claude-sonnet-4-20250514",
  "num_turns": 25
}
```
