# Technical Specification: omega-sandbox/src/lib.rs

## File

| Field | Value |
|-------|-------|
| **Path** | `crates/omega-sandbox/src/lib.rs` |
| **Crate** | `omega-sandbox` |
| **Role** | OS-level system protection for provider subprocesses |

## Purpose

`omega-sandbox` provides always-on OS-level protection that prevents AI provider subprocesses from writing to dangerous system directories and OMEGA's core database. It uses a **blocklist** approach: everything is allowed by default, then specific dangerous paths are denied.

The crate exports two public functions:
- `protected_command()` — wraps a program with OS-level write restrictions
- `is_write_blocked()` — code-level enforcement for HTTP provider tool executors

No configuration is needed. Protection is always active.

## Architecture

### Blocklist Approach

Instead of an allowlist ("only write here"), the sandbox uses a blocklist ("write anywhere except here"):

| What's Blocked | Why |
|----------------|-----|
| `/System`, `/bin`, `/sbin`, `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/libexec` | OS binaries and system libraries |
| `/private/etc`, `/Library` (macOS) | System configuration |
| `/etc`, `/boot`, `/proc`, `/sys`, `/dev` | Linux system paths |
| `{data_dir}/data/` | OMEGA's core database (memory.db) |

Everything else is writable, including `$HOME`, `/tmp`, `/usr/local`, `{data_dir}/workspace/`, `{data_dir}/skills/`, `{data_dir}/projects/`, `{data_dir}/stores/`.

### Platform Dispatch

```
protected_command(program, data_dir)
  │
  ├── macOS → seatbelt::protected_command()
  │     └── sandbox-exec -p <blocklist profile> -- <program>
  │
  ├── Linux → landlock_sandbox::protected_command()
  │     └── Command::new(program) + pre_exec Landlock
  │
  └── Other → warn + plain Command::new(program)
```

## Modules

### `lib.rs` — Public API

**Public function: `protected_command`**

```rust
pub fn protected_command(program: &str, data_dir: &Path) -> Command
```

Builds a `tokio::process::Command` with OS-level system protection applied. Always active — no mode parameter needed.

- `program` — the binary to wrap (e.g., `"claude"`)
- `data_dir` — the Omega data directory (e.g., `~/.omega/`). Writes to `{data_dir}/data/` are blocked to protect memory.db. All other paths under `data_dir` remain writable.

**Public function: `is_write_blocked`**

```rust
pub fn is_write_blocked(path: &Path, data_dir: &Path) -> bool
```

Code-level write enforcement for HTTP provider tool executors. Returns `true` if the path targets a protected location:
- Dangerous OS directories (`/System`, `/bin`, `/sbin`, `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/libexec`, `/private/etc`, `/Library`, `/etc`, `/boot`, `/proc`, `/sys`, `/dev`)
- OMEGA's core data directory (`{data_dir}/data/`)

Relative paths return `false` (cannot be resolved without a cwd).

Used by the `ToolExecutor` in HTTP-based providers (OpenAI, Anthropic, Ollama, OpenRouter, Gemini) to enforce write restrictions on all platforms, including those without OS-level sandbox support.

**Internal function: `platform_command`**

```rust
fn platform_command(program: &str, data_dir: &Path) -> Command
```

Platform-dispatched via `#[cfg]` attributes.

### `seatbelt.rs` — macOS (compiled only on `target_os = "macos"`)

Uses Apple's Seatbelt framework via `sandbox-exec -p <profile>`.

**Profile (blocklist):**
```scheme
(version 1)
(allow default)
(deny file-write*
  (subpath "/System")
  (subpath "/bin")
  (subpath "/sbin")
  (subpath "/usr/bin")
  (subpath "/usr/sbin")
  (subpath "/usr/lib")
  (subpath "/usr/libexec")
  (subpath "/private/etc")
  (subpath "/Library")
  (subpath "{data_dir}/data")
)
```

Note: `(allow default)` permits everything, then `(deny file-write* ...)` blocks specific paths. This is the inverse of the previous allowlist approach.

**Fallback:** If `/usr/bin/sandbox-exec` does not exist, logs a warning and returns a plain command.

### `landlock_sandbox.rs` — Linux (compiled only on `target_os = "linux"`)

Uses the Landlock LSM (kernel 5.13+) via the `landlock` crate. Applied in a `pre_exec` hook on the child process.

Because Landlock cannot deny subdirectories of an allowed parent, a broad allowlist achieves the same blocklist effect:

