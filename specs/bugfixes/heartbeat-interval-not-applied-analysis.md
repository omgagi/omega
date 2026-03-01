# Bugfix Analysis: Heartbeat Interval Change Not Applied

**Date:** 2026-03-01
**Severity:** Medium — user-facing feature doesn't work as expected
**Status:** In Progress

## Bug Description

When the user changes the heartbeat interval at runtime (via `HEARTBEAT_INTERVAL:` marker), the change is persisted to config.toml and the in-memory `AtomicU64` is updated, but the heartbeat loop continues firing on the OLD interval. The loop only picks up the new interval after the current sleep completes — which can be up to 1 full old-interval (e.g., 60 minutes) later.

## Reproduction

1. Heartbeat running at 60-minute interval (cycles at :00)
2. User sends "change the heartbeat loop to 10 minutes"
3. OMEGA responds "Done. Heartbeat interval updated to 10 minutes."
4. `HEARTBEAT_INTERVAL: 10` marker processed — AtomicU64 updated, config persisted
5. **Expected:** Next cycle fires at the next 10-minute boundary (~10 minutes)
6. **Actual:** Next cycle fires at the next 60-minute boundary (~60 minutes)

## Evidence from Logs

```
2026-03-01T11:00:00 heartbeat: cycle started at 11:00
2026-03-01T11:00:50 heartbeat: cycle completed in 50s
2026-03-01T11:11:16 heartbeat interval_minutes persisted to config: 10
2026-03-01T11:11:16 heartbeat: interval changed to 10 minutes
2026-03-01T12:00:00 heartbeat: cycle started at 12:00  ← still 60-min boundary
```

## Root Cause

In `gateway/heartbeat.rs`, the heartbeat loop uses `tokio::time::sleep()` (line 114) to sleep until the next clock-aligned boundary. This sleep is calculated once per iteration using the interval loaded at the top of the loop (line 86). When the `AtomicU64` is updated by another task (via process_markers or scheduler), the sleeping task is not notified — `tokio::time::sleep()` has no built-in cancellation mechanism.

The same issue affects the quiet-hours sleep (line 100).

## Impact

- **User experience:** Interval changes appear to be ignored for up to 1 full old-interval
- **Config persistence:** Works correctly (not affected)
- **In-memory state:** Updated correctly (not affected)
- **Only the running sleep is stale**

## Fix Requirements

| ID | Priority | Description |
|----|----------|-------------|
| REQ-HBINT-001 | Must | Heartbeat loop must react to interval changes within 1 minute |
| REQ-HBINT-002 | Must | Use `tokio::sync::Notify` to wake sleeping heartbeat loop |
| REQ-HBINT-003 | Must | All 3 sites that process `HEARTBEAT_INTERVAL:` must trigger the notify |
| REQ-HBINT-004 | Should | Both sleep sites (quiet-hours and boundary-aligned) must be interruptible |

## Sites That Change Interval

1. `gateway/process_markers.rs:328` — regular message pipeline
2. `gateway/heartbeat_helpers.rs:174` — heartbeat loop's own marker processing
3. `gateway/scheduler_action.rs:402` — scheduler action execution

## Fix Approach

Add `Arc<tokio::sync::Notify>` to Gateway. Pass to heartbeat_loop and all marker processors. Wrap both `tokio::time::sleep()` calls in `tokio::select!` with `notify.notified()`. When notified, `continue` to recalculate from the new interval.
