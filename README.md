# Ω Omega

**Personal AI agent infrastructure, forged in Rust.**

A lightweight, self-hosted AI agent that connects to messaging platforms and delegates reasoning to configurable AI backends — with Claude Code CLI as the zero-config, first-class citizen.

## Quick Start

```bash
cargo build --release
cp config.example.toml config.toml  # Edit with your settings
./target/release/omega start
```

## Features

- **Single binary** — no runtime dependencies, no Docker required
- **Claude Code as first-class provider** — zero API keys needed, uses local `claude` CLI auth
- **Model-agnostic** — supports Claude Code, Anthropic API, OpenAI, Ollama, OpenRouter
- **Telegram integration** — long polling, Markdown formatting, message splitting
- **Persistent memory** — SQLite-backed conversation history with context windowing
- **Audit log** — every interaction recorded with timestamp, input, output, and status
- **Auth enforcement** — per-channel user ID allowlists, unauthorized attempts logged and denied
- **Prompt injection defense** — sanitizes role tags, instruction overrides, and delimiter injection
- **macOS LaunchAgent** — runs as a persistent background service

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                    OMEGA GATEWAY                      │
│  Auth → Sanitize → Memory → Provider → Audit → Send  │
├──────────────┬──────────────┬────────────────────────┤
│   Channels   │  Orchestrator │      Services          │
├──────────────┼──────────────┼────────────────────────┤
│ • Telegram   │ • Router     │ • Memory (SQLite)      │
│ (future:     │ • Context    │ • Audit log            │
│  WhatsApp,   │ • Middleware  │ • Skills registry      │
│  Discord)    │   pipeline   │ • Sandbox executor     │
├──────────────┴──────────────┴────────────────────────┤
│                  PROVIDER LAYER                       │
├──────────────────────────────────────────────────────┤
│ ClaudeCode │ Anthropic │ OpenAI │ Ollama │ OpenRouter │
└──────────────────────────────────────────────────────┘
```

## Project Structure

```
omega/
├── Cargo.toml              # Workspace root
├── config.example.toml     # Example configuration
├── crates/
│   ├── omega-core/         # Types, traits, config, error handling, sanitization
│   ├── omega-providers/    # AI provider implementations (Claude Code, etc.)
│   ├── omega-channels/     # Messaging platforms (Telegram, etc.)
│   ├── omega-memory/       # SQLite storage, audit log
│   ├── omega-skills/       # Skill/plugin system (planned)
│   └── omega-sandbox/      # Secure execution (planned)
└── src/
    ├── main.rs             # CLI entry point
    └── gateway.rs          # Main event loop
```

## Commands

```bash
omega start              # Start the agent (connects to enabled channels)
omega status             # Check provider and channel availability
omega ask "question"     # One-shot query via CLI
```

## Configuration

Copy `config.example.toml` to `config.toml`. Key sections:

```toml
[auth]
enabled = true                    # Enforce user ID allowlists

[provider]
default = "claude-code"           # No API key needed

[channel.telegram]
enabled = true
bot_token = "YOUR_TOKEN"          # From @BotFather
allowed_users = [123456789]       # Telegram user IDs (empty = allow all)

[memory]
db_path = "~/.omega/memory.db"
max_context_messages = 50
```

## Security

- **Auth**: Per-channel user ID allowlists. Unauthorized messages are rejected and audit-logged.
- **Sanitization**: Neutralizes prompt injection patterns (role tags, instruction overrides, delimiter injection) before they reach the provider.
- **Audit log**: Every interaction is recorded in SQLite with full traceability.
- **Root guard**: Omega refuses to run as root to prevent privilege escalation.
- **No secrets in repo**: `config.toml` is gitignored; credentials never committed.

## macOS Service

Install as a persistent LaunchAgent:

```bash
cp com.ilozada.omega.plist ~/Library/LaunchAgents/
launchctl load ~/Library/LaunchAgents/com.ilozada.omega.plist
```

Logs: `~/.omega/omega.log`

## Development

```bash
cargo check                  # Type check
cargo clippy --workspace     # Lint
cargo test --workspace       # Run tests
cargo fmt                    # Format
cargo build --release        # Build optimized binary
```

## License

MIT
