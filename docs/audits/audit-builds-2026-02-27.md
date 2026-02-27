# Code Review: Builds Module Audit

## Scope Reviewed

**Files reviewed (in full):**
- `backend/src/gateway/builds.rs` (493 lines)
- `backend/src/gateway/builds_loop.rs` (329 lines)
- `backend/src/gateway/builds_parse.rs` (1462 lines)
- `backend/src/gateway/builds_agents.rs` (1368 lines)
- `backend/src/gateway/mod.rs` (builds-related lines only)

**Specs reviewed:**
- `specs/SPECS.md`
- `specs/src-gateway-rs.md`
- `specs/improvements/build-agent-pipeline-improvement.md`
- `specs/improvements/build-pipeline-safety-controls.md`
- `specs/improvements/builds-routing-improvement.md`
- `specs/improvements/build-discovery-phase-improvement.md`

**Docs reviewed:**
- `docs/DOCS.md`
- `docs/functionalities/builds-functionalities.md`

## Summary

| Severity | Count |
|----------|-------|
| CRITICAL | 1 |
| MAJOR | 4 |
| MINOR | 6 |
| INFO | 3 |
| Specs/Docs Drift | 8 |

**Overall status: Requires changes**

The builds module has clean separation of concerns and excellent test coverage on pure parsing functions. However, it has one critical concurrency bug, significant spec drift, zero test coverage on orchestration logic, and incomplete input validation.

---

## Critical Findings

### C1. [CRITICAL] AgentFilesGuard race condition with concurrent builds from different users

**Location:** `builds_agents.rs` lines 416-441, `builds.rs` lines 50-51

`AgentFilesGuard` writes agent files to a shared directory (`~/.omega/workspace/.claude/agents/`) and deletes them on `Drop`. The `active_senders` mutex only prevents concurrent requests from the *same* sender. Two different users can start builds simultaneously:

1. User A starts build at T=0, guard writes agent files to `~/.omega/workspace/.claude/agents/`
2. User B starts build at T=1s, guard writes same files (same directory, same content -- no corruption)
3. User A's build finishes at T=120s, guard drops, calls `std::fs::remove_dir_all(.claude/agents/)`
4. User B's build is at phase 4, starts phase 5 at T=125s -- `claude --agent build-qa` fails because files no longer exist

The `AgentFilesGuard` struct at line 416-418:
```rust
pub(super) struct AgentFilesGuard {
    agents_dir: PathBuf,
}
```

And its `Drop` at line 433-441:
```rust
impl Drop for AgentFilesGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.agents_dir);
        if let Some(claude_dir) = self.agents_dir.parent() {
            let _ = std::fs::remove_dir(claude_dir);
        }
    }
}
```

No reference counting, no coordination between guards. The drop unconditionally deletes.

**Suggested fix:** Add an `Arc<AtomicUsize>` reference counter. `write()` increments, `Drop` decrements. Only delete files when counter reaches zero.

---

## Major Findings

### M1. [MAJOR] All 7 phases use `model_complex` (Opus) -- deviates from spec without spec update

**Location:** `builds.rs` (all `run_build_phase` calls use `&self.model_complex`), `builds_loop.rs` (same)

Every phase invocation passes `&self.model_complex`. Spec REQ-BAP-015 states: "Phase 1-2: model_complex; Phases 3-7: model_fast". Commit `4a471b0` changed this intentionally but the spec was never updated. Using Opus for all 7 phases significantly increases cost and latency.

**Suggested fix:** Update the spec to document this decision with rationale, or revert to the spec-defined model routing.

### M2. [MAJOR] Project name validation allows spaces, shell metacharacters, and unlimited length

**Location:** `builds_parse.rs` lines 91-101

```rust
let name = name.trim_matches('`').trim().to_string();
if name.is_empty()
    || name.contains('/')
    || name.contains('\\')
    || name.contains("..")
    || name.starts_with('.')
{
    return None;
}
```

This rejects path traversal characters but accepts:
- Spaces: `"my cool project"` creates `~/.omega/workspace/builds/my cool project/`
- Shell metacharacters: `"test;rm -rf /"` would be accepted as a valid name
- Unlimited length: a 10,000-character name would be accepted
- Special Unicode: zero-width characters, RTL overrides

The analyst agent prompt says "Keep the project name short and snake-case (max 3 words)" but the parser does not enforce this.

**Suggested fix:** Validate with `[a-z0-9][a-z0-9_-]{0,63}` regex -- lowercase alphanumeric with hyphens/underscores, max 64 characters.

### M3. [MAJOR] No tests for core orchestration functions

**Location:** `builds.rs`, `builds_loop.rs`

The following functions have zero test coverage:

| Function | Lines | Complexity |
|----------|-------|-----------|
| `handle_build_request()` | 380 | 7 phases, 10+ error paths, typing handle cleanup |
| `run_build_phase()` | 27 | Retry loop, context creation |
| `run_qa_loop()` | 55 | 3-iteration loop with developer re-invocation |
| `run_review_loop()` | 60 | 2-iteration loop with developer re-invocation |
| `audit_build()` | 27 | Status mapping |

**Suggested fix:** Create a mock `Provider` that returns canned responses and test the full orchestration flow.

### M4. [MAJOR] Synchronous recursive filesystem traversal in async context without depth limit

**Location:** `builds_loop.rs` lines 183-204

```rust
fn has_files_matching(dir: &Path, patterns: &[&str]) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    // recursive traversal...
}
```

Issues:
1. Uses `std::fs::read_dir()` (blocking I/O) called from an async context -- could block the tokio runtime thread
2. No recursion depth limit -- deeply nested directories or symlink loops would cause stack overflow
3. Does not detect symlinks -- a symlink cycle causes infinite recursion

**Suggested fix:** Add a max recursion depth parameter (e.g., 10). Use `tokio::task::spawn_blocking()` for the entire validation check. Consider `path.is_symlink()` to skip symlinks.

---

## Minor Findings

### m1. [MINOR] `unreachable!()` macro violates project's no-panic rule

**Location:** `builds_loop.rs` lines 73 and 139

Both `run_qa_loop()` and `run_review_loop()` end with `unreachable!()`. The project's CLAUDE.md states: "No `unwrap()` -- use `?` and proper error types. Never panic in production code."

**Suggested fix:** Replace with `Err("loop completed without resolution".to_string())`.

### m2. [MINOR] `#[allow(dead_code)]` on ProjectBrief struct

