# Technical Specification: backend/crates/omega-sandbox/src/lib.rs

## File

| Field | Value |
|-------|-------|
| **Path** | `backend/crates/omega-sandbox/src/lib.rs` |
| **Crate** | `omega-sandbox` |
| **Role** | OS-level system protection for provider subprocesses |

## Purpose

`omega-sandbox` provides always-on OS-level protection that prevents AI provider subprocesses from reading or writing to OMEGA's core data and from writing to dangerous system directories. It uses a **blocklist** approach: everything is allowed by default, then specific dangerous paths are denied.

The crate exports three public functions:
- `protected_command()` — wraps a program with OS-level read/write restrictions
- `is_write_blocked()` — code-level write enforcement for HTTP provider tool executors
- `is_read_blocked()` — code-level read enforcement for HTTP provider tool executors

No configuration is needed. Protection is always active.

## Architecture

### Blocklist Approach

Instead of an allowlist ("only write here"), the sandbox uses a blocklist ("write anywhere except here"):

| What's Blocked (Writes) | Why |
|-------------------------|-----|
| `/System`, `/bin`, `/sbin`, `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/libexec` | OS binaries and system libraries |
| `/private/etc`, `/Library` (macOS) | System configuration |
| `/etc`, `/boot`, `/proc`, `/sys`, `/dev` | Linux system paths |
| `{data_dir}/data/` | OMEGA's core database (memory.db) |
| `{data_dir}/config.toml` | API keys and auth settings |

| What's Blocked (Reads) | Why |
|------------------------|-----|
| `{data_dir}/data/` | OMEGA's core database — prevents subprocess from querying memory.db directly |
| `{data_dir}/config.toml` | API keys and secrets — prevents subprocess from exfiltrating credentials |

Everything else is readable and writable, including `$HOME`, `/tmp`, `/usr/local`, `{data_dir}/workspace/`, `{data_dir}/skills/`, `{data_dir}/projects/`, `{data_dir}/stores/`.

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

**Internal function: `try_canonicalize`**

```rust
fn try_canonicalize(path: &Path) -> PathBuf
```

