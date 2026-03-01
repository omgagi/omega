# Technical Specification: `provider_builder.md`

## File

| Property | Value |
|----------|-------|
| **Path** | `backend/src/provider_builder.rs` |
| **Crate** | `omega` (binary crate root) |
| **Module** | `mod provider_builder` |
| **Status** | Implemented |

## Purpose

Factory function that constructs the configured AI provider from `Config`. Returns a tuple of `(Box<dyn Provider>, model_fast, model_complex)` so the gateway can route between fast (classification) and complex (execution) models. This is the single point where provider selection and construction happens for the entire application.

## Public Functions

### `build_provider(cfg, workspace_path) -> anyhow::Result<(Box<dyn Provider>, String, String)>`

Reads `cfg.provider.default` to determine which provider to instantiate. Returns:

- `Box<dyn Provider>` -- the provider instance implementing the `Provider` trait
- `model_fast: String` -- model identifier for fast/classification calls
- `model_complex: String` -- model identifier for complex/execution calls

```rust
pub fn build_provider(
    cfg: &config::Config,
    workspace_path: &std::path::Path,
) -> anyhow::Result<(Box<dyn Provider>, String, String)>
```

## Provider Construction Table

| `cfg.provider.default` | Config Section | Constructor | `model_fast` | `model_complex` |
|------------------------|----------------|-------------|-------------|----------------|
| `"claude-code"` | `provider.claude_code` (defaults if absent) | `ClaudeCodeProvider::from_config(max_turns, allowed_tools, timeout_secs, ws, max_resume_attempts, model)` | `cc.model` (e.g. `"claude-sonnet-4-6"`) | `cc.model_complex` (e.g. `"claude-opus-4-6"`) |
| `"ollama"` | `provider.ollama` (required) | `OllamaProvider::from_config(base_url, model, ws)` | `model` | `model` (same) |
| `"openai"` | `provider.openai` (required) | `OpenAiProvider::from_config(base_url, api_key, model, ws)` | `model` | `model` (same) |
| `"anthropic"` | `provider.anthropic` (required) | `AnthropicProvider::from_config(api_key, model, max_tokens, ws)` | `model` | `model` (same) |
| `"openrouter"` | `provider.openrouter` (required) | `OpenRouterProvider::from_config(api_key, model, ws)` | `model` | `model` (same) |
| `"gemini"` | `provider.gemini` (required) | `GeminiProvider::from_config(api_key, model, ws)` | `model` | `model` (same) |
| anything else | -- | -- | `anyhow::bail!("unsupported provider: {other}")` | -- |

## Key Design Decisions

### Claude Code: Dual-Model Routing

Claude Code is the only provider with distinct `model_fast` and `model_complex` values. This comes from its config having both a `model` field (Sonnet, for classification) and a `model_complex` field (Opus, for execution). All other providers set both to the same configured model string.

### Claude Code: Optional Config Section

The `claude_code` config section is optional. When absent, `unwrap_or_default()` provides `ClaudeCodeConfig::default()` (max_turns=25, allowed_tools=[], model="claude-sonnet-4-6", model_complex="claude-opus-4-6"). All other providers require their config section to be present and return `anyhow::bail!` if missing.

### Workspace Path

All providers receive `workspace_path` as `Option<PathBuf>` set to `Some(workspace_path.to_path_buf())`. This tells providers where the AI subprocess working directory is, enabling tool execution scoped to that directory.

## Imports

| Source | Items |
|--------|-------|
| `omega_core::config` | `Config` (and all sub-config types via `config::*` in tests) |
| `omega_core::traits` | `Provider` trait |
| `omega_providers::anthropic` | `AnthropicProvider` |
| `omega_providers::claude_code` | `ClaudeCodeProvider` |
| `omega_providers::gemini` | `GeminiProvider` |
| `omega_providers::ollama` | `OllamaProvider` |
| `omega_providers::openai` | `OpenAiProvider` |
| `omega_providers::openrouter` | `OpenRouterProvider` |

## Error Handling

| Condition | Error |
|-----------|-------|
| Unsupported provider name | `anyhow::bail!("unsupported provider: {other}")` |
| Missing config section (non-claude-code) | `anyhow::anyhow!("provider.<name> section missing in config")` |
| Provider `from_config()` failure | Propagated via `?` operator |

Claude Code never fails due to missing config because it falls back to defaults.

## Tests

| Test | Description |
|------|-------------|
| `test_unsupported_provider_returns_error` | Verifies `"nonexistent"` provider name returns error containing "unsupported provider" |
| `test_claude_code_defaults_succeeds` | Verifies default Claude Code builds successfully with Sonnet fast model and Opus complex model |
| `test_claude_code_custom_models` | Verifies custom model strings propagate to `model_fast` and `model_complex` |
| `test_ollama_missing_config_returns_error` | Verifies missing `provider.ollama` section returns descriptive error |
| `test_openai_missing_config_returns_error` | Verifies missing `provider.openai` section returns descriptive error |
| `test_anthropic_missing_config_returns_error` | Verifies missing `provider.anthropic` section returns descriptive error |
| `test_openrouter_missing_config_returns_error` | Verifies missing `provider.openrouter` section returns descriptive error |
| `test_gemini_missing_config_returns_error` | Verifies missing `provider.gemini` section returns descriptive error |
| `test_ollama_with_config_returns_same_model_for_both` | Verifies Ollama returns same model string for both `model_fast` and `model_complex` |
| `test_anthropic_with_config_succeeds` | Verifies Anthropic builds successfully with `max_tokens` passed through |

### Test Helper

`test_config(provider_name: &str) -> Config` -- builds a minimal `Config` with all defaults and the given provider name set as `cfg.provider.default`. Uses `Default::default()` for all config sections (OmegaConfig, AuthConfig, ProviderConfig, ChannelConfig, MemoryConfig, HeartbeatConfig, SchedulerConfig, ApiConfig).
