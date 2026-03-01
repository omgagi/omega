# omega-sandbox -- Developer Guide

## What is this crate?

`omega-sandbox` is the OS-level system protection layer of the Omega project. It prevents AI provider subprocesses from writing to dangerous system directories and from reading or writing OMEGA's core data (memory.db, config.toml). Protection is always active -- no configuration needed.

## Crate structure

```
backend/crates/omega-sandbox/
  Cargo.toml
  src/
    lib.rs               # Public API: protected_command() + is_write_blocked() + is_read_blocked()
    seatbelt.rs          # macOS: Seatbelt blocklist (deny writes to system dirs + config, deny reads to data/config)
    landlock_sandbox.rs  # Linux: Landlock allowlist + Refer-only restrictions on data/config, pre-creates dirs
```

---

## How It Works

The sandbox uses a **blocklist** approach: everything is allowed by default, then specific dangerous paths are denied. No modes, no configuration, no opt-in.

Protection works in three layers:

### Layer 1: Code-Level Protection (all platforms, primary)

Two functions provide enforcement for HTTP-based providers (OpenAI, Anthropic, Ollama, OpenRouter, Gemini):
- `is_write_blocked(path, data_dir)` -- called before write/edit operations. Blocks `{data_dir}/data/`, `{data_dir}/config.toml`, and dangerous OS directories. Uses component-aware `Path::starts_with()` matching to avoid false positives (e.g. `/binaries/test` does not match `/bin`).
- `is_read_blocked(path, data_dir, config_path)` -- called before read operations. Blocks `{data_dir}/data/`, `{data_dir}/config.toml`, and an optional external config path.

Both functions resolve symlinks before comparison (via `try_canonicalize()`) and **fail closed for relative paths** -- any relative path returns `true` (blocked) to prevent traversal bypass.

This works on all platforms, including those without OS-level sandbox support.

### Layer 2: OS-Level Protection (CLI provider)

Platform-native mechanisms block the AI subprocess at the OS level:

| Platform | Mechanism | Approach |
|----------|-----------|----------|
| **macOS** | Apple Seatbelt | `sandbox-exec` with deny profile for writes to system dirs + reads/writes to data/config |
| **Linux** | Landlock LSM | Read-only on `/`, full access to `$HOME` + `/tmp`, Refer-only on data/config |
| **Other** | None | Logs warning, uses code-level enforcement only |

### Layer 3: Prompt-Level Protection (all platforms)

`WORKSPACE_CLAUDE.md` informs the subprocess that access to `memory.db` and `config.toml` is sandbox-enforced, discouraging attempts before they hit the enforcement layers.

---

## What's Blocked

### Blocked from writes

| Path | What it protects |
|------|-----------------|
| `~/.omega/data/` | OMEGA's core database (memory.db, audit trail, facts) |
| `~/.omega/config.toml` | API keys and auth settings |
| `/System` | macOS system |
| `/bin`, `/sbin` | System binaries |
| `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/libexec` | System binaries and libraries |
| `/private/etc` | macOS system configuration |
| `/Library` | macOS system libraries |
| `/etc`, `/boot`, `/proc`, `/sys`, `/dev` | Linux system paths |

### Blocked from reads

| Path | What it protects |
|------|-----------------|
| `~/.omega/data/` | OMEGA's core database — prevents subprocess from querying memory.db |
| `~/.omega/config.toml` | API keys and secrets — prevents credential exfiltration |

### Why protect memory.db and config.toml?

OMEGA's database at `~/.omega/data/memory.db` contains the audit trail, conversation history, scheduled tasks, user facts, and learned lessons. If the AI could read it, it would confabulate architectural details from raw data instead of relying on curated gateway-injected context. If it could write to it, it could tamper with its own memory. Only the Omega binary itself (via `omega-memory`) should access this file.

`config.toml` contains API keys and secrets. The subprocess has no legitimate need to read it — all relevant configuration is injected by the gateway.

### The `stores/` directory

Domain-specific databases live in `~/.omega/stores/` (e.g., `~/.omega/stores/quant.db`). This directory is **not** blocked -- skills and tools can freely read and write their own storage. Only the core `data/` directory is protected.

---

## What's Allowed

Everything not in the blocklist is writable:

