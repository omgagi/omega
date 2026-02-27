# Bugfix Analysis: Builds Module Audit 2026-02-27

> Source: `docs/audits/audit-builds-2026-02-27.md`

## Scope

Code bugs and safety issues found in the builds pipeline audit. Spec drift fixes are bundled since they touch the same files.

## Findings to Fix

### BUG-C1 [Must]: AgentFilesGuard race condition on concurrent builds

**Root cause:** `AgentFilesGuard` writes to a shared directory (`~/.omega/workspace/.claude/agents/`) with no reference counting. When the first build finishes, `Drop` unconditionally deletes all agent files, breaking any concurrent build mid-flight.

**Impact:** Any multi-user deployment is unsafe — concurrent builds will fail at random phases.

**Fix:** Add a static `Arc<AtomicUsize>` reference counter. `write()` increments on creation. `Drop` decrements and only deletes when counter reaches zero.

**AC:** Two guards for the same directory can coexist. Files are only deleted when the last guard is dropped.

### BUG-M2 [Must]: Project name validation allows dangerous characters

**Root cause:** `parse_project_brief()` only rejects path traversal (`/`, `\`, `..`, leading `.`) but accepts spaces, shell metacharacters, unlimited length, and unicode control characters.

**Impact:** A malicious or poorly-formatted LLM response could create directories with problematic names that break shell commands or leak data.

**Fix:** Restrict to `[a-zA-Z0-9][a-zA-Z0-9_-]{0,63}` — alphanumeric start, hyphens/underscores allowed, max 64 chars.

**AC:** Names with spaces, semicolons, pipes, backticks, and >64 chars are rejected. Existing valid names (kebab-case, snake_case) still pass.

### BUG-M4 [Must]: Recursive fs traversal has no depth limit and ignores symlinks

**Root cause:** `has_files_matching()` recurses without a depth cap and uses `is_dir()` which follows symlinks, creating infinite loop risk.

**Impact:** Symlink cycles or deeply nested dirs cause stack overflow (panic in production).

**Fix:** Add `max_depth` parameter (default 10). Skip entries where `path.is_symlink()` is true.

**AC:** Recursion stops at depth 10. Symlink directories are not followed.

### BUG-m1 [Should]: `unreachable!()` violates no-panic rule

**Root cause:** `run_qa_loop()` and `run_review_loop()` end with `unreachable!()` which panics if reached.

**Impact:** A refactoring mistake could cause a production panic instead of a graceful error.

**Fix:** Replace with `Err("loop terminated without resolution".to_string())`.

**AC:** No `unreachable!()` in production code in builds_loop.rs.

### BUG-m3 [Should]: Full server paths leaked in user-facing messages

**Root cause:** Error messages include `project_dir_str` which contains the full path like `/Users/isudoajl/.omega/workspace/builds/...`.

**Impact:** Reveals server username and filesystem layout to Telegram/WhatsApp users.

**Fix:** Use project name in user messages. Log full path to audit only.

**AC:** No user-facing message contains `/Users/` or `/.omega/`. Full paths still logged in audit entries.

### SPEC-M1 [Should]: Spec drift — all phases use model_complex

**Root cause:** Commit `4a471b0` changed all phases to use Opus but never updated the spec.

**Fix:** Update `specs/improvements/build-agent-pipeline-improvement.md` REQ-BAP-015 to document the intentional decision.

### SPEC-SD1-SD8 [Should]: Multiple stale specs/docs entries

**Fix:** Update `specs/src-gateway-rs.md`, `specs/SPECS.md`, `specs/improvements/build-agent-pipeline-improvement.md`, `docs/DOCS.md` to match current code.

## Out of Scope

- M3 (orchestration tests) — requires mock Provider infrastructure, tracked separately
- m2 (dead code on ProjectBrief) — cosmetic, fields reserved for future use
- m4 (per-phase audit logging) — enhancement, not a bug
- m5 (phase timing) — enhancement, not a bug
- m6 (exponential backoff) — enhancement, not a bug
