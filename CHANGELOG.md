# Changelog

All notable changes to Omega are documented in this file.

Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.6] - 2026-03-01

### Added
- Autonomous build-intent detection via `BUILD_PROPOSAL` marker — OMEGA now recognizes when users want software built from natural language in any of the 8 supported languages, without relying solely on keyword matching

[0.2.6]: https://github.com/omgagi/omega/compare/v0.2.5...v0.2.6

## [0.2.0] - 2026-03-01

### Added

#### Providers
- 5 new API providers: Ollama, OpenAI, Anthropic, OpenRouter, Gemini
- Agentic tool-execution loop + MCP client for all HTTP providers
- Claude Code CLI `--agent` flag support for build pipeline phases
- Dual-model routing: Sonnet for simple queries, Opus for complex tasks

#### Build Pipeline
- Multi-phase build orchestrator replacing single-shot builds
- 7-phase agent pipeline with embedded agent definitions
- Config-driven topology extraction (TOPOLOGY.toml + external agent files)
- Interactive discovery phase with multi-round clarification
- QA retry loop (3 iterations), review loop (2 iterations, fatal)
- Inter-step validation and chain state recovery
- Build confirmation gate with TTL and cancellation support
- Typo-tolerant build keywords for all 8 languages

#### WhatsApp
- Voice message transcription via Whisper
- Image upload support (send_photo)
- Group chat message handling
- Markdown sanitization for WhatsApp formatting
- Send retry with exponential backoff
- Cross-channel user identity

#### Projects & Self-Configuration
- OMEGA Brain `/setup` command for non-technical domain onboarding
- Project-scoped learning isolation (outcomes + lessons per project)
- Project-scoped sessions with SQLite persistence
- Project persona greeting on activation (8 languages)
- Always-on project awareness with ungated ROLE.md injection
- Heartbeat fully scoped to active project

#### Learning & Memory
- Two-tier reward-based learning system (outcomes + lessons)
- Multi-lesson support per domain with content dedup and cap
- Progressive onboarding system with stage-based hints
- FTS5 cross-conversation recall

#### Scheduler
- Action tasks: provider-backed execution with audit + outcome verification
- Task retry with failure tracking (retry_count, last_error)
- Anti-hallucination task confirmation + duplicate detection
- UPDATE_TASK marker for modifying scheduled tasks
- Quiet hours enforcement for action tasks
- User aliases (custom name/emoji per sender)

#### Gateway & UX
- Keyword-gated prompt injection for token-efficient context
- Session-based prompt persistence for Claude Code CLI
- Dynamic heartbeat interval via HEARTBEAT_INTERVAL marker
- Conversational command markers for zero-friction UX
- Heartbeat classify-then-route with parallel grouped execution
- `/learning` command to expose self-learning data
- `/forget` now summarizes and extracts facts before closing

#### Infrastructure
- Inbound webhook endpoint for external tool integration (direct + AI modes)
- HTTP API server (health check, WhatsApp QR pairing)
- Auto-maintained workspace CLAUDE.md for Claude Code subprocess
- Non-interactive `omega init` for programmatic deployment
- `omega pair` command for standalone WhatsApp QR pairing
- Branded visual identity for init wizard (console::Style)
- OS-aware service management (macOS LaunchAgent / Linux systemd)
- Intel Mac (x86_64-apple-darwin) added to release build matrix
- SHA256 checksums for release artifacts

#### Internationalization
- Full 8-language coverage: English, Spanish, Portuguese, French, German, Italian, Dutch, Russian
- Localized bot commands, onboarding hints, discovery messages, build keywords
- OMEGA branded as *OMEGA &Omega;* across all languages

### Changed

- Repository moved from `omega-cortex/omega` to `omgagi/omega`
- Backend restructured into `backend/` directory with Cargo workspace
- Gateway split from monolithic `gateway.rs` into 24-file directory module
- 10 monolithic files modularized into directory modules (500-line limit enforced)
- Provider code deduplicated across OpenAI/OpenRouter/Anthropic
- Sandbox changed from allowlist to always-on blocklist protection
- Conversation idle timeout increased from 30 min to 2 hours
- Classification disabled for conversations (always DIRECT routing)
- Prescriptive behavioral rules replaced with reward-based learning
- omega-quant extracted to standalone [omega-trader](https://github.com/omgagi/omega-trader) repo

### Fixed

#### Security (Audit Resolutions)
- **P0**: Auth bypass, sandbox path traversal, config.toml protection, UTF-8 panics, HTTP timeouts, FTS5 injection
- **P1**: 25 findings across security, performance, and testing
- **P2**: 66 findings across security, performance, compliance, and testing
- Read protection added to block subprocess access to memory.db and config.toml
- System-managed facts protected from AI overwrite
- Prompt injection neutralization hardened

#### Heartbeat
- Clock-alignment drift after quiet hours / system sleep
- Duplicate reports when project section exists in both global and project files
- Verbose "nothing to report" suppressed instead of sent to user
- Learned behavioral rules enforced over checklist verbosity
- Interval changes persisted to config.toml and interrupt sleep
- Trading section suppression via learned rules

#### Scheduler & Tasks
- Sender_id propagation + task dedup strengthened
- REWARD/LESSON markers processed from action task responses
- Context pollution from trading project in casual conversations prevented
- PROJECT_ACTIVATE/DEACTIVATE and FORGET markers added to action tasks

#### Channels
- WhatsApp empty `allowed_users` restored to allow-all behavior
- Group messages dropped at channel level
- Telegram bot commands registered at startup
- Underscores escaped in /facts and /purge for Telegram Markdown

#### Other
- Config reads from `~/.omega/` instead of source directory
- Claude Code `--resume` used instead of deprecated `--session-id`
- Classification prompt made complexity-aware instead of count-based
- Max_turns capped to 25; ROLE.md no longer re-injected in continuations

## [0.1.0] - 2025-02-18

Initial release.

[0.2.0]: https://github.com/omgagi/omega/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/omgagi/omega/releases/tag/v0.1.0