| Path | Purpose |
|------|---------|
| `~/.omega/workspace/` | AI working directory |
| `~/.omega/skills/` | Skill definitions and lessons |
| `~/.omega/projects/` | Project contexts |
| `~/.omega/stores/` | Domain-specific databases |
| `~/.omega/prompts/` | Prompt templates |
| `$HOME` | User's home directory |
| `/tmp` | Temporary files |
| `/usr/local` | Homebrew, user-installed software |

Full network access is available. Package installs (`npm install`, `pip install`, etc.) work without restriction.

---

## Platform Details

### macOS -- Seatbelt

The crate generates a Seatbelt profile and invokes `sandbox-exec -p <profile> -- claude ...`. The profile:

1. Allows all operations by default (`(allow default)`)
2. Denies writes to system dirs + data dir + config.toml (`(deny file-write* ...)`)
3. Denies reads to data dir + config.toml (`(deny file-read* ...)`)

If `/usr/bin/sandbox-exec` does not exist (unlikely on macOS), falls back to code-level enforcement with a warning.

### Linux -- Landlock

The crate uses a broad allowlist plus restrictive overrides:

- Read + execute on `/` (system dirs become read-only)
- Full access to `$HOME`, `/tmp`
- Optional full access to `/var/tmp`, `/opt`, `/srv`, `/run`, `/media`, `/mnt`
- Refer-only access to `~/.omega/data/` -- via Landlock intersection semantics (`full_access intersection Refer = Refer`), this blocks both reads and writes
- Refer-only access to `~/.omega/config.toml` (only if the file exists)

The `~/.omega/data/` directory is pre-created via `create_dir_all()` before the Landlock rule is applied, ensuring protection is active even on first run. Config.toml cannot be safely pre-created (an empty file breaks the TOML parser), so code-level enforcement covers the gap.

Code-level enforcement via `is_read_blocked()` and `is_write_blocked()` provides additional protection.

If the kernel does not support Landlock (pre-5.13), logs a warning and continues with code-level enforcement.

### Other Platforms

On unsupported platforms, logs a warning and returns a plain command. Code-level enforcement via `is_write_blocked()` still applies for HTTP providers.

---

## How It Fits in the Architecture

```
User message arrives via channel
    |
    v
Gateway pipeline: auth -> sanitize -> context -> provider
    |
    v
Provider calls omega_sandbox::protected_command("claude", data_dir)
    |
    v
OS-level protection applied:
  macOS: sandbox-exec wraps the process (blocklist)
  Linux: Landlock applied via pre_exec (broad allowlist)
    |
    v
Claude CLI runs with system dirs blocked + current_dir = workspace
    |
    v
Gateway: store in memory -> audit log -> send response
```

For HTTP providers:
```
Tool executor receives read request
    |
    v
omega_sandbox::is_read_blocked(path, data_dir) -> true? reject
    |
    v
Tool executor receives write/edit request
    |
    v
omega_sandbox::is_write_blocked(path, data_dir) -> true? reject
    |
    v
Execute tool operation
```

---

## Graceful Fallback

The sandbox uses a "best effort" approach. If OS-level enforcement is unavailable:

1. **macOS without sandbox-exec** -- warning log, code-level enforcement only
2. **Linux without Landlock** -- warning log, code-level enforcement only
3. **Unsupported OS** -- warning log, code-level enforcement only

The system never fails to start because of sandbox limitations. Working directory confinement (`current_dir`) and `is_write_blocked()` always apply.

---

## Observability

- **Warning logs**: Emitted when OS enforcement falls back
- **Protection is silent**: No startup log for the mode (there is no mode -- it's always on)

---

## Quick Reference

| You want to... | Where to look |
|----------------|---------------|
| See the public API | `backend/crates/omega-sandbox/src/lib.rs` |
| See the macOS implementation | `backend/crates/omega-sandbox/src/seatbelt.rs` |
| See the Linux implementation | `backend/crates/omega-sandbox/src/landlock_sandbox.rs` |
| See code-level enforcement in HTTP providers | `backend/crates/omega-providers/src/tools.rs` |
| See CLI provider integration | `backend/crates/omega-providers/src/claude_code/` |
| See workspace creation | `backend/src/main.rs`, startup sequence |
