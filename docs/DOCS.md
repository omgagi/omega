# DOCS.md — Omega Documentation

> Master index of all developer-facing documentation for the Omega project.

## Overview

Omega is a personal AI agent infrastructure written in Rust. This `docs/` directory contains guides, references, and explanations organized by topic.

## Documentation Files

### Architecture
- [architecture.md](architecture.md) — End-to-end message flow (Telegram -> Gateway -> Claude Code -> response), concurrency model, session persistence, multi-agent pipeline architecture (topology-driven sequential chain, TOPOLOGY.toml format, file-mediated handoffs, bounded corrective loops, pre/post validation, self-healing audit), background loops, efficiency summary

### Binary (`backend/src/`)
- [src-main-rs.md](src-main-rs.md) — Entry point, CLI parsing (Start/Status/Ask/Init/Pair/Service), root guard, provider/channel bootstrap
- [src-gateway-rs.md](src-gateway-rs.md) — Gateway module (`backend/src/gateway/`) — 24-file directory module: orchestrator, pipeline, routing, markers, auth, scheduler, heartbeat, summarizer, keywords, builds (orchestrator + loop + parse + agents + topology + i18n), setup + setup_response (Brain orchestrator)
- [src-commands-rs.md](src-commands-rs.md) — Built-in bot commands (status, memory, history, facts, forget, tasks, cancel, skills, purge, help)
- [src-selfcheck-rs.md](src-selfcheck-rs.md) — Startup health checks
- [src-service-rs.md](src-service-rs.md) — OS-aware service management (macOS LaunchAgent / Linux systemd)
- [src-init-rs.md](src-init-rs.md) — Setup wizard (interactive + non-interactive modes), config generation
- [src-init-style-rs.md](src-init-style-rs.md) — Branded CLI output helpers for init wizard (console::Style, gutter-bar visual language)
- [src-init-wizard-rs.md](src-init-wizard-rs.md) — Interactive-only init helpers (browser detection, Anthropic auth, WhatsApp QR, Google OAuth)
- [src-i18n-rs.md](src-i18n-rs.md) — Internationalization module (8 languages, static lookups, format helpers)
- [src-task-confirmation-rs.md](src-task-confirmation-rs.md) — Task scheduling confirmation (anti-hallucination, duplicate detection, localized messages)
- [src-markers-rs.md](src-markers-rs.md) — Marker module — 5 source submodules + tests (40+ functions, ~180 tests)
- [src-api-rs.md](src-api-rs.md) — HTTP API server (axum, health check, webhook, WhatsApp QR pairing)
- [claudemd.md](claudemd.md) — Workspace CLAUDE.md maintenance (auto-creation and periodic refresh for Claude Code subprocess context)

### omega-core
- [core-lib.md](core-lib.md) — Core crate overview, module re-exports
- [core-config.md](core-config.md) — Configuration system (TOML + env, all config structs)
- [core-context.md](core-context.md) — Context struct, McpServer, conversation history, prompt flattening
- [core-error.md](core-error.md) — Error types (OmegaError enum)
- [core-message.md](core-message.md) — Message types (incoming, outgoing, metadata, attachments)
- [core-sanitize.md](core-sanitize.md) — Prompt injection sanitization
- [core-traits.md](core-traits.md) — Provider and Channel trait definitions
- [core-cargo-toml.md](core-cargo-toml.md) — omega-core Cargo manifest

### omega-providers
- [providers.md](providers.md) — AI backend configuration (Claude Code, Ollama, OpenAI, Anthropic, OpenRouter, Gemini)
- [providers-lib.md](providers-lib.md) — Providers crate overview
- [providers-claude-code.md](providers-claude-code.md) — Claude Code CLI provider, MCP settings, JSON response handling
- [providers-openai.md](providers-openai.md) — OpenAI-compatible provider (HTTP, Bearer auth)
- [providers-ollama.md](providers-ollama.md) — Ollama local provider (HTTP, no auth)
- [providers-anthropic.md](providers-anthropic.md) — Anthropic Messages API provider (HTTP, x-api-key header)
- [providers-openrouter.md](providers-openrouter.md) — OpenRouter proxy provider (reuses OpenAI types)
- [agentic-tools.md](agentic-tools.md) — Built-in tool executor, MCP client, agentic loop pattern
- [providers-cargo-toml.md](providers-cargo-toml.md) — omega-providers Cargo manifest

