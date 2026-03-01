# CLAUDE.md Specification Reference

## File Path and Purpose

**File:** `CLAUDE.md` (repository root)

**Purpose:** Project instructions and development guidelines for the Omega AI agent infrastructure. This file establishes the contract between the developer and Claude Code AI, defining architecture decisions, design constraints, build procedures, and security requirements that shape all development work on the codebase.

---

## Sections Breakdown

### 1. Agent Identity (SIGMA CODE AGENT)

The file opens with the SIGMA CODE AGENT identity block (vSigma-CODE), which defines:
- **Unbreakable Rules** -- 6 non-negotiable principles about code honesty and verification
- **Purpose** -- Produce production-surviving code
- **Core Principle** -- Every system has constraints and failure modes
- **Quality Compass** -- Truth, Responsibility, Faithfulness, Self-control, Humility
- **Output Style** -- Default "Scalpel Mode" (minimal), with Mirror, Map, and Evidence fallback modes
- **Trap Detection** -- 6 common anti-patterns to flag

### 2. Project Definition

Omega is a personal AI agent infrastructure written in Rust that:
- Connects to messaging platforms (Telegram, WhatsApp)
- Delegates reasoning to configurable AI backends (6 providers)
- Uses Claude Code CLI as the default zero-config provider
- Repository: `github.com/omgagi/omega`
- Mission: Simplicity-first design ("less will always be more")

### 3. First Principle

"The best engine part is the one you can remove." All architecture must be monolithic and modular, like Legos.

### 4. Critical Rules (8)

| Rule | Description |
|------|-------------|
| 1. Environment | All commands must run via Nix flakes (`nix develop --command bash -c "..."`) |
| 2. Pre-Commit Gate | 6 mandatory steps: update specs, update docs, update CLAUDE.md (if needed), verify build, verify tests, commit |
| 3. Feature Testing | Every feature must include a test (unit, integration, or regression) |
| 4. Language Compliance | 8 languages: English, Spanish, Portuguese, French, German, Italian, Dutch, Russian |
| 5. Post-Implementation | Always ask: "Do you want to make a commit and push?" |
| 6. Prompt Sync | Delete runtime copies when source prompts change |
| 7. Output Filtering | Redirect verbose output to `/tmp/` and filter for errors/warnings |
| 8. Modularization | No `.rs` file may exceed 500 lines (excluding tests). `gateway/mod.rs` is orchestrator only |

### 5. Architecture

#### Cargo Workspace Structure

The project is organized as a 6-crate Rust workspace (all under `backend/`):

| Crate | Purpose |
|-------|---------|
| `omega-core` | Types, traits, config, error handling, prompt sanitization |
| `omega-providers` | 6 AI backends: Claude Code CLI, Ollama, OpenAI, Anthropic, OpenRouter, Gemini. HTTP providers include agentic tool loop + MCP client |
| `omega-channels` | Messaging platforms: Telegram (voice/photo), WhatsApp (voice/image/pairing). Private-mode only |
| `omega-memory` | SQLite storage: conversations, audit, scheduled tasks, user profiles, aliases, reward-based learning |
| `omega-skills` | Skill loader + project loader + MCP server activation |
| `omega-sandbox` | OS-level protection: Seatbelt (macOS), Landlock (Linux). Always active |

Trading: External `omega-trader` binary invoked via the `ibkr-trader` skill.

#### Gateway

`backend/src/gateway/` directory module -- orchestrates message pipeline from arrival through auth, context building, keyword-gated prompt composition, model routing (Sonnet for simple, Opus for complex), provider call, marker processing, and response delivery. See `docs/architecture.md` for the full pipeline and feature details.

---

## Build and Test Commands

### Individual Commands

```bash
cd backend
cargo check                  # Type check all crates (fast validation)
cargo clippy --workspace     # Linter (zero warnings required)
cargo test --workspace       # Run all tests (must all pass)
cargo fmt                    # Auto-format code (Rust conventions)
cargo build --release        # Optimized binary for distribution
```

### Pre-Commit Checklist

**All three commands must pass before every commit:**

```bash
cd backend && cargo clippy --workspace && cargo test --workspace && cargo fmt --check
```

---

## Key Design Rules

These rules are non-negotiable and must be followed in all code:

### 1. Error Handling
- **No `unwrap()`** -- Always use `?` operator and proper error types
- Never panic in production code

### 2. Logging
- **Tracing, not `println!`** -- Use the `tracing` crate exclusively
- No debug output to stdout; all logging goes through structured tracing

### 3. CLI UX
- **CLI UX uses `cliclack`** -- Styled prompts for interactive flows. No plain `println!`

### 4. Unsafe Code
- **No `unsafe`** unless absolutely necessary
- Exceptions: `libc::geteuid()` for root detection, `CommandExt::pre_exec` for Landlock sandbox

