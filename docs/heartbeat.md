# Omega Heartbeat: Periodic AI Check-Ins

## What Is the Heartbeat?

The heartbeat is Omega's proactive monitoring and execution feature. Instead of passively reviewing a checklist, Omega **actively executes** each item: reminders and accountability items are sent to the user, system checks are performed, and results are reported. If everything is fine and no item requires user notification, the result is silently logged. Otherwise, Omega sends the results to your messaging channel.

Think of it as an active agent that uses AI reasoning to execute a periodic to-do list — not just check boxes, but actually do the work.

## How It Works

The heartbeat runs as a background loop inside the gateway, firing at clock-aligned boundaries (e.g., :00 and :30 for a 30-minute interval). Each cycle follows this sequence:

1. **Check active hours** -- If you configured an active hours window (e.g., 08:00-22:00), the heartbeat checks the current local time. Outside the window, it sleeps until the next cycle.
2. **Read the checklist** -- Looks for `~/.omega/prompts/HEARTBEAT.md`. If the file does not exist or is empty, the entire cycle is **skipped** — no API call is made. This prevents wasted provider calls when no checklist is configured.
3. **Enrich with context** -- The heartbeat enriches the prompt with data from memory (computed once, shared across all groups):
   - **User facts** (name, timezone, interests, etc.) from all users — gives the AI awareness of who it's monitoring for.
   - **Recent conversation summaries** (last 3 closed conversations) — gives the AI context about recent activity.
4. **Compose system prompt** -- The heartbeat attaches the full Identity/Soul/System prompt (plus sandbox constraints if applicable) to the provider call. Computed once and shared across all groups.
5. **Classify by domain** -- A fast Sonnet classification call (no tools) groups related checklist items by domain. If all items are closely related or there are 3 or fewer items, the classifier returns DIRECT and a single Opus call handles everything. Otherwise, items are grouped (e.g., trading tasks together, personal reminders together, system monitoring together).
6. **Execute groups in parallel** -- Each group gets its own focused Opus session via `tokio::spawn`. Related items stay together (5 trading items = 1 call), unrelated domains are separated (crypto vs training = 2 concurrent calls). MCP servers are matched per-group so each group gets only the tools it needs. For DIRECT, a single Opus call processes the full checklist (unchanged behavior).
7. **Process markers** -- Each group's response markers are processed independently:
   - `SCHEDULE` → creates reminder tasks
   - `SCHEDULE_ACTION` → creates action tasks
   - `HEARTBEAT_ADD/REMOVE/INTERVAL` → updates checklist/interval
   - `CANCEL_TASK` → cancels pending tasks
   - `UPDATE_TASK` → modifies existing tasks
   - All markers are stripped from the response before evaluating it.
8. **Evaluate per group** -- HEARTBEAT_OK is evaluated independently per group. A training group fires even when a crypto group is OK. Groups returning OK are logged silently. Non-OK results are joined with `---` separators and delivered as a single message.

```
At next clock-aligned boundary (e.g. :00, :30):
  → Is it within active hours?
    → No: skip, sleep
    → Yes: read HEARTBEAT.md
      → File missing or empty? → skip, no API call
      → Has content? → Build enrichment + system prompt (once)
        → Sonnet classification: group by domain
          → DIRECT? → 1 Opus call (existing behavior)
          → Grouped? → tokio::spawn per group (parallel)
            ↓ Opus call A (domain 1)     ↓ Opus call B (domain 2)
            ↓ markers processed           ↓ markers processed
            ↓ HEARTBEAT_OK eval           ↓ HEARTBEAT_OK eval
          → Consolidate non-OK results → audit + send to channel
```

## The HEARTBEAT_OK Suppression Mechanism

When the AI determines that everything is fine and no item requires user notification, it responds with exactly `HEARTBEAT_OK`. The gateway detects this keyword and suppresses the message -- it is logged but never sent to you.

This prevents notification fatigue. Without suppression, you would receive a message every 30 minutes telling you everything is fine. The suppression mechanism ensures you only hear from the heartbeat when something actually needs attention.

**Content-aware suppression:** The gateway strips `HEARTBEAT_OK` from the response and checks if meaningful content remains. If the AI included both a user-facing message (e.g., a training reminder) and `HEARTBEAT_OK`, the reminder is still delivered -- only the `HEARTBEAT_OK` token is removed. This prevents accountability items from being silently swallowed when the AI mistakenly appends `HEARTBEAT_OK` to a response that should reach the user.

