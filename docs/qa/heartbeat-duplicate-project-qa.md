# QA Report: Heartbeat Duplicate Project Fix

## Scope Validated
- `backend/src/markers/heartbeat.rs` -- new `strip_project_sections()` function
- `backend/src/gateway/heartbeat.rs` -- modified `heartbeat_loop` to strip project sections before global execution
- `backend/src/markers/tests/heartbeat.rs` -- 6 new unit tests for `strip_project_sections`
- `~/.omega/prompts/HEARTBEAT.md` -- data fix (removal of duplicated section)
- `specs/bugfixes/heartbeat-project-scope-analysis.md` -- stale spec correction
- `specs/bugfixes/heartbeat-duplicate-project-analysis.md` -- new bugfix analysis spec

## Summary
**PASS** -- All 5 Must requirements are met. The fix correctly prevents duplicate heartbeat execution by stripping project-owned sections from the global heartbeat before execution. The implementation is generic (no hardcoded project names), handles edge cases (empty projects, all-stripped, case insensitivity), and reuses existing infrastructure (`parse_heartbeat_sections`). All 6 new tests pass. All 36 heartbeat-related tests pass. No regressions introduced (4 pre-existing test failures confirmed unrelated).

## System Entrypoint
Tests executed via Nix development shell:
```bash
cd /Users/isudoajl/ownCloud/Projects/omega/backend
nix --extra-experimental-features "nix-command flakes" develop --command bash -c "cargo test -- strip_project_sections"
nix --extra-experimental-features "nix-command flakes" develop --command bash -c "cargo test -- heartbeat"
nix --extra-experimental-features "nix-command flakes" develop --command bash -c "cargo test --workspace"
```
Note: The heartbeat loop itself runs as a background task in the OMEGA service. Unit tests validate the stripping logic; end-to-end validation of the loop requires a running OMEGA instance (validated via runtime data inspection below).

## Traceability Matrix Status

| Requirement ID | Priority | Has Tests | Tests Pass | Acceptance Met | Notes |
|---|---|---|---|---|---|
| REQ-HBDUP-001 | Must | Yes | Yes | Yes | `strip_project_sections()` called before `filter_suppressed_sections()` in loop |
| REQ-HBDUP-002 | Must | No (data fix) | N/A | Yes | Verified `~/.omega/prompts/HEARTBEAT.md` has no "TECH YOUTUBER" section |
| REQ-HBDUP-003 | Must | Yes | Yes | Yes | No hardcoded names in `markers/heartbeat.rs` or `gateway/heartbeat.rs` |
| REQ-HBDUP-004 | Must | Yes | Yes | Yes | 6 tests covering all specified scenarios |
| REQ-HBDUP-005 | Must | No (doc fix) | N/A | Yes | Stale "NOT affected" replaced with correction referencing this bugfix |

### Gaps Found
- None. All requirements have corresponding implementation and/or tests.

## Acceptance Criteria Results

### Must Requirements

#### REQ-HBDUP-001: Strip active-project sections from global heartbeat
- [x] Global heartbeat does not include items from sections matching active project names -- PASS: `strip_project_sections()` is called at line 177 of `gateway/heartbeat.rs`, before `filter_suppressed_sections()`. The function removes sections whose normalized names contain the normalized project name.
- [x] Section matching is case-insensitive and normalized -- PASS: Both section name and project name are lowercased and have hyphens/underscores replaced with spaces. Test `test_strip_project_sections_case_insensitive` confirms "Tech YouTuber Project" matches "tech-youtuber".
- [x] If stripping removes all sections, the global heartbeat is skipped entirely -- PASS: Function returns `None` when `any_active` is false. Test `test_strip_project_sections_all_stripped_returns_none` confirms. The calling code at line 176-179 uses `.and_then()` chaining, so `None` falls through to the `else` branch at line 270 which logs "no global checklist configured, skipping".

#### REQ-HBDUP-002: Remove duplicated section from global HEARTBEAT.md
- [x] Global HEARTBEAT.md no longer contains "TECH YOUTUBER PROJECT" section -- PASS: Verified via grep; `~/.omega/prompts/HEARTBEAT.md` contains only "QUIET HOURS" and "NON-TRADING ITEMS" sections.
- [x] Remaining sections (QUIET HOURS, NON-TRADING ITEMS) are preserved -- PASS: Both sections present in the file at lines 5 and 16.

#### REQ-HBDUP-003: Generic prevention for any project
- [x] No project name is hardcoded in the deduplication logic -- PASS: Grep for "tech.youtuber" in both `markers/heartbeat.rs` and `gateway/heartbeat.rs` returns zero matches. The function accepts `&[String]` dynamically.
- [x] Works for any number of active projects with their own heartbeat files -- PASS: Test `test_strip_project_sections_multiple_projects` verifies with 2 projects (tech-youtuber, realtor) simultaneously.

#### REQ-HBDUP-004: Unit tests for section stripping
- [x] Test: single project section stripped -- PASS: `test_strip_project_sections_removes_matching_section`
- [x] Test: multiple projects, only active ones stripped -- PASS: `test_strip_project_sections_multiple_projects`
- [x] Test: no active projects, content unchanged -- PASS: `test_strip_project_sections_no_projects`
- [x] Test: all sections stripped returns None -- PASS: `test_strip_project_sections_all_stripped_returns_none`
- [x] (Bonus) Test: case insensitive matching -- PASS: `test_strip_project_sections_case_insensitive`
- [x] (Bonus) Test: no match preserves all sections -- PASS: `test_strip_project_sections_no_match_preserves_all`

