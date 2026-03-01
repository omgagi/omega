# SPECS.md — Omega Technical Specifications

> Master index of all technical specification documents for the Omega codebase.

## Overview

Omega is a personal AI agent infrastructure written in Rust. This `specs/` directory contains detailed technical specifications for every file in the repository, organized by functional domain (milestone).

## Specification Files

### Milestone 1: Root / Workspace
- [cargo-toml-root.md](cargo-toml-root.md) — Root Cargo.toml workspace configuration
- [cargo-lock.md](cargo-lock.md) — Cargo.lock dependency snapshot
- [config-example-toml.md](config-example-toml.md) — config.example.toml reference
- [claude-settings-local.md](claude-settings-local.md) — Claude Code local settings
- [claude-md.md](claude-md.md) — CLAUDE.md project instructions
- [readme-md.md](readme-md.md) — README.md documentation
- [license.md](license.md) — License file
- [gitignore.md](gitignore.md) — .gitignore rules

### Milestone 2: Binary (`backend/src/`)
- [src-main-rs.md](src-main-rs.md) — Entry point, CLI parsing (Start/Status/Ask/Init/Pair/Service), root guard, provider/channel bootstrap
- [src-gateway-rs.md](src-gateway-rs.md) — Gateway module (`backend/src/gateway/`) — 24-file directory module: orchestrator, pipeline, routing, process_markers, auth, scheduler, scheduler_action, heartbeat, heartbeat_helpers, summarizer, keywords, keywords_data, builds, builds_loop, builds_parse, builds_agents, builds_topology, builds_i18n, prompt_builder, shared_markers, pipeline_builds, setup, setup_response, tests
- [provider-builder.md](provider-builder.md) — Provider factory (`build_provider()`) — constructs providers from config, dual-model routing for Claude Code
- [src-markers-rs.md](src-markers-rs.md) — Marker module (`backend/src/markers/`) — 5 source submodules + test directory (40+ functions, ~145 tests)
- [src-task-confirmation-rs.md](src-task-confirmation-rs.md) — Task scheduling confirmation (anti-hallucination, duplicate detection, localized confirmation messages)
- [src-commands-rs.md](src-commands-rs.md) — Built-in bot commands (status, memory, history, facts, forget, tasks, cancel, skills, purge, help)
- [src-init-rs.md](src-init-rs.md) — Setup wizard (interactive + non-interactive modes), config generation
- [src-init-style-rs.md](src-init-style-rs.md) — Branded CLI output helpers for init wizard (console::Style, gutter-bar visual language)
- [src-init-wizard-rs.md](src-init-wizard-rs.md) — Interactive-only init helpers (browser detection, Anthropic auth, WhatsApp QR, Google OAuth)
- [src-selfcheck-rs.md](src-selfcheck-rs.md) — Startup health checks
- [src-service-rs.md](src-service-rs.md) — OS-aware service management (macOS LaunchAgent / Linux systemd)
- [src-claudemd-rs.md](src-claudemd-rs.md) — Workspace CLAUDE.md maintenance (init + periodic refresh via claude CLI subprocess)
- [src-api-rs.md](src-api-rs.md) — HTTP API server (axum, health check, WhatsApp QR pairing for SaaS dashboards)
- [src-i18n-rs.md](src-i18n-rs.md) — Internationalization module (8 languages, static lookups, format helpers)

### Milestone 3: omega-core
- [core-lib.md](core-lib.md) — Core crate overview, module re-exports
- [core-config.md](core-config.md) — Configuration system (TOML + env, all config structs)
- [core-context.md](core-context.md) — Conversation context model for AI providers
- [core-error.md](core-error.md) — Error types (OmegaError enum)
- [core-message.md](core-message.md) — Message types (incoming, outgoing, metadata, attachments)
- [core-sanitize.md](core-sanitize.md) — Prompt injection sanitization
- [core-traits.md](core-traits.md) — Provider and Channel trait definitions
- [core-cargo-toml.md](core-cargo-toml.md) — omega-core Cargo manifest

### Milestone 4: omega-providers
- [providers-lib.md](providers-lib.md) — Providers crate overview (all 6 modules public)
- [providers-claude-code.md](providers-claude-code.md) — Claude Code CLI provider (primary, subprocess-based)
- [providers-ollama.md](providers-ollama.md) — Ollama local provider (HTTP, no auth)
- [providers-openai.md](providers-openai.md) — OpenAI-compatible provider (HTTP, Bearer auth, exports shared types)
- [providers-openrouter.md](providers-openrouter.md) — OpenRouter proxy provider (reuses OpenAI types)
- [providers-anthropic.md](providers-anthropic.md) — Anthropic Messages API provider (HTTP, x-api-key header)
- [providers-gemini.md](providers-gemini.md) — Google Gemini API provider (HTTP, x-goog-api-key header auth)
- [providers-mcp-client.md](providers-mcp-client.md) — MCP client over stdio (JSON-RPC 2.0, tool discovery, tool calling)
- [providers-tools.md](providers-tools.md) — Shared tool executor (bash/read/write/edit + MCP routing + sandbox enforcement)
- [providers-cargo-toml.md](providers-cargo-toml.md) — omega-providers Cargo manifest

