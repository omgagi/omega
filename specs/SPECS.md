# SPECS.md — Omega Technical Specifications

> Master index of all technical specification documents for the Omega codebase.

## Overview

Omega is a personal AI agent infrastructure written in Rust. This `specs/` directory contains detailed technical specifications for every file in the repository, organized by functional domain (milestone).

## Specification Files

### Milestone 1: Root / Workspace
- [workspace.md](workspace.md) — Cargo workspace, dependencies, config, gitignore, license, and Claude integration

### Milestone 2: Binary (`src/`)
- [binary-main.md](binary-main.md) — Entry point, CLI parsing, root guard, provider/channel bootstrap
- [src-gateway-rs.md](src-gateway-rs.md) — Gateway module (`src/gateway/`) — 9-file directory module: orchestrator, pipeline, routing, markers, auth, scheduler, heartbeat, summarizer, keywords
- [src-markers-rs.md](src-markers-rs.md) — Marker extraction, parsing, stripping (40+ functions extracted from gateway)
- [src-task-confirmation-rs.md](src-task-confirmation-rs.md) — Task scheduling confirmation (anti-hallucination, duplicate detection, localized confirmation messages)
- [binary-commands.md](binary-commands.md) — Built-in bot commands (status, memory, history, facts, forget, tasks, cancel, skills, purge, help)
- [src-init-rs.md](src-init-rs.md) — Setup wizard (interactive + non-interactive modes), config generation
- [src-init-wizard-rs.md](src-init-wizard-rs.md) — **New** — Interactive-only init helpers (browser detection, Anthropic auth, WhatsApp QR, Google OAuth)
- [binary-selfcheck.md](binary-selfcheck.md) — Startup health checks
- [binary-service.md](binary-service.md) — OS-aware service management (macOS LaunchAgent / Linux systemd)
- [src-claudemd-rs.md](src-claudemd-rs.md) — Workspace CLAUDE.md maintenance (init + periodic refresh via claude CLI subprocess)
- [src-api-rs.md](src-api-rs.md) — HTTP API server (axum, health check, WhatsApp QR pairing for SaaS dashboards)

### Milestone 3: omega-core
- [core-lib.md](core-lib.md) — Core crate overview, module re-exports
- [core-config.md](core-config.md) — Configuration system (TOML + env, all config structs)
- [core-context.md](core-context.md) — Conversation context model for AI providers
- [core-error.md](core-error.md) — Error types (OmegaError enum)
- [core-message.md](core-message.md) — Message types (incoming, outgoing, metadata, attachments)
- [core-sanitize.md](core-sanitize.md) — Prompt injection sanitization
- [core-traits.md](core-traits.md) — Provider and Channel trait definitions

### Milestone 4: omega-providers
- [providers-lib.md](providers-lib.md) — Providers crate overview (all 6 modules public)
- [providers-claude-code.md](providers-claude-code.md) — Claude Code CLI provider (primary, subprocess-based)
- [providers-ollama.md](providers-ollama.md) — Ollama local provider (HTTP, no auth)
- [providers-openai.md](providers-openai.md) — OpenAI-compatible provider (HTTP, Bearer auth, exports shared types)
- [providers-openrouter.md](providers-openrouter.md) — OpenRouter proxy provider (reuses OpenAI types)
- [providers-anthropic.md](providers-anthropic.md) — Anthropic Messages API provider (HTTP, x-api-key header)
- [providers-gemini.md](providers-gemini.md) — Google Gemini API provider (HTTP, URL query param auth)
- [providers-mcp-client.md](providers-mcp-client.md) — MCP client over stdio (JSON-RPC 2.0, tool discovery, tool calling)
- [providers-tools.md](providers-tools.md) — Shared tool executor (bash/read/write/edit + MCP routing + sandbox enforcement)

### Milestone 5: omega-channels
- [channels-lib.md](channels-lib.md) — Channels crate overview
- [channels-telegram.md](channels-telegram.md) — Telegram Bot API channel (long polling)
- [channels-whatsapp.md](channels-whatsapp.md) — WhatsApp Web protocol channel (text, image, voice, group chat, markdown, retry)

