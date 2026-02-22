# Agentic Tool Executor and MCP Client

**Files:**
- `crates/omega-providers/src/tools.rs` — Built-in tool executor + MCP routing
- `crates/omega-providers/src/mcp_client.rs` — Minimal MCP client over stdio (JSON-RPC 2.0)

All five HTTP-based providers (OpenAI, Anthropic, Ollama, OpenRouter, Gemini) include an agentic
tool-execution loop. This means they can autonomously call tools, observe the results, and continue
reasoning — the same pattern that Claude Code CLI uses via the `claude` subprocess.

---

## What the Agentic Loop Does

Without the loop, an HTTP provider does one thing: send a prompt, receive a text reply.

With the loop, the provider can:

1. Infer — the model decides whether to respond in text or call a tool.
2. Execute — if a tool is requested, `ToolExecutor` runs it and captures the output.
3. Feed results back — tool output is appended to the conversation as a tool result message.
4. Repeat — until the model returns a plain text response or the turn limit is reached.

This transforms every HTTP provider from a simple completion endpoint into an autonomous agent that
can run shell commands, read and write files, query MCP servers, and compose its own multi-step
work.

Classification calls use `allowed_tools = Some(vec![])` to prevent any tool use during routing
decisions, keeping classification fast and cheap.

---

## Built-in Tools

`ToolExecutor` in `tools.rs` registers four built-in tools:

### `bash`

Runs an arbitrary shell command.

- **Sandbox enforcement**: In `Sandbox` and `Rx` modes, write access is restricted to
  `~/.omega/` + `/tmp`. In `Rwx` mode, there are no write restrictions.
- **Timeout**: 120 seconds. Commands that exceed this are killed and an error is returned.
- **Output truncation**: stdout + stderr combined are truncated to **30,000 characters** to
  prevent a runaway command from filling the context window.

### `read`

Reads the contents of a file at an absolute path.

- **Output truncation**: file contents are truncated to **50,000 characters**.
- No sandbox restriction on reads (read-only operation).

### `write`

Writes content to a file at an absolute path, creating parent directories as needed.

- **Sandbox enforcement**: same rules as `bash` — writes outside `~/.omega/` and `/tmp` are
  blocked in `Sandbox` and `Rx` modes.

### `edit`

Performs a targeted string replacement in an existing file (old string → new string).

- **Sandbox enforcement**: same rules as `write`.
- Returns an error if the file does not exist or the old string is not found.

---

## MCP Client

`McpClient` in `mcp_client.rs` provides a minimal JSON-RPC 2.0 client over stdio. It connects
to MCP servers that skills declare in their frontmatter (the same servers Claude Code CLI uses via
`settings.local.json`, but now accessible directly from HTTP providers).

### Lifecycle

1. **Connect** — spawns the MCP server process (`command` + `args` from the skill definition).
2. **Initialize** — sends the `initialize` JSON-RPC request and waits for acknowledgment.
3. **Discover tools** — calls `tools/list` to enumerate all tools the server exposes.
4. **Execute tools** — calls `tools/call` with the tool name and arguments when the model
   requests an MCP tool.
5. **Shutdown** — sends `shutdown` + `exit` notifications and waits for the process to exit.

### Tool naming

MCP tools are registered in `ToolExecutor` with the prefix `mcp__<server_name>__<tool_name>`,
matching the naming convention the Claude Code CLI uses. This means the model sees a flat tool
namespace and does not need to know whether a tool is built-in or MCP-backed.

### Error handling

If the MCP server process fails to start, `McpClient::connect()` returns an error. The
`ToolExecutor` logs the failure and continues without that server's tools — it does not abort the
provider call.

---

## Provider-specific Loop Formats

Each HTTP provider has its own wire format for tool calls. The loop logic is adapted per provider.

### OpenAI and OpenRouter (shared loop)

OpenAI's function-calling format. Tools are declared in the `tools` array as JSON Schema objects.
The model returns `tool_calls` on the assistant message, each with a `tool_call_id`, `function.name`,
and `function.arguments` (JSON string). Results are fed back as messages with `role: "tool"` and the
matching `tool_call_id`.

OpenRouter reuses OpenAI types and follows the same loop.

### Ollama (own loop)

Ollama uses a similar function-calling format but does not return a `tool_call_id`. Tool results
are fed back without an ID field. The loop otherwise follows the same infer → execute → feed pattern.

### Anthropic (content blocks)

Anthropic's Messages API returns tool use as content blocks with `type: "tool_use"`. Each block
has an `id`, `name`, and `input` (already parsed JSON object). Results are fed back as a `user`
message containing a content block with `type: "tool_result"` and the matching `tool_use_id`. The
loop continues until the `stop_reason` is `"end_turn"` rather than `"tool_use"`.

### Gemini (functionCall / functionResponse)

Gemini uses `functionCall` parts in the model's response. Each part has a `name` and `args`
(JSON object). Results are fed back as a `user` message with `functionResponse` parts containing
`name` and `response`. Role mapping differs from OpenAI: `assistant` messages use the role `"model"`
in Gemini's wire format.

---

## Output Truncation

Long tool outputs are truncated before being appended to the conversation:

| Tool | Limit |
|------|-------|
| `bash` | 30,000 characters |
| `read` | 50,000 characters |
| MCP tools | No truncation (server controls output size) |

Truncation appends `\n[output truncated]` so the model knows the output was cut. This prevents
a single tool result from exhausting the provider's context window.

---

## Sandbox Enforcement

Filesystem protection is always-on. The `ToolExecutor` uses `omega_sandbox::is_write_blocked()` to
check every write operation, and `omega_sandbox::protected_command()` wraps subprocess execution
with OS-level protection:

- **`bash` tool:** Subprocess launched via `protected_command()`, which applies OS-level blocklist
  enforcement (Seatbelt on macOS, Landlock on Linux).
- **`write` / `edit` tools:** Path checked via `is_write_blocked()` before any write operation.
  Writes to dangerous system directories and OMEGA's core database are blocked. Writes to the
  workspace (`~/.omega/workspace/`), data directory (`~/.omega/`), and `/tmp` are allowed.

No configuration is needed -- protection is automatic.

---

## Model Routing for Non-Claude Providers

`build_provider()` in `omega-providers/src/lib.rs` now returns a tuple:

```
(Box<dyn Provider>, model_fast: String, model_complex: String)
```

For the Claude Code CLI provider, `model_fast` is Sonnet and `model_complex` is Opus, enabling
the gateway's classify-and-route system to use different models for classification vs. execution.

For all HTTP providers (OpenAI, Anthropic, Ollama, OpenRouter, Gemini), there is only one
configured model field. Both `model_fast` and `model_complex` are set to that same model. The
provider still supports per-request model override via `Context.model`, but the gateway will not
switch between two distinct models for these providers unless the config is extended.
