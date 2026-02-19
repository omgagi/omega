# Technical Specification: Claude Code CLI Provider

**File:** `crates/omega-providers/src/claude_code.rs`

**Module:** `omega_providers::claude_code`

**Purpose:** Implements the `Provider` trait from `omega-core` by invoking the locally installed `claude` CLI as an async subprocess. This is the default, zero-config AI backend for Omega -- it requires no API keys and relies on the user's existing Claude Code authentication.

---

## Dependencies

| Crate | Items Used |
|-------|-----------|
| `async_trait` | `async_trait` macro |
| `omega_core::context` | `Context`, `McpServer` |
| `omega_core::error` | `OmegaError` |
| `omega_core::message` | `MessageMetadata`, `OutgoingMessage` |
| `omega_core::traits` | `Provider` trait |
| `serde` | `Deserialize` |
| `std::time` | `Duration`, `Instant` |
| `tokio::process` | `Command` |
| `tracing` | `debug`, `warn` |
| `std::path` | `Path`, `PathBuf` |

---

## Structs

### `ClaudeCodeProvider`

Public struct. The provider that wraps the Claude Code CLI.

| Field | Type | Visibility | Description |
|-------|------|------------|-------------|
| `session_id` | `Option<String>` | Private | Optional session ID passed to the CLI for conversation continuity across invocations. |
| `max_turns` | `u32` | Private | Maximum number of agentic turns the CLI is allowed per single invocation. Default: `10`. |
| `allowed_tools` | `Vec<String>` | Private | List of tool names the CLI is permitted to use. Default: `["Bash", "Read", "Write", "Edit"]`. |
| `timeout` | `Duration` | Private | Maximum time to wait for the CLI subprocess to complete. Constructed from `Duration::from_secs(timeout_secs)`. Default: `3600` seconds (60 minutes). |
| `working_dir` | `Option<PathBuf>` | Private | Optional working directory for the CLI subprocess. When `Some`, sets the `current_dir` on the subprocess `Command`. Used by sandbox mode to confine the provider to a workspace directory (e.g., `~/.omega/workspace/`). Default: `None`. |
| `max_resume_attempts` | `u32` | Private | Maximum number of auto-resume attempts when the CLI hits `error_max_turns` with a `session_id`. Default: `5`. |
| `model` | `String` | Private | Default model to pass to the CLI via `--model`. Can be overridden per-request by `Context.model`. Default: `"claude-sonnet-4-6"`. |

### `ClaudeCliResponse`

Private struct. Deserializes the JSON output from `claude -p --output-format json`.

| Field | Type | Serde Attributes | Description |
|-------|------|-------------------|-------------|
| `response_type` | `Option<String>` | `#[serde(default, rename = "type")]` | The top-level type field. Expected value: `"result"`. |
| `subtype` | `Option<String>` | `#[serde(default)]` | Result subtype. Known values: `"success"`, `"error_max_turns"`. |
| `result` | `Option<String>` | `#[serde(default)]` | The actual text content of the response. |
| `is_error` | `bool` | `#[serde(default)]` | Whether the response is an error. Defaults to `false`. |
| `cost_usd` | `Option<f64>` | `#[serde(default)]` | Cost of this invocation in USD. |
| `total_cost_usd` | `Option<f64>` | `#[serde(default)]` | Cumulative session cost in USD. |
| `session_id` | `Option<String>` | `#[serde(default)]` | Session ID returned by the CLI. |
| `model` | `Option<String>` | `#[serde(default)]` | Model identifier used (e.g., `"claude-sonnet-4-20250514"`). |
| `num_turns` | `Option<u32>` | `#[serde(default)]` | Number of agentic turns consumed. |

**Note:** The struct is annotated with `#[allow(dead_code)]` because some fields (e.g., `cost_usd`, `total_cost_usd`, `num_turns`) are deserialized but not currently read. They exist for future use and diagnostic purposes.

---

## Functions and Methods

### `ClaudeCodeProvider::new() -> Self`

**Visibility:** Public

Constructs a new `ClaudeCodeProvider` with default settings:

- `session_id`: `None`
- `max_turns`: `10`
- `allowed_tools`: `["Bash", "Read", "Write", "Edit"]`
- `timeout`: `Duration::from_secs(3600)` (60 minutes)
- `working_dir`: `None`
- `max_resume_attempts`: `5`
- `model`: `""` (empty, no `--model` flag passed)

```rust
pub fn new() -> Self
```

### `ClaudeCodeProvider::from_config(max_turns: u32, allowed_tools: Vec<String>, timeout_secs: u64, working_dir: Option<PathBuf>, max_resume_attempts: u32, model: String) -> Self`

**Visibility:** Public