### omega-channels
- [channels-lib.md](channels-lib.md) — Channels crate overview
- [channels-telegram.md](channels-telegram.md) — Telegram Bot API channel (long polling, voice, photo)
- [channels-whatsapp.md](channels-whatsapp.md) — WhatsApp Web protocol channel (text, image, voice, group chat, markdown, retry)
- [channels-cargo-toml.md](channels-cargo-toml.md) — omega-channels Cargo manifest

### omega-memory
- [memory-lib.md](memory-lib.md) — Memory crate overview
- [memory-store.md](memory-store.md) — SQLite persistent store, conversations, facts, context building
- [memory-audit.md](memory-audit.md) — Audit logging system
- [memory-cargo-toml.md](memory-cargo-toml.md) — omega-memory Cargo manifest
- [memory-migration-001.md](memory-migration-001.md) — Initial schema (conversations, messages, facts, summaries)
- [memory-migration-002.md](memory-migration-002.md) — Audit log table
- [memory-migration-003.md](memory-migration-003.md) — Background summarization support
- [memory-migration-004.md](memory-migration-004.md) — FTS5 cross-conversation recall
- [memory-migration-005.md](memory-migration-005.md) — Scheduled tasks table and indexes
- [memory-migration-006.md](memory-migration-006.md) — Limitations table (historical — originally for self-introspection, now used by SKILL_IMPROVE)
- [memory-migration-007.md](memory-migration-007.md) — Task type column for action scheduler
- [memory-migration-008.md](memory-migration-008.md) — User aliases table (custom name/emoji mapping per sender)
- [memory-migration-009.md](memory-migration-009.md) — Task retry columns for action failure handling
- [memory-migration-010.md](memory-migration-010.md) — Reward-based learning tables (outcomes + lessons)
- [memory-migration-011.md](memory-migration-011.md) — Project-scoped learning (project column on outcomes, lessons, scheduled_tasks)
- [memory-migration-012.md](memory-migration-012.md) — Project-scoped sessions (project_sessions table, project column on conversations)
- [memory-migration-013.md](memory-migration-013.md) — Multi-lesson support (remove UNIQUE constraint, content dedup, per-domain cap)

### omega-skills
- [skills-lib.md](skills-lib.md) — Skill loader, trigger matching, MCP server definitions
- [skills-cargo-toml.md](skills-cargo-toml.md) — omega-skills Cargo manifest

### omega-sandbox
- [sandbox-lib.md](sandbox-lib.md) — Blocklist-based system protection (always-on, blocks writes to OS dirs + memory.db)
- [sandbox-cargo-toml.md](sandbox-cargo-toml.md) — omega-sandbox Cargo manifest

### Proactive Features
- [scheduler.md](scheduler.md) — Task queue: reminders, recurring tasks, natural language scheduling
- [heartbeat.md](heartbeat.md) — Periodic AI check-ins, health monitoring, alert suppression
- [introspection.md](introspection.md) — Autonomous skill improvement, reward-based learning, self-audit
- [api.md](api.md) — HTTP API for SaaS dashboard integration (health, WhatsApp QR pairing)
- [webhook.md](webhook.md) — Inbound webhook for external tool integration (direct + AI delivery modes, curl examples, integration guide)

### Self-Configuration
- [omega-brain.md](omega-brain.md) — OMEGA Brain: `/setup` command for non-technical onboarding, multi-round session, automatic project creation (ROLE.md, HEARTBEAT.md, schedules)

### Audits
- [audits/audit-builds-2026-02-27.md](audits/audit-builds-2026-02-27.md) — Builds module code review (guard race, name validation, depth limit, spec drift)
- [audits/audit-full-2026-03-01.md](audits/audit-full-2026-03-01.md) — Full codebase audit (109 findings: 4 P0, 15 P1, 30 P2, 12 P3, 48 drift — all P0-P2 fixed)

### Root / Workspace
- [cargo-toml-root.md](cargo-toml-root.md) — Root Cargo.toml workspace configuration
- [cargo-lock.md](cargo-lock.md) — Cargo.lock dependency snapshot
- [config-example-toml.md](config-example-toml.md) — config.example.toml reference
- [claude-settings-local.md](claude-settings-local.md) — Claude Code local settings
- [claude-md.md](claude-md.md) — CLAUDE.md project instructions
- [readme-md.md](readme-md.md) — README.md documentation
- [license.md](license.md) — License file
- [gitignore.md](gitignore.md) — .gitignore rules

### Audits
- [audits/audit-2026-02-23.md](audits/audit-2026-02-23.md) — Full code + specs/docs drift audit
