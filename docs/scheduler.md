# Omega Scheduler: Tasks and Reminders

## What Is the Scheduler?

The scheduler is Omega's built-in task queue. It lets you create reminders and recurring tasks using natural language -- no cron syntax, no manual setup. You tell Omega what you want to be reminded about and when, and it handles the rest.

**Example:**
```
You: Remind me to call John at 3pm
Omega: Sure, I'll remind you to call John at 3pm today.

[At 3:00 PM]
Omega: Reminder: Call John
```

The scheduler runs as a background loop inside the gateway, polling the database every 60 seconds for tasks that are due. When a task's time arrives, it delivers the reminder through the same channel where the task was created.

## How Scheduling Works

### Step 1: You Ask — Or Omega Decides On Its Own

You can request tasks explicitly:
- "Remind me to call John at 3pm"
- "Set a daily standup reminder at 9am"
- "Remind me every Monday to submit the weekly report"
- "Remind me on weekdays at 8:30am to check email"

But Omega also schedules proactively. After every action it takes, it asks itself: "Does this need follow-up?" If yes, it schedules the check itself without being asked. This applies universally — any action that could need a later check, verification, or status update gets scheduled automatically. An autonomous agent closes its own loops.

### Step 2: The Provider Translates

The AI provider understands your request and includes a special marker in its response:

```
Sure, I'll remind you to call John at 3pm today.
SCHEDULE: Call John | 2026-02-16T15:00:00 | once
```

The `SCHEDULE:` line is a structured marker with three pipe-separated fields:
1. **Description** -- What to remind you about.
2. **Due date** -- When to fire, in ISO 8601 format.
3. **Repeat type** -- How often to repeat: `once`, `daily`, `weekly`, `monthly`, or `weekdays`.

### Step 3: The Gateway Extracts, Stores, and Confirms

The gateway scans the provider's response for ALL `SCHEDULE:` markers (not just the first). When found, it:
1. Parses the three fields from each marker line.
2. Creates a task in the `scheduled_tasks` SQLite table via `store.create_task()` for each marker.
3. Collects the result of each operation (`TaskCreated`, `TaskFailed`, or `TaskParseError`).
4. Strips all `SCHEDULE:` lines from the response so you only see the friendly text.
5. After sending the AI's response, sends a separate **gateway confirmation message** showing exactly what was saved.

This means a single response can create multiple tasks at once — for example, asking "set 5 reminders for my Hostinger cancellation" will emit 5 `SCHEDULE:` lines and create 5 separate tasks.

### Anti-Hallucination: Gateway Confirmation

The AI composes its response text *before* the gateway processes markers. If a marker fails to parse or the database write errors, the AI's text would be a lie ("I've set 3 reminders" when only 1 was saved). The gateway solves this by sending its own confirmation as a follow-up message:

**Single task:**
```
✓ Scheduled: Cancel Hostinger VPS — 2026-03-15T09:00:00 (once)
```

**Multiple tasks:**
```
✓ Scheduled 3 tasks:
  • Cancel Hostinger VPS — 2026-03-15T09:00:00 (once)
  • Daily standup — 2026-02-22T09:00:00 (daily)
  • Call dentist — 2026-02-25T10:00:00 (once)
```

**Similar task warning:**
```
✓ Scheduled: Cancel VPS — 2026-03-15T09:00:00 (once)
⚠ Similar task exists: "Cancel Hostinger VPS" — 2026-03-15T09:00:00
```

**Failure:**
```
✗ Failed to save 1 task(s). Please try again.
```

The confirmation is localized to the user's preferred language.

### Duplicate Prevention

Task deduplication operates at two levels:

1. **Storage-level dedup** (in `create_task()`): Before inserting, the store checks for (a) exact match on sender + description + normalized datetime, and (b) fuzzy match — same sender, similar description (word overlap ≥ 50%, min 3 significant words), and `due_at` within 30 minutes. Datetime normalization ensures `2026-02-22T07:00:00Z` and `2026-02-22 07:00:00` are treated as identical. If either check matches, the existing task ID is returned without creating a duplicate.