Constructs a `ClaudeCodeProvider` from explicit configuration values. Sets `session_id` to `None`, `timeout` to `Duration::from_secs(timeout_secs)`, `working_dir` to the provided value, `max_resume_attempts` to the provided value, and `model` to the provided value.

```rust
pub fn from_config(max_turns: u32, allowed_tools: Vec<String>, timeout_secs: u64, working_dir: Option<PathBuf>, max_resume_attempts: u32, model: String) -> Self
```

### `ClaudeCodeProvider::check_cli() -> bool` (async)

**Visibility:** Public (associated function, no `&self`)

Checks whether the `claude` CLI binary is installed and accessible by running `claude --version`. Returns `true` if the process exits successfully, `false` otherwise (including if the binary is not found).

```rust
pub async fn check_cli() -> bool
```

**Behavior:** Spawns `claude --version` via `tokio::process::Command`. On any I/O error (e.g., binary not found), returns `false` via `unwrap_or(false)`.

### `Default for ClaudeCodeProvider`

Delegates to `Self::new()`.

### `mcp_tool_patterns(servers: &[McpServer]) -> Vec<String>`

**Visibility:** Public (free function)

Generates `--allowedTools` wildcard patterns from MCP servers. Each server produces a pattern of the form `mcp__<name>__*`.

```rust
pub fn mcp_tool_patterns(servers: &[McpServer]) -> Vec<String>
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `servers` | `&[McpServer]` | Slice of MCP server definitions. |

**Returns:** A `Vec<String>` of tool patterns (e.g., `["mcp__playwright__*"]`). Empty input produces empty output.

### `write_mcp_settings(workspace: &Path, servers: &[McpServer]) -> Result<PathBuf, OmegaError>`

**Visibility:** Private

Creates `{workspace}/.claude/settings.local.json` with MCP server configuration. Creates the `.claude/` directory if it does not exist.

```rust
fn write_mcp_settings(workspace: &Path, servers: &[McpServer]) -> Result<PathBuf, OmegaError>
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `workspace` | `&Path` | The workspace directory (e.g., `~/.omega/workspace/`). |
| `servers` | `&[McpServer]` | MCP servers to configure. |

**Returns:** `Ok(PathBuf)` with the path to the written settings file, or `Err(OmegaError)` on I/O failure.

### `cleanup_mcp_settings(path: &Path)`

**Visibility:** Private

Removes the temporary MCP settings file. Logs a warning on failure instead of panicking.

```rust
fn cleanup_mcp_settings(path: &Path)
```

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `path` | `&Path` | Path to the settings file to remove. |

---

## `Provider` Trait Implementation

### `fn name(&self) -> &str`

Returns the string literal `"claude-code"`.

### `fn requires_api_key(&self) -> bool`

Returns `false`. The Claude Code CLI uses the user's existing authentication; no API key is needed in Omega's configuration.

### `async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError>`

The core method. Invokes the Claude Code CLI as a subprocess and parses the result.

**Full execution flow:**

1. **Prompt construction:** Calls `context.to_prompt_string()` to flatten the `Context` (system prompt + history + current message) into a single string.

