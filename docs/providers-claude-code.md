# Claude Code CLI Provider

**File:** `crates/omega-providers/src/claude_code.rs`

The Claude Code CLI provider is Omega's default AI backend. It works by shelling out to the `claude` command-line tool that is already installed on your machine, so you do not need to configure any API keys -- it piggybacks on whatever authentication the CLI already has.

---

## How It Works

When Omega receives a message (from Telegram, CLI, etc.), the gateway assembles a `Context` containing the system prompt, conversation history, and the current message. That context is flattened into a single prompt string and passed to the `claude` CLI as a subprocess:

```
claude -p "the prompt" --output-format json --max-turns 10 --allowedTools Bash --allowedTools Read ...
```

The CLI does its work (potentially taking multiple agentic turns), then returns a JSON object on stdout. Omega parses that JSON, extracts the response text and model info, and sends it back through the messaging channel.

The entire flow is asynchronous. Omega uses `tokio::process::Command` so the event loop is never blocked while waiting for Claude to respond.

---

## Configuration

You can create the provider in two ways:

**Default settings** (recommended for getting started):

```rust
let provider = ClaudeCodeProvider::new();
// max_turns: 10
// allowed_tools: ["Bash", "Read", "Write", "Edit"]
```

**Custom settings** from your config:

```rust
let provider = ClaudeCodeProvider::from_config(
    20,                                   // allow up to 20 agentic turns
    vec!["Bash".into(), "Read".into()],   // restrict to just Bash and Read
    600,                                  // timeout in seconds (10 minutes)
);
```

### `max_turns`

Controls how many agentic turns the Claude CLI is allowed to take in a single invocation. An "agentic turn" is one cycle of the CLI using a tool (running a command, reading a file, etc.) and then deciding what to do next. The default of `10` is a reasonable balance between capability and cost.

If Claude hits the max turns limit, Omega still extracts whatever partial response was generated. It does not treat this as a fatal error -- you will get back whatever Claude managed to produce, plus a warning in the logs.

### `allowed_tools`

The list of tools Claude is permitted to use during its agentic turns. Each tool is passed as a separate `--allowedTools` argument. The defaults are:

| Tool | What it does |
|------|-------------|
| `Bash` | Execute shell commands |
| `Read` | Read files from disk |
| `Write` | Write files to disk |
| `Edit` | Edit existing files |

If you want a more restricted provider (e.g., one that can only answer questions without touching the filesystem), pass a smaller list to `from_config`.

### `timeout_secs`

Controls how long Omega will wait for the Claude Code CLI to finish before aborting the subprocess. The default is `600` seconds (10 minutes), which serves as a ceiling to prevent runaway invocations from blocking the gateway indefinitely.

This timeout is configurable via the `[provider.claude-code]` section in `config.toml`:

```toml
[provider.claude-code]
timeout_secs = 600
```

If the CLI does not produce a response within the configured timeout, the subprocess is killed and Omega returns a friendly error message to the user. The 10-minute default is generous enough for complex multi-turn agentic tasks while still protecting against hangs.

---

## JSON Response Format

The CLI outputs a JSON object with this shape:

```json
{
  "type": "result",
  "subtype": "success",
  "result": "Here is my response to your question...",
  "is_error": false,
  "cost_usd": 0.003,
  "total_cost_usd": 0.003,
  "session_id": "abc123",
  "model": "claude-sonnet-4-20250514",
  "num_turns": 1
}
```

**Key fields Omega cares about:**

- **`result`** -- The actual response text. This is what gets sent back to the user.
- **`subtype`** -- Either `"success"` or `"error_max_turns"`. Omega handles both gracefully.
- **`model`** -- Included in the response metadata so you can see which model was used.
- **`is_error`** -- If `true` and there is no result text, Omega generates a fallback error message.

The `cost_usd`, `total_cost_usd`, `session_id`, and `num_turns` fields are deserialized but not currently surfaced. They are available in the struct for future features (cost tracking, session persistence, etc.).

---

## Environment Variable Handling

The provider removes the `CLAUDECODE` environment variable before spawning the subprocess:

```rust
cmd.env_remove("CLAUDECODE");
```

