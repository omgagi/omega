# Technical Specification: `providers-ollama.md`

## File

| Property | Value |
|----------|-------|
| **Path** | `crates/omega-providers/src/ollama.rs` |
| **Crate** | `omega-providers` |
| **Module** | `pub mod ollama` |
| **Status** | Implemented |

## Purpose

Connects to a locally running Ollama server over HTTP. No API key required.

## Struct: `OllamaProvider`

| Field | Type | Description |
|-------|------|-------------|
| `client` | `reqwest::Client` | HTTP client |
| `base_url` | `String` | Ollama server endpoint (default: `http://localhost:11434`) |
| `model` | `String` | Model name (default: `llama3`) |
| `workspace_path` | `Option<PathBuf>` | Working directory for tool execution; `None` disables tool calling |
### Constructor

```rust
pub fn from_config(base_url: String, model: String, workspace_path: Option<PathBuf>) -> Self
```

## Provider Trait Implementation

| Method | Behavior |
|--------|----------|
| `name()` | Returns `"ollama"` |
| `requires_api_key()` | Returns `false` |
| `complete()` | POST `{base_url}/api/chat` with `{ model, messages, stream: false }` |
| `is_available()` | GET `{base_url}/api/tags` → checks for 200 OK |

## Request/Response Types

### Tool Types

| Type | Description |
|------|-------------|
| `OllamaToolCall` | `{ function: OllamaFunctionCall }` — a single tool call returned by the model |
| `OllamaFunctionCall` | `{ name, arguments: serde_json::Value }` — parsed (not string-encoded) arguments |
| `OllamaToolDef` | `{ r#type: "function", function: OllamaFunctionDef }` — top-level tool descriptor |
| `OllamaFunctionDef` | `{ name, description, parameters }` — JSON-schema parameters as `serde_json::Value` |

### Request: `OllamaChatRequest`

```json
{
  "model": "llama3",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
  ],
  "stream": false,
  "tools": [
    {
      "type": "function",
      "function": {
        "name": "read_file",
        "description": "Read a file from the workspace",
        "parameters": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }
      }
    }
  ]
}
```

`tools` is omitted (`#[serde(skip_serializing_if = "Option::is_none")]`) when `workspace_path` is `None` or no tools are available.

### Message: `OllamaChatMessage`

| Field | Type | Description |
|-------|------|-------------|
| `role` | `String` | `"user"`, `"assistant"`, or `"tool"` |
| `content` | `Option<String>` | Message text; absent when the model returns tool calls |
| `tool_calls` | `Option<Vec<OllamaToolCall>>` | Tool calls requested by the model |

### Response: `OllamaChatResponse`

| Field | Type | Description |
|-------|------|-------------|
| `message` | `Option<OllamaChatMessage>` | Response message with role and content (or tool_calls) |
| `model` | `Option<String>` | Model used |
| `eval_count` | `Option<u64>` | Tokens generated |
| `prompt_eval_count` | `Option<u64>` | Tokens in prompt |

Token count = `eval_count + prompt_eval_count` when both present.

## Agentic Tool-Calling Loop: `agentic_loop()`

Private async method on `OllamaProvider` that drives the tool-calling cycle:

1. Build the message list and, if `workspace_path` is set, attach `OllamaToolDef` descriptors.
2. POST to `{base_url}/api/chat`.
3. If the response message contains `tool_calls`, dispatch each through `ToolExecutor`, append the assistant message and one `tool`-role result message per call, then loop back to step 2.
4. If the response message has text content (no tool calls), return it as the final response.
5. Loop is bounded by a max-iterations guard to prevent runaway cycles.

`complete()` calls `agentic_loop()` when `workspace_path` is set; falls back to the plain single-shot POST when tools are unavailable.

## Tests

| Test | Description |
|------|-------------|
| `test_ollama_provider_name` | Verifies name and requires_api_key |
| `test_ollama_request_serialization` | Verifies request JSON structure without tools |
| `test_ollama_request_with_tools` | Verifies that `tools` field is serialized when tool definitions are provided |
| `test_ollama_response_parsing` | Verifies response deserialization with plain text content |
| `test_ollama_tool_call_response_parsing` | Verifies deserialization of a response whose `message.tool_calls` is populated |
| `test_ollama_function_call_args_parsing` | Verifies that `OllamaFunctionCall.arguments` deserializes as a `serde_json::Value` (not a string) |
