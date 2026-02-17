# omega-sandbox -- Developer Guide

## What is this crate?

`omega-sandbox` is the workspace isolation layer of the Omega project. It defines the `SandboxMode` enum and provides the logic for creating and managing the AI provider's workspace directory (`~/.omega/workspace/`). The sandbox controls how much host access the AI provider has, ranging from full isolation (workspace only) to full host access.

## Crate structure

```
crates/omega-sandbox/
  Cargo.toml
  src/
    lib.rs
```

---

## The Workspace Directory

The sandbox centers around a single concept: the **workspace directory** at `~/.omega/workspace/`. This directory is:

- **Created on startup** by `main.rs` if it does not already exist.
- **Set as `current_dir`** for the Claude Code CLI subprocess, so all AI file operations default to this location.
- **Always writable** regardless of sandbox mode -- the AI can always read and write within its workspace.

The workspace gives the AI a safe, isolated area to work in without risking modifications to the host filesystem.

---

## Sandbox Modes

The `SandboxMode` enum defines three levels of host access:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode {
    Sandbox,
    Rx,
    Rwx,
}
```

| Mode | Workspace Access | Host Read | Host Write | Host Execute | Use Case |
|------|-----------------|-----------|------------|--------------|----------|
| `Sandbox` | Full | No | No | No | Default. AI is confined to `~/.omega/workspace/`. Maximum isolation. |
| `Rx` | Full | Yes | No | Yes | AI can read and execute anywhere on the host, but writes are restricted to the workspace. Good for system inspection. |
| `Rwx` | Full | Yes | Yes | Yes | Full host access. For power users who trust the AI provider completely. |

### Default

The default mode is `Sandbox`, which provides maximum isolation. If the `[sandbox]` section is omitted from `config.toml`, or if the `mode` field is not specified, the sandbox defaults to this mode.

---

## Configuration

The `SandboxConfig` struct in `omega-core::config` has been simplified to a single field:

```rust
pub struct SandboxConfig {
    #[serde(default)]
    pub mode: SandboxMode,
}
```

Users configure the sandbox in `config.toml`:

```toml
[sandbox]
mode = "sandbox"   # "sandbox" | "rx" | "rwx"
```

---

## How Sandbox Mode Is Enforced

Sandbox mode is enforced at two complementary levels:

### 1. Working Directory (Hard Enforcement)

The Claude Code CLI subprocess is always spawned with `current_dir` set to `~/.omega/workspace/`. This means:

- Relative file paths resolve within the workspace.
- The AI starts in the workspace and must explicitly navigate elsewhere (if its mode permits).

This is configured in `ClaudeCodeProvider::from_config()` via the `working_dir` parameter.

### 2. System Prompt Injection (Soft Enforcement)

The gateway injects sandbox rules into the system prompt during message processing (Stage 3c of the pipeline). The injected rules tell the AI what it is allowed to do:

- **sandbox mode**: "Your workspace is `~/.omega/workspace/`. You may only read, write, and execute within this directory. Do not attempt to access files or run commands outside the workspace."
- **rx mode**: "Your workspace is `~/.omega/workspace/`. You may read files and execute commands anywhere on the host, but you may only write files within the workspace."
- **rwx mode**: "You have full access to the host filesystem. You may read, write, and execute anywhere."

This relies on the AI provider respecting the instructions, which is effective with well-aligned models like Claude.

---

## How It Fits in the Architecture

```
User message arrives via channel
    |
    v
Gateway pipeline: auth -> sanitize -> sandbox prompt injection -> context -> provider
    |
    v
Provider (Claude Code CLI) runs with current_dir = ~/.omega/workspace/
    |
    v
AI operates within its sandbox boundaries
    |
    v
Gateway: store in memory -> audit log -> send response
```

The sandbox is no longer a command-execution chokepoint. Instead, it defines the boundaries within which the AI provider operates, enforced through working directory isolation and system prompt rules.

---

## Observability

- **Startup log**: `main.rs` logs the active sandbox mode at INFO level on startup (e.g., "Sandbox mode: sandbox").
- **`/status` command**: The bot's `/status` command displays the current sandbox mode alongside uptime, provider, and database info.

---

## Error Handling

`OmegaError::Sandbox(String)` is defined in `omega-core::error` for sandbox-related errors:

```rust
use omega_core::error::OmegaError;

// Example: workspace directory creation failure
return Err(OmegaError::Sandbox(
    "failed to create workspace directory: permission denied".to_string()
));
```

---

## Key Project Rules That Apply

- **No `unwrap()`** -- use `?` and `OmegaError::Sandbox` for all error paths.
- **Tracing, not `println!`** -- use `tracing::{info, warn, error, debug}` for logging.
- **Async everywhere** -- any I/O operations must be async via tokio.
- **Every public function gets a doc comment.**
- **`cargo clippy --workspace` must pass with zero warnings.**

---

## Quick Reference

| You want to... | Where to look |
|----------------|---------------|
| See the sandbox config fields | `omega-core::config::SandboxConfig` |
| See the sandbox mode enum | `omega-core::config::SandboxMode` |
| See the error variant | `omega-core::error::OmegaError::Sandbox` |
| See example config values | `config.example.toml`, `[sandbox]` section |
| See prompt injection logic | `src/gateway.rs`, Stage 3c |
| See workspace creation | `src/main.rs`, startup sequence |
| See working_dir usage | `omega-providers::claude_code::ClaudeCodeProvider` |
