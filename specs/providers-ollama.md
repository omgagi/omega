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

### Constructor

```rust
pub fn from_config(base_url: String, model: String) -> Self
```

## Provider Trait Implementation

| Method | Behavior |
|--------|----------|
| `name()` | Returns `"ollama"` |
| `requires_api_key()` | Returns `false` |
| `complete()` | POST `{base_url}/api/chat` with `{ model, messages, stream: false }` |
| `is_available()` | GET `{base_url}/api/tags` → checks for 200 OK |

## Request/Response Types

### Request: `OllamaChatRequest`

```json
{
  "model": "llama3",
  "messages": [
    {"role": "system", "content": "..."},
    {"role": "user", "content": "..."}
  ],
  "stream": false
}
```

### Response: `OllamaChatResponse`

| Field | Type | Description |
|-------|------|-------------|
| `message` | `Option<OllamaChatMessage>` | Response message with role and content |
| `model` | `Option<String>` | Model used |
| `eval_count` | `Option<u64>` | Tokens generated |
| `prompt_eval_count` | `Option<u64>` | Tokens in prompt |

Token count = `eval_count + prompt_eval_count` when both present.

## Tests

| Test | Description |
|------|-------------|
| `test_ollama_provider_name` | Verifies name and requires_api_key |
| `test_ollama_request_serialization` | Verifies request JSON structure |
| `test_ollama_response_parsing` | Verifies response deserialization |