Best-effort path canonicalization. Resolves symlinks and returns the canonical path, or returns the original path if canonicalization fails (file doesn't exist yet, permission errors, etc.). Used by `is_write_blocked` and `is_read_blocked` to prevent symlink-based bypass attacks.

**Public function: `is_write_blocked`**

```rust
pub fn is_write_blocked(path: &Path, data_dir: &Path) -> bool
```

Code-level write enforcement for HTTP provider tool executors. Resolves symlinks via `try_canonicalize()` before comparison. Returns `true` if the path targets a protected location:
- Dangerous OS directories (`/System`, `/bin`, `/sbin`, `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/libexec`, `/private/etc`, `/Library`, `/etc`, `/boot`, `/proc`, `/sys`, `/dev`) -- uses `Path::starts_with()` (component-aware) to prevent false positives like `/binaries/test` matching `/bin`
- OMEGA's core data directory (`{data_dir}/data/`)
- OMEGA's config file (`{data_dir}/config.toml`) -- protects API keys and auth settings

Relative paths return `true` (fail closed -- relative paths could bypass protection via traversal).

Used by the `ToolExecutor` in HTTP-based providers (OpenAI, Anthropic, Ollama, OpenRouter, Gemini) to enforce write restrictions on all platforms, including those without OS-level sandbox support.

**Public function: `is_read_blocked`**

```rust
pub fn is_read_blocked(path: &Path, data_dir: &Path, config_path: Option<&Path>) -> bool
```

Code-level read enforcement for HTTP provider tool executors. Resolves symlinks via `try_canonicalize()` before comparison. Returns `true` if the path targets a protected location:
- OMEGA's core data directory (`{data_dir}/data/`) — protects memory.db
- OMEGA's config file (`{data_dir}/config.toml`) — protects API keys
- The actual config file at `config_path` (may live outside `data_dir`) — protects secrets when config is not co-located with data

Relative paths return `true` (fail closed -- relative paths could bypass protection via traversal).

Used by the `ToolExecutor` in HTTP-based providers to enforce read restrictions on all platforms.

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
  (literal "{data_dir}/config.toml")
)
(deny file-read*
  (subpath "{data_dir}/data")
  (literal "{data_dir}/config.toml")
)
```

Note: `(allow default)` permits everything, then `(deny file-write* ...)` and `(deny file-read* ...)` block specific paths. Config.toml is blocked from both reads and writes.

**Fallback:** If `/usr/bin/sandbox-exec` does not exist, logs a warning and returns a plain command.

### `landlock_sandbox.rs` — Linux (compiled only on `target_os = "linux"`)

Uses the Landlock LSM (kernel 5.13+) via the `landlock` crate. Applied in a `pre_exec` hook on the child process.

Uses a broad allowlist plus restrictive overrides for protected paths:

**Access rules:**
- Read + execute on `/` (entire filesystem -- system dirs become read-only)
- Full access to `$HOME` (covers `~/.omega/` and everything else under home)
- Full access to `/tmp`
- Optional full access to `/var/tmp`, `/opt`, `/srv`, `/run`, `/media`, `/mnt` (skipped if they don't exist)
- Refer-only access to `{data_dir}/data/` -- Landlock intersection semantics: `full_access intersection Refer = Refer`, effectively blocking both reads and writes
- Refer-only access to `{data_dir}/config.toml` (only if the file already exists -- cannot safely pre-create an empty TOML file)

**Pre-creation of directories:** The `{data_dir}/data/` directory is pre-created via `std::fs::create_dir_all()` before the Landlock rule is applied. This ensures the restriction is always active, even on first-run scenarios where the directory hasn't been created yet by another component. Without this, the Landlock rule would be skipped for non-existent paths, leaving memory.db unprotected once the directory is later created.

**Config.toml caveat:** The config file cannot be safely pre-created because an empty file would break the TOML parser on startup. The code-level enforcement via `is_read_blocked()` and `is_write_blocked()` provides protection for config.toml even when it doesn't exist yet on first run.

Code-level enforcement via `is_read_blocked()` and `is_write_blocked()` provides additional protection on all platforms.

**Fallback:** If the kernel does not support Landlock or enforcement is partial, logs a warning and continues with best-effort restrictions.

## Workspace Integration Points

| Integration | Location | Description |
|-------------|----------|-------------|
| Root Cargo.toml | `backend/Cargo.toml` | Listed as workspace member and dependency |
| main.rs | `backend/src/main.rs` | Creates `~/.omega/workspace/`, passes `data_dir` to `build_provider()` |
| Gateway | `backend/src/gateway.rs` | No sandbox-specific state needed (protection is always on) |
| Provider (CLI) | `omega-providers::claude_code` | `ClaudeCodeProvider` calls `omega_sandbox::protected_command()` in `run_cli()` |
| Provider (HTTP) | `omega-providers::tools` | `ToolExecutor` calls `omega_sandbox::is_write_blocked()` and `omega_sandbox::is_read_blocked()` for tool operations |
| Binary | `backend/Cargo.toml` (root) | Declared as a dependency of the binary |

## Protected Paths

### Blocked from writes (all platforms, via `is_write_blocked`)

| Path | Reason |
|------|--------|
| `{data_dir}/data/` | OMEGA's core database (memory.db, etc.) |
| `{data_dir}/config.toml` | API keys and auth settings |
| `/System` | macOS system |
| `/bin`, `/sbin` | System binaries |
| `/usr/bin`, `/usr/sbin`, `/usr/lib`, `/usr/libexec` | System binaries and libraries |
| `/private/etc` | macOS system config |
| `/Library` | macOS system libraries |
| `/etc`, `/boot`, `/proc`, `/sys`, `/dev` | Linux system paths |

Note: `is_write_blocked` uses `Path::starts_with()` (component-aware) for OS directory matching, preventing false positives like `/binaries/test` matching `/bin`. Relative paths return `true` (fail closed).

### Blocked from reads (all platforms, via `is_read_blocked`)

| Path | Reason |
|------|--------|
| `{data_dir}/data/` | OMEGA's core database — prevents subprocess from querying memory.db |
| `{data_dir}/config.toml` | API keys and secrets — prevents credential exfiltration |

### Always accessible (read + write)

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

### `lib.rs` tests (14)
- `test_protected_command_returns_command` -- returns a valid command
- `test_is_write_blocked_data_dir` -- blocks writes to `{data_dir}/data/`
- `test_is_write_blocked_allows_workspace` -- allows writes to workspace, skills
- `test_is_write_blocked_system_dirs` -- blocks `/System`, `/bin`, `/usr/bin`, `/private/etc`, `/Library`
- `test_is_write_blocked_allows_normal_paths` -- allows `/tmp`, home, `/usr/local`
- `test_is_write_blocked_no_string_prefix_false_positive` -- `/binaries/test` does NOT match `/bin` (component-aware)
- `test_is_write_blocked_relative_path` -- relative paths return true (fail closed)
- `test_is_write_blocked_config_toml` -- blocks writes to `{data_dir}/config.toml`
- `test_is_read_blocked_data_dir` -- blocks reads to `{data_dir}/data/`
- `test_is_read_blocked_config` -- blocks reads to `{data_dir}/config.toml`
- `test_is_read_blocked_external_config` -- blocks reads to external config_path, allows non-matching
- `test_is_read_blocked_allows_workspace` -- allows reads to workspace, skills
- `test_is_read_blocked_allows_stores` -- allows reads to stores/
- `test_is_read_blocked_relative_path` -- relative paths return true (fail closed)

### `seatbelt.rs` tests (8, macOS only)
- `test_profile_blocks_system_dirs` -- profile denies writes to all system directories
- `test_profile_blocks_data_dir` -- profile denies writes to `{data_dir}/data/`
- `test_profile_blocks_data_dir_reads` -- profile denies reads to `{data_dir}/data/`
- `test_profile_blocks_config_reads` -- profile denies reads to `{data_dir}/config.toml`
- `test_profile_blocks_config_writes` -- profile denies writes to `{data_dir}/config.toml`
- `test_profile_allows_usr_local` -- `/usr/local` is not blocked
- `test_profile_allows_by_default` -- profile starts with `(allow default)`
- `test_command_structure` -- command program is sandbox-exec or claude (fallback)

### `landlock_sandbox.rs` tests (4, Linux only)
- `test_read_access_flags` -- read flags include ReadFile, ReadDir, Execute
- `test_full_access_contains_writes` -- full flags include WriteFile, MakeDir
- `test_refer_only_blocks_reads_and_writes` -- Refer-only flag excludes ReadFile and WriteFile
- `test_command_structure` -- command program is "claude"

## File Size

| Metric | Value |
|--------|-------|
| Lines of code (lib.rs) | ~350 |
| Lines of code (seatbelt.rs) | ~173 |
| Lines of code (landlock_sandbox.rs) | ~163 |
| Public functions | 3 |
| Modules | 2 (platform-conditional) |
| Tests | 26 (14 cross-platform + 8 macOS + 4 Linux) |

## Implementation Status

| Component | Status |
|-----------|--------|
| `protected_command()` public API | Complete |
| `is_write_blocked()` public API | Complete |
| `is_read_blocked()` public API | Complete |
| macOS Seatbelt write blocklist enforcement (`seatbelt.rs`) | Complete |
| macOS Seatbelt read blocklist enforcement (`seatbelt.rs`) | Complete |
| Linux Landlock broad-allowlist enforcement (`landlock_sandbox.rs`) | Complete |
| Linux Landlock Refer-only data/config restriction | Complete |
| Unsupported platform fallback | Complete |
| CLI provider integration (`ClaudeCodeProvider`) | Complete |
| HTTP provider integration (`ToolExecutor`) — write + read | Complete |
| Unit tests | Complete |
