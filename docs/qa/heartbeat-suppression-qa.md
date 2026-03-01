# QA Report: Heartbeat Section Suppression Fix

## Scope Validated
- `backend/src/markers/heartbeat.rs` -- section parsing, suppression I/O, filter logic, marker extract/strip
- `backend/src/gateway/heartbeat.rs` -- global and project heartbeat loops (filter integration)
- `backend/src/gateway/process_markers.rs` -- HEARTBEAT_SUPPRESS_SECTION/UNSUPPRESS_SECTION handling in conversation flow
- `backend/src/gateway/heartbeat_helpers.rs` -- same marker handling in heartbeat response flow
- `backend/src/markers/mod.rs` -- safety net marker list
- `backend/src/markers/tests/heartbeat.rs` -- all suppression-related tests
- `specs/bugfixes/heartbeat-trading-suppression-analysis.md` -- requirements spec
- `prompts/SYSTEM_PROMPT.md` -- AI prompt instructions for new markers
- `docs/heartbeat.md` -- user-facing heartbeat documentation

## Summary
**CONDITIONAL APPROVAL** -- All Must requirements (REQ-HB-010 through REQ-HB-014) are fully met. The implementation correctly filters suppressed sections from heartbeat content before the AI provider call, persists suppression state in file-based `.suppress` companions, handles both new markers in all relevant flows, and strips them from output. One Should requirement (REQ-HB-015) is not met: the SYSTEM_PROMPT.md marker quick-reference does not include the new markers, and docs/heartbeat.md does not document the feature. Tests are comprehensive (14 dedicated tests) and all 717 workspace tests pass with zero clippy warnings.

## System Entrypoint
Build and test executed via Nix develop shell:
```bash
cd /Users/isudoajl/ownCloud/Projects/omega/backend
nix --extra-experimental-features "nix-command flakes" develop --command bash -c "cargo test --workspace"
nix --extra-experimental-features "nix-command flakes" develop --command bash -c "cargo clippy --workspace -- -D warnings"
```
Note: This is a background agent service -- it cannot be started interactively for E2E testing without a running Telegram/WhatsApp channel. Validation is code-level and test-level.

## Traceability Matrix Status

| Requirement ID | Priority | Has Tests | Tests Pass | Acceptance Met | Notes |
|---|---|---|---|---|---|
| REQ-HB-010 | Must | Yes | Yes | Yes | `filter_suppressed_sections()` called at lines 157 and 269 of `gateway/heartbeat.rs`, BEFORE classify and provider calls |
| REQ-HB-011 | Must | Yes | Yes | Yes | `read_suppress_file()` / `add_suppression()` / `remove_suppression()` use file I/O at `HEARTBEAT.suppress`. Tested: create, dedupe, remove, missing file |
| REQ-HB-012 | Must | Yes | Yes | Yes | Marker handling in both `process_markers.rs` (lines 339-348) and `heartbeat_helpers.rs` (lines 238-248). Marker stripped from output |
| REQ-HB-013 | Must | Yes | Yes | Yes | `HEARTBEAT_UNSUPPRESS_SECTION` extracted, applied, and stripped in both flows. Unsuppressing non-existent entry is a no-op |
| REQ-HB-014 | Must | Yes | Yes | Yes | `parse_heartbeat_sections()` splits on `## ` headers. Em-dash extraction tested. Preamble handling tested |
| REQ-HB-015 | Should | No | N/A | No | `SYSTEM_PROMPT.md` marker quick-reference (line 53) does NOT include `HEARTBEAT_SUPPRESS_SECTION` / `HEARTBEAT_UNSUPPRESS_SECTION`. AI will not know to use these markers without prompt guidance |
| REQ-HB-018 | Should | Yes | Yes | Yes | `info!()` logging in `add_suppression`, `remove_suppression`, and `filter_suppressed_sections` for skip/stale events |

### Gaps Found
- **REQ-HB-015 (Should)**: No prompt instructions for the AI to emit `HEARTBEAT_SUPPRESS_SECTION` markers. The marker quick-reference in `prompts/SYSTEM_PROMPT.md` line 53 lists `HEARTBEAT_ADD / HEARTBEAT_REMOVE / HEARTBEAT_INTERVAL` but not the new suppress markers. The AI will only use these markers if explicitly taught or if it discovers them from some other context. This is a significant gap -- the whole point of the fix is to let the AI structurally suppress sections rather than using ineffective LESSON markers.
- **Specs index not updated**: `specs/SPECS.md` (line 99-102) does not reference `specs/bugfixes/heartbeat-trading-suppression-analysis.md`.
- **Docs not updated**: `docs/heartbeat.md` does not document the section suppression feature, the `.suppress` companion file, or the new markers.

## Acceptance Criteria Results

### Must Requirements

