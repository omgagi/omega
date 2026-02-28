# Bugfix: Heartbeat Trading Suppression Failure

## Bug Report

User (Antonio) told OMEGA via Telegram: "I don't interested anymore in the traders report." OMEGA acknowledged and said it would stop all trader reports. OMEGA later confirmed: "Both items permanently gated by learned rules -- suppressing" and "These were already flagged for removal at 14:00 today. Removing now to stop wasting cycles."

The next morning at 08:01 UTC, OMEGA still sent a "Quant Scan" trading report with crypto pair analysis (BTCUSDT, BNBUSDT, XRPUSDT, etc.).

## Root Cause

The heartbeat loop has zero code-level enforcement of user suppression preferences. Three failures compounded:

1. **No code-level gate** -- `heartbeat.rs:156` calls `read_heartbeat_file()` and sends the raw ~400-line HEARTBEAT.md content to the AI. No filtering against learned rules or suppression state.

2. **Wrong marker emitted** -- OMEGA emitted `LESSON: trading|Stop trading reports` (advisory prompt text) instead of a structural removal mechanism. Even HEARTBEAT_REMOVE only works on individual lines via partial match, not 400-line sections.

3. **Prompt imbalance** -- The 1-line learned rule "stop trading reports" cannot overcome 400 lines of detailed trading instructions that explicitly say "You ARE the trader" and include "ANTI-PARALYSIS" directives that punish NOT trading.

## Architectural Gap

The heartbeat system has two mechanisms that are not connected:
- **Lessons** (soft): Stored in `lessons` table, injected as prompt text. Advisory only.
- **HEARTBEAT_REMOVE** (hard): Physically edits HEARTBEAT.md. Line-level only, cannot handle sections.

There is no **section-level suppression** mechanism.

## Fix

Add a code-level section suppression gate that filters out `##`-delimited sections from HEARTBEAT.md content BEFORE sending to the AI provider. New markers `HEARTBEAT_SUPPRESS_SECTION:` / `HEARTBEAT_UNSUPPRESS_SECTION:` let the AI reliably disable/enable entire sections. Suppression state is persisted in a companion `.suppress` file.

## Requirements

| ID | Requirement | Priority |
|----|------------|----------|
| REQ-HB-010 | Section-level suppression gate in heartbeat loop — filters suppressed sections BEFORE AI call | Must |
| REQ-HB-011 | Suppression storage in `HEARTBEAT.suppress` companion file (one section name per line) | Must |
| REQ-HB-012 | `HEARTBEAT_SUPPRESS_SECTION: <name>` marker — adds section to suppress file | Must |
| REQ-HB-013 | `HEARTBEAT_UNSUPPRESS_SECTION: <name>` marker — removes section from suppress file | Must |
| REQ-HB-014 | Section parsing from HEARTBEAT.md using `##` headers | Must |
| REQ-HB-015 | Prompt instructions for AI to use HEARTBEAT_SUPPRESS_SECTION over LESSON | Should |
| REQ-HB-018 | Logging for suppression events | Should |

## Acceptance Criteria

### REQ-HB-010
- Given `## TRADING` is suppressed, heartbeat sends only non-trading sections to AI
- Given all sections suppressed, no AI call is made (empty checklist)
- Filtering happens BEFORE classify step (Sonnet grouping)

### REQ-HB-011
- Suppressed sections persist across restarts (file-based)
- No suppress file = all sections active (default)
- Location: `~/.omega/prompts/HEARTBEAT.suppress` (global), `~/.omega/projects/<name>/HEARTBEAT.suppress` (per-project)

### REQ-HB-012
- `HEARTBEAT_SUPPRESS_SECTION: TRADING` adds "TRADING" to suppress file
- Duplicate adds are no-ops
- Works in both regular conversation and heartbeat response flows
- Marker stripped from response before delivery

### REQ-HB-013
- `HEARTBEAT_UNSUPPRESS_SECTION: TRADING` removes "TRADING" from suppress file
- Unsuppressing a non-suppressed section is a no-op

### REQ-HB-014
- Parses `##` headers into sections (header to next header or EOF)
- Section name: text before first ` — ` in header, case-insensitive matching
- Content before first `##` header = preamble (never suppressed)

## Files Affected

| File | Change |
|------|--------|
| `backend/src/markers/heartbeat.rs` | New: section parsing, suppression gate, suppress file I/O, new marker extract/strip |
| `backend/src/gateway/heartbeat.rs` | Filter checklist through suppression gate after `read_heartbeat_file()` |
| `backend/src/gateway/process_markers.rs` | Handle HEARTBEAT_SUPPRESS_SECTION / HEARTBEAT_UNSUPPRESS_SECTION markers |
| `backend/src/gateway/heartbeat_helpers.rs` | Handle same markers in heartbeat response context |
| `backend/src/markers/mod.rs` | Add new markers to `strip_all_remaining_markers()` safety net |

## Risks

- **Over-suppression**: Fuzzy matching could suppress wrong sections. Mitigated by exact header-text matching (case-insensitive).
- **Stale entries**: If section headers change, suppress file entries become stale. Mitigated by logging unmatched suppress entries.
