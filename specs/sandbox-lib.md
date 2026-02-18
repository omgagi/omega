# Technical Specification: omega-sandbox/src/lib.rs

## File

| Field | Value |
|-------|-------|
| **Path** | `crates/omega-sandbox/src/lib.rs` |
| **Crate** | `omega-sandbox` |
| **Role** | OS-level filesystem enforcement for provider subprocesses |

## Purpose

`omega-sandbox` provides real OS-level write restrictions for the AI provider subprocess. It wraps the provider command with platform-native sandbox mechanisms so that even a determined or confused AI cannot write files outside permitted directories.

The crate exports a single public function, `sandboxed_command()`, which takes a program name, sandbox mode, and workspace path, and returns a `tokio::process::Command` with appropriate OS enforcement applied.

## Architecture

### Two-Layer Enforcement

| Layer | Mechanism | Scope |
|-------|-----------|-------|
| **OS-level** (this crate) | Seatbelt (macOS) / Landlock (Linux) | Restricts file **writes** to data dir (`~/.omega/`) + `/tmp` + `~/.claude` |
| **Prompt-level** (omega-core) | System prompt injection via `SandboxMode::prompt_constraint()` | Restricts file **reads** in Sandbox mode |

The OS sandbox restricts writes only. The claude CLI process needs to read system files, node.js runtime, etc. Read restriction in Sandbox mode stays prompt-level.

### Platform Dispatch

```
sandboxed_command(program, mode, data_dir)
  │
  ├── Rwx → plain Command::new(program)
  │
  └── Sandbox / Rx → platform_command()
        │
        ├── macOS → seatbelt::sandboxed_command()
        │     └── sandbox-exec -p <profile> -- <program>
        │
        ├── Linux → landlock_sandbox::sandboxed_command()
        │     └── Command::new(program) + pre_exec Landlock
        │
        └── Other → warn + plain Command::new(program)
```

## Modules

### `lib.rs` — Public API

**Public function:**

```rust
pub fn sandboxed_command(program: &str, mode: SandboxMode, data_dir: &Path) -> Command
```

- `Rwx` → plain `Command::new(program)` (no restrictions)
- `Sandbox` / `Rx` → delegates to `platform_command()`

`data_dir` is the Omega data directory (e.g. `~/.omega/`) — writes are allowed to the entire tree (workspace, skills, projects, etc.).

**Internal function:**

```rust
fn platform_command(program: &str, data_dir: &Path) -> Command
```

Platform-dispatched via `#[cfg]` attributes.

### `seatbelt.rs` — macOS (compiled only on `target_os = "macos"`)

Uses Apple's Seatbelt framework via `sandbox-exec -p <profile>`.

**Profile:**
```scheme
(version 1)
(allow default)
(deny file-write*)
(allow file-write*
  (subpath "{data_dir}")
  (subpath "/private/tmp")
  (subpath "/private/var/folders")
  (subpath "{home}/.claude")
)
```

**Fallback:** If `/usr/bin/sandbox-exec` does not exist, logs a warning and returns a plain command.

### `landlock_sandbox.rs` — Linux (compiled only on `target_os = "linux"`)

Uses the Landlock LSM (kernel 5.13+) via the `landlock` crate. Applied in a `pre_exec` hook on the child process.

**Access rules:**
- Read + execute on `/` (entire filesystem)
- Full access to data dir (`~/.omega/`), `/tmp`, `~/.claude`

**Fallback:** If the kernel does not support Landlock or enforcement is partial, logs a warning and continues with best-effort restrictions.

## Configuration Surface

The sandbox is configurable through `omega-core::config::SandboxConfig` and `omega-core::config::SandboxMode`:

```rust
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode {
    #[default]
    Sandbox,
    Rx,
    Rwx,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default)]
    pub mode: SandboxMode,
}
```

`SandboxMode` methods:
- `prompt_constraint(&self, workspace_path: &str) -> Option<String>` — returns mode-specific system prompt constraint text.
- `display_name(&self) -> &str` — returns `"sandbox"`, `"rx"`, or `"rwx"`.

## Error Integration

The unified error enum in `omega-core::error::OmegaError` includes a `Sandbox` variant:

```rust
#[error("sandbox error: {0}")]
Sandbox(String),
```

## Workspace Integration Points

| Integration | Location | Description |
|-------------|----------|-------------|
| Root Cargo.toml | `Cargo.toml` | Listed as workspace member and dependency |
| Config | `omega-core::config::SandboxConfig` | Configuration struct with `mode: SandboxMode` field |
| SandboxMode enum | `omega-core::config::SandboxMode` | Mode enum with `prompt_constraint()` and `display_name()` methods |
| Error | `omega-core::error::OmegaError::Sandbox` | Error variant for sandbox failures |
| main.rs | `src/main.rs` | Creates `~/.omega/workspace/`, passes `sandbox_mode` to `build_provider()` |
| Gateway | `src/gateway.rs` | Stores `sandbox_mode` and `sandbox_prompt` fields; injects sandbox constraint into system prompt |
| Provider | `omega-providers::claude_code` | `ClaudeCodeProvider` calls `omega_sandbox::sandboxed_command()` in `run_cli()` |
| Commands | `src/commands.rs` | `CommandContext` includes `sandbox_mode` field; `/status` displays it |
| Binary | `Cargo.toml` (root) | Declared as a dependency of the binary |

## Permitted Directories

For both Sandbox and Rx modes, OS enforcement allows writes to:

| Directory | Reason |
|-----------|--------|
| `~/.omega/` | Omega data directory (workspace, skills, projects, etc.) |
| `/tmp` (macOS: `/private/tmp`) | Temporary files |
| `/private/var/folders` (macOS only) | macOS temp directories |
| `~/.claude` | Claude CLI session data |

## Tests

### `lib.rs` tests (3)
- `test_rwx_returns_plain_command` — Rwx mode returns unwrapped command
- `test_sandbox_mode_returns_command` — Sandbox mode returns a valid command
- `test_rx_mode_returns_command` — Rx mode returns a valid command

### `seatbelt.rs` tests (4, macOS only)
- `test_profile_contains_data_dir` — profile includes data directory path
- `test_profile_denies_writes_then_allows` — profile has deny + allow structure
- `test_profile_allows_claude_dir` — profile includes ~/.claude
- `test_command_structure` — command program is sandbox-exec or claude (fallback)

### `landlock_sandbox.rs` tests (3, Linux only)
- `test_read_access_flags` — read flags include ReadFile, ReadDir, Execute
- `test_full_access_contains_writes` — full flags include WriteFile, MakeDir
- `test_command_structure` — command program is "claude"

## File Size

| Metric | Value |
|--------|-------|
| Lines of code (lib.rs) | ~85 |
| Lines of code (seatbelt.rs) | ~95 |
| Lines of code (landlock_sandbox.rs) | ~95 |
| Public functions | 1 |
| Modules | 2 (platform-conditional) |
| Tests | 10 (3 cross-platform + 4 macOS + 3 Linux) |

## Implementation Status

| Component | Status |
|-----------|--------|
| `sandboxed_command()` public API | Complete |
| macOS Seatbelt enforcement (`seatbelt.rs`) | Complete |
| Linux Landlock enforcement (`landlock_sandbox.rs`) | Complete |
| Unsupported platform fallback | Complete |
| Provider integration (`ClaudeCodeProvider`) | Complete |
| `main.rs` passes sandbox mode to provider | Complete |
| Unit tests | Complete |
| Prompt-level enforcement (omega-core) | Complete |
| Working directory confinement (omega-providers) | Complete |
