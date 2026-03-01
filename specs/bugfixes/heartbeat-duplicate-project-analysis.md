# Bugfix Analysis: Duplicate Project Heartbeat

**Date:** 2026-03-01
**Severity:** Medium — duplicate messages sent to user every heartbeat cycle
**Scope:** `backend/src/gateway/heartbeat.rs` (heartbeat loop, global phase)

---

## Bug Description

OMEGA sends the tech-youtuber heartbeat report TWICE every hourly cycle. The user has 3 projects (realtor, tech-youtuber active, trader) and receives duplicate tech-youtuber reports at each heartbeat boundary.

## Root Cause

The heartbeat loop in `gateway/heartbeat.rs` runs two phases sequentially:

1. **Global heartbeat** (lines 155-251): Reads `~/.omega/prompts/HEARTBEAT.md` which contains a `## TECH YOUTUBER PROJECT` section with 3 checklist items.
2. **Project heartbeats** (lines 253-306): Iterates `get_all_facts_by_key("active_project")`, finds tech-youtuber is active, reads `~/.omega/projects/tech-youtuber/HEARTBEAT.md` which has the **same 3 items**.

Result: tech-youtuber items execute twice — once from the global phase, once from the project phase. Both produce reports sent to the user.

The recent commit `e765b96` ("fix(heartbeat): remove global fallback, fully scope to active project") fixed the **interactive** paths (`pipeline.rs` line 986, `settings.rs` line 197) but did NOT fix the **heartbeat loop**, which is the hourly automated path causing the duplicate.

## Specs/Docs Drift

`specs/bugfixes/heartbeat-project-scope-analysis.md` line 19 states:
> "The heartbeat **loop** (`gateway/heartbeat.rs:253-306`) is NOT affected"

This is **factually wrong** — the loop IS affected because the global phase still includes project-specific sections. This incorrect statement caused the bug to be missed in commit `e765b96`.

## Impact Analysis

- Wastes Opus API calls (each duplicate is a full provider call)
- Clutters user's Telegram with redundant heartbeat reports
- Affects ANY project that has items in BOTH global HEARTBEAT.md AND its own project HEARTBEAT.md
- Not limited to tech-youtuber — any future project with the same pattern would duplicate

## Files Changed

| File | Change |
|------|--------|
| `backend/src/gateway/heartbeat.rs` | Strip active-project sections from global heartbeat before execution |
| `backend/src/markers/heartbeat.rs` | Add `strip_project_sections()` helper (uses existing `parse_heartbeat_sections`) |
| `~/.omega/prompts/HEARTBEAT.md` | Remove "TECH YOUTUBER PROJECT" section (data cleanup) |
| `specs/bugfixes/heartbeat-project-scope-analysis.md` | Correct the "NOT affected" statement |

---

## Requirements

### REQ-HBDUP-001 — Strip active-project sections from global heartbeat (Must)

**Description:** Before executing the global heartbeat, identify active projects that have their own `HEARTBEAT.md`. Strip any sections from the global checklist whose names match those project names.

**Acceptance criteria:**
- Global heartbeat does not include items from sections matching active project names
- Section matching is case-insensitive and normalized (e.g., "TECH YOUTUBER PROJECT" matches project "tech-youtuber")
- If stripping removes all sections, the global heartbeat is skipped entirely

### REQ-HBDUP-002 — Remove duplicated section from global HEARTBEAT.md (Must)

**Description:** Remove the `## TECH YOUTUBER PROJECT` section from `~/.omega/prompts/HEARTBEAT.md` since the project has its own `HEARTBEAT.md` at `~/.omega/projects/tech-youtuber/HEARTBEAT.md`.

**Acceptance criteria:**
- Global HEARTBEAT.md no longer contains "TECH YOUTUBER PROJECT" section
- Remaining sections (QUIET HOURS, NON-TRADING ITEMS) are preserved

### REQ-HBDUP-003 — Generic prevention for any project (Must)

**Description:** The fix must work generically for any project, not be hardcoded to tech-youtuber. If a new project "trader" later gets its own HEARTBEAT.md and has items in the global file, they should also be automatically deduplicated.

**Acceptance criteria:**
- No project name is hardcoded in the deduplication logic
- Works for any number of active projects with their own heartbeat files

### REQ-HBDUP-004 — Unit tests for section stripping (Must)

**Description:** Add tests verifying that project sections are correctly stripped from global heartbeat content.

**Acceptance criteria:**
- Test: global content with one project section, that project is active with own HEARTBEAT.md → section stripped
- Test: global content with multiple project sections, only active ones stripped
- Test: no active projects → global content unchanged
- Test: all sections stripped → returns None (skip global)

### REQ-HBDUP-005 — Correct stale spec (Must)

**Description:** Update `specs/bugfixes/heartbeat-project-scope-analysis.md` to remove the incorrect statement that the heartbeat loop is NOT affected.

**Acceptance criteria:**
- The spec no longer claims the heartbeat loop is unaffected
- A note references this bugfix analysis as the correction