**Prompt enforcement:** The heartbeat checklist prompt explicitly instructs the AI that items requiring user interaction (reminders, accountability, motivation) are NEVER "fine" and always require notification. The AI must NOT respond with `HEARTBEAT_OK` when any checklist item involves reminding, pushing, or motivating the user.

## Active Hours

Active hours define a time window during which heartbeats are allowed to fire. Outside this window, the heartbeat sleeps without calling the provider.

**Configuration:**
```toml
[heartbeat]
active_start = "08:00"
active_end = "22:00"
```

This means heartbeats only fire between 8:00 AM and 10:00 PM local time. At night, no provider calls are made and no alerts are sent.

**Midnight wrapping is supported.** If you set `active_start = "22:00"` and `active_end = "06:00"`, heartbeats will fire from 10 PM to 6 AM (useful for overnight monitoring).

**To disable active hours** and run heartbeats 24/7, leave both fields empty:
```toml
active_start = ""
active_end = ""
```

## The HEARTBEAT.md Checklist

`~/.omega/prompts/HEARTBEAT.md` is an optional file you create to customize what the heartbeat checks. When this file exists and has content, its contents are appended to the heartbeat prompt.

### Example HEARTBEAT.md

```markdown
## System Health Checklist

- Is the system load below 80%?
- Are all Docker containers in the "running" state?
- Is disk usage on / below 90%?
- Are there any CRITICAL or ERROR entries in /var/log/syslog from the last hour?
- Is the backup process completing without errors?
- Is the network latency to 8.8.8.8 below 100ms?
```

The AI evaluates each item. If all checks pass, it responds with `HEARTBEAT_OK`. If any check fails or raises concern, the AI describes the issue in its response, which is then delivered as an alert.

### What Happens Without HEARTBEAT.md

If the file does not exist or is empty, the heartbeat **skips the cycle entirely** — no API call is made. This is an intentional optimization: without a specific checklist, a generic health check provides limited value but still costs provider credits. Create a `HEARTBEAT.md` file with your monitoring items to activate the heartbeat.

## Conversational Management

You can add and remove heartbeat checklist items through natural conversation — no need to manually edit `HEARTBEAT.md`.

### Adding Items

Ask Omega to monitor something:

- "Keep an eye on my exercise habits"
- "Monitor whether I'm drinking enough water"
- "Add disk usage checks to your watchlist"

Omega also adds items proactively. After any action it takes, it evaluates whether the outcome will evolve over time and could need attention. If yes, it adds the item to its watchlist without being asked.

Omega will emit a `HEARTBEAT_ADD:` marker in its response, which the gateway intercepts to add the item to `~/.omega/prompts/HEARTBEAT.md`. The marker is stripped before the response reaches you.

### Removing Items

Ask Omega to stop monitoring:

- "Stop monitoring exercise"
- "Remove the disk usage check"
- "Don't watch that anymore"

Omega will emit a `HEARTBEAT_REMOVE:` marker. The gateway uses case-insensitive partial matching to find and remove the item. Comment lines (starting with `#`) are never removed.

### Querying the Interval

OMEGA knows its current heartbeat pulse — the interval is injected into its context when heartbeat is enabled. You can ask naturally:

- "What's your heartbeat pulse?"
- "How often do you check in?"

OMEGA will report the current value directly.

### Changing the Interval

You can dynamically change how often the heartbeat checks in through conversation:

- "Check every 15 minutes"
- "Make the heartbeat run hourly"
- "Set the heartbeat interval to 5 minutes"

Omega will emit a `HEARTBEAT_INTERVAL:` marker with the new value (in minutes, 1–1440). The gateway updates the interval atomically — the very next heartbeat cycle will use the new value. No restart required. The confirmation notification is localized to the user's preferred language.

The interval is held in memory and resets to the configured `interval_minutes` on service restart. To make a permanent change, update `config.toml`.

### How It Works Under the Hood

