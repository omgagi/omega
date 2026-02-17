# DOCS.md — Omega Documentation


> Master index of all user-facing and developer documentation for the Omega project.

## Overview


Omega is a personal AI agent infrastructure written in Rust. This `docs/` directory contains guides, references, and explanations organized by topic.

## Documentation Files

### Getting Started
- [quickstart.md](quickstart.md) — Build, configure, and run Omega in 2 minutes

### Architecture
- [architecture.md](architecture.md) — System design, crate structure, data flow

### Configuration
- [configuration.md](configuration.md) — config.toml reference, environment variables, defaults

### Security
- [security.md](security.md) — Auth, sanitization, root guard, prompt injection defense
- [sandbox-lib.md](sandbox-lib.md) — Sandbox modes (sandbox/rx/rwx), workspace isolation, system prompt enforcement

### Providers
- [providers.md](providers.md) — AI backend configuration (Claude Code, Anthropic, OpenAI, Ollama, OpenRouter)

### Channels
- [channels.md](channels.md) — Messaging platform integration (Telegram, WhatsApp)

### Memory
- [memory.md](memory.md) — Conversation storage, facts, summaries, context building
- [memory-migration-004.md](memory-migration-004.md) — FTS5 cross-conversation recall
- [memory-migration-005.md](memory-migration-005.md) — Scheduled tasks table and indexes

### Operations
- [operations.md](operations.md) — LaunchAgent setup, logging, self-check, graceful shutdown
- [service.md](service.md) — OS-aware service management (install, uninstall, status)

### Proactive Features
- [scheduler.md](scheduler.md) — Task queue: reminders, recurring tasks, natural language scheduling
- [heartbeat.md](heartbeat.md) — Periodic AI check-ins, health monitoring, alert suppression

### Commands
- [commands.md](commands.md) — Bot command reference (/status, /memory, /history, /facts, /forget, /tasks, /cancel, /skills, /projects, /project, /help)

### Development
- [development.md](development.md) — Build, test, lint, code conventions, contribution guidelines
