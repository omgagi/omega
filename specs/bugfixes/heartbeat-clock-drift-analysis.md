# Heartbeat Clock-Drift Bugfix Analysis

## Bug Report

**Title:** Heartbeat clock-alignment drifts after quiet hours / system sleep
**Severity:** Medium — timing is unpredictable, not a data-loss issue

**Observed behavior:**
- 09:01 — heartbeat fired (first post-quiet-hours fire)
- 10:52 — next heartbeat fired (1h51m later, expected ~1h)
- Expected: snap to wall clock boundaries (09:00, 10:00, 11:00)

## Root Cause

Three interacting issues in `backend/src/gateway/heartbeat.rs` lines 46-63:

### Cause 1 (Primary): macOS System Sleep
`tokio::time::sleep` (line 54) is suspended when the MacBook sleeps. When the machine
wakes at 09:01, the pending sleep (targeting a boundary during the night) expires
immediately. The heartbeat fires at wake-up time (09:01), not at a clean boundary.

### Cause 2 (Contributing): Long Opus Execution Masks Alignment
The Opus provider call can take 10-60+ minutes (agentic tool use). The loop is sequential:
`sleep → execute → loop`. If the 09:01 heartbeat takes 51 minutes, it completes at 09:52.
Without start-time logging, the delivery time (10:52) looks like drift even if the next
cycle started on time.

### Cause 3 (Waste): Quiet Hours Boundary Polling
During 10 hours of quiet time (22:00-08:00) with interval=60, the loop wakes 10 times
(lines 56-63: check `is_within_active_hours`, log "skipping", `continue`). Each wake-up
recalculates and sleeps to the next boundary. The loop should sleep directly to `active_start`.

## Affected Code

| File | Lines | What |
|------|-------|------|
| `backend/src/gateway/heartbeat.rs` | 46-63 | Clock alignment + active hours check |
| `backend/src/markers/helpers.rs` | 72-108 | `is_within_active_hours`, `next_active_start_utc` |

## Requirements

| ID | Requirement | Priority |
|----|-------------|----------|
| REQ-HB-001 | Wall-clock re-snap: after sleep, compare actual time vs target. If overshot (system sleep), recalculate next boundary from current time. | Must |
| REQ-HB-002 | Quiet-hours jump-ahead: when outside active hours, sleep directly to active_start instead of boundary-by-boundary polling. | Must |
| REQ-HB-003 | Cycle start/end logging: log local time when cycle starts and elapsed seconds when it completes. | Must |
| REQ-HB-004 | Post-execution re-alignment: document and test that next boundary is always calculated from current time (already works). | Should |
| REQ-HB-005 | Unit tests for `next_clock_boundary()` pure function. | Should |

## Acceptance Criteria

### REQ-HB-001
- After system sleep, heartbeat fires at the next clean boundary, not at wake-up time
- If wake-up time is exactly on a boundary within active hours, fire immediately
- Implementation uses `chrono::Local::now()` after sleep to verify alignment

### REQ-HB-002
- During quiet hours 22:00-08:00, the loop sleeps once until ~08:00
- First heartbeat after quiet hours fires at the first clock-aligned boundary >= active_start
- Log message: "heartbeat: quiet hours, sleeping until HH:MM"

### REQ-HB-003
- Log before provider call: "heartbeat: cycle started at HH:MM"
- Log after completion: "heartbeat: cycle completed in Xs"

### REQ-HB-005
- Pure function: `fn next_clock_boundary(current_minute: u64, interval: u64) -> u64`
- Test cases: exact boundary, mid-interval, near-midnight, non-divisor interval

## Impact Analysis

- **Heartbeat timing**: Core change — miscalculation could cause fires too frequently or not at all
- **Log grep patterns**: "outside active hours, skipping" (repeated hourly) replaced by single "quiet hours, sleeping until HH:MM"
- **No impact on**: scheduler loop, summarizer loop, other gateway functions

## Specs Drift Detected
- `docs/heartbeat.md`: Does not document system-sleep vulnerability
- No heartbeat tests exist in the codebase — zero coverage for timing-critical code