1. The current heartbeat checklist is injected into the system prompt so the provider knows what is already being monitored.
2. `build_system_prompt()` includes instructions telling the provider when to emit `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, and `HEARTBEAT_INTERVAL:` markers.
3. After the provider responds, the gateway extracts markers, updates `~/.omega/prompts/HEARTBEAT.md` (for add/remove) or the runtime interval (for interval changes), and strips the markers from the response.
4. Duplicate adds are prevented (case-insensitive check).
5. Interval values are validated: must be between 1 and 1440 (24 hours). Invalid values are silently ignored.

### Manual Editing

You can still edit `~/.omega/prompts/HEARTBEAT.md` manually. Conversational management and manual editing coexist — the file is the single source of truth.

## Configuration

The heartbeat is controlled by the `[heartbeat]` section in `config.toml`:

```toml
[heartbeat]
enabled = false
interval_minutes = 30
active_start = "08:00"
active_end = "22:00"
channel = "telegram"
reply_target = ""
```

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `false` | Whether the heartbeat loop runs. Disabled by default. |
| `interval_minutes` | integer | `30` | How often the heartbeat fires (in minutes). |
| `active_start` | string | `""` | Start of the active window (`HH:MM`). Empty = always active. |
| `active_end` | string | `""` | End of the active window (`HH:MM`). Empty = always active. |
| `channel` | string | `""` | Channel for alert delivery (e.g., `"telegram"`). |
| `reply_target` | string | `""` | Platform-specific target (e.g., Telegram chat ID). |

**Important:** The heartbeat is disabled by default because it requires `channel` and `reply_target` to be set. Enabling it without configuring a delivery target will cause alerts to be dropped (with a warning in the log).

## Use Cases

### System Monitoring

Create a `HEARTBEAT.md` checklist that covers your infrastructure health. The AI evaluates each item and alerts you only when something is wrong.

### Daily Check-Ins

Set `interval_minutes = 1440` (24 hours) and `active_start = "09:00"` / `active_end = "09:05"` to get a once-daily morning briefing.

### Proactive Alerts

Use the heartbeat to monitor external services, log files, or system metrics. The AI can reason about whether conditions are normal or concerning, catching issues that simple threshold alerts might miss.

### Development Watchdog

During long CI/CD runs or deployments, enable the heartbeat to periodically check build status or deployment health.

## Design Decisions

### Why Not Store Results?

Heartbeat results are not persisted to the database. Each check is stateless -- the AI evaluates the current state without reference to previous checks. This keeps the implementation simple and avoids unbounded storage growth from periodic polling.

If you need a history of heartbeat results, enable audit logging -- the provider calls are captured in the audit log.

### Why Suppress OK?

Notification fatigue is a real problem. A message every 30 minutes saying "everything is fine" trains you to ignore your notifications, which means you might also ignore the one message that says something is wrong. Suppressing OK responses ensures that when you do get a heartbeat alert, it is meaningful and worth your attention.

### Why Use the AI Provider?

Simple threshold checks (CPU > 90%, disk > 95%) can be done with shell scripts. The value of using an AI provider is that it can reason about context, correlate multiple signals, and describe issues in plain language. A shell script tells you "disk at 92%"; the AI can tell you "disk usage is at 92% and growing fast -- the backup directory has 15GB of stale snapshots that could be cleaned."

### Why Clock-Aligned Timing?

A naive `sleep(N minutes)` fires at times relative to process start -- if the service starts at :04, a 30-minute interval fires at :04 and :34, never at round numbers. Clock alignment computes the next clean boundary (e.g., :00 and :30 for 30-minute intervals, :00 for 60-minute intervals) and sleeps until that exact time. This makes heartbeat behavior predictable and debuggable from the logs.

### Why Attach the Full System Prompt?

Without the Identity/Soul/System prompt, the heartbeat provider call has no role context -- the AI behaves as a generic assistant and may produce incoherent responses (echoing stale conversation context, listing unrelated options). Attaching the full system prompt ensures the AI stays in character with proper behavioral boundaries, identical to how scheduled action tasks and regular messages receive the system prompt.

### Why Classify Then Route?

When the heartbeat has 10+ items spanning diverse domains (trading, training reminders, system monitoring), cramming them all into a single AI context increases the risk of hallucination and missed items. Unrelated domains compete for attention, and the AI defaults to HEARTBEAT_OK when overwhelmed.

The classify-then-route approach uses a fast, cheap Sonnet call to group related items by domain. Each group gets its own focused Opus session, so the AI can concentrate on one domain at a time. Related items stay together (5 trading items = 1 call, not 5 calls), while unrelated domains run in parallel for faster total execution.

The classification falls back to a single call when all items are related or there are 3 or fewer items, preserving the existing behavior for simple checklists. On classification failure, the system also falls back to single-call (zero regression risk).

### Why Active Hours?

Provider calls have a cost (API credits or local compute time). Active hours prevent unnecessary calls during times when you are unlikely to act on alerts anyway. If the server catches fire at 3 AM, you want to know about it in the morning -- not have 6 hours of suppressed heartbeat alerts queued up.
