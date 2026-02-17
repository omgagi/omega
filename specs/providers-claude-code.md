# Technical Specification: Claude Code CLI Provider

**File:** `crates/omega-providers/src/claude_code.rs`

**Module:** `omega_providers::claude_code`

**Purpose:** Implements the `Provider` trait from `omega-core` by invoking the locally installed `claude` CLI as an async subprocess. This is the default, zero-config AI backend for Omega -- it requires no API keys and relies on the user's existing Claude Code authentication.

---

## Dependencies

| Crate | Items Used |
|-------|-----------|
| `async_trait` | `async_trait` macro |
| `omega_core::context` | `Context` |
| `omega_core::error` | `OmegaError` |
| `omega_core::message` | `MessageMetadata`, `OutgoingMessage` |
| `omega_core::traits` | `Provider` trait |
| `serde` | `Deserialize` |
| `std::time` | `Duration`, `Instant` |
| `tokio::process` | `Command` |
| `tracing` | `debug`, `warn` |

---

## Structs

### `ClaudeCodeProvider`

Public struct. The provider that wraps the Claude Code CLI.

| Field | Type | Visibility | Description |
|-------|------|------------|-------------|
| `session_id` | `Option<String>` | Private | Optional session ID passed to the CLI for conversation continuity across invocations. |
| `max_turns` | `u32` | Private | Maximum number of agentic turns the CLI is allowed per single invocation. Default: `10`. |
| `allowed_tools` | `Vec<String>` | Private | List of tool names the CLI is permitted to use. Default: `["Bash", "Read", "Write", "Edit"]`. |
| `timeout` | `Duration` | Private | Maximum time to wait for the CLI subprocess to complete. Constructed from `Duration::from_secs(timeout_secs)`. Default: `600` seconds (10 minutes). |

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
- `timeout`: `Duration::from_secs(600)` (10 minutes)

```rust
pub fn new() -> Self
```

### `ClaudeCodeProvider::from_config(max_turns: u32, allowed_tools: Vec<String>, timeout_secs: u64) -> Self`

**Visibility:** Public

Constructs a `ClaudeCodeProvider` from explicit configuration values. Sets `session_id` to `None` and `timeout` to `Duration::from_secs(timeout_secs)`.

```rust
pub fn from_config(max_turns: u32, allowed_tools: Vec<String>, timeout_secs: u64) -> Self
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

2. **Command assembly:** Builds the subprocess command:
   ```
   claude -p <prompt> --output-format json --max-turns <N>
         [--session-id <id>]
         [--allowedTools <tool>]...
   ```

3. **Environment sanitization:** Removes the `CLAUDECODE` environment variable via `cmd.env_remove("CLAUDECODE")` to prevent the CLI from detecting a nested session and erroring out.

4. **Timing:** Records `Instant::now()` before execution and computes elapsed milliseconds after.

5. **Subprocess execution:** Awaits `cmd.output()`. If the spawn itself fails (e.g., binary not found), returns `OmegaError::Provider` with the I/O error message.

6. **Exit code check:** If the process exits with a non-zero status, reads stderr and returns `OmegaError::Provider` with the exit code and stderr content.

7. **JSON parsing:** Attempts `serde_json::from_str::<ClaudeCliResponse>(&stdout)`.

8. **Response extraction** (on successful parse):
   - If `subtype == "error_max_turns"`: logs a warning but continues to extract whatever result exists.
   - If `result` is `Some` and non-empty: uses it as the response text.
   - If `result` is `None` or empty:
     - If `is_error == true`: returns `"Error from Claude: <subtype>"`.
     - Otherwise: returns `"(No response text returned)"`.
   - Extracts `model` from the response.

9. **JSON parse failure fallback:** If serde fails, logs a warning and uses the raw stdout (trimmed) as the response text. `model` is set to `None`.

10. **Return value:** Constructs and returns an `OutgoingMessage`:

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

The subprocess is invoked with the following arguments:

| Argument | Value | Always Present |
|----------|-------|:--------------:|
| `-p` | The flattened prompt string | Yes |
| `--output-format` | `json` | Yes |
| `--max-turns` | `self.max_turns` (default `10`) | Yes |
| `--session-id` | `self.session_id` | Only if `session_id` is `Some` |
| `--allowedTools` | One per tool in `self.allowed_tools` | Yes (repeated per tool) |

**Environment modifications:**

| Variable | Action | Reason |
|----------|--------|--------|
| `CLAUDECODE` | Removed | Prevents the CLI from detecting a nested Claude Code session and refusing to run. |

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
| `error_max_turns` subtype | No error (graceful degradation) | Warning logged, result extracted if available |
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
- `timeout` is `Duration::from_secs(600)`

```rust
#[test]
fn test_default_provider() {
    let provider = ClaudeCodeProvider::new();
    assert_eq!(provider.name(), "claude-code");
    assert!(!provider.requires_api_key());
    assert_eq!(provider.max_turns, 10);
    assert_eq!(provider.allowed_tools.len(), 4);
    assert_eq!(provider.timeout, Duration::from_secs(600));
}
```

### `test_from_config_with_timeout`

**Type:** Synchronous unit test (`#[test]`)

Verifies that `from_config()` correctly sets the `timeout` field from the provided `timeout_secs` parameter:

- Constructs a provider with `timeout_secs = 300`.
- Asserts `timeout` is `Duration::from_secs(300)`.

```rust
#[test]
fn test_from_config_with_timeout() {
    let provider = ClaudeCodeProvider::from_config(5, vec!["Bash".into()], 300);
    assert_eq!(provider.timeout, Duration::from_secs(300));
}
```

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