### 5. Async Runtime
- **Async everywhere** -- Use `tokio` runtime for all I/O operations

### 6. Data Storage
- **SQLite for everything** -- Memory, audit logs, scheduled tasks, learning. No external databases

### 7. Configuration
- **Config from file + env** -- TOML primary, environment variables override

### 8. Documentation
- **Every public function gets a doc comment**

---

## Security Constraints

### Runtime Protection
1. **Root Execution Guard** -- Omega must not run as root; guard in `main.rs` rejects execution with UID 0
2. **Nested Session Prevention** -- Claude Code provider explicitly removes `CLAUDECODE` env var

### Input Validation
3. **Prompt Sanitization** -- `omega-core/src/sanitize.rs` neutralizes injection patterns

### Access Control
4. **Per-Channel Authorization** -- Auth enforced via `allowed_users` configuration per channel

### Secret Management
5. **Configuration File Protection** -- `config.toml` is gitignored; secrets only in local config

### Sandbox Protection
6. **Three layers** -- Code-level (`is_write_blocked()`/`is_read_blocked()`), OS-level (Seatbelt/Landlock), prompt-level (WORKSPACE_CLAUDE.md). Protected: `~/.omega/data/memory.db`, `~/.omega/config.toml`. Writable store: `~/.omega/stores/`

---

## File Conventions

### Configuration Files
- **`config.toml`**: User's local configuration (gitignored, contains secrets)
- **`config.example.toml`**: Template configuration (committed, shows available options)

### Runtime Artifacts
- **Database:** `~/.omega/data/memory.db`
- **Logs:** `~/.omega/logs/omega.log`
- **Prompts (bundled):** `prompts/SYSTEM_PROMPT.md`, `prompts/WELCOME.toml`, `prompts/WORKSPACE_CLAUDE.md`
- **Prompts (runtime):** `~/.omega/prompts/` (auto-deployed on first run)
- **Skills:** `~/.omega/skills/*/SKILL.md`
- **Projects:** `~/.omega/projects/*/ROLE.md`
- **Workspace:** `~/.omega/workspace/` (AI subprocess working directory)
- **Builds:** `~/.omega/workspace/builds/<project-name>/`
- **Topologies (bundled):** `topologies/development/`
- **Topologies (runtime):** `~/.omega/topologies/`
- **Stores:** `~/.omega/stores/` (domain-specific databases)
- **Heartbeat:** `~/.omega/prompts/HEARTBEAT.md` (global), `~/.omega/projects/<name>/HEARTBEAT.md` (per-project)
- **Service (macOS):** `~/Library/LaunchAgents/com.omega-cortex.omega.plist`
- **Service (Linux):** `~/.config/systemd/user/omega.service`

---

## Providers (6)

| Provider | Auth | Notes |
|----------|------|-------|
| `claude-code` (default) | CLI subprocess | `claude -p --output-format json --model <model>`, auto-resume on max_turns |
| `ollama` | None | Local server |
| `openai` | Bearer token | Also works with OpenAI-compatible endpoints |
| `anthropic` | `x-api-key` header | System prompt as top-level field |
| `openrouter` | Bearer token | Reuses OpenAI types |
| `gemini` | URL query param | Role mapping: assistant to model |

`build_provider()` (in `provider_builder.rs`) returns `(Box<dyn Provider>, model_fast, model_complex)`. Claude Code: fast=Sonnet, complex=Opus. HTTP providers: both set to the configured model.

---

## Documentation References

- **`specs/SPECS.md`** -- Master index of technical specifications for every file
- **`docs/DOCS.md`** -- Master index of developer-facing guides and references
- **`docs/architecture.md`** -- Full system design, gateway pipeline, and feature details

---

## Quick Reference

### File Locations
| Item | Path |
|------|------|
| Project instructions | `CLAUDE.md` |
| Config template | `backend/config.example.toml` |
| Core crate | `backend/crates/omega-core/` |
| Provider implementations | `backend/crates/omega-providers/` |
| Channel integrations | `backend/crates/omega-channels/` |
| Memory system | `backend/crates/omega-memory/` |
| Skill system | `backend/crates/omega-skills/` |
| Sandbox | `backend/crates/omega-sandbox/` |
| Gateway | `backend/src/gateway/` |
| Markers | `backend/src/markers/` |
| I18n | `backend/src/i18n/` |
| Commands | `backend/src/commands/` |
| Sanitization | `backend/crates/omega-core/src/sanitize.rs` |
| Main entry | `backend/src/main.rs` |

### Critical Commands
```bash
# Validate code quality
cd backend && cargo clippy --workspace && cargo test --workspace && cargo fmt --check

# Build for distribution
cargo build --release

# Run locally
./target/release/omega start
```