2. **Confirmation-level warning** (in `task_confirmation.rs`): The gateway checks all newly created tasks against the user's existing pending tasks using `descriptions_are_similar()`. If a similar task exists, the confirmation includes a warning. The system prompt also instructs the AI to review existing tasks before creating new ones and to never pre-confirm task creation in its response text.

### Step 4: The Scheduler Delivers

Every 60 seconds (configurable), the background scheduler loop:
1. Queries for all pending tasks where `due_at <= now`.
2. Sends a `"Reminder: {description}"` message through the original channel.
3. Marks one-shot tasks as delivered, or advances recurring tasks to their next due date.

## Task Types

### One-Shot Tasks

Fire once and are done. The task is marked as `'delivered'` after delivery.

```
You: Remind me to buy milk at 5pm
→ SCHEDULE: Buy milk | 2026-02-16T17:00:00 | once
```

### Daily Tasks

Fire every day at the same time. After delivery, the due date advances by 1 day.

```
You: Set a daily standup reminder at 9am
→ SCHEDULE: Stand-up meeting | 2026-02-17T09:00:00 | daily
```

### Weekly Tasks

Fire once a week on the same day. After delivery, the due date advances by 7 days.

```
You: Remind me every Monday to submit the report
→ SCHEDULE: Submit weekly report | 2026-02-17T09:00:00 | weekly
```

### Monthly Tasks

Fire once a month on the same date. After delivery, the due date advances by 1 month.

```
You: Remind me on the 1st of every month to pay rent
→ SCHEDULE: Pay rent | 2026-03-01T09:00:00 | monthly
```

### Weekday Tasks

Fire every weekday (Monday through Friday). After delivery, the due date advances by 1 day, but skips Saturday and Sunday.

```
You: Remind me on weekdays at 8:30am to check email
→ SCHEDULE: Check email | 2026-02-17T08:30:00 | weekdays
```

**How weekday skipping works:** When a weekday task is delivered on Friday, `complete_task()` advances the due date to Monday (skipping Saturday and Sunday). When delivered on any other weekday, it advances by 1 day. The skip logic runs at completion time, so it always calculates the correct next weekday.

## Action Tasks

Action tasks are a powerful extension of the scheduler. While regular reminder tasks simply deliver a message to you, action tasks invoke the AI provider with full tool and MCP access when they come due. This means Omega can schedule autonomous follow-up work -- checking deployments, verifying services, analyzing data -- without needing you to be present.

### How Action Tasks Differ from Reminders

| Aspect | Reminder | Action |
|--------|----------|--------|
| When due | Sends a text message to you | Invokes the provider with the task description as a prompt |
| Tool access | None (just a message) | Full tool access + MCP servers |
| Follow-up | One and done | Can emit further SCHEDULE, SCHEDULE_ACTION, HEARTBEAT, and SKILL_IMPROVE markers |
| Badge in `/tasks` | (none) | `[action]` |

### The `SCHEDULE_ACTION:` Marker

The marker format is identical to `SCHEDULE:`, just with a different prefix:

```
SCHEDULE_ACTION: <description> | <ISO 8601 datetime> | <once|daily|weekly|monthly|weekdays>
```

**Example:**
```
SCHEDULE_ACTION: Check deployment status for api-v2 | 2026-02-17T16:00:00 | once
```

The gateway extracts ALL `SCHEDULE_ACTION:` markers the same way it extracts `SCHEDULE:` markers — multiple markers in a single response each create a separate task. The only difference is that the task is stored with `task_type = 'action'` instead of `'reminder'`.

### How Action Tasks Execute

When the scheduler loop finds a due action task:

1. **Invokes the provider** -- The task description is sent to the AI provider as a prompt, with full tool access and MCP server configuration.
2. **Processes the response** -- The provider's response is scanned for all standard markers:
   - `SCHEDULE:` -- Creates new reminder tasks (chaining).
   - `SCHEDULE_ACTION:` -- Creates new action tasks (recursive autonomous scheduling).
   - `HEARTBEAT_ADD:` / `HEARTBEAT_REMOVE:` / `HEARTBEAT_INTERVAL:` -- Modifies the monitoring checklist or heartbeat interval.
   - `SKILL_IMPROVE:` -- Records skill improvement suggestions.
   - `CANCEL_TASK:` -- Cancels pending tasks by ID prefix (all markers processed).
   - `UPDATE_TASK:` -- Updates pending task fields by ID prefix (all markers processed).
3. **Delivers the result** -- The provider's response (with markers stripped) is sent to the user through the original channel.
4. **Completes the task** -- Same as reminders: one-shot tasks are marked delivered, recurring tasks advance to the next due date.

This enables autonomous chains: an action task can schedule further action tasks, creating self-sustaining monitoring and follow-up loops.

### When to Use Action Tasks

Use `SCHEDULE_ACTION:` instead of `SCHEDULE:` when the follow-up requires Omega to **do** something rather than just **remind** you:

- Checking if a deployment succeeded
- Verifying that a DNS change propagated
- Running a health check on a service
- Analyzing updated data after a waiting period
- Following up on a long-running process

### Example: Autonomous Deployment Check

```
You: Deploy the new API version to staging
Omega: Deploying api-v2 to staging now...

[Omega deploys, then schedules a follow-up check]
SCHEDULE_ACTION: Check staging deployment health for api-v2 | 2026-02-17T16:00:00 | once

[At 4:00 PM, the action task fires]
[Omega runs health checks with full tool access]
Omega: Staging deployment check complete. api-v2 is healthy -- all endpoints returning 200, response times under 50ms.
```

## Managing Tasks

### Listing Tasks: `/tasks`

Send `/tasks` to see all your pending tasks:

```
Scheduled Tasks

[a1b2c3d4] Call John
  Due: 2026-02-17T15:00:00 (once)

[e5f6g7h8] Stand-up meeting
  Due: 2026-02-18T09:00:00 (daily)

[c9d0e1f2] [action] Check staging deployment health
  Due: 2026-02-18T16:00:00 (once)
```

