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

### Constructor

```rust
pub fn from_config(base_url: String, api_key: String, model: String) -> Self
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
| `ChatMessage` | `{ role, content }` |
| `ChatCompletionRequest` | `{ model, messages }` |
| `ChatCompletionResponse` | `{ choices, model, usage }` |
| `ChatChoice` | `{ message }` |
| `ChatUsage` | `{ total_tokens }` |
| `build_openai_messages()` | Converts system prompt + ApiMessages into ChatMessages |

## Request/Response Format

### Request

```json
{
  "model": "gpt-4o",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
  ]
}
```

Headers: `Authorization: Bearer {api_key}`

### Response

| Field | Type | Description |
|-------|------|-------------|
| `choices[0].message.content` | `String` | Response text |
| `model` | `String` | Model used |
| `usage.total_tokens` | `u64` | Total token count |

## Tests

| Test | Description |
|------|-------------|
| `test_openai_provider_name` | Verifies name and requires_api_key |
| `test_build_openai_messages` | Verifies message building with system prompt |
| `test_build_openai_messages_empty_system` | Verifies system prompt omission when empty |
| `test_openai_response_parsing` | Verifies response deserialization |