**Access rules:**
- Read + execute on `/` (entire filesystem — system dirs become read-only)
- Full access to `$HOME` (covers `~/.omega/` and everything else under home)
- Full access to `/tmp`
- Optional full access to `/var/tmp`, `/opt`, `/srv`, `/run`, `/media`, `/mnt` (skipped if they don't exist)

Note: memory.db protection on Linux relies on `is_write_blocked()` (code-level) rather than Landlock, since Landlock cannot deny a subdirectory of `$HOME`.

**Fallback:** If the kernel does not support Landlock or enforcement is partial, logs a warning and continues with best-effort restrictions.

## Workspace Integration Points

| Integration | Location | Description |
|-------------|----------|-------------|
| Root Cargo.toml | `Cargo.toml` | Listed as workspace member and dependency |
| main.rs | `src/main.rs` | Creates `~/.omega/workspace/`, passes `data_dir` to `build_provider()` |
| Gateway | `src/gateway.rs` | No sandbox-specific state needed (protection is always on) |
| Provider (CLI) | `omega-providers::claude_code` | `ClaudeCodeProvider` calls `omega_sandbox::protected_command()` in `run_cli()` |
| Provider (HTTP) | `omega-providers::tools` | `ToolExecutor` calls `omega_sandbox::is_write_blocked()` for write/edit/bash tools |
| Binary | `Cargo.toml` (root) | Declared as a dependency of the binary |

## Protected Paths

### Blocked from writes (all platforms, via `is_write_blocked`)

| Path | Reason |
|------|--------|
| `{data_dir}/data/` | OMEGA's core database (memory.db, etc.) |
| `/System` | macOS system |
| `/bin`, `/sbin` | System binaries |
| `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/libexec` | System binaries and libraries |
| `/private/etc` | macOS system config |
| `/Library` | macOS system libraries |
| `/etc`, `/boot`, `/proc`, `/sys`, `/dev` | Linux system paths |

### Always writable

| Path | Reason |
|------|--------|
| `{data_dir}/workspace/` | Sandbox working directory |
| `{data_dir}/skills/` | Skill definitions |
| `{data_dir}/projects/` | Project contexts |
| `{data_dir}/stores/` | Domain-specific databases |
| `$HOME` | User's home directory |
| `/tmp` | Temporary files |
| `/usr/local` | Homebrew, user-installed software |

## Tests

### `lib.rs` tests (6)
- `test_protected_command_returns_command` — returns a valid command
- `test_is_write_blocked_data_dir` — blocks writes to `{data_dir}/data/`
- `test_is_write_blocked_allows_workspace` — allows writes to workspace, skills
- `test_is_write_blocked_system_dirs` — blocks `/System`, `/bin`, `/usr/bin`, `/private/etc`, `/Library`
- `test_is_write_blocked_allows_normal_paths` — allows `/tmp`, home, `/usr/local`
- `test_is_write_blocked_relative_path` — relative paths return false

### `seatbelt.rs` tests (5, macOS only)
- `test_profile_blocks_system_dirs` — profile denies writes to all system directories
- `test_profile_blocks_data_dir` — profile denies writes to `{data_dir}/data/`
- `test_profile_allows_usr_local` — `/usr/local` is not blocked
- `test_profile_allows_by_default` — profile starts with `(allow default)`
- `test_command_structure` — command program is sandbox-exec or claude (fallback)

### `landlock_sandbox.rs` tests (3, Linux only)
- `test_read_access_flags` — read flags include ReadFile, ReadDir, Execute
- `test_full_access_contains_writes` — full flags include WriteFile, MakeDir
- `test_command_structure` — command program is "claude"

## File Size

| Metric | Value |
|--------|-------|
| Lines of code (lib.rs) | ~180 |
| Lines of code (seatbelt.rs) | ~120 |
| Lines of code (landlock_sandbox.rs) | ~120 |
| Public functions | 2 |
| Modules | 2 (platform-conditional) |
| Tests | 14 (6 cross-platform + 5 macOS + 3 Linux) |

## Implementation Status

| Component | Status |
|-----------|--------|
| `protected_command()` public API | Complete |
| `is_write_blocked()` public API | Complete |
| macOS Seatbelt blocklist enforcement (`seatbelt.rs`) | Complete |
| Linux Landlock broad-allowlist enforcement (`landlock_sandbox.rs`) | Complete |
| Unsupported platform fallback | Complete |
| CLI provider integration (`ClaudeCodeProvider`) | Complete |
| HTTP provider integration (`ToolExecutor`) | Complete |
| Unit tests | Complete |
