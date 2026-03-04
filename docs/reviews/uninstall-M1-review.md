# Code Review: omega uninstall (Milestone M1)

## Scope Reviewed
- `backend/src/uninstall.rs` (new, ~380 non-test lines + tests)
- `backend/src/main.rs` (CLI integration)
- `backend/src/service.rs` (visibility changes)
- `specs/uninstall-requirements.md`
- `specs/uninstall-architecture.md`
- `specs/SPECS.md`
- `docs/qa/uninstall-M1-qa-report.md`

## Verdict: APPROVED

All 14 requirements (REQ-UNINST-001 through REQ-UNINST-014) implemented and verified.

## Findings Addressed Post-Review

### 1. Broken symlink detection (Major — FIXED)
`Path::exists()` follows symlinks, so dangling symlinks were invisible. Fixed by using `std::fs::symlink_metadata().is_ok()` in both `scan_artifacts()` and `step_remove_symlink()`. Added regression test `test_step_remove_symlink_dangling_symlink`.

### 2. Unnecessary `msg.clone()` (Minor — FIXED)
Reordered calls: `omega_warning(&msg)` first, then `result.warn(msg)` by move. Eliminated 7 unnecessary allocations.

## Non-Blocking Observations
- `step_stop_service` reports success unconditionally (pre-existing `service.rs` behavior)
- `ArtifactEntry.path` has `#[allow(dead_code)]` (only used in tests)
- Keep-config mode may leave empty `~/.omega/` if config.toml doesn't exist (by design)

## Code Quality

| Criterion | Status |
|-----------|--------|
| No `unwrap()` in non-test code | PASS |
| No `unsafe` | PASS |
| Under 500 lines (non-test) | PASS (383 lines) |
| Partial failure tolerance | PASS |
| Architecture alignment | PASS |

## Validation Results
- Build: clean
- Clippy: 0 warnings
- Tests: 1,061 passed (10 new uninstall tests)
- Fmt: clean