### Milestone 5: omega-channels
- [channels-lib.md](channels-lib.md) — Channels crate overview
- [channels-telegram.md](channels-telegram.md) — Telegram Bot API channel (long polling)
- [channels-whatsapp.md](channels-whatsapp.md) — WhatsApp Web protocol channel (text, image, voice, group chat, markdown, retry)
- [channels-cargo-toml.md](channels-cargo-toml.md) — omega-channels Cargo manifest

### Milestone 6: omega-memory
- [memory-lib.md](memory-lib.md) — Memory crate overview
- [memory-store.md](memory-store.md) — SQLite persistent store, conversations, facts, context building
- [memory-audit.md](memory-audit.md) — Audit logging system
- [memory-cargo-toml.md](memory-cargo-toml.md) — omega-memory Cargo manifest
- [memory-migration-001.md](memory-migration-001.md) — Initial schema (conversations, messages, facts, summaries)
- [memory-migration-002.md](memory-migration-002.md) — Audit log table
- [memory-migration-003.md](memory-migration-003.md) — Background summarization support
- [memory-migration-004.md](memory-migration-004.md) — FTS5 cross-conversation recall migration
- [memory-migration-005.md](memory-migration-005.md) — Scheduled tasks table migration
- [memory-migration-006.md](memory-migration-006.md) — Limitations table (historical — originally for self-introspection, now used by SKILL_IMPROVE)
- [memory-migration-007.md](memory-migration-007.md) — Task type column for action scheduler (reminder vs provider-backed execution)
- [memory-migration-008.md](memory-migration-008.md) — User aliases table (custom name/emoji mapping per sender)
- [memory-migration-009.md](memory-migration-009.md) — Task retry columns (retry_count, last_error) for action task failure handling
- [memory-migration-010.md](memory-migration-010.md) — Reward-based learning tables (outcomes for working memory, lessons for long-term behavioral rules)
- [memory-migration-011.md](memory-migration-011.md) — Project-scoped learning (project columns on outcomes, lessons, scheduled_tasks)
- [memory-migration-012.md](memory-migration-012.md) — Project-scoped sessions (project_sessions table, project column on conversations)
- [memory-migration-013.md](memory-migration-013.md) — Multi-lesson support (remove UNIQUE constraint, content dedup, cap enforcement)

### Milestone 7: omega-skills
- [skills-lib.md](skills-lib.md) — Skill loader + project loader + MCP trigger matching (skills from `~/.omega/skills/*/SKILL.md`, projects from `~/.omega/projects/*/ROLE.md`). Split into `skills.rs`, `projects.rs`, `parse.rs` submodules; `lib.rs` is a thin re-export orchestrator
- [skills-cargo-toml.md](skills-cargo-toml.md) — omega-skills crate Cargo.toml

### Milestone 8: omega-sandbox
- [sandbox-lib.md](sandbox-lib.md) — Blocklist-based system protection (always-on, blocks writes to OS dirs + memory.db)
- [sandbox-cargo-toml.md](sandbox-cargo-toml.md) — Sandbox crate Cargo.toml

### Features
- [webhook-requirements.md](webhook-requirements.md) — Inbound webhook endpoint for external tool integration (direct + AI delivery modes)
- [webhook-architecture.md](webhook-architecture.md) — Architecture design for inbound webhook (handler, gateway plumbing, source tracking, failure modes, security model)
- [omega-brain-requirements.md](omega-brain-requirements.md) — OMEGA Brain self-configuration agent: `/setup` command, multi-round session, ROLE.md + HEARTBEAT.md + schedule creation via existing primitives
- [omega-brain-architecture.md](omega-brain-architecture.md) — Architecture design for OMEGA Brain: setup.rs orchestrator, pipeline integration, session state machine, agent definition, write_single lifecycle, localized messages, failure modes, security model
- [omega-init-visual-requirements.md](omega-init-visual-requirements.md) — OMEGA Init Wizard Visual Identity: branded chrome for init.rs + init_wizard.rs, init_style.rs helper module, dark/technical aesthetic using console::Style
- [omega-init-visual-architecture.md](omega-init-visual-architecture.md) — Architecture design for Init Visual Identity: init_style.rs module layout, cyan color palette, gutter-bar visual language, 10 helper functions, integration pattern, visual coexistence with cliclack widgets

