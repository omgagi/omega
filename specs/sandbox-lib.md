# Technical Specification: omega-sandbox/src/lib.rs

## File

| Field | Value |
|-------|-------|
| **Path** | `crates/omega-sandbox/src/lib.rs` |
| **Crate** | `omega-sandbox` |
| **Role** | Crate root -- placeholder for the secure command execution environment |

## Purpose

`omega-sandbox` is the secure execution environment for the Omega agent. Its responsibility is to provide a controlled context in which the AI provider operates, using a combination of workspace directory confinement and system prompt constraints to enforce operating boundaries.

The sandbox design uses a mode-based approach (`SandboxMode` enum in `omega-core`) rather than command-level allowlists. The three modes -- `sandbox`, `rx`, and `rwx` -- control the provider's working directory and the system prompt constraints injected before each interaction. The actual enforcement is handled via:
1. **Working directory confinement:** The provider subprocess `current_dir` is set to `~/.omega/workspace/` in sandbox and rx modes.
2. **System prompt injection:** Mode-specific constraint text is prepended to the system prompt, instructing the provider about its operating boundaries.

The crate is currently a **placeholder**. The `lib.rs` file contains only a module-level doc comment and no types, traits, functions, or submodules. The core sandbox logic (mode enum, prompt constraints) lives in `omega-core::config`. Future implementation in this crate may add process-level isolation.

## Current Contents

The entire file consists of a single doc comment:

```rust
//! # omega-sandbox
//!
//! Secure execution environment for Omega.
```

### Module Declarations

None.

### Public Types

None.

### Public Functions

None.

### Traits

None.

### Tests

None.

---

## Dependencies (Cargo.toml)

The crate's `Cargo.toml` already declares the dependencies it will need once implementation begins:

| Dependency | Workspace | Planned Usage |
|------------|-----------|---------------|
| `omega-core` | Yes | Access to `SandboxConfig`, `OmegaError::Sandbox`, and shared types |
| `tokio` | Yes | Async command execution (`tokio::process::Command`), timeouts, task spawning |
| `serde` | Yes | Serialization of execution results and configuration |
| `tracing` | Yes | Structured logging of command execution, policy decisions, and errors |
| `thiserror` | Yes | Potential sandbox-specific error subtypes (or direct use of `OmegaError::Sandbox`) |
| `anyhow` | Yes | Ergonomic error handling during development |

---

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
- `prompt_constraint(&self, workspace_path: &str) -> Option<String>` -- returns mode-specific system prompt constraint text. `Sandbox` returns SANDBOX mode instructions referencing the workspace path. `Rx` returns READ-ONLY mode instructions. `Rwx` returns `None`.
- `display_name(&self) -> &str` -- returns `"sandbox"`, `"rx"`, or `"rwx"`.

The example configuration (`config.example.toml`) demonstrates the setup:

```toml
[sandbox]
mode = "sandbox"   # "sandbox" | "rx" | "rwx"
```

---

## Error Integration

The unified error enum in `omega-core::error::OmegaError` already includes a `Sandbox` variant:

```rust
#[error("sandbox error: {0}")]
Sandbox(String),
```

This variant is manually constructed (no `#[from]` conversion). All sandbox errors should wrap into this variant using descriptive messages that include the command attempted, the policy that blocked it, and the reason.

---

## Workspace Integration Points

| Integration | Location | Description |
|-------------|----------|-------------|
| Root Cargo.toml | `Cargo.toml` | Listed as workspace member and dependency |
| Config | `omega-core::config::SandboxConfig` | Configuration struct with `mode: SandboxMode` field |
| SandboxMode enum | `omega-core::config::SandboxMode` | Mode enum with `prompt_constraint()` and `display_name()` methods |
| Error | `omega-core::error::OmegaError::Sandbox` | Error variant for sandbox failures |
| main.rs | `src/main.rs` | Creates `~/.omega/workspace/` directory, resolves sandbox mode, passes workspace path to `build_provider()` and sandbox mode/prompt to `Gateway::new()` |
| Gateway | `src/gateway.rs` | Stores `sandbox_mode` and `sandbox_prompt` fields; injects sandbox constraint into system prompt; logs sandbox mode at startup |
| Provider | `omega-providers::claude_code` | `ClaudeCodeProvider` accepts `working_dir: Option<PathBuf>` and sets `current_dir` on subprocess |
| Commands | `src/commands.rs` | `CommandContext` includes `sandbox_mode` field; `/status` displays it |
| Skills | `omega-skills` | Future skills may invoke sandbox for command execution |
| Binary | `Cargo.toml` (root) | Already declared as a dependency of the binary |

---

## Current Architecture

The sandbox design follows the "less is more" principle. Rather than implementing a complex process-level sandbox with command allowlists and path blocklists, the current approach uses two complementary mechanisms:

### 1. Working Directory Confinement

In `sandbox` and `rx` modes, the provider subprocess `current_dir` is set to `~/.omega/workspace/`. This directory is automatically created by `main.rs` at startup. The provider naturally operates within this directory.

### 2. System Prompt Constraints

`SandboxMode::prompt_constraint()` returns mode-specific instructions that are prepended to the system prompt:

| Mode | Constraint Behavior |
|------|-------------------|
| `Sandbox` | Instructs the provider to confine all operations to the workspace directory |
| `Rx` | Instructs the provider to only read files; no writes, deletes, or command execution |
| `Rwx` | No constraint injected (unrestricted) |

### Security Considerations

| Concern | Mitigation |
|---------|------------|
| Filesystem access | Working directory confinement + system prompt instructions |
| Write operations | `Rx` mode explicitly forbids writes via prompt constraint |
| Privilege escalation | Omega refuses to run as root (guard in `main.rs`) |
| Environment leakage | `CLAUDECODE` env var removed from provider subprocess |
| Default safety | `SandboxMode::Sandbox` is the default, ensuring new installations start confined |

## Planned Extensions

Future implementation in this crate may add:
- Process-level isolation (e.g., Linux namespaces, macOS sandbox profiles)
- Runtime enforcement beyond prompt-based constraints
- Execution result capture and audit logging

---

## File Size

| Metric | Value |
|--------|-------|
| Lines of code | 3 |
| Public types | 0 |
| Public functions | 0 |
| Tests | 0 |
| Submodules | 0 |

---

## Implementation Status

| Component | Status |
|-----------|--------|
| Crate scaffolding (Cargo.toml, lib.rs) | Complete |
| Configuration (`SandboxConfig` with `SandboxMode`) | Complete (in omega-core) |
| `SandboxMode` enum with `prompt_constraint()` and `display_name()` | Complete (in omega-core) |
| Error variant (`OmegaError::Sandbox`) | Complete (in omega-core) |
| Example config (`config.example.toml` sandbox section) | Complete |
| Workspace directory creation (`~/.omega/workspace/`) | Complete (in main.rs) |
| Provider working directory confinement | Complete (in omega-providers) |
| System prompt constraint injection | Complete (in gateway.rs) |
| Sandbox mode in `/status` command | Complete (in commands.rs) |
| Startup logging of sandbox mode | Complete (in gateway.rs) |
| Unit tests for SandboxMode | Complete (in omega-core) |
| Process-level isolation | Not started (planned) |
| Runtime enforcement beyond prompt constraints | Not started (planned) |

The core sandbox functionality (mode-based confinement via working directory and system prompt) is complete. The `omega-sandbox` crate itself remains a placeholder for future process-level isolation features.