This is important. If Omega itself is running inside a Claude Code session (for development or testing), the CLI would detect the `CLAUDECODE` env var and refuse to start, thinking it is a nested invocation. Removing the variable prevents this issue.

No other environment variables are modified. The CLI inherits the rest of Omega's environment, including `PATH` and any authentication tokens the CLI needs.

---

## Error Handling

The provider is designed to be resilient. Here is how different failure modes are handled:

### CLI not installed

If `claude` is not on the PATH, the `output()` call returns an I/O error. Omega wraps it as:

```
OmegaError::Provider("failed to run claude CLI: No such file or directory (os error 2)")
```

You can check for this ahead of time by calling:

```rust
if ClaudeCodeProvider::check_cli().await {
    println!("CLI is available");
}
```

### CLI exits with an error

If the process exits with a non-zero status code, Omega reads stderr and returns:

```
OmegaError::Provider("claude CLI exited with exit status: 1: <stderr content>")
```

### Max turns exceeded

Not treated as an error. Omega logs a warning and extracts whatever partial result was returned. The user gets a response -- it just might be incomplete.

### Malformed JSON output

If the CLI returns something that is not valid JSON (unlikely but possible), Omega logs a warning and falls back to using the raw stdout text as the response. This ensures the user always gets *something* back.

### Empty response

If the JSON parses correctly but `result` is empty or missing:

- If `is_error` is `true`: the user sees `"Error from Claude: <subtype>"`.
- Otherwise: the user sees `"(No response text returned)"`.

---

## Session Continuity

The `session_id` field exists on the provider struct but is not currently populated by the constructors (it is always `None`). When set, it would be passed as `--session-id <id>` to the CLI, allowing multiple invocations to share the same conversation context inside Claude.

This is separate from Omega's own memory system (which handles conversation history via SQLite and the `Context` struct). Session continuity at the CLI level would allow Claude to maintain its own internal state across calls. This is reserved for future use.

---

## Debugging Common Issues

### "failed to run claude CLI"

The `claude` binary is not found. Make sure it is installed and on your PATH. Test with:

```bash
which claude
claude --version
```

### "claude CLI exited with exit status: 1"

The CLI itself encountered an error. Common causes:

- **Authentication expired.** Run `claude` interactively to re-authenticate.
- **Network issues.** The CLI needs internet access to reach Anthropic's API.
- **Invalid arguments.** Check the logs for the full stderr output.

### Nested session detection

If you see errors about nested sessions or `CLAUDECODE`, the env var removal might not be working. Verify that Omega is correctly calling `cmd.env_remove("CLAUDECODE")`. This can happen if you are running Omega from within a Claude Code terminal session.

### Slow responses

Claude Code CLI invocations can take anywhere from a few seconds to several minutes, depending on the complexity of the prompt and how many agentic turns are needed. The `processing_time_ms` field in the response metadata tells you exactly how long each invocation took.

The default timeout is 600 seconds (10 minutes). If the CLI exceeds this limit, the subprocess is killed and the user receives a friendly error message. You can tune the timeout via `timeout_secs` in `[provider.claude-code]`:

```toml
[provider.claude-code]
timeout_secs = 300   # 5-minute ceiling instead of 10
```

If responses are consistently slow, consider:

- Reducing `max_turns` to limit how much work Claude does per invocation.
- Simplifying the system prompt or reducing conversation history length.
- Lowering `timeout_secs` to fail fast on runaway invocations.

### "(No response text returned)"

This means the JSON parsed successfully, there was no error flag, but the `result` field was empty. This is rare but can happen if the CLI produces a response with no text content. Check the raw JSON output in the debug logs for more context.

---

## Provider Trait Compliance

`ClaudeCodeProvider` implements the `Provider` trait from `omega-core`:

| Method | Behavior |
|--------|----------|
| `name()` | Returns `"claude-code"` |
| `requires_api_key()` | Returns `false` |
| `complete(context)` | Invokes the CLI, parses JSON, returns `OutgoingMessage` |
| `is_available()` | Runs `claude --version` to check if the CLI is installed |

The struct is `Send + Sync`, so it can be shared across async tasks and used safely in the gateway event loop.