1b. **Effective model resolution:** Resolves the model to use via `context.model.as_deref().unwrap_or(&self.model)`. The per-request `context.model` override (set by the gateway's classify-and-route logic) takes precedence over the provider's default `self.model`.

2. **MCP setup:** If `context.mcp_servers` is non-empty:
   - Calls `write_mcp_settings()` to create `{workspace}/.claude/settings.local.json` with MCP server configuration.
   - Calls `mcp_tool_patterns()` to generate `--allowedTools` wildcard patterns for the MCP servers.

3. **Command assembly:** Builds the subprocess command:
   ```
   claude -p <prompt> --output-format json --max-turns <N>
         [--session-id <id>]
         [--allowedTools <tool>]...
   ```

4. **Working directory:** If `self.working_dir` is `Some(path)`, sets `cmd.current_dir(path)` on the subprocess so the CLI operates within the specified workspace directory.

5. **Environment sanitization:** Removes the `CLAUDECODE` environment variable via `cmd.env_remove("CLAUDECODE")` to prevent the CLI from detecting a nested session and erroring out.

6. **Timing:** Records `Instant::now()` before execution and computes elapsed milliseconds after.

7. **Subprocess execution:** Awaits `cmd.output()`. If the spawn itself fails (e.g., binary not found), returns `OmegaError::Provider` with the I/O error message.

8. **Exit code check:** If the process exits with a non-zero status, reads stderr and returns `OmegaError::Provider` with the exit code and stderr content.

9. **JSON parsing:** Attempts `serde_json::from_str::<ClaudeCliResponse>(&stdout)`.

10. **Response extraction** (on successful parse):
    - If `subtype == "error_max_turns"` and `session_id` is present: enters auto-resume loop (see below).
    - If `subtype == "error_max_turns"` without `session_id`: logs a warning and continues to extract whatever result exists.
    - If `result` is `Some` and non-empty: uses it as the response text.
    - If `result` is `None` or empty:
      - If `is_error == true`: returns `"Error from Claude: <subtype>"`.
      - Otherwise: returns `"(No response text returned)"`.
    - Extracts `model` from the response.

10b. **Auto-resume loop:** When `subtype == "error_max_turns"` and `session_id` is present in the response, the provider automatically retries using `run_cli_with_session()`, passing the same prompt and the returned `session_id` via `--session-id`. This continues up to `max_resume_attempts` times. If the resumed call also hits `error_max_turns` with a `session_id`, it loops again. The loop breaks when the response has `subtype != "error_max_turns"`, when no `session_id` is returned, or when the attempt limit is reached. The final accumulated result text is used as the response.

11. **JSON parse failure fallback:** If serde fails, logs a warning and uses the raw stdout (trimmed) as the response text. `model` is set to `None`.

12. **MCP cleanup:** If MCP settings were written in step 2, calls `cleanup_mcp_settings()` to remove the temporary settings file. Cleanup runs on both success and error paths.

13. **Return value:** Constructs and returns an `OutgoingMessage`:

```rust
OutgoingMessage {
    text,               // Extracted or fallback text
    metadata: MessageMetadata {
        provider_used: "claude-code".to_string(),
        tokens_used: None,          // CLI does not report token counts
        processing_time_ms: elapsed_ms,
        model,                      // From JSON response, or None
    },
    reply_target: None,  // Set downstream by the gateway
}
```

### `async fn is_available(&self) -> bool`

Delegates to `Self::check_cli()`. Returns `true` if the `claude` binary is installed and responds to `--version`.

---

## CLI Invocation Detail

### `run_cli(prompt, extra_allowed_tools, model, context_disabled_tools)` (private, async)

Private helper that assembles and executes the `claude` CLI subprocess. Called by `complete()`. Takes an additional `model: &str` parameter; when non-empty, passes `--model <value>` to the CLI. The `context_disabled_tools: bool` parameter is `true` when the caller explicitly set `context.allowed_tools = Some(vec![])` — in that case, if both `allowed_tools` and `extra_allowed_tools` are empty, `run_cli` passes `--allowedTools ""` to disable all tool use in the CLI (used by classification calls).

### `run_cli_with_session(prompt, extra_allowed_tools, session_id, model)` (private, async)

Private helper that assembles and executes the `claude` CLI subprocess with an explicit `--session-id` argument. Called by the auto-resume loop in `complete()` when a previous invocation returned `error_max_turns` with a `session_id`. Behaves identically to `run_cli()` except it always includes `--session-id <session_id>` in the CLI arguments, overriding `self.session_id`. Takes an additional `model: &str` parameter; when non-empty, passes `--model <value>` to the CLI.

| Parameter | Type | Description |
|-----------|------|-------------|
| `prompt` | `&str` | The flattened prompt string. |
| `extra_allowed_tools` | `&[String]` | Additional `--allowedTools` entries (e.g., MCP tool patterns). Appended to the provider's base `allowed_tools` list. |

The subprocess is invoked with the following arguments:

| Argument | Value | Always Present |
|----------|-------|:--------------:|
| `-p` | The flattened prompt string | Yes |
| `--output-format` | `json` | Yes |
| `--max-turns` | `self.max_turns` (default `10`) | Yes |
| `--model` | Effective model string | Only if model is non-empty |
| `--session-id` | `self.session_id` | Only if `session_id` is `Some` |
| `--allowedTools` | One per tool in `self.allowed_tools` | Yes (repeated per tool) |

**Environment modifications:**

| Variable | Action | Reason |
|----------|--------|--------|
| `CLAUDECODE` | Removed | Prevents the CLI from detecting a nested Claude Code session and refusing to run. |

**Working directory:**

When `working_dir` is `Some(path)`, the subprocess is started with `current_dir` set to the given path. This confines the CLI to the workspace directory (e.g., `~/.omega/workspace/`) when sandbox mode is active.

---

## JSON Response Parsing Logic

```
stdout from CLI
       |
       v
  serde_json::from_str::<ClaudeCliResponse>
       |
  +----+----+
  |         |
 Ok(resp)  Err(e)
  |         |
  |         +---> warn!() -> use raw stdout as text, model = None
  |
  +---> subtype == "error_max_turns"? -> warn!()
  |
  +---> resp.result is Some and non-empty? -> use as text
  |         |
  |        No
  |         |
  |         +---> resp.is_error? -> "Error from Claude: <subtype>"
  |         |         |
  |         |        No
  |         |         |
  |         |         +---> "(No response text returned)"
  |
  +---> model = resp.model
```

---

## Error Handling

| Scenario | Error Type | Message Pattern |
|----------|-----------|----------------|
| CLI binary not found / spawn failure | `OmegaError::Provider` | `"failed to run claude CLI: {io_error}"` |
| CLI exits with non-zero status | `OmegaError::Provider` | `"claude CLI exited with {status}: {stderr}"` |
| JSON parse failure | No error (graceful degradation) | Warning logged, raw stdout used |
| `error_max_turns` subtype with `session_id` | No error (auto-resume) | Warning logged, auto-resume loop retries with `--session-id` up to `max_resume_attempts` times |
| `error_max_turns` subtype without `session_id` | No error (graceful degradation) | Warning logged, result extracted if available |
| Empty result + `is_error == true` | No error (fallback text) | `"Error from Claude: {subtype}"` |
| Empty result + no error | No error (fallback text) | `"(No response text returned)"` |

---

## Async Behavior

- All public async methods use `tokio::process::Command` for non-blocking subprocess execution.
- The `Provider` trait requires `Send + Sync`, which `ClaudeCodeProvider` satisfies (all fields are `Send + Sync`).
- `complete()` measures wall-clock time using `std::time::Instant` (not async-aware, but correct for single-invocation latency).

---

## Tests

### `test_default_provider`

**Type:** Synchronous unit test (`#[test]`)

Verifies the default constructor:

- `name()` returns `"claude-code"`
- `requires_api_key()` returns `false`
- `max_turns` is `10`
- `allowed_tools` has 4 entries
- `timeout` is `Duration::from_secs(3600)`
- `working_dir` is `None`
- `max_resume_attempts` is `5`
- `model` is `""` (empty)

```rust
#[test]
fn test_default_provider() {
    let provider = ClaudeCodeProvider::new();
    assert_eq!(provider.name(), "claude-code");
    assert!(!provider.requires_api_key());
    assert_eq!(provider.max_turns, 10);
    assert_eq!(provider.allowed_tools.len(), 4);
    assert_eq!(provider.timeout, Duration::from_secs(3600));
    assert!(provider.working_dir.is_none());
    assert_eq!(provider.max_resume_attempts, 5);
    assert!(provider.model.is_empty());
}
```

### `test_from_config_with_timeout`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `from_config()` correctly sets the `timeout` field from the provided `timeout_secs` parameter:

- Constructs a provider with `timeout_secs = 300`, `working_dir = None`, `max_resume_attempts = 5`, and `model = "claude-sonnet-4-6"`.
- Asserts `timeout` is `Duration::from_secs(300)`.

```rust
#[test]
fn test_from_config_with_timeout() {
    let provider = ClaudeCodeProvider::from_config(5, vec!["Bash".into()], 300, None, 5, "claude-sonnet-4-6".into());
    assert_eq!(provider.timeout, Duration::from_secs(300));
}
```

### `test_from_config_with_working_dir`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `from_config()` correctly sets the `working_dir` field when provided:

- Constructs a provider with `working_dir = Some(PathBuf::from("/tmp/workspace"))` and `model = "claude-sonnet-4-6"`.
- Asserts `working_dir` is `Some(PathBuf::from("/tmp/workspace"))`.

```rust
#[test]
fn test_from_config_with_working_dir() {
    let provider = ClaudeCodeProvider::from_config(
        10,
        vec!["Bash".into()],
        600,
        Some(PathBuf::from("/tmp/workspace")),
        5,
        "claude-sonnet-4-6".into(),
    );
    assert_eq!(provider.working_dir, Some(PathBuf::from("/tmp/workspace")));
}
```

### `test_mcp_tool_patterns_empty`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `mcp_tool_patterns()` returns an empty `Vec` when given an empty slice of MCP servers.

### `test_mcp_tool_patterns`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `mcp_tool_patterns()` generates correct `mcp__<name>__*` patterns for each MCP server in the input slice.

### `test_write_and_cleanup_mcp_settings`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `write_mcp_settings()` writes a valid JSON structure to `{workspace}/.claude/settings.local.json`, and that `cleanup_mcp_settings()` removes the file afterwards.

### `test_cleanup_mcp_settings_nonexistent`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `cleanup_mcp_settings()` does not panic when called with a path to a non-existent file.

**Note:** There are no integration tests that actually invoke the `claude` CLI, as that would require the binary to be installed in CI.

---

## Expected JSON Response Format

The CLI outputs a single JSON object on stdout:

```json
{
  "type": "result",
  "subtype": "success",
  "result": "The response text from Claude.",
  "is_error": false,
  "cost_usd": 0.003,
  "total_cost_usd": 0.003,
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
  "num_turns": 10
}
```
