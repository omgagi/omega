# Feature Evaluation: `omega uninstall` CLI Command

## Feature Description

Add a top-level `omega uninstall` CLI command that offers two modes:
1. **Complete removal** -- delete everything related to Omega without leaving a single trace (config files, data directory `~/.omega`, system service, binary, logs, stores, workspace, prompts, skills, projects, etc.)
2. **Keep configuration** -- remove everything except configuration files so the user can reinstall later without reconfiguring

## Evaluation Summary

| Dimension | Score (1-5) | Assessment |
|-----------|-------------|------------|
| D1: Necessity | 4 | Real gap: Omega scatters files across `~/.omega/`, service files, and `/usr/local/bin/omega`. No clean way to undo an installation today. |
| D2: Impact | 3 | Useful for the install/uninstall lifecycle, but used rarely (once per user per uninstall). Does not affect daily usage. |
| D3: Complexity Cost | 5 | Isolated addition: one new subcommand variant in `main.rs`, one new module (~100-150 lines). Reuses existing `service::uninstall()` and `cliclack` patterns. No cross-cutting changes. |
| D4: Alternatives | 4 | Manual removal (`rm -rf ~/.omega && omega service uninstall && rm /usr/local/bin/omega`) works but requires knowing all paths. No external tool handles this. The "keep config" mode has no manual equivalent without careful path selection. |
| D5: Alignment | 5 | Perfect fit. Omega already has `init`, `setup`, `service install`, and `service uninstall`. An `uninstall` command completes the lifecycle. Aligns with "less is more" -- one command replaces needing to know 5+ paths. |
| D6: Risk | 4 | Destructive operation, but mitigated by interactive confirmation via `cliclack`. Isolated module, cannot break existing functionality. Only risk: accidentally deleting user data without adequate warning, which is addressed by the two-mode design and confirmation prompts. |
| D7: Timing | 5 | No prerequisites missing. No conflicts with in-progress work. Project is stable (clean main branch). The service module (`backend/src/service.rs`) already provides the `stop_service()` and `service_file_path()` helpers this feature needs. |

**Feature Viability Score: 4.2 / 5.0**

```
FVS = (D1:4 + D2:3 + D5:5) x 2 + (D3:5 + D4:4 + D6:4 + D7:5)
    = (12) x 2 + (18)
    = 24 + 18
    = 42 / 10
    = 4.2
```

## Verdict: GO

This feature fills a genuine lifecycle gap in the CLI. Omega creates files in at least 5 distinct locations (`~/.omega/`, service files, symlink binary, logs, stores), and today a user must know all of them to cleanly uninstall. The implementation is small and isolated -- a single new module that reuses existing patterns. The two-mode design (complete vs. keep-config) is well-thought-out and covers the two most common uninstall scenarios.

## Detailed Analysis

### What Problem Does This Solve?

Omega's `init` and `service install` commands scatter state across multiple filesystem locations:

- **Data directory**: `~/.omega/` (config.toml, data/memory.db, logs/, workspace/, stores/, prompts/, skills/, projects/)
- **Service file**: `~/Library/LaunchAgents/com.omega-cortex.omega.plist` (macOS) or `~/.config/systemd/user/omega.service` (Linux)
- **Binary symlink**: `/usr/local/bin/omega` -> `~/.cargo/target-global/release/omega`
- **WhatsApp session**: `{data_dir}/whatsapp_session/` (per `backend/crates/omega-core/src/config/channels.rs:26`)

A user who wants to cleanly remove Omega currently must: (1) run `omega service uninstall`, (2) manually delete `~/.omega/`, (3) remove `/usr/local/bin/omega`, and (4) know to check for any other scattered artifacts. This is error-prone and undocumented. The feature solves this by providing a single command that handles complete cleanup.

### What Already Exists?