#### REQ-HB-010: Section-level suppression gate in heartbeat loop
- [x] Given `## TRADING` is suppressed, heartbeat sends only non-trading sections to AI -- **PASS**: `filter_suppressed_sections()` removes matched sections by case-insensitive name comparison (line 314 of `markers/heartbeat.rs`). Test `test_apply_heartbeat_add` (line 444-461) confirms TRADING content and header are filtered, NON-TRADING ITEMS remain.
- [x] Given all sections suppressed, no AI call is made (empty checklist) -- **PASS**: Returns `None` (lines 323-330). Test at line 471 confirms. The `.and_then()` chain in `gateway/heartbeat.rs` line 157 means `None` skips the entire provider call block.
- [x] Filtering happens BEFORE classify step -- **PASS**: `filter_suppressed_sections` is called at line 157, classify at line 162. The filtered `checklist` is what gets classified and executed.

#### REQ-HB-011: Suppression storage in companion file
- [x] Suppressed sections persist across restarts -- **PASS**: Uses file I/O (`std::fs::write`, `std::fs::read_to_string`). No in-memory-only state.
- [x] No suppress file = all sections active -- **PASS**: `read_suppress_file()` returns empty vec on `Err` from `read_to_string` (line 243). `filter_suppressed_sections` returns `Some(content.to_string())` immediately when suppressed list is empty (line 296).
- [x] Location: `~/.omega/prompts/HEARTBEAT.suppress` (global), `~/.omega/projects/<name>/HEARTBEAT.suppress` (per-project) -- **PASS**: `suppress_file_path()` (lines 219-227) produces correct paths for both cases.

#### REQ-HB-012: HEARTBEAT_SUPPRESS_SECTION marker
- [x] `HEARTBEAT_SUPPRESS_SECTION: TRADING` adds "TRADING" to suppress file -- **PASS**: Tested in `test_extract_suppress_section_markers` and integration test (line 413).
- [x] Duplicate adds are no-ops -- **PASS**: `add_suppression()` checks `eq_ignore_ascii_case` before adding (line 254). Test at line 418 confirms.
- [x] Works in both regular conversation and heartbeat response flows -- **PASS**: Verified in `process_markers.rs` lines 339-348 and `heartbeat_helpers.rs` lines 238-248.
- [x] Marker stripped from response before delivery -- **PASS**: `strip_suppress_section_markers()` called after apply in both flows. Also in `strip_all_remaining_markers()` safety net.

#### REQ-HB-013: HEARTBEAT_UNSUPPRESS_SECTION marker
- [x] `HEARTBEAT_UNSUPPRESS_SECTION: TRADING` removes "TRADING" from suppress file -- **PASS**: `remove_suppression()` filters entries case-insensitively (line 277). Tested in integration test (line 426).
- [x] Unsuppressing a non-suppressed section is a no-op -- **PASS**: `remove_suppression()` returns early if `filtered.len() == entries.len()` (line 279). Tested at line 430.

#### REQ-HB-014: Section parsing uses `##` headers
- [x] Parses `##` headers into sections -- **PASS**: Line 195 checks `line.starts_with("## ")`. Tests confirm 2-section, 0-section, no-preamble cases.
- [x] Section name: text before first ` — ` (em dash) -- **PASS**: `extract_section_name()` splits on ` — ` (line 181). Test `test_parse_heartbeat_sections_emdash_extraction` confirms `"## TRADING — Autonomous Quant-Driven Execution Engine"` yields `"TRADING"`.
- [x] Preamble (content before first `##`) is never suppressed -- **PASS**: Preamble is collected separately (line 206) and always included in result (line 311). Test at line 461 confirms `"# Title"` survives filtering.

### Should Requirements

#### REQ-HB-015: Prompt instructions for AI to use HEARTBEAT_SUPPRESS_SECTION over LESSON
- [ ] AI prompt updated with new marker instructions -- **FAIL**: `prompts/SYSTEM_PROMPT.md` line 53 marker quick-reference does not include `HEARTBEAT_SUPPRESS_SECTION` or `HEARTBEAT_UNSUPPRESS_SECTION`. The AI will not know these markers exist unless it discovers them through other means. This undermines the fix's effectiveness: without prompt instructions, the AI will continue to use LESSON markers for suppression requests, which is exactly the behavior that caused the original bug.

#### REQ-HB-018: Logging for suppression events
- [x] Suppression/unsuppression events logged -- **PASS**: `add_suppression()` logs at info level (line 265), `remove_suppression()` logs at info level (line 284), `filter_suppressed_sections()` logs skipped sections (line 316) and stale entries as warnings (line 306).

## End-to-End Flow Results

