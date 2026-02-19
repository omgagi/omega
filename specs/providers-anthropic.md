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

### Constructor

```rust
pub fn from_config(api_key: String, model: String) -> Self
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

### Request

```json
{
  "model": "claude-sonnet-4-20250514",
  "max_tokens": 8192,
  "system": "...",
  "messages": [
    {"role": "user", "content": "..."}
  ]
}
```

Key difference: `system` is a top-level field, not a message role. Empty system is omitted via `#[serde(skip_serializing_if = "String::is_empty")]`.

### Response

| Field | Type | Description |
|-------|------|-------------|
| `content[0].text` | `String` | Response text |
| `model` | `String` | Model used |
| `usage.input_tokens` | `u64` | Input tokens |
| `usage.output_tokens` | `u64` | Output tokens |

Token count = `input_tokens + output_tokens`.

## Tests

| Test | Description |
|------|-------------|
| `test_anthropic_provider_name` | Verifies name and requires_api_key |
| `test_anthropic_request_serialization` | Verifies request JSON structure |
| `test_anthropic_request_empty_system_omitted` | Verifies empty system field is excluded from JSON |
| `test_anthropic_response_parsing` | Verifies response deserialization |
