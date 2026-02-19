# Providers — AI Backend Configuration

Omega supports 6 AI providers. Set `provider.default` in `config.toml` to switch between them.

## Provider Summary

| Provider | Auth | Transport | Default Model |
|----------|------|-----------|---------------|
| `claude-code` | None (local CLI) | Subprocess | `claude-sonnet-4-6` |
| `ollama` | None (local server) | HTTP | `llama3` |
| `openai` | Bearer token | HTTP | `gpt-4o` |
| `anthropic` | x-api-key header | HTTP | `claude-sonnet-4-20250514` |
| `openrouter` | Bearer token | HTTP | (namespaced, e.g. `anthropic/claude-sonnet-4`) |
| `gemini` | URL query param | HTTP | `gemini-2.0-flash` |

## Claude Code (Default)

Uses the locally installed `claude` CLI. Zero API keys needed.

```toml
[provider]
default = "claude-code"

[provider.claude-code]
enabled = true
max_turns = 100
allowed_tools = ["Bash", "Read", "Write", "Edit"]
timeout_secs = 3600
max_resume_attempts = 5
model = "claude-sonnet-4-6"
model_complex = "claude-opus-4-6"
```

## Ollama (Local, Free)

Requires a running Ollama server. No API key needed.

```toml
[provider]
default = "ollama"

[provider.ollama]
enabled = true
base_url = "http://localhost:11434"
model = "llama3"
```

Install and run Ollama: `curl -fsSL https://ollama.ai/install.sh | sh && ollama run llama3`

## OpenAI

Requires an OpenAI API key.

```toml
[provider]
default = "openai"

[provider.openai]
enabled = true
api_key = "sk-..."  # Or env: OPENAI_API_KEY
model = "gpt-4o"
base_url = "https://api.openai.com/v1"
```

The `base_url` can point to any OpenAI-compatible endpoint (Azure, local proxies, etc.).

## Anthropic API

Calls the Anthropic Messages API directly (different from the `claude-code` CLI provider).

```toml
[provider]
default = "anthropic"

[provider.anthropic]
enabled = true
api_key = "sk-ant-..."  # Or env: ANTHROPIC_API_KEY
model = "claude-sonnet-4-20250514"
```

## OpenRouter

Routes requests through OpenRouter's proxy. Access to many models from different vendors.

```toml
[provider]
default = "openrouter"

[provider.openrouter]
enabled = true
api_key = "sk-or-..."  # Or env: OPENROUTER_API_KEY
model = "anthropic/claude-sonnet-4-20250514"
```

Models use namespaced identifiers like `anthropic/claude-sonnet-4`, `openai/gpt-4o`, `meta-llama/llama-3-70b`.

## Google Gemini

Requires a Google AI API key.

```toml
[provider]
default = "gemini"

[provider.gemini]
enabled = true
api_key = "AIza..."  # Or env: GEMINI_API_KEY
model = "gemini-2.0-flash"
```

## How It Works

All HTTP-based providers (everything except `claude-code`) follow the same pattern:

1. Context is converted to structured API messages via `context.to_api_messages()`
2. The system prompt is separated (Anthropic and Gemini need it outside the messages array)
3. A non-streaming HTTP POST is made with the provider-specific request format
4. The response is parsed and returned as an `OutgoingMessage` with metadata (tokens, model, timing)

The gateway handles status timers ("This is taking a moment...") for all providers uniformly.

## Model Override

The gateway's classify-and-route system can override the model per-request via `Context.model`. When set, the provider uses the override instead of its configured default.