Each task shows:
- An 8-character short ID (the prefix of the task's UUID).
- An `[action]` badge if the task is an action task (provider-backed execution).
- The task description.
- The next due date and repeat type.

### Cancelling Tasks: `/cancel <id>`

Use the short ID from `/tasks` to cancel a task:

```
You: /cancel a1b2c3d4
Omega: Task cancelled.
```

Cancelling a recurring task stops all future deliveries. Cancelled tasks are not deleted from the database -- they remain with `status = 'cancelled'` for audit purposes.

## Configuration

The scheduler is controlled by the `[scheduler]` section in `config.toml`:

```toml
[scheduler]
enabled = true              # Zero cost when no tasks exist
poll_interval_secs = 60     # How often to check for due tasks
```

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | bool | `true` | Whether the scheduler loop runs. |
| `poll_interval_secs` | integer | `60` | Polling interval in seconds. |

The scheduler is enabled by default and has zero overhead when no tasks exist -- the poll query returns immediately with an empty result set.

## Examples

### Personal Reminder

```
You: Remind me to call the dentist tomorrow at 10am
Omega: I'll remind you tomorrow at 10am to call the dentist.

[Next day at 10:00 AM]
Omega: Reminder: Call the dentist
```

### Daily Standup

```
You: Set up a daily standup reminder at 9am starting Monday
Omega: Done! I'll remind you every day at 9am for standup.

[Every day at 9:00 AM]
Omega: Reminder: Stand-up meeting
```

### Weekday Morning Routine

```
You: Every weekday at 7:30am remind me to review my calendar
Omega: Set! Weekday reminders at 7:30am for calendar review.

[Mon-Fri at 7:30 AM]
Omega: Reminder: Review my calendar
```

### Stopping a Recurring Reminder

```
You: /tasks
Omega:
Scheduled Tasks

[abc12345] Stand-up meeting
  Due: 2026-02-18T09:00:00 (daily)

You: /cancel abc12345
Omega: Task cancelled.
```

## Design Decisions

### Why No Cron Syntax?

Cron syntax (`0 9 * * 1-5`) is powerful but hostile to casual users. Omega is a personal assistant, not a sysadmin tool. Natural language is the right interface for reminders. The AI provider handles the translation from "every weekday at 9am" to a structured marker, which is what LLMs are good at.

### Why Provider-Based NLP?

The provider (e.g., Claude) already understands temporal expressions in context. Building a custom NLP parser for date/time expressions would add significant complexity for marginal gain. By delegating to the provider, Omega gets timezone awareness, relative dates ("tomorrow", "next Monday"), and natural phrasing for free.

### Why Poll Instead of Precise Timers?

A polling loop with a 60-second interval is simpler and more resilient than scheduling precise timers for each task. It tolerates clock drift, process restarts, and database changes without special handling. The worst-case delivery delay is one poll interval (60 seconds), which is acceptable for reminder-style tasks.

### Why At-Least-Once Delivery?

If the scheduler delivers a task but fails to mark it as complete (e.g., database error), the task may be re-delivered on the next poll. This is intentional: a duplicate reminder is better than a missed one. For one-shot tasks, the `delivered_at` timestamp provides an audit trail.

## Action Task Verification and Retry

### The Problem

Action tasks invoke the AI provider, which executes commands (e.g., sending emails via `gog gmail send`). Before verification, the scheduler blindly marked tasks as `delivered` the moment the provider returned *any* response — even if the action actually failed. There was no audit trail and no retry mechanism.

### The Solution: Context Enrichment + `ACTION_OUTCOME` Marker

The scheduler enriches the action task system prompt with:

1. **User profile** — Facts from the database (name, preferences, context) so the AI knows who the task owner is.
2. **Language preference** — The user's `preferred_language` fact, ensuring responses match their language.
3. **Delivery context** — Explicit instruction that the AI's text response will be delivered directly to the task owner via their messaging channel (Telegram/WhatsApp). To communicate with the owner, compose the message as the response. To perform external actions (send email, call APIs), use tools normally and report the result. Never search contacts to reach the owner — the response IS the delivery channel.

This prevents the AI from hallucinating external delivery mechanisms (e.g., searching contacts or fabricating email sends) when the task simply means "talk to the owner", while still allowing genuine external actions like sending emails to explicit addresses.

The provider must also end its response with one of:

```
ACTION_OUTCOME: success
ACTION_OUTCOME: failed | <brief reason>
```

The gateway strips this marker before delivering the response to the user.

### Outcome Handling

| Outcome | What Happens |
|---------|-------------|
| `ACTION_OUTCOME: success` | Task is completed normally (marked `delivered` or due_at advanced for recurring) |
| `ACTION_OUTCOME: failed \| reason` | Task failure: `fail_task()` is called — retries up to 3 times with 2-minute delays |
| No marker present | Backward compatibility: treated as success with a warning logged |
| Provider error (exception) | Task failure: same retry logic as explicit failure |

### Retry Logic

Failed action tasks are retried up to 3 times:

1. **First failure**: `retry_count` → 1, task rescheduled 2 minutes ahead, user notified
2. **Second failure**: `retry_count` → 2, task rescheduled again
3. **Third failure**: `retry_count` → 3, task permanently marked as `failed`, user notified of permanent failure with reason

The `last_error` column stores the most recent error message for debugging.

### Audit Logging

Every action task execution (success or failure) is logged to the `audit_log` table with:
- `input_text`: prefixed with `[ACTION]` for easy filtering
- `output_text`: full provider response (including command output)
- `provider_used`, `model`, `processing_ms`: performance tracking
- `status`: `ok` for success, `error` for failure