### Improvements
- [improvements/builds-routing-improvement.md](improvements/builds-routing-improvement.md) — [SUPERSEDED] Multi-phase builds pipeline replacing single-shot build execution
- [improvements/build-agent-pipeline-improvement.md](improvements/build-agent-pipeline-improvement.md) — Replace 5-phase hardcoded prompts with 7-phase agent pipeline using `--agent` flag and embedded agent definitions
- [improvements/build-pipeline-safety-controls.md](improvements/build-pipeline-safety-controls.md) — QA retry loop (3 iter), review loop (2 iter, fatal), inter-step validation, chain state recovery, agent prompt improvements
- [improvements/build-discovery-phase-improvement.md](improvements/build-discovery-phase-improvement.md) — Interactive discovery session before build pipeline (multi-round clarification, cancel support)
- [improvements/topology-extraction-requirements.md](improvements/topology-extraction-requirements.md) — Phase 1: Extract hardcoded 7-phase build pipeline into config-driven TOPOLOGY.toml + external agent .md files
- [improvements/topology-extraction-architecture.md](improvements/topology-extraction-architecture.md) — Architecture design for topology extraction: builds_topology.rs schema/loader, orchestrator refactoring, agent lifecycle, validation, failure modes, security model
- [improvements/project-personality-improvement.md](improvements/project-personality-improvement.md) — Project-specific OMEGA persona greeting on activation (i18n template, 8 languages, auto-derived persona name from directory)

### Bugfixes
- [bugfixes/p0-audit-2026-02-23-analysis.md](bugfixes/p0-audit-2026-02-23-analysis.md) — P0 audit findings (UTF-8 panics, HTTP timeouts, etc.)
- [bugfixes/heartbeat-clock-drift-analysis.md](bugfixes/heartbeat-clock-drift-analysis.md) — Heartbeat clock-alignment drift after quiet hours / system sleep
- [bugfixes/builds-audit-2026-02-27-analysis.md](bugfixes/builds-audit-2026-02-27-analysis.md) — Builds module audit fixes (guard race, name validation, depth limit, path leaks)
- [bugfixes/heartbeat-project-scope-analysis.md](bugfixes/heartbeat-project-scope-analysis.md) — Heartbeat returns global content when project is active (pipeline + settings paths)
- [bugfixes/heartbeat-trading-suppression-analysis.md](bugfixes/heartbeat-trading-suppression-analysis.md) — Heartbeat trading section suppression
- [bugfixes/heartbeat-verbose-suppression-analysis.md](bugfixes/heartbeat-verbose-suppression-analysis.md) — Heartbeat verbose/learned-rule suppression
- [bugfixes/heartbeat-duplicate-project-analysis.md](bugfixes/heartbeat-duplicate-project-analysis.md) — Heartbeat loop sends duplicate reports when project section exists in both global and project files

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
│    gateway/scheduler_action.rs (action exec)     │
│    gateway/heartbeat.rs  (periodic check-ins)    │
│    gateway/heartbeat_helpers.rs (HB helpers)     │
│    gateway/summarizer.rs (conversation summary)  │
│    gateway/keywords.rs   (constants & matching)  │
│              markers.rs  task_confirmation.rs    │
│       claudemd.rs init.rs init_style.rs init_wizard.rs │
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
│  store.rs       │ lib.rs (re-exp) │ lib.rs         │
│  audit.rs       │ skills.rs       │                │
│  migrations/    │ projects.rs     │                │
│                 │ parse.rs        │                │
└─────────────────┴─────────────────┴───────────────┘
```

## Data Flow

```
Message → Auth → Sanitize → Memory (context + project-scoped outcomes/lessons) → Provider (protected_command) → process_markers (SCHEDULE/SCHEDULE_ACTION/CANCEL_TASK/UPDATE_TASK/HEARTBEAT/SKILL_IMPROVE/REWARD/LESSON/..., active_project threading) → Memory (store) → Audit → Send → Task confirmation

Webhook (direct): POST /api/webhook → check_auth → resolve channel/target → OutgoingMessage → channel.send() → audit → 200
Webhook (ai):     POST /api/webhook → check_auth → resolve channel/target → IncomingMessage → tx.send() → 202 → (enters Message flow above)

Background:
  Scheduler: poll due_tasks → channel.send(reminder) → complete_task
             action tasks: provider.complete → parse ACTION_OUTCOME → audit_log → complete/fail_task → notify
  Heartbeat: provider.complete(check-in) → process markers (REWARD/LESSON/SCHEDULE/...) → strip HEARTBEAT_OK → no content? suppress / has content? channel.send(alert)
  Summarizer: find idle convos → summarize → close
  CLAUDE.md: ensure on startup → refresh every 24h (claude -p subprocess)
```