| Flow | Steps | Result | Notes |
|---|---|---|---|
| Suppress via conversation marker | User says "stop trading" -> AI emits HEARTBEAT_SUPPRESS_SECTION: TRADING -> gateway writes .suppress file -> next heartbeat skips TRADING section | PASS (code path verified) | Cannot run live E2E without Telegram channel. Code path fully traced and all units tested |
| Unsuppress via conversation marker | User says "resume trading" -> AI emits HEARTBEAT_UNSUPPRESS_SECTION: TRADING -> gateway removes from .suppress file -> next heartbeat includes TRADING section | PASS (code path verified) | Same limitation |
| Suppress via heartbeat response | During heartbeat execution, AI emits suppress marker -> heartbeat_helpers processes it -> .suppress file updated for next cycle | PASS (code path verified) | Both flows handle identical marker set |
| All sections suppressed | .suppress contains all section names -> filter returns None -> no AI call made -> "no global checklist configured, skipping" logged | PASS (code path verified) | Correctly prevents wasted API calls |
| Project heartbeat suppression | Per-project .suppress file at `~/.omega/projects/<name>/HEARTBEAT.suppress` -> filter uses project scope -> independent from global | PASS (code path verified) | `suppress_file_path()` correctly routes based on project parameter |

## Exploratory Testing Findings

| # | What Was Tried | Expected | Actual | Severity |
|---|---|---|---|---|
| 1 | Checked if `### Subsection` inside `## TRADING` gets treated as new section | Should remain part of TRADING section body | Correct: only `## ` (h2 with space) starts a new section. `###` stays in current section body | low (no issue) |
| 2 | Checked empty `## ` header (no section name) | Should not crash | Produces empty string section name. No crash. Edge case is benign for real-world heartbeat files | low (no issue) |
| 3 | Checked `.suppress` file with empty lines and whitespace | Should ignore empty entries | `read_suppress_file()` filters with `.filter(\|l\| !l.is_empty())` and trims. Correct | low (no issue) |
| 4 | Checked what happens if `HOME` env var is unset | Suppress file operations should fail gracefully | `suppress_file_path()` returns `None`, all callers handle `None` by returning early or using empty defaults | low (no issue) |
| 5 | Checked if suppressing "TRADING" also suppresses "NON-TRADING ITEMS" via partial match | Should NOT match -- exact section name matching | Correct: uses `eq_ignore_ascii_case` (exact match, not `contains`). "TRADING" does not match "NON-TRADING ITEMS" | low (no issue) |
| 6 | Checked `strip_all_remaining_markers` for inline marker handling | Inline markers (mid-sentence) should still be stripped | Both markers are in the MARKERS list (mod.rs lines 109-110). `strip_inline_marker` handles mid-line markers. Correct | low (no issue) |
| 7 | Checked if stale suppress entries (section renamed in HEARTBEAT.md) are detected | Should log a warning | `filter_suppressed_sections()` line 306 logs `warn!` for unmatched suppress entries. Correct per spec's risk mitigation | low (no issue) |
| 8 | Checked if concurrent suppress file writes could corrupt the file | Potential race condition if two heartbeat responses write simultaneously | `add_suppression` and `remove_suppression` are not atomic -- they read, modify, write back. In practice, heartbeat is single-threaded per cycle, so this is unlikely but theoretically possible if a user conversation marker and a heartbeat response marker fire simultaneously | low (theoretical) |

## Failure Mode Validation

| Failure Scenario | Triggered | Detected | Recovered | Degraded OK | Notes |
|---|---|---|---|---|---|
| Suppress file missing | Yes (test) | Yes | Yes | Yes | `read_suppress_file()` returns empty vec. All sections active. Tested |
| Suppress file unreadable (permissions) | Not Triggered (safe default) | Yes | Yes | Yes | Same code path as missing -- `read_to_string` returns `Err`, falls back to empty vec |
| Suppress file write failure | Not Triggered | Partial | Yes | Yes | `add_suppression` uses `let _ = std::fs::write(...)` -- error is silently ignored. Suppression won't persist but won't crash. The `info!` log still fires before the write, which could be misleading |
| All sections suppressed | Yes (test) | Yes | N/A | Yes | Returns None, heartbeat loop logs "no global checklist configured, skipping" and moves on |
| Stale suppress entries | Yes (code review) | Yes | N/A | Yes | `warn!` log for unmatched entries. Does not crash or block valid sections |
| Heartbeat file deleted mid-cycle | Not Triggered (untestable safely) | Yes | Yes | Yes | `read_heartbeat_file()` returns None, cycle skipped. Existing behavior, not affected by this change |

## Security Validation

| Attack Surface | Test Performed | Result | Notes |
|---|---|---|---|
| Path traversal in section names | Code review of `add_suppression()` | PASS | Section names are written as plain text entries in a line-delimited file. They are not used as file paths. No injection risk |
| Malicious marker injection | Code review of `extract_suppress_section_markers()` | PASS | Markers are extracted line-by-line with prefix matching. The section name is just stored as a string in the .suppress file. No code execution, no SQL, no path construction from user input |
| Suppress file location hijack | Code review of `suppress_file_path()` | PASS | Path is constructed from `$HOME` + hardcoded subpaths. No user-supplied path components (project names come from internal system state, not raw user input to this function) |

