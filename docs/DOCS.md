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
- [sandbox-lib.md](sandbox-lib.md) — Blocklist-based system protection (always-on, blocks writes to OS dirs + memory.db)

### Providers
- [providers.md](providers.md) — AI backend configuration (Claude Code, Ollama, OpenAI, Anthropic, OpenRouter, Gemini)

### Channels
- [channels.md](channels.md) — Messaging platform integration (Telegram, WhatsApp)

### Memory
- [memory.md](memory.md) — Conversation storage, facts, summaries, context building
- [memory-migration-004.md](memory-migration-004.md) — FTS5 cross-conversation recall
- [memory-migration-005.md](memory-migration-005.md) — Scheduled tasks table and indexes
- [memory-migration-007.md](memory-migration-007.md) — Task type column for action scheduler
- [memory-migration-009.md](memory-migration-009.md) — Task retry columns for action failure handling
- [memory-migration-010.md](memory-migration-010.md) — Reward-based learning tables (outcomes + lessons)

### HTTP API
- [api.md](api.md) — HTTP API for SaaS dashboard integration (health, WhatsApp QR pairing)

### Operations
- [operations.md](operations.md) — LaunchAgent setup, logging, self-check, graceful shutdown
- [service.md](service.md) — OS-aware service management (install, uninstall, status)

### Proactive Features
- [scheduler.md](scheduler.md) — Task queue: reminders, recurring tasks, natural language scheduling
- [heartbeat.md](heartbeat.md) — Periodic AI check-ins, health monitoring, alert suppression
- [introspection.md](introspection.md) — Autonomous skill improvement (SKILL_IMPROVE marker), reward-based learning (REWARD/LESSON markers, outcomes + lessons tables), self-audit (anomaly flagging + audit DB access)
- [claudemd.md](claudemd.md) — Workspace CLAUDE.md maintenance (auto-creation and periodic refresh for Claude Code subprocess context)

### Commands
- [commands.md](commands.md) — Bot command reference (/status, /memory, /history, /facts, /forget, /tasks, /cancel, /skills, /purge, /projects, /project, /help)

### Skills & MCP
- [skills-lib.md](skills-lib.md) — Skill loader, trigger matching, MCP server definitions
- [skills-cargo-toml.md](skills-cargo-toml.md) — omega-skills Cargo manifest and dependency guide

### Core
- [core-context.md](core-context.md) — Context struct, McpServer, conversation history, prompt flattening

### Provider Internals
- [providers-claude-code.md](providers-claude-code.md) — Claude Code CLI provider, MCP settings, JSON response handling
- [agentic-tools.md](agentic-tools.md) — Built-in tool executor, MCP client, agentic loop pattern

### Quantitative Trading
- [quant.md](quant.md) — Standalone CLI tool (omega-quant), signal format, safety guardrails, skill-based invocation

### Development
- [development.md](development.md) — Build, test, lint, code conventions, contribution guidelines
