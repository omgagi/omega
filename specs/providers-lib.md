# Technical Specification: `backend/crates/omega-providers/src/lib.rs`

## File

| Field | Value |
|-------|-------|
| Path | `backend/crates/omega-providers/src/lib.rs` |
| Crate | `omega-providers` |
| Role | Crate root -- declares modules and controls public API surface |

## Purpose

`lib.rs` is the entry point for the `omega-providers` crate. It serves two purposes:

1. Declare each AI-provider module (one per backend).
2. Control which modules are publicly accessible to downstream crates.

The file itself contains no types, traits, or functions. Its entire job is module wiring and visibility.

## Module Declarations

| Module | Visibility | Status | Description |
|--------|-----------|--------|-------------|
| `anthropic` | `pub mod` (public) | Implemented | Anthropic Messages API provider. Non-streaming HTTP via `reqwest`. |
| `claude_code` | `pub mod` (public) | Implemented | Claude Code CLI provider. Invokes `claude` as a subprocess. |
| `gemini` | `pub mod` (public) | Implemented | Google Gemini API provider. Non-streaming HTTP via `reqwest`. |
| `mcp_client` | `pub(crate) mod` | Implemented | MCP (Model Context Protocol) client used by providers to call MCP-exposed tools. |
| `ollama` | `pub mod` (public) | Implemented | Ollama local model provider. Non-streaming HTTP via `reqwest`. |
| `openai` | `pub mod` (public) | Implemented | OpenAI-compatible API provider. Non-streaming HTTP via `reqwest`. Exports `pub(crate)` types and the shared agentic loop reused by OpenRouter. |
| `openrouter` | `pub mod` (public) | Implemented | OpenRouter proxy provider. Reuses OpenAI request/response types and delegates to `openai_agentic_complete()`. |
| `tools` | `pub(crate) mod` | Implemented | `ToolExecutor` — wraps workspace execution of tool calls dispatched by the agentic loop across all HTTP providers. |

## Re-exports

There are **no** explicit `pub use` re-exports in `lib.rs`. All modules are declared with `pub mod`, so downstream consumers access contents as:

```rust
use omega_providers::claude_code::ClaudeCodeProvider;
use omega_providers::ollama::OllamaProvider;
use omega_providers::openai::OpenAiProvider;
use omega_providers::anthropic::AnthropicProvider;
use omega_providers::openrouter::OpenRouterProvider;
use omega_providers::gemini::GeminiProvider;
```

## Public API Surface

Each module exports a provider struct with `from_config()` constructor and `Provider` trait impl:

| Module | Struct | `from_config()` Params |
|--------|--------|----------------------|
| `claude_code` | `ClaudeCodeProvider` | max_turns, allowed_tools, timeout_secs, working_dir, max_resume_attempts, model |
| `ollama` | `OllamaProvider` | base_url, model, workspace_path |
| `openai` | `OpenAiProvider` | base_url, api_key, model, workspace_path |
| `anthropic` | `AnthropicProvider` | api_key, model, max_tokens, workspace_path |
| `openrouter` | `OpenRouterProvider` | api_key, model, workspace_path |
| `gemini` | `GeminiProvider` | api_key, model, workspace_path |

Additionally, `openai` exports `pub(crate)` types and functions reused by `openrouter` and the shared agentic loop:
- `ChatMessage`, `ChatCompletionRequest`, `ChatCompletionResponse`, `ChatChoice`, `build_openai_messages()`
- `ToolCallMsg`, `FunctionCall`, `OpenAiToolDef`, `OpenAiFunctionDef`
- `openai_agentic_complete()` — the shared agentic tool-calling loop function

## Dependencies

Declared in `Cargo.toml` (all workspace-level):

| Dependency | Usage |
|------------|-------|
| `omega-core` | `Provider` trait, `Context`, `ApiMessage`, `OmegaError`, `OutgoingMessage`, `MessageMetadata` |
| `omega-sandbox` | `protected_command()` for Claude Code CLI filesystem protection |
| `tokio` | Async runtime, `tokio::process::Command` for Claude Code subprocess |
| `serde` / `serde_json` | Serialize/deserialize request and response JSON |
| `tracing` | `debug!` and `warn!` log macros |
| `thiserror` | (available but unused) |
| `anyhow` | (available but unused) |
| `async-trait` | `#[async_trait]` attribute on `Provider` impls |
| `reqwest` | HTTP client for all API-based providers (Ollama, OpenAI, Anthropic, OpenRouter, Gemini) |

## Shared Provider Pattern

All HTTP-based providers follow the same pattern:
1. `let (system, messages) = context.to_api_messages()`
2. `let effective_model = context.model.as_deref().unwrap_or(&self.model)`
3. Check `allowed_tools` / `workspace_path` — determine whether the agentic tool-calling path is active
4. If `workspace_path` is set, create a `ToolExecutor` scoped to that directory
5. If tools are available, enter the provider-specific agentic loop (or call `openai_agentic_complete()` for OpenAI/OpenRouter), iterating until the model stops requesting tool calls or the max-iterations guard fires
6. Otherwise (no workspace or no tools), fall back to the plain single-shot POST → parse response → return
7. Return `OutgoingMessage { text, metadata: { provider_used, tokens_used, processing_time_ms, model } }`

All use non-streaming (`"stream": false` or no streaming parameter). The gateway handles status timers for long responses.