### Milestone 6: omega-memory
- [memory-lib.md](memory-lib.md) — Memory crate overview
- [memory-store.md](memory-store.md) — SQLite persistent store, conversations, facts, context building
- [memory-audit.md](memory-audit.md) — Audit logging system
- [memory-migrations.md](memory-migrations.md) — Database schema and migration system
- [memory-migration-004.md](memory-migration-004.md) — FTS5 cross-conversation recall migration
- [memory-migration-005.md](memory-migration-005.md) — Scheduled tasks table migration
- [memory-migration-006.md](memory-migration-006.md) — Limitations table (historical — originally for self-introspection, now used by SKILL_IMPROVE)
- [memory-migration-007.md](memory-migration-007.md) — Task type column for action scheduler (reminder vs provider-backed execution)
- [memory-migration-009.md](memory-migration-009.md) — Task retry columns (retry_count, last_error) for action task failure handling
- [memory-migration-010.md](memory-migration-010.md) — Reward-based learning tables (outcomes for working memory, lessons for long-term behavioral rules)

### Milestone 7: omega-skills
- [skills-lib.md](skills-lib.md) — Skill loader + project loader + MCP trigger matching (skills from `~/.omega/skills/*/SKILL.md`, projects from `~/.omega/projects/*/ROLE.md`)
- [skills-cargo-toml.md](skills-cargo-toml.md) — omega-skills crate Cargo.toml

### Milestone 8: omega-quant
- [quant.md](quant.md) — Standalone CLI binary + library (Kalman filter, HMM regime detection, Kelly sizing, IBKR connectivity, execution, skill-based invocation)

### Milestone 9: omega-sandbox
- [sandbox-lib.md](sandbox-lib.md) — 3-level workspace sandbox (sandbox/rx/rwx modes)
- [sandbox-cargo-toml.md](sandbox-cargo-toml.md) — Sandbox crate Cargo.toml

## Architecture Diagram

```
┌─────────────────────────────────────────────────┐
│                   omega (binary)                 │
│  main.rs → gateway/ → commands.rs               │
│    gateway/mod.rs        (orchestrator)          │
│    gateway/pipeline.rs   (message processing)   │
│    gateway/routing.rs    (classify & execute)    │
│    gateway/process_markers.rs (marker handling)  │
│    gateway/auth.rs       (authentication)        │
│    gateway/scheduler.rs  (task scheduling)       │
│    gateway/heartbeat.rs  (periodic check-ins)    │
│    gateway/summarizer.rs (conversation summary)  │
│    gateway/keywords.rs   (constants & matching)  │
│              markers.rs  task_confirmation.rs    │
│              claudemd.rs init.rs  init_wizard.rs │
│              selfcheck.rs  service.rs  i18n.rs   │
├─────────────────────────────────────────────────┤
│  omega-core     │ omega-providers │ omega-channels│
│  config.rs      │ claude_code.rs  │ telegram.rs   │
│  context.rs     │ anthropic.rs    │ whatsapp.rs   │
│  error.rs       │ openai.rs       │               │
│  message.rs     │ ollama.rs       │               │
│  sanitize.rs    │ openrouter.rs   │               │
│  traits.rs      │ gemini.rs       │               │
│                 │ mcp_client.rs   │               │
│                 │ tools.rs        │               │
├─────────────────┼─────────────────┼───────────────┤
│  omega-memory   │ omega-skills    │ omega-sandbox  │
│  store.rs       │ lib.rs (loader) │ (planned)      │
│  audit.rs       │                 │                │
│  migrations/    │                 │                │
└─────────────────┴─────────────────┴───────────────┘
```

## Data Flow

```
Message → Auth → Sanitize → Sandbox constraint → Memory (context + outcomes/lessons) → Provider → process_markers (SCHEDULE/SCHEDULE_ACTION/CANCEL_TASK/UPDATE_TASK/HEARTBEAT/SKILL_IMPROVE/REWARD/LESSON/...) → Memory (store) → Audit → Send → Task confirmation

Background:
  Scheduler: poll due_tasks → channel.send(reminder) → complete_task
             action tasks: provider.complete → parse ACTION_OUTCOME → audit_log → complete/fail_task → notify
  Heartbeat: provider.complete(check-in) → process markers (REWARD/LESSON/SCHEDULE/...) → strip HEARTBEAT_OK → no content? suppress / has content? channel.send(alert)
  Summarizer: find idle convos → summarize → close
  CLAUDE.md: ensure on startup → refresh every 24h (claude -p subprocess)
```
