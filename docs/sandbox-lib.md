# omega-sandbox — Developer Guide

## What is this crate?

`omega-sandbox` is the OS-level filesystem enforcement layer of the Omega project. It wraps the AI provider subprocess with platform-native write restrictions so that the provider cannot write files outside permitted directories, regardless of what the AI attempts.

## Crate structure

```
crates/omega-sandbox/
  Cargo.toml
  src/
    lib.rs               # Public API: sandboxed_command()
    seatbelt.rs          # macOS: sandbox-exec write restrictions
    landlock_sandbox.rs  # Linux: Landlock LSM write restrictions
```

---

## How It Works

Omega uses a **two-layer** sandbox enforcement strategy:

### Layer 1: OS-Level Write Restrictions (this crate)

Platform-native mechanisms prevent the AI subprocess from writing files outside permitted directories:

| Platform | Mechanism | How |
|----------|-----------|-----|
| **macOS** | Apple Seatbelt | Wraps command with `sandbox-exec -p <profile>` |
| **Linux** | Landlock LSM | Applies filesystem restrictions via `pre_exec` hook |
| **Other** | None | Logs warning, uses prompt-only enforcement |

### Layer 2: Prompt-Level Read Restrictions (omega-core)

The system prompt tells the AI what it may read. This is effective with well-aligned models like Claude but is not enforced at the OS level.

### Why writes only?

The claude CLI process needs to read system files (node.js, shared libraries, etc.) to function. Restricting reads at the OS level would break the subprocess. Read restrictions in Sandbox mode stay prompt-level.

---

## The Data Directory

The sandbox centers around the **Omega data directory** at `~/.omega/`. This directory contains the workspace, skills, projects, and all other Omega data. The entire tree is writable under sandbox enforcement.

The **workspace** subdirectory (`~/.omega/workspace/`) is:

- **Created on startup** by `main.rs` if it does not already exist.
- **Set as `current_dir`** for the Claude Code CLI subprocess.

---

## Sandbox Modes

| Mode | OS Write Restriction | Prompt Read Restriction | Use Case |
|------|---------------------|------------------------|----------|
| `Sandbox` | Writes only to ~/.omega/ + /tmp + ~/.claude | Reads only in workspace | Default. Maximum isolation. |
| `Rx` | Writes only to ~/.omega/ + /tmp + ~/.claude | Reads anywhere | System inspection. |
| `Rwx` | None (unrestricted) | None (unrestricted) | Power users. |

### Permitted Write Directories (Sandbox and Rx modes)

| Directory | Purpose |
|-----------|---------|
| `~/.omega/` | Omega data directory (workspace, skills, projects, etc.) |
| `/tmp` | Temporary files |
| `/private/var/folders` (macOS) | macOS temp directories |
| `~/.claude` | Claude CLI session data |

### Dependency Installation

All three modes have **full network access**. In `sandbox` and `rx` modes, packages must be installed locally within the workspace (e.g., `npm install`, `pip install --target .`). In `rwx` mode, global installs also work.

---

## Configuration

Users configure the sandbox in `config.toml`:

```toml
[sandbox]
mode = "sandbox"   # "sandbox" | "rx" | "rwx"
```

The default mode is `Sandbox` (safest).

---

## Platform Details

### macOS — Seatbelt

The crate generates a Seatbelt profile and invokes `sandbox-exec -p <profile> -- claude ...`. The profile:

1. Allows all operations by default
2. Denies all file writes
3. Re-allows writes to the permitted directories

If `/usr/bin/sandbox-exec` does not exist (unlikely on macOS), falls back to prompt-only enforcement with a warning.

### Linux — Landlock

The crate uses the `landlock` crate (Landlock LSM, kernel 5.13+) in a `pre_exec` hook. The child process gets:

- Read + execute access to the entire filesystem
- Full access (read + write + create) to the permitted directories

If the kernel does not support Landlock, logs a warning and continues with best-effort enforcement.

### Other Platforms

On unsupported platforms, logs a warning and returns a plain command. Prompt-level enforcement still applies.

---

## How It Fits in the Architecture

```
User message arrives via channel
    │
    ▼
Gateway pipeline: auth → sanitize → sandbox prompt injection → context → provider
    │
    ▼
Provider calls omega_sandbox::sandboxed_command("claude", mode, data_dir)
    │
    ▼
OS-level enforcement applied:
  macOS: sandbox-exec wraps the process
  Linux: Landlock applied via pre_exec
    │
    ▼
Claude CLI runs with write restrictions + current_dir = workspace
    │
    ▼
Gateway: store in memory → audit log → send response
```

---

## Graceful Fallback

The sandbox uses a "best effort" approach. If OS-level enforcement is unavailable:

1. **macOS without sandbox-exec** → warning log, prompt-only enforcement
2. **Linux without Landlock** → warning log, prompt-only enforcement
3. **Unsupported OS** → warning log, prompt-only enforcement

The system never fails to start because of sandbox limitations. Working directory confinement (`current_dir`) and prompt constraints always apply.

---

## Observability

- **Startup log**: `main.rs` logs the active sandbox mode at INFO level
- **`/status` command**: Displays the current sandbox mode
- **Warning logs**: Emitted when OS enforcement falls back

---

## Error Handling

`OmegaError::Sandbox(String)` is defined in `omega-core::error` for sandbox-related errors.

---

## Quick Reference

| You want to... | Where to look |
|----------------|---------------|
| See the public API | `crates/omega-sandbox/src/lib.rs` |
| See the macOS implementation | `crates/omega-sandbox/src/seatbelt.rs` |
| See the Linux implementation | `crates/omega-sandbox/src/landlock_sandbox.rs` |
| See the sandbox config fields | `omega-core::config::SandboxConfig` |
| See the sandbox mode enum | `omega-core::config::SandboxMode` |
| See the error variant | `omega-core::error::OmegaError::Sandbox` |
| See prompt injection logic | `src/gateway.rs` |
| See workspace creation | `src/main.rs`, startup sequence |
| See provider integration | `omega-providers::claude_code::ClaudeCodeProvider::run_cli()` |
