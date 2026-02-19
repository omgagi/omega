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

### Constructor

```rust
pub fn from_config(api_key: String, model: String) -> Self
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

### Request

```json
{
  "contents": [
    {"role": "user", "parts": [{"text": "..."}]}
  ],
  "systemInstruction": {
    "parts": [{"text": "..."}]
  }
}
```

- `systemInstruction` is a top-level field (separate from contents), omitted when empty
- Uses `camelCase` serialization via `#[serde(rename_all = "camelCase")]`

### Response

| Field | Type | Description |
|-------|------|-------------|
| `candidates[0].content.parts[0].text` | `String` | Response text |
| `usageMetadata.totalTokenCount` | `u64` | Total tokens |

## Tests

| Test | Description |
|------|-------------|
| `test_gemini_provider_name` | Verifies name and requires_api_key |
| `test_gemini_request_serialization` | Verifies request JSON structure with systemInstruction |
| `test_gemini_request_no_system` | Verifies systemInstruction omitted when None |
| `test_gemini_role_mapping` | Verifies assistant→model role conversion |
| `test_gemini_response_parsing` | Verifies response deserialization |
