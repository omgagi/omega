# CLAUDE.md Specification Reference

## File Path and Purpose

**File:** `/Users/isudoajl/ownCloud/Projects/omega/CLAUDE.md`

**Purpose:** Project instructions and development guidelines for the Omega AI agent infrastructure. This file establishes the contract between the developer and Claude Code AI, defining architecture decisions, design constraints, build procedures, and security requirements that shape all development work on the codebase.

---

## Sections Breakdown

### 1. Project Definition

Omega is a personal AI agent infrastructure written in Rust that:
- Connects to messaging platforms (Telegram, WhatsApp)
- Delegates reasoning to configurable AI backends
- Uses Claude Code CLI as the default zero-config provider
- Repository: `github.com/omega-cortex/omega`

### 2. Architecture

#### Cargo Workspace Structure

The project is organized as a 6-crate Rust workspace:

| Crate | Purpose | Status |
|-------|---------|--------|
| `omega-core` | Types, traits, config, error handling, prompt sanitization | Complete |
| `omega-providers` | AI backends (Claude Code CLI, Anthropic, OpenAI, Ollama, OpenRouter) | Complete |
| `omega-channels` | Messaging platforms (Telegram, WhatsApp) | Telegram complete, WhatsApp planned |
| `omega-memory` | SQLite storage, conversation history, audit log | Complete |
| `omega-skills` | Skill/plugin system | Planned |
| `omega-sandbox` | Secure command execution | Planned |

#### Gateway Event Loop

The core processing pipeline in `src/gateway.rs` follows this flow:

```
Message → Auth → Sanitize → Memory (context) → Provider → Memory (store) → Audit → Send
```

Each stage is responsible for:
1. **Message**: Incoming message from a channel
2. **Auth**: Validate user permissions per channel
3. **Sanitize**: Neutralize injection patterns
4. **Memory (context)**: Retrieve conversation history and context
5. **Provider**: Delegate to AI backend (Claude Code CLI by default)
6. **Memory (store)**: Persist response and metadata
7. **Audit**: Log all operations for compliance
8. **Send**: Return response to user via channel

---

## Build and Test Commands

### Individual Commands

```bash
cargo check                  # Type check all crates (fast validation)
cargo clippy --workspace     # Linter (zero warnings required)
cargo test --workspace       # Run all tests (must all pass)
cargo fmt                    # Auto-format code (Rust conventions)
cargo build --release        # Optimized binary for distribution
```

### Pre-Commit Checklist

**All three commands must pass before every commit:**

```bash
cargo clippy --workspace && cargo test --workspace && cargo fmt --check
```

This ensures:
- No compiler warnings
- All tests pass
- Code is properly formatted (check mode)
- Ready for merge

### Command Explanations

- **`cargo check`**: Fast validation without code generation; useful during development
- **`cargo clippy --workspace`**: Catches common mistakes and idiomatic Rust violations; zero warnings policy
- **`cargo test --workspace`**: Executes all unit and integration tests across all crates
- **`cargo fmt`**: Enforces consistent formatting; `--check` mode validates without modifying
- **`cargo build --release`**: Produces optimized binary with minimal size and maximum performance

---

## Key Design Rules

These rules are non-negotiable and must be followed in all code:

### 1. Error Handling
- **No `unwrap()`** — Always use `?` operator and proper error types
- Never panic in production code
- Rationale: Provides graceful degradation and proper error propagation

### 2. Logging
- **Tracing, not `println!`** — Use the `tracing` crate exclusively
- No debug output to stdout; all logging goes through structured tracing
- Rationale: Enables log filtering, async-safe logging, and production observability

### 3. Unsafe Code
- **No `unsafe`** unless absolutely necessary
- Only exception: `libc::geteuid()` for root detection in security guard
- Rationale: Maintains memory safety and prevents subtle bugs

### 4. Async Runtime
- **Async everywhere** — Use `tokio` runtime for all I/O operations
- No blocking I/O in async contexts
- Rationale: Enables concurrent message handling and responsive system

### 5. Data Storage
- **SQLite for everything** — Use SQLite for memory, audit logs, and state
- No external databases (PostgreSQL, MySQL, etc.)
- Rationale: Single embedded dependency, self-contained, portable

### 6. Configuration
- **Config from file + env** — TOML is primary, environment variables override
- Follow hierarchy: env vars > config file > defaults
- Rationale: Flexible deployment (dev, staging, prod) without code changes

### 7. Documentation
- **Every public function gets a doc comment**
- Include examples for complex APIs
- Document panic conditions (though avoid panics)
- Rationale: Self-documenting code aids maintenance and AI assistance

---

## Security Constraints

