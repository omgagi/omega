# Technical Specification: `providers-openrouter.md`

## File

| Property | Value |
|----------|-------|
| **Path** | `crates/omega-providers/src/openrouter.rs` |
| **Crate** | `omega-providers` |
| **Module** | `pub mod openrouter` |
| **Status** | Implemented |

## Purpose

Routes requests through OpenRouter's API proxy, which provides access to many models via an OpenAI-compatible interface. Reuses OpenAI's request/response types to minimize code duplication.

## Struct: `OpenRouterProvider`

| Field | Type | Description |
|-------|------|-------------|
| `client` | `reqwest::Client` | HTTP client |
| `api_key` | `String` | OpenRouter API key |
| `model` | `String` | Namespaced model (e.g., `anthropic/claude-sonnet-4`) |
| `workspace_path` | `Option<PathBuf>` | Working directory for tool execution; `None` disables tool calling |
### Constructor

```rust
pub fn from_config(api_key: String, model: String, workspace_path: Option<PathBuf>) -> Self
```

## Provider Trait Implementation

| Method | Behavior |
|--------|----------|
| `name()` | Returns `"openrouter"` |
| `requires_api_key()` | Returns `true` |
| `complete()` | POST `https://openrouter.ai/api/v1/chat/completions` with Bearer auth |
| `is_available()` | GET `https://openrouter.ai/api/v1/models` with Bearer auth → checks for 200 |

## Code Reuse

Imports from `crate::openai`:
- `build_openai_messages()` — converts system + ApiMessages into ChatMessages
- `openai_agentic_complete()` — shared agentic tool-calling loop; `complete()` delegates to this when tools are enabled
- `ChatCompletionRequest` — request body type
- `ChatCompletionResponse` — response body type
- `OpenAiToolDef`, `ToolCallMsg`, `FunctionCall` — tool-related types

The only differences from OpenAI:
1. Base URL: `https://openrouter.ai/api/v1` (constant `OPENROUTER_BASE_URL`)
2. Provider name: `"openrouter"`
3. Model names are namespaced: `vendor/model` (e.g., `anthropic/claude-sonnet-4`)

## Tests

| Test | Description |
|------|-------------|
| `test_openrouter_provider_name` | Verifies name and requires_api_key |
| `test_openrouter_base_url` | Verifies constant URL value |
