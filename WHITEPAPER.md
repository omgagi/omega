# OMEGA Ω

## Personal AI Agent Infrastructure

**Your AI. Your server. Your rules.**

---

Technical Whitepaper v1.0
February 2026

Ivan Lozada
ivan@omegacortex.ai
https://omegacortex.ai
https://github.com/omega-cortex/omega

`Rust · 11,127 lines · 6 crates · SQLite · MIT License`

---

## Contents

1. [Abstract](#1-abstract)
2. [The Problem](#2-the-problem)
3. [Architecture](#3-architecture)
4. [The Gateway Pipeline](#4-the-gateway-pipeline)
5. [Memory System](#5-memory-system)
6. [Provider Strategy](#6-provider-strategy)
7. [Security Model](#7-security-model)
8. [Skills & Extensibility](#8-skills--extensibility)
9. [Distribution & Installation](#9-distribution--installation)
10. [Competitive Analysis](#10-competitive-analysis)
11. [Trading Integration](#11-trading-integration)
12. [Roadmap](#12-roadmap)
13. [Conclusion](#13-conclusion)

---

## 1. Abstract

Omega is a personal AI agent built in Rust that connects messaging platforms (Telegram, WhatsApp) to AI providers (Claude Code CLI, Anthropic API, OpenAI, Ollama) through a secure, memory-persistent gateway. It runs as a single binary on user-owned hardware with no cloud dependency, no subscription, and no data leaving the machine beyond the AI provider API calls.

At 11,127 lines of Rust across 6 crates, Omega delivers persistent conversation memory, fact extraction, scheduled tasks, OS-level sandboxing (Seatbelt on macOS, Landlock on Linux), prompt injection filtering, and a markdown-based skill system — in a binary that uses approximately 20 MB of RAM at runtime.

This paper describes the architecture, security model, and design philosophy behind Omega, and positions it within the emerging landscape of personal AI agents.

---

## 2. The Problem

The current generation of AI agents suffers from three fundamental weaknesses: bloated codebases that expand the attack surface, cloud dependencies that surrender user data to third parties, and shallow security models that treat sandboxing as an afterthought.

OpenClaw, the most popular open-source AI agent as of early 2026 with over 100,000 GitHub stars, exemplifies these tradeoffs. Built in JavaScript across 430,000+ lines of code, it carries CVE-2026-25253 (CVSS 8.8), consumes 200–500 MB of RAM, and exposes over 21,000 instances to the public internet. Users routinely connect it to funded cryptocurrency wallets, email accounts, and messaging platforms with minimal isolation between the AI's execution environment and sensitive system resources.

The AI agent space has prioritized feature velocity over architectural integrity. Omega takes the opposite approach: security-first design, minimal attack surface, and memory safety guaranteed by the Rust compiler.

---

## 3. Architecture

Omega is a Cargo workspace with 6 crates, each responsible for a single domain. The workspace compiles to a single binary with no runtime dependencies beyond the AI provider CLI.

| Crate | Purpose | Lines |
|-------|---------|-------|
| `omega-core` | Types, traits, config, error handling, prompt sanitization | 1,288 |
| `omega-providers` | AI backends: Claude Code CLI, Anthropic, OpenAI, Ollama, OpenRouter | 669 |
| `omega-channels` | Messaging: Telegram, WhatsApp (with session store) | 1,904 |
| `omega-memory` | SQLite storage, conversations, facts, audit log, migrations | 1,516 |
| `omega-skills` | Markdown-based skill/plugin system, MCP integration | 971 |
| `omega-sandbox` | OS-level isolation: Seatbelt (macOS), Landlock (Linux) | 294 |
| **`src/` (binary)** | **Gateway, commands, init wizard, selfcheck, service manager** | **4,478** |

The architecture follows a strict dependency hierarchy: channels and providers depend on core, memory depends on core, skills depends on core and memory, and the binary crate depends on everything. No circular dependencies exist. The Rust compiler enforces this at build time.

---

## 4. The Gateway Pipeline

Every message entering Omega passes through a 7-stage pipeline in `gateway.rs` (2,304 lines). No stage can be bypassed. The pipeline is sequential and deterministic:

| Stage | Function | Failure Mode |
|-------|----------|--------------|
| 1. `AUTH` | Verify sender against allowed_users list | Reject with "Not authorized" |
| 2. `SANITIZE` | Strip prompt injection patterns from input | Clean input, log attempt |
| 3. `WELCOME` | Detect first-time users, send localized greeting | Skip if returning user |
| 4. `COMMANDS` | Handle /commands locally (no AI call) | Pass through if not a command |
| 5. `MEMORY` | Build context from history + facts + summaries | Proceed with empty context |
| 6. `PROVIDER` | Send to AI backend, receive response | Return error message to user |
| 7. `STORE+AUDIT` | Save exchange to SQLite, log to audit trail | Log failure, continue |

Three background loops run concurrently: a **Summarizer** that compresses idle conversations (30+ minutes of inactivity) into summaries for future context, a **Scheduler** that executes timed tasks, and a **Heartbeat** that monitors system health. All loops are async Tokio tasks with graceful shutdown via SIGTERM handling.

---

## 5. Memory System

Omega's memory is backed by SQLite via the `sqlx` crate with compile-time verified queries. The memory store (1,413 lines) manages four data types:

- **Conversations** — Message exchanges grouped by session. Each conversation has a status (active, summarized, closed) and optional summary text.
- **Facts** — Extracted user preferences and knowledge. Facts are attached to user IDs and injected into the system prompt for personalization.
- **Summaries** — Compressed representations of past conversations. When a new conversation starts, recent summaries provide continuity without loading full history.
- **Audit log** — Every interaction is recorded with timestamps, user IDs, provider used, token counts, and response times.

The database schema evolves through numbered migrations (currently 5). Each migration is idempotent and runs automatically on startup. The memory database is local to the machine and never transmitted externally.

Context assembly follows a priority hierarchy: current conversation messages first, then extracted facts, then recent summaries. A configurable `max_context_messages` parameter (default: 50) prevents token overflow while maintaining conversational coherence.

---

## 6. Provider Strategy

Omega implements a trait-based provider abstraction that decouples the gateway from any specific AI backend. The `Provider` trait defines a single async method: send a context and receive a response.

| Provider | Status | Mechanism | Cost |
|----------|--------|-----------|------|
| Claude Code CLI | Production | Subprocess: `claude -p --output-format json` | Claude Max ($200/mo) |
| Anthropic API | Stub | Direct HTTPS to api.anthropic.com | Pay per token |
| OpenAI / Codex | Stub | Direct HTTPS or Codex CLI subprocess | Pay per token |
| Ollama | Stub | Local HTTP to localhost:11434 | Free (local GPU) |
| OpenRouter | Stub | HTTPS proxy to multiple providers | Pay per token |

The Claude Code CLI provider (657 lines) is the primary production backend. It invokes the `claude` binary as a subprocess with structured JSON output, handles max_turns auto-resume (when Claude hits its turn limit, Omega automatically restarts with context), and parses tool use results. The provider respects the user's local Claude authentication, requiring no additional API key management.

---

## 7. Security Model

Security in Omega operates at four layers, each independent of the others.

### 7.1 Memory Safety

Rust's ownership model eliminates buffer overflows, use-after-free, and data races at compile time. The entire codebase compiles with zero `unsafe` blocks. This is not a feature — it is a structural property of the language choice.

### 7.2 Authentication

Every incoming message is checked against an `allowed_users` list configured in `config.toml`. Messages from unauthorized senders are rejected immediately at stage 1 of the pipeline, before any processing occurs. The auth system supports both Telegram user IDs (numeric) and WhatsApp phone numbers.

### 7.3 Prompt Injection Filtering

The sanitize module (146 lines) strips known prompt injection patterns from user input before it reaches the AI provider. This includes attempts to override the system prompt, inject hidden instructions, or manipulate the conversation context. Patterns are matched and neutralized without blocking the underlying user intent.

### 7.4 OS-Level Sandboxing

When the AI provider executes commands (via Claude Code's Bash tool, for example), Omega can restrict what the AI is allowed to access at the operating system level:

| Mode | Read | Write | Use Case |
|------|------|-------|----------|
| `sandbox` | Workspace only | Workspace only | Non-technical users (default) |
| `rx` | Entire host | Workspace only | Analysis tasks, log reading |
| `rwx` | Entire host | Entire host | Developers, sysadmins |

On macOS, sandboxing is enforced via Seatbelt (the same mechanism used by the App Store). On Linux, it uses Landlock, a kernel-level access control system available since Linux 5.13. Both mechanisms are OS-native and cannot be circumvented by the AI provider.

---

## 8. Skills & Extensibility

Omega's skill system (971 lines) allows users to extend the agent's capabilities by dropping `SKILL.md` files into a directory. Skills are markdown documents that define:

- A name and description (for the AI to understand when to use it)
- Instructions and constraints (injected into the system prompt)
- Reference materials (additional context files)
- Scripts (executable tools the AI can invoke)

Four skills ship with Omega: `claude-code` (default reasoning engine), `google-workspace` (Gmail, Drive, Calendar integration), `playwright-mcp` (browser automation via MCP), and `skill-creator` (a meta-skill for creating new skills). Users can create custom skills by writing a markdown file — no code compilation required.

The Model Context Protocol (MCP) is supported through skill definitions that reference MCP servers. When a skill with an MCP configuration is active, Omega connects to the specified MCP server and makes its tools available to the AI provider.

---

## 9. Distribution & Installation

Omega is distributed as a single binary for three platforms. The installation experience is designed around a 3-minute target for non-technical users.

### 9.1 Native Installers

macOS users download a `.dmg`, Windows users download a `.exe`, and Linux users download a `.deb` or `.AppImage`. All platforms also support a one-line shell installer:

```
curl -fsSL https://omegacortex.ai/install.sh | bash
```

### 9.2 The Installer Wizard

The native installer runs a guided setup that asks four questions: which messaging platform (WhatsApp, Telegram, or Discord), which AI provider (Ollama for free local inference, Claude API, or OpenAI), which security mode (sandbox, read+execute, or full access), and then displays a QR code for WhatsApp or prompts for a bot token for Telegram/Discord. The entire process completes in under 3 minutes.

### 9.3 Developer Installation

Developers can build from source via Cargo. The repository includes a `flake.nix` for reproducible builds via Nix. After building, `omega init` runs an interactive TUI wizard (built with `cliclack`) that configures the agent and optionally installs it as a system service (macOS LaunchAgent or Linux systemd unit).

---

## 10. Competitive Analysis

| Metric | Omega | OpenClaw | Typical Agent |
|--------|-------|----------|---------------|
| Language | Rust | JavaScript | Python / JS |
| Lines of code | 11,127 | 430,000+ | varies |
| Runtime memory | ~20 MB | 200–500 MB | varies |
| Known CVEs | 0 | CVE-2026-25253 | varies |
| Distribution | Single binary | npm + dependencies | pip / npm |
| OS-level sandbox | ✓ | ✗ | ✗ |
| Prompt injection filter | ✓ | ✗ | ✗ |
| Persistent memory | ✓ (SQLite) | ✓ (SQLite) | △ |
| Multi-provider | ✓ (5 backends) | ✗ (Claude only) | varies |
| Algo trading | ✓ (IBKR TWAP) | LLM-based (vibes) | ✗ |
| Self-hosted | ✓ | ✓ | ✗ (usually cloud) |

The comparison is not about feature count. OpenClaw offers more integrations (13+ messaging platforms, dozens of community skills). Omega's thesis is that for a personal AI agent — one that manages your messages, files, and potentially your finances — security properties are more important than integration breadth. A smaller, auditable codebase with OS-level isolation is a fundamentally different product than a large, extensible platform with known vulnerabilities.

---

## 11. Trading Integration

Omega includes an experimental algorithmic trading module (`omega-quant`) that interfaces with Interactive Brokers via the TWS API (`ibapi` crate). The trading system supports TWAP (Time-Weighted Average Price) and Immediate execution algorithms — institutional-grade order types designed to minimize market impact.

The design philosophy explicitly rejects the "vibes trading" approach prevalent in the AI agent space, where users give LLMs direct access to funded wallets and ask them to make buy/sell decisions based on sentiment analysis. This approach has been shown to produce poor risk-adjusted returns (Sortino ratios below 0.05 in controlled benchmarks).

Instead, Omega's trading integration follows three principles:

- **Human-in-the-loop** — Every trade requires explicit user confirmation via the messaging interface. The LLM translates intent into structured orders; the user approves.
- **Paper trading by default** — New installations connect to IB Gateway's paper trading port (4002) with simulated funds. Switching to live trading requires an explicit, confirmed action.
- **Hardcoded risk limits** — Maximum position size (2% of equity), daily loss limit (3%), and circuit breaker (5% drawdown) are enforced at the CLI level, not the LLM level. The AI cannot override these limits.

---

## 12. Roadmap

| Phase | Deliverable | Status |
|-------|-------------|--------|
| 1 | Foundation: gateway, auth, memory, Telegram, Claude Code | ✓ Complete |
| 2 | WhatsApp channel, skill system, sandbox, init wizard | ✓ Complete |
| 3 | Native installers (.dmg, .exe, .deb), QR-based setup | In progress |
| 4 | Ollama + OpenAI + Anthropic API providers | Planned |
| 5 | Discord channel, voice messages (Whisper), filesystem watcher | Planned |
| 6 | Trading skill GA, additional exchange integrations | Planned |

---

## 13. Conclusion

Omega represents a different philosophy in the AI agent space: that the agent managing your personal communications, files, and financial transactions should be built with the same engineering rigor applied to critical infrastructure. Memory safety is not optional. Sandboxing is not an afterthought. Auditability is not a feature — it is a requirement.

At 11,127 lines of Rust, Omega is not the most feature-rich agent available. It is, by design, the most auditable. Every line can be read and understood by a single developer in a single sitting. The entire binary fits in 20 MB of RAM. The entire codebase fits in a single code review.

The question is not whether AI agents will become ubiquitous — they will. The question is whether users will trust them with access to the most sensitive parts of their digital lives. Omega is built for the users who demand that trust be earned through engineering, not marketing.

---

Omega is open source under the MIT license.
Source: https://github.com/omega-cortex/omega
Website: https://omegacortex.ai
Contact: ivan@omegacortex.ai