### Runtime Protection

1. **Root Execution Guard**
   - Omega **must not run as root**
   - Guard in `main.rs` rejects execution with UID 0
   - Rationale: Prevents privilege escalation attacks; limits blast radius

2. **Nested Session Prevention**
   - Claude Code provider explicitly removes `CLAUDECODE` env var
   - Prevents infinite session recursion if Omega calls itself
   - Rationale: Protects against resource exhaustion DoS

### Input Validation

3. **Prompt Sanitization**
   - Implemented in `omega-core/src/sanitize.rs`
   - Neutralizes injection patterns before reaching provider
   - Protects against prompt injection attacks
   - Rationale: Defense-in-depth; untrusted user input is sanitized

### Access Control

4. **Per-Channel Authorization**
   - Auth enforced via `allowed_users` configuration per channel
   - Prevents unauthorized users from issuing commands
   - Rationale: Multi-tenant isolation; principle of least privilege

### Secret Management

5. **Configuration File Protection**
   - `config.toml` is gitignored — never commit secrets
   - API keys, tokens stored only in local config
   - Environment variables can override for deployment
   - Rationale: Prevents accidental credential leakage in version control

---

## File Conventions

### Configuration Files

- **`config.toml`**: User's local configuration (gitignored, contains secrets)
- **`config.example.toml`**: Template configuration (committed, shows available options)
- Deployment: Copy `config.example.toml` to `config.toml` and customize

### Runtime Artifacts

- **Database**: `~/.omega/data/memory.db` (SQLite, conversation history)
- **Logs**: `~/.omega/logs/omega.log` (structured logs from tracing)
- **Service**: `~/Library/LaunchAgents/com.omega-cortex.omega.plist` (macOS LaunchAgent)

### Directory Structure

- `~/.omega/` created automatically on first run
- Permissions: User-read/write only (0600 for security-sensitive files)
- Backup: Recommended to back up `~/.omega/data/memory.db` periodically

---

## Provider Priority and Implementation

### Claude Code CLI (Primary Provider)

Claude Code is the default and primary provider. Implementation details:

#### Invocation
```bash
claude -p --output-format json
```

#### Response Format
```json
{
  "type": "result",
  "subtype": "success",
  "result": "...",
  "model": "claude-opus-4.6",
  "session_id": "..."
}
```

#### Response Subtypes
- `"success"`: Normal response; use `result` field
- `"error_max_turns"`: Hit conversation turn limit; extract `result` if available, otherwise return meaningful fallback
- Other errors: Log and return error message

#### Integration Points
- `omega-providers/src/lib.rs`: Provider trait and implementations
- All responses are JSON-decoded and processed uniformly
- Model information captured in audit logs

### Secondary Providers (Planned)

For Phase 4, additional providers will be supported:
- Anthropic API (direct API calls)
- OpenAI (GPT models via API)
- Ollama (local LLM inference)
- OpenRouter (multi-model proxy)

Each provider implements the same `Provider` trait for uniform handling.

---

## Design Philosophy Summary

**Omega embodies these core principles:**

1. **Zero-Config Default**: Claude Code CLI works out-of-the-box; minimal setup
2. **Security First**: Root guard, sanitization, auth enforcement, no secrets in git
3. **Production Ready**: Proper error handling, structured logging, graceful degradation
4. **Rust Idioms**: Type safety, ownership-driven design, zero-cost abstractions
5. **Extensibility**: Crate-based modularity, trait-based provider pattern, planned plugin system
6. **Observability**: Structured logging, audit trail, conversation history

---

## Quick Reference

### File Locations
| Item | Path |
|------|------|
| Project instructions | `./CLAUDE.md` |
| Config template | `./config.example.toml` |
| Core crate | `./crates/omega-core/` |
| Provider implementations | `./crates/omega-providers/` |
| Channel integrations | `./crates/omega-channels/` |
| Memory system | `./crates/omega-memory/` |
| Gateway | `./src/gateway.rs` |
| Sanitization | `./crates/omega-core/src/sanitize.rs` |
| Main entry | `./src/main.rs` |

### Critical Commands
```bash
# Validate code quality
cargo clippy --workspace && cargo test --workspace && cargo fmt --check

# Build for distribution
cargo build --release

# Run locally
./target/release/omega ask "What is Rust?"
```

### Security Checklist
- [ ] No `unwrap()` without explicit error handling
- [ ] No `println!()` — use `tracing`
- [ ] No secrets in config.toml (use config.example.toml)
- [ ] User auth configured in config.toml
- [ ] All public functions documented
- [ ] Tests pass: `cargo test --workspace`
- [ ] No warnings: `cargo clippy --workspace`
