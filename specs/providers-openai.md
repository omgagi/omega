# Technical Specification: `providers-openai.md`

## File

| Property | Value |
|----------|-------|
| **Path** | `crates/omega-providers/src/openai.rs` |
| **Crate** | `omega-providers` |
| **Module** | `pub mod openai` |
| **Status** | Implemented |

## Purpose

OpenAI-compatible API provider. Works with OpenAI's API and any compatible endpoint (e.g., Azure OpenAI). Exports `pub(crate)` types reused by the OpenRouter provider.

## Struct: `OpenAiProvider`

| Field | Type | Description |
|-------|------|-------------|
| `client` | `reqwest::Client` | HTTP client |
| `base_url` | `String` | API base URL (default: `https://api.openai.com/v1`) |
| `api_key` | `String` | Bearer token |
| `model` | `String` | Model ID (default: `gpt-4o`) |
| `workspace_path` | `Option<PathBuf>` | Working directory for tool execution; `None` disables tool calling |
### Constructor

```rust
pub fn from_config(base_url: String, api_key: String, model: String, workspace_path: Option<PathBuf>) -> Self
```

## Provider Trait Implementation

| Method | Behavior |
|--------|----------|
| `name()` | Returns `"openai"` |
| `requires_api_key()` | Returns `true` |
| `complete()` | POST `{base_url}/chat/completions` with Bearer auth |
| `is_available()` | GET `{base_url}/models` with Bearer auth → checks for 200 |

## Shared Types (pub(crate))

These types are reused by the OpenRouter provider:

| Type | Description |
|------|-------------|
| `ChatMessage` | `{ role, content: Option<String>, tool_calls: Option<Vec<ToolCallMsg>>, tool_call_id: Option<String> }` |
| `ChatCompletionRequest` | `{ model, messages, tools: Option<Vec<OpenAiToolDef>> }` |
| `ChatCompletionResponse` | `{ choices, model, usage }` |
| `ChatChoice` | `{ message, finish_reason: Option<String> }` |
| `ChatUsage` | `{ total_tokens }` |
| `ToolCallMsg` | `{ id, r#type, function: FunctionCall }` — a single tool call returned by the model |
| `FunctionCall` | `{ name, arguments }` — name and JSON-encoded arguments string |
| `OpenAiToolDef` | `{ r#type: "function", function: OpenAiFunctionDef }` — top-level tool descriptor |
| `OpenAiFunctionDef` | `{ name, description, parameters }` — JSON-schema parameters as `serde_json::Value` |
| `build_openai_messages()` | Converts system prompt + ApiMessages into ChatMessages |
| `openai_agentic_complete()` | Shared agentic tool-calling loop used by both OpenAI and OpenRouter providers |

## Request/Response Format

### Request

```json
{
  "model": "gpt-4o",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
  ],
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

Headers: `Authorization: Bearer {api_key}`

`tools` is omitted when `workspace_path` is `None` or `allowed_tools` is empty.

### Response

| Field | Type | Description |
|-------|------|-------------|
| `choices[0].message.content` | `Option<String>` | Response text (absent when model returns tool calls) |
| `choices[0].message.tool_calls` | `Option<Vec<ToolCallMsg>>` | Tool calls requested by the model |
| `choices[0].finish_reason` | `Option<String>` | `"stop"`, `"tool_calls"`, etc. |
| `model` | `String` | Model used |
| `usage.total_tokens` | `u64` | Total token count |

## Agentic Tool-Calling Loop: `openai_agentic_complete()`

Shared `pub(crate)` async function implementing the tool-calling loop for both OpenAI and OpenRouter:

1. Build the initial message list including available tool definitions.
2. POST to the completions endpoint.
3. If `finish_reason == "tool_calls"`, dispatch each tool call through `ToolExecutor`, append `assistant` message with `tool_calls` and one `tool` message per result, then loop back to step 2.
4. If `finish_reason == "stop"` (or no tool calls), return the final `content` as the response text.
5. Loop is bounded by a max-iterations guard to prevent runaway cycles.

## Tests

| Test | Description |
|------|-------------|
| `test_openai_provider_name` | Verifies name and requires_api_key |
| `test_build_openai_messages` | Verifies message building with system prompt |
| `test_build_openai_messages_empty_system` | Verifies system prompt omission when empty |
| `test_openai_response_parsing` | Verifies response deserialization |
| `test_tool_call_response_parsing` | Verifies deserialization of a response whose `finish_reason` is `"tool_calls"` and `tool_calls` array is populated |
| `test_to_openai_tools` | Verifies that `ToolExecutor::to_openai_tools()` returns the correct `OpenAiToolDef` descriptors for the available tools |
| `test_request_with_tools` | Verifies that `ChatCompletionRequest` serializes the `tools` field when provided |