**Location:** `builds_parse.rs` line 16

Four out of six fields (`language`, `database`, `frontend`, `components`) are parsed but unused.

### m3. [MINOR] Filesystem paths leaked in user-facing messages

**Location:** `builds.rs` lines 209, 254, 372, 405

Error messages send the full server path (e.g., `/Users/isudoajl/.omega/workspace/builds/my-tool`) to the user's Telegram/WhatsApp channel. This reveals the server's username and filesystem structure.

**Suggested fix:** Use relative paths or project name only in user-facing messages. Log full paths to audit.

### m4. [MINOR] No per-phase audit logging

**Location:** `builds.rs`

`audit_build()` is called only on final success and on QA/review exhaustion. Individual phase completions/failures are not logged.

### m5. [MINOR] No phase timing measurement

**Location:** `builds.rs`

No `Instant::now()` / elapsed timing exists. Build phases can take 1-10 minutes each. Without timing data, impossible to identify bottleneck phases.

### m6. [MINOR] Fixed 2-second retry delay without exponential backoff

**Location:** `builds.rs` line 457

Flat 2s delay between all retry attempts. For rate-limit errors, exponential backoff would be more effective.

---

## Info Notes

### I1. [INFO] `max_turns` defaults to 100 when not specified

Most phases pass `None` for max_turns, which resolves to 100. This is a deliberate safety cap but is undocumented.

### I2. [INFO] All file line counts are within the 500-line rule (excluding tests)

| File | Non-test lines |
|------|---------------|
| `builds.rs` | 493 |
| `builds_loop.rs` | 245 |
| `builds_parse.rs` | 490 |
| `builds_agents.rs` | 444 |

### I3. [INFO] No `unwrap()`, `unsafe`, `TODO`, `HACK`, or `FIXME` in production code

All `unwrap()` calls are exclusively in `#[cfg(test)]` modules. Clean on standard code quality patterns.

---

## Specs/Docs Drift

### SD1. `specs/src-gateway-rs.md` -- Describes old 5-phase pipeline

The `handle_build_request()` spec still says "runs 5 sequential phases" with `model_fast` for phases 3-5. The code runs 7 phases, all with `model_complex`, using `--agent` flag.

### SD2. `specs/src-gateway-rs.md` -- `phase_message()` description outdated

Says "Supports English, Spanish, Portuguese with specific strings for phases 1 and 5". The code supports all 8 languages for all 7 phases.

### SD3. `specs/src-gateway-rs.md` -- Missing detailed function specs

No per-function specs for: `run_qa_loop()`, `run_review_loop()`, `validate_phase_output()`, `save_chain_state()`, `has_files_matching()`, `parse_review_result()`, `parse_discovery_output()`, `parse_discovery_round()`, `discovery_file_path()`, `truncate_brief_preview()`, `DiscoveryOutput`, `ReviewResult`, `ChainState`.

### SD4. `specs/src-gateway-rs.md` -- Stale line counts

`builds_parse.rs`: spec says ~395, actual ~490 (non-test). `builds_agents.rs`: spec says ~300, actual ~444 (non-test).

### SD5. `specs/SPECS.md` -- Missing discovery improvement spec entry

`specs/improvements/build-discovery-phase-improvement.md` exists on disk but is not listed in the `SPECS.md` Improvements section.

### SD6. `specs/improvements/build-agent-pipeline-improvement.md` REQ-BAP-015 -- Stale

Spec says phases 3-7 use `model_fast`. Code uses `model_complex` for all phases since commit `4a471b0`.

### SD7. `specs/improvements/build-agent-pipeline-improvement.md` -- BUILD_AGENTS count stale

Spec says "7 build agent definitions". Code has 8 (7 pipeline + 1 discovery).

### SD8. `docs/DOCS.md` -- No reference to build discovery functionality

The discovery session, state file, and cancel logic are implemented but not referenced in DOCS.md.

---

## Final Verdict

**Requires iteration.** The critical concurrency bug (C1 -- AgentFilesGuard race) must be fixed before multi-user deployments are safe. The spec drift (SD1-SD8) should be addressed to prevent confusion during future development. The missing orchestration tests (M3) represent the largest long-term risk.