1. **`omega service uninstall`** (`backend/src/service.rs:207-229`): Removes only the service file (LaunchAgent/systemd unit) and stops the running service. Does NOT touch `~/.omega/`, the binary, or any data.
2. **`service::stop_service()`** (`backend/src/service.rs:263-276`): Helper to stop/unload the service. Can be reused directly.
3. **`service::service_file_path()`** (`backend/src/service.rs:107-126`): Returns the OS-appropriate service file path. Can be reused directly.
4. **`cliclack` UX patterns**: Used throughout `init.rs`, `service.rs`, `pair.rs`. The uninstall command should follow the same interactive confirmation style.
5. **No existing uninstall, cleanup, or self-removal logic exists** anywhere in the codebase (confirmed via grep for `uninstall|cleanup|remove|purge|clean` across `backend/src/`).

### Complexity Assessment

**Estimated scope**: 1 new file (`backend/src/uninstall.rs`, ~100-150 lines), plus ~15 lines of changes in `main.rs` (new `Commands::Uninstall` variant and match arm).

**What needs to change**:
1. `backend/src/main.rs` -- add `Uninstall` variant to `Commands` enum, add `mod uninstall`, add match arm (~15 lines)
2. `backend/src/uninstall.rs` -- new module implementing the two-mode uninstall logic (~100-150 lines)

**Reusable infrastructure**:
- `service::stop_service()` and `service::service_file_path()` for service cleanup
- `cliclack::confirm()`, `cliclack::select()`, `cliclack::spinner()` for interactive UX
- `omega_core::shellexpand()` for path expansion
- `config::load()` to read `data_dir` from config before deleting it

**Maintenance burden**: Near-zero. The module is purely additive and isolated. The only maintenance trigger would be if new artifact locations are added to Omega in the future (e.g., a new cache directory), which would require updating the uninstall path list. This is a low-frequency concern.

### Risk Assessment

- **Data loss risk**: The feature is inherently destructive, but this is by design. Mitigated by: (a) requiring explicit user confirmation via `cliclack::confirm()`, (b) offering the "keep configuration" mode as a safety net, (c) displaying exactly what will be deleted before proceeding.
- **Binary self-deletion**: The running binary cannot delete itself on some platforms. The standard pattern is to delete the symlink (`/usr/local/bin/omega`) rather than the actual binary in `~/.cargo/target-global/`, which sidesteps this issue. The actual compiled binary under the cargo target directory is not Omega's responsibility to manage.
- **No existing tests break**: This is purely additive -- no existing code paths change.
- **Security**: No new attack surface. The command runs as the current user and only deletes files the user owns.

## Conditions

None -- feature approved for pipeline entry.

## Alternatives Considered

- **Manual removal via documentation**: Could document the 4-5 paths users need to delete. Pros: zero code. Cons: error-prone, users must find and follow docs, the "keep config" mode is hard to document clearly, and the path list will drift as the project evolves.
- **Shell script (`scripts/uninstall.sh`)**: Pros: simpler than compiled code. Cons: not discoverable via `omega --help`, not cross-platform (would need separate macOS/Linux versions), cannot reuse Rust helpers like `service_file_path()`, and breaks the project's convention of using `cliclack` for all CLI interactions.
- **Extend `omega service uninstall` with `--all` flag**: Pros: reuses existing subcommand. Cons: semantically wrong -- "service uninstall" is about the service, not about removing Omega entirely. Conflating these concepts would be confusing.

## Recommendation

Proceed with implementation. This is a clean, well-scoped feature that completes Omega's CLI lifecycle (init -> setup -> start -> uninstall). The implementation is small (~150 lines), isolated (one new module + minor `main.rs` changes), and reuses existing infrastructure. No architectural concerns.

The analyst should pay attention to:
- Ensuring the confirmation UX is unambiguous (users must understand that "complete removal" is irreversible)
- Deciding whether to delete the binary symlink at `/usr/local/bin/omega` or just the `~/.omega/` directory (the symlink deletion may require elevated permissions depending on how it was created)
- Handling the edge case where config.toml is not at the default path (the `--config` flag should be respected)
- Whether the `omg-gog` binary at `/usr/local/bin/omg-gog` (installed by `init_google.rs:93-104`) should also be cleaned up

## User Decision

[Awaiting user response]
