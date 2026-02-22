# Technical Specification: `providers-anthropic.md`

## File

| Property | Value |
|----------|-------|
| **Path** | `crates/omega-providers/src/anthropic.rs` |
| **Crate** | `omega-providers` |
| **Module** | `pub mod anthropic` |
| **Status** | Implemented |

## Purpose

Calls the Anthropic Messages API directly (not via Claude Code CLI). Non-streaming HTTP.

## Struct: `AnthropicProvider`

| Field | Type | Description |
|-------|------|-------------|
| `client` | `reqwest::Client` | HTTP client |
| `api_key` | `String` | Anthropic API key |
| `model` | `String` | Model ID (default: `claude-sonnet-4-20250514`) |
| `workspace_path` | `Option<PathBuf>` | Working directory for tool execution; `None` disables tool calling |
### Constructor

```rust
pub fn from_config(api_key: String, model: String, workspace_path: Option<PathBuf>) -> Self
```

## Provider Trait Implementation

| Method | Behavior |
|--------|----------|
| `name()` | Returns `"anthropic"` |
| `requires_api_key()` | Returns `true` |
| `complete()` | POST `https://api.anthropic.com/v1/messages` |
| `is_available()` | Returns `true` if api_key is non-empty (no lightweight health endpoint) |

## API Details

### Headers

| Header | Value |
|--------|-------|
| `x-api-key` | API key |
| `anthropic-version` | `2023-06-01` |
| `content-type` | `application/json` |

### Message Types

| Type | Description |
|------|-------------|
| `AnthropicMessage` | `{ role, content: AnthropicContent }` — outgoing message sent to the API |
| `AnthropicContent` | Enum: `Text(String)` for simple text; `Blocks(Vec<AnthropicContentBlock>)` for structured content |
| `AnthropicContentBlock` | Enum: `Text { text }` — plain text block; `ToolUse { id, name, input }` — tool call request; `ToolResult { tool_use_id, content }` — tool execution result |
| `AnthropicResponseBlock` | Enum used for deserializing response content: `Text { text }` or `ToolUse { id, name, input }` |
| `AnthropicToolDef` | `{ name, description, input_schema }` — JSON-schema descriptor sent in the `tools` array |

### Request

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 8192,
  "system": "...",
  "messages": [
    {"role": "user", "content": "..."}
  ],
  "tools": [
    {
      "name": "read_file",
      "description": "Read a file from the workspace",
      "input_schema": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }
    }
  ]
}
```

Key difference: `system` is a top-level field, not a message role. Empty system is omitted via `#[serde(skip_serializing_if = "String::is_empty")]`. `tools` is omitted when `workspace_path` is `None`.

### Response

| Field | Type | Description |
|-------|------|-------------|
| `content` | `Vec<AnthropicResponseBlock>` | Response blocks (text and/or tool_use entries) |
| `stop_reason` | `Option<String>` | `"end_turn"` or `"tool_use"` |
| `model` | `String` | Model used |
| `usage.input_tokens` | `u64` | Input tokens |
| `usage.output_tokens` | `u64` | Output tokens |

Token count = `input_tokens + output_tokens`.

## Agentic Tool-Calling Loop: `agentic_loop()`

Private async method on `AnthropicProvider`:

1. Build the message list using `AnthropicContent` blocks and attach `AnthropicToolDef` descriptors when tools are available.
2. POST to `https://api.anthropic.com/v1/messages`.
3. If `stop_reason == "tool_use"`, extract all `AnthropicResponseBlock::ToolUse` entries, dispatch each through `ToolExecutor`, append the assistant turn (with `Blocks`) and a `user` turn carrying `ToolResult` blocks, then loop back to step 2.
4. If `stop_reason == "end_turn"`, extract the `Text` block and return as the final response.
5. Loop is bounded by a max-iterations guard.

`complete()` calls `agentic_loop()` when `workspace_path` is set; falls back to the plain single-shot POST otherwise.

## Tests

| Test | Description |
|------|-------------|
| `test_anthropic_provider_name` | Verifies name and requires_api_key |
| `test_anthropic_request_serialization` | Verifies request JSON structure |
| `test_anthropic_request_empty_system_omitted` | Verifies empty system field is excluded from JSON |
| `test_anthropic_response_parsing` | Verifies response deserialization for a plain text response |
| `test_anthropic_tool_use_response_parsing` | Verifies deserialization of a response with `stop_reason: "tool_use"` and `tool_use` content blocks |
| `test_anthropic_content_block_serialization` | Verifies that `AnthropicContentBlock::ToolResult` serializes correctly for the follow-up user turn |
| `test_anthropic_tool_def_serialization` | Verifies that `AnthropicToolDef` serializes to the expected JSON schema shape |
