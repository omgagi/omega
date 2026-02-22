# Technical Specification: `providers-gemini.md`

## File

| Property | Value |
|----------|-------|
| **Path** | `crates/omega-providers/src/gemini.rs` |
| **Crate** | `omega-providers` |
| **Module** | `pub mod gemini` |
| **Status** | Implemented |

## Purpose

Calls the Google Gemini `generateContent` endpoint. Authentication via URL query parameter.

## Struct: `GeminiProvider`

| Field | Type | Description |
|-------|------|-------------|
| `client` | `reqwest::Client` | HTTP client |
| `api_key` | `String` | Google API key |
| `model` | `String` | Model ID (default: `gemini-2.0-flash`) |
| `workspace_path` | `Option<PathBuf>` | Working directory for tool execution; `None` disables tool calling |
### Constructor

```rust
pub fn from_config(api_key: String, model: String, workspace_path: Option<PathBuf>) -> Self
```

## Provider Trait Implementation

| Method | Behavior |
|--------|----------|
| `name()` | Returns `"gemini"` |
| `requires_api_key()` | Returns `true` |
| `complete()` | POST `https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={key}` |
| `is_available()` | GET `.../models?key={key}` → checks for 200 |

## API Details

### Authentication

API key passed as URL query parameter `?key={api_key}` (not in headers).

### Role Mapping

| Omega Role | Gemini Role |
|------------|-------------|
| `user` | `user` |
| `assistant` | `model` |

### Tool Types

| Type | Description |
|------|-------------|
| `GeminiFunctionCall` | `{ name, args: serde_json::Value }` — a function call requested by the model |
| `GeminiFunctionResponse` | `{ name, response: serde_json::Value }` — result returned to the model |
| `GeminiToolDeclaration` | `{ function_declarations: Vec<GeminiFunctionDef> }` — top-level tool descriptor sent in `tools` |
| `GeminiFunctionDef` | `{ name, description, parameters }` — JSON-schema parameters as `serde_json::Value` |

### Part Type: `GeminiPart`

`GeminiPart` is a multi-variant struct where all fields are `Option`:

| Field | Type | Description |
|-------|------|-------------|
| `text` | `Option<String>` | Plain text content |
| `function_call` | `Option<GeminiFunctionCall>` | Tool call requested by the model |
| `function_response` | `Option<GeminiFunctionResponse>` | Tool result being returned to the model |

Serialization uses `#[serde(skip_serializing_if = "Option::is_none")]` on each field so only the relevant variant is emitted.

### Request

```json
{
  "contents": [
    {"role": "user", "parts": [{"text": "..."}]}
  ],
  "systemInstruction": {
    "parts": [{"text": "..."}]
  },
  "tools": [
    {
      "function_declarations": [
        {
          "name": "read_file",
          "description": "Read a file from the workspace",
          "parameters": { "type": "object", "properties": { "path": { "type": "string" } }, "required": ["path"] }
        }
      ]
    }
  ]
}
```

- `systemInstruction` is a top-level field (separate from contents), omitted when empty
- `tools` is omitted when `workspace_path` is `None`
- Uses `camelCase` serialization via `#[serde(rename_all = "camelCase")]`

### Response

| Field | Type | Description |
|-------|------|-------------|
| `candidates[0].content.parts` | `Vec<GeminiPart>` | Response parts (text and/or function_call entries) |
| `usageMetadata.totalTokenCount` | `u64` | Total tokens |

## Agentic Tool-Calling Loop: `agentic_loop()`

Private async method on `GeminiProvider`:

1. Build the `contents` list and attach `GeminiToolDeclaration` when tools are available.
2. POST to the `generateContent` endpoint.
3. If any response part contains a `function_call`, dispatch each through `ToolExecutor`, append the model turn and a `user` turn with `function_response` parts, then loop back to step 2.
4. If all parts are text, concatenate them and return as the final response.
5. Loop is bounded by a max-iterations guard.

`complete()` calls `agentic_loop()` when `workspace_path` is set; falls back to the plain single-shot POST otherwise.

## Tests

| Test | Description |
|------|-------------|
| `test_gemini_provider_name` | Verifies name and requires_api_key |
| `test_gemini_request_serialization` | Verifies request JSON structure with systemInstruction |
| `test_gemini_request_no_system` | Verifies systemInstruction omitted when None |
| `test_gemini_role_mapping` | Verifies assistant→model role conversion |
| `test_gemini_response_parsing` | Verifies response deserialization for a plain text response |
| `test_gemini_function_call_response_parsing` | Verifies deserialization of a response part carrying a `function_call` |
| `test_gemini_part_serialization` | Verifies that `GeminiPart` serializes only the non-None variant field (text, function_call, or function_response) |
| `test_gemini_tool_declaration_serialization` | Verifies that `GeminiToolDeclaration` and `GeminiFunctionDef` serialize to the expected JSON schema shape |