## Specs/Docs Drift

| File | Documented Behavior | Actual Behavior | Severity |
|------|-------------------|-----------------|----------|
| `specs/SPECS.md` | Bugfixes section (line 99-102) lists 3 bugfix specs | Missing reference to `specs/bugfixes/heartbeat-trading-suppression-analysis.md` | medium |
| `docs/heartbeat.md` | Documents HEARTBEAT_ADD/REMOVE/INTERVAL markers and HEARTBEAT_OK suppression | Does not document HEARTBEAT_SUPPRESS_SECTION / HEARTBEAT_UNSUPPRESS_SECTION markers or the `.suppress` companion file mechanism | high |
| `prompts/SYSTEM_PROMPT.md` line 53 | Marker quick-reference lists `HEARTBEAT_ADD / HEARTBEAT_REMOVE / HEARTBEAT_INTERVAL` | Missing `HEARTBEAT_SUPPRESS_SECTION / HEARTBEAT_UNSUPPRESS_SECTION` from the quick-reference. AI will not know to use these markers | high |
| `docs/architecture.md` line 377 | Heartbeat description mentions classification and HEARTBEAT_OK suppression | Does not mention section-level suppression or the `.suppress` file | medium |

## Blocking Issues (must fix before merge)
None. All Must requirements pass.

## Non-Blocking Observations

- **[OBS-001]**: `prompts/SYSTEM_PROMPT.md` -- The marker quick-reference (line 53) must include `HEARTBEAT_SUPPRESS_SECTION: name / HEARTBEAT_UNSUPPRESS_SECTION: name`. Without this, the AI has no way to know these markers exist and will continue using ineffective LESSON markers for section suppression -- which is the exact behavior that caused the original bug. Strongly recommended to fix before merge to realize the value of this bugfix.

- **[OBS-002]**: `prompts/SYSTEM_PROMPT.md` -- Should add behavioral guidance near line 10 or line 47 instructing the AI: "When the user asks to stop an entire heartbeat domain (e.g., trading reports), emit HEARTBEAT_SUPPRESS_SECTION: <section-name> instead of using LESSON markers. LESSON markers are advisory and cannot override 400 lines of heartbeat instructions."

- **[OBS-003]**: `docs/heartbeat.md` -- Needs a new section documenting: (a) section suppression mechanism, (b) `.suppress` companion file, (c) how to manually suppress/unsuppress sections, (d) interaction with HEARTBEAT_ADD/REMOVE.

- **[OBS-004]**: `specs/SPECS.md` -- Should add `heartbeat-trading-suppression-analysis.md` to the bugfixes index.

- **[OBS-005]**: `docs/architecture.md` line 377 -- Heartbeat description should mention section-level suppression filtering.

- **[OBS-006]**: `markers/heartbeat.rs` `add_suppression()` line 264 -- `std::fs::write` error is silently discarded with `let _ =`. While this prevents panics, it means suppression could silently fail to persist (e.g., disk full, permissions). Consider logging on write failure.

- **[OBS-007]**: Regarding OBS-001/OBS-002 -- these are categorized as non-blocking because the code infrastructure is correct and the markers work. However, the practical effectiveness of the bugfix depends on the AI knowing to use these markers. If the prompt is not updated, the user will need to manually manage `.suppress` files, which defeats the purpose of the autonomous suppression system.

## Modules Not Validated (if context limited)
- **Live E2E flow**: Cannot start the system interactively (requires active Telegram/WhatsApp channel and configured API keys). All flows verified via code path tracing and unit/integration tests.
- **Concurrent access**: File-based suppress I/O is not atomic. Theoretical race condition between conversation marker processing and heartbeat response marker processing writing to the same `.suppress` file simultaneously. Low risk in practice since heartbeat cycles are spaced apart.

## Final Verdict

**CONDITIONAL APPROVAL** -- All Must requirements (REQ-HB-010 through REQ-HB-014) are met. The code correctly implements section-level suppression filtering in the heartbeat pipeline, with file-based persistence, marker handling in both conversation and heartbeat response flows, proper stripping, case-insensitive matching, and comprehensive test coverage (14 dedicated tests, 717 total tests passing). No blocking issues found.

The following Should requirement failed and is tracked as non-blocking:
- **REQ-HB-015**: `SYSTEM_PROMPT.md` does not include the new markers in the marker quick-reference or behavioral guidance. This significantly reduces the practical effectiveness of the fix -- the AI infrastructure is ready but the AI does not know to use it. Strongly recommended to resolve before merge.
