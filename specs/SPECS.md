# SPECS.md — Omega Technical Specifications

> Master index of all technical specification documents for the Omega codebase.

## Overview

Omega is a personal AI agent infrastructure written in Rust. This `specs/` directory contains detailed technical specifications for every file in the repository, organized by functional domain (milestone).

## Specification Files

### Milestone 1: Root / Workspace
- [workspace.md](workspace.md) — Cargo workspace, dependencies, config, gitignore, license, and Claude integration

### Milestone 2: Binary (`src/`)
- [binary-main.md](binary-main.md) — Entry point, CLI parsing, root guard, provider/channel bootstrap
- [binary-gateway.md](binary-gateway.md) — Gateway event loop, message pipeline, auth, summarization, shutdown
- [binary-commands.md](binary-commands.md) — Built-in bot commands (status, memory, history, facts, forget, tasks, cancel, skills, help)
- [binary-init.md](binary-init.md) — Interactive setup wizard
- [binary-selfcheck.md](binary-selfcheck.md) — Startup health checks
- [binary-service.md](binary-service.md) — OS-aware service management (macOS LaunchAgent / Linux systemd)

### Milestone 3: omega-core
- [core-lib.md](core-lib.md) — Core crate overview, module re-exports
- [core-config.md](core-config.md) — Configuration system (TOML + env, all config structs)
- [core-context.md](core-context.md) — Conversation context model for AI providers
- [core-error.md](core-error.md) — Error types (OmegaError enum)
- [core-message.md](core-message.md) — Message types (incoming, outgoing, metadata, attachments)
- [core-sanitize.md](core-sanitize.md) — Prompt injection sanitization
- [core-traits.md](core-traits.md) — Provider and Channel trait definitions

### Milestone 4: omega-providers
- [providers-lib.md](providers-lib.md) — Providers crate overview
- [providers-claude-code.md](providers-claude-code.md) — Claude Code CLI provider (primary)
- [providers-anthropic.md](providers-anthropic.md) — Anthropic API provider (placeholder)
- [providers-openai.md](providers-openai.md) — OpenAI-compatible provider (placeholder)
- [providers-ollama.md](providers-ollama.md) — Ollama local provider (placeholder)
- [providers-openrouter.md](providers-openrouter.md) — OpenRouter proxy provider (placeholder)

### Milestone 5: omega-channels
- [channels-lib.md](channels-lib.md) — Channels crate overview
- [channels-telegram.md](channels-telegram.md) — Telegram Bot API channel (long polling)
- [channels-whatsapp.md](channels-whatsapp.md) — WhatsApp bridge channel (placeholder)

### Milestone 6: omega-memory
- [memory-lib.md](memory-lib.md) — Memory crate overview
- [memory-store.md](memory-store.md) — SQLite persistent store, conversations, facts, context building
- [memory-audit.md](memory-audit.md) — Audit logging system
- [memory-migrations.md](memory-migrations.md) — Database schema and migration system
- [memory-migration-004.md](memory-migration-004.md) — FTS5 cross-conversation recall migration
- [memory-migration-005.md](memory-migration-005.md) — Scheduled tasks table migration

### Milestone 7: omega-skills
- [skills-lib.md](skills-lib.md) — Skill loader + project loader (skills from `~/.omega/skills/*/SKILL.md`, projects from `~/.omega/projects/*/INSTRUCTIONS.md`)

### Milestone 8: omega-sandbox
- [sandbox-lib.md](sandbox-lib.md) — 3-level workspace sandbox (sandbox/rx/rwx modes)
- [sandbox-cargo-toml.md](sandbox-cargo-toml.md) — Sandbox crate Cargo.toml

## Architecture Diagram

```
┌─────────────────────────────────────────────────┐
│                   omega (binary)                 │
│  main.rs → gateway.rs → commands.rs             │
│              init.rs    selfcheck.rs             │
│              service.rs                          │
├─────────────────────────────────────────────────┤
│  omega-core     │ omega-providers │ omega-channels│
│  config.rs      │ claude_code.rs  │ telegram.rs   │
│  context.rs     │ anthropic.rs    │ whatsapp.rs   │
│  error.rs       │ openai.rs       │               │
│  message.rs     │ ollama.rs       │               │
│  sanitize.rs    │ openrouter.rs   │               │
│  traits.rs      │                 │               │
├─────────────────┼─────────────────┼───────────────┤
│  omega-memory   │ omega-skills    │ omega-sandbox  │
│  store.rs       │ lib.rs (loader) │ (planned)      │
│  audit.rs       │                 │                │
│  migrations/    │                 │                │
└─────────────────┴─────────────────┴───────────────┘
```

## Data Flow

```
Message → Auth → Sanitize → Sandbox constraint → Memory (context) → Provider → SCHEDULE extract → Memory (store) → Audit → Send

Background:
  Scheduler: poll due_tasks → channel.send(reminder) → complete_task
  Heartbeat: provider.complete(check-in) → suppress HEARTBEAT_OK / channel.send(alert)
  Summarizer: find idle convos → summarize → close
```
