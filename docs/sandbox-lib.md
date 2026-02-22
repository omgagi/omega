# omega-sandbox -- Developer Guide

## What is this crate?

`omega-sandbox` is the OS-level system protection layer of the Omega project. It prevents AI provider subprocesses from writing to dangerous system directories and OMEGA's core database. Protection is always active -- no configuration needed.

## Crate structure

```
crates/omega-sandbox/
  Cargo.toml
  src/
    lib.rs               # Public API: protected_command() + is_write_blocked()
    seatbelt.rs          # macOS: Seatbelt blocklist (deny system dirs)
    landlock_sandbox.rs  # Linux: Landlock broad allowlist (read-only on /)
```

---

## How It Works

The sandbox uses a **blocklist** approach: everything is allowed by default, then specific dangerous paths are denied. No modes, no configuration, no opt-in.

### OS-Level Protection (CLI provider)

Platform-native mechanisms block the AI subprocess from writing to system directories:

| Platform | Mechanism | Approach |
|----------|-----------|----------|
| **macOS** | Apple Seatbelt | `sandbox-exec` with a deny profile for system dirs |
| **Linux** | Landlock LSM | Read-only on `/`, full access to `$HOME` + `/tmp` |
| **Other** | None | Logs warning, uses code-level enforcement only |

### Code-Level Protection (HTTP providers)

The `is_write_blocked()` function provides write enforcement for HTTP-based providers (OpenAI, Anthropic, Ollama, OpenRouter, Gemini). Their `ToolExecutor` calls this before executing any write/edit/bash operation. This works on all platforms, including those without OS-level sandbox support.

---

## What's Blocked

The sandbox blocks writes to these locations:

| Path | What it protects |
|------|-----------------|
| `~/.omega/data/` | OMEGA's core database (memory.db, audit trail, facts) |
| `/System` | macOS system |
| `/bin`, `/sbin` | System binaries |
| `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/libexec` | System binaries and libraries |
| `/private/etc` | macOS system configuration |
| `/Library` | macOS system libraries |
| `/etc`, `/boot`, `/proc`, `/sys`, `/dev` | Linux system paths |

### Why protect memory.db?

OMEGA's database at `~/.omega/data/memory.db` contains the audit trail, conversation history, scheduled tasks, user facts, and learned lessons. If the AI could write to it directly, it could tamper with its own memory, delete audit records, or corrupt the database. Only the Omega binary itself (via `omega-memory`) should write to this file.

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
2. Denies writes to the blocklisted directories (`(deny file-write* ...)`)

If `/usr/bin/sandbox-exec` does not exist (unlikely on macOS), falls back to code-level enforcement with a warning.

### Linux -- Landlock

Landlock cannot deny subdirectories of an allowed parent. So the crate uses a broad allowlist that achieves the same effect:

- Read + execute on `/` (system dirs become read-only)
- Full access to `$HOME`, `/tmp`
- Optional full access to `/var/tmp`, `/opt`, `/srv`, `/run`, `/media`, `/mnt`

Note: memory.db protection on Linux uses `is_write_blocked()` (code-level) since Landlock cannot deny `~/.omega/data/` while allowing `~/.omega/workspace/`.

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
Tool executor receives write/edit/bash request
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
| See the public API | `crates/omega-sandbox/src/lib.rs` |
| See the macOS implementation | `crates/omega-sandbox/src/seatbelt.rs` |
| See the Linux implementation | `crates/omega-sandbox/src/landlock_sandbox.rs` |
| See code-level enforcement in HTTP providers | `crates/omega-providers/src/tools.rs` |
| See CLI provider integration | `crates/omega-providers/src/claude_code/` |
| See workspace creation | `src/main.rs`, startup sequence |