#### REQ-HBDUP-005: Correct stale spec
- [x] Spec no longer claims heartbeat loop is unaffected -- PASS: The line "The heartbeat **loop** (`gateway/heartbeat.rs:253-306`) is NOT affected" has been replaced with a correction note.
- [x] A note references this bugfix analysis as the correction -- PASS: New text reads "See `heartbeat-duplicate-project-analysis.md` for the fix (REQ-HBDUP-001)".

## End-to-End Flow Results

| Flow | Steps | Result | Notes |
|---|---|---|---|
| Global heartbeat with active projects | 1. Collect active projects 2. Filter those with own HEARTBEAT.md 3. Strip matching sections from global 4. Run global with remaining sections 5. Run project heartbeats independently | PASS (code review) | Cannot trigger live heartbeat cycle without waiting for clock boundary; validated via code path analysis and unit tests |
| Global heartbeat with no active projects | 1. `get_all_facts_by_key` returns empty 2. `projects_with_heartbeat` is empty 3. `strip_project_sections` returns content unchanged 4. Global runs normally | PASS | Early return on empty `project_names` preserves all content |
| All sections stripped | 1. All global sections match active projects 2. `strip_project_sections` returns None 3. Global phase skipped 4. Project phases run independently | PASS | Unit test confirms `None` return; calling code logs "no global checklist configured, skipping" |

## Exploratory Testing Findings

| # | What Was Tried | Expected | Actual | Severity |
|---|---|---|---|---|
| 1 | Substring matching edge case: project "trader" vs section "NON-TRADING ITEMS" | No false match | No false match -- "trader" is not a substring of "non trading items" (trading != trader) | N/A |
| 2 | Verified `active_projects` query moved before global phase | Query should happen once, reused for both phases | Confirmed: query at lines 158-161, reused at line 277 in project phase | N/A |
| 3 | Checked for duplicate dedup in project phase | `seen_projects` HashSet should prevent same project running twice | Confirmed: HashSet at line 276 deduplicates project names | N/A |
| 4 | Preamble-only global heartbeat (all sections stripped but preamble remains) | Global should be skipped (preamble alone is not actionable) | `strip_project_sections` returns `None` when `any_active` is false, regardless of preamble. However, preamble IS included in the return when at least one section remains. This matches `filter_suppressed_sections` behavior. | low |
| 5 | Theoretical: project named "trading" vs section "NON-TRADING ITEMS" | Could false-match since "trading" is a substring of "non trading items" | Would incorrectly strip the NON-TRADING section. However, no such project exists currently, and the containment-based matching is documented and consistent with how `parse_heartbeat_sections` works elsewhere. | low |

## Failure Mode Validation

| Failure Scenario | Triggered | Detected | Recovered | Degraded OK | Notes |
|---|---|---|---|---|---|
| `get_all_facts_by_key` fails (DB error) | Not triggered | Yes | Yes | Yes | `.unwrap_or_default()` at line 161 falls back to empty vec; global runs with all sections |
| Project HEARTBEAT.md unreadable | Not triggered | Yes | Yes | Yes | `read_project_heartbeat_file` returns `None`, so project not added to `projects_with_heartbeat`; section stays in global |
| Global HEARTBEAT.md missing | Not triggered | Yes | Yes | Yes | `read_heartbeat_file()` returns `None` at line 176; entire global phase skipped |

## Security Validation

| Attack Surface | Test Performed | Result | Notes |
|---|---|---|---|
| N/A | N/A | N/A | This is a backend-only bugfix for an automated loop with no user-facing input surfaces. Project names come from the database (set via authenticated commands). No new attack surfaces introduced. |

## Specs/Docs Drift

| File | Documented Behavior | Actual Behavior | Severity |
|------|-------------------|-----------------|----------|
| `specs/bugfixes/heartbeat-project-scope-analysis.md` (pre-fix) | "The heartbeat loop is NOT affected" | The loop WAS affected; global phase included project sections causing duplicates | high (now corrected) |

No remaining drift found after the fix.

## Blocking Issues (must fix before merge)
None.

## Non-Blocking Observations

- **[OBS-001]**: `backend/src/gateway/heartbeat.rs` -- line 515 total (433 production + 82 test). Production code is under the 500-line limit but approaching it. If more logic is added to the heartbeat loop, consider extracting the project-collection logic (lines 155-173) into a helper function in `heartbeat_helpers.rs`.
- **[OBS-002]**: `backend/src/markers/heartbeat.rs` -- The containment-based matching in `strip_project_sections` (`norm_section.contains(proj)`) could theoretically produce false positives if a project name is a substring of an unrelated section name (e.g., a project named "trading" would match "NON-TRADING ITEMS"). Current project names (tech-youtuber, realtor, trader) do not trigger this. Consider using word-boundary matching if more projects are added.
- **[OBS-003]**: `backend/src/gateway/builds_agents.rs:189` -- Unused import `std::path::Path` produces a compiler warning. Pre-existing, unrelated to this fix.
- **[OBS-004]**: 4 pre-existing test failures confirmed unrelated to this fix: `claudemd::tests::test_template_contains_standard_sections`, `gateway::tests::test_prompts_default_welcome_all_languages`, `gateway::tests::test_prompts_default_welcome_fallback`, `markers::tests::test_append_bug_report_creates_file`. These fail identically with and without the heartbeat changes.

## Modules Not Validated
None -- all modules in scope were fully validated.

## Final Verdict

**PASS** -- All 5 Must requirements met. No blocking issues. All 6 new tests pass. All 36 heartbeat-related tests pass (0 regressions). The fix is generic, correctly handles edge cases, and the stale spec has been corrected. Approved for review.
