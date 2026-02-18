# Omega Self-Introspection: Autonomous Limitation Detection

## What Is Self-Introspection?

Self-introspection is Omega's ability to detect and report its own capability gaps. When Omega encounters something it cannot do but should be able to (missing tools, unavailable services, missing integrations), it reports the limitation using a `LIMITATION:` marker. The gateway stores it with deduplication, sends an immediate Telegram alert, and auto-adds it to the heartbeat checklist for ongoing monitoring.

## How It Works

### Detection

The AI detects limitations during:
1. **Normal conversations** — when asked to do something it can't
2. **Heartbeat checks** — the heartbeat prompt includes a self-audit instruction

### The LIMITATION Marker

Format: `LIMITATION: <title> | <description> | <proposed plan>`

Example: `LIMITATION: No email | Cannot send emails directly | Add SMTP provider integration`

### Processing Pipeline

When a LIMITATION marker is found:

1. Parse the title, description, and proposed plan
2. Store in the `limitations` DB table (INSERT OR IGNORE for dedup by title, case-insensitive)
3. If new (not a duplicate):
   - Send immediate Telegram alert to the owner via heartbeat channel
   - Auto-add to `~/.omega/HEARTBEAT.md` as a critical item
4. Strip the marker from the response before delivery

### Heartbeat Integration

The heartbeat loop enriches its prompt with:
- All open limitations from the database
- A self-audit instruction asking the AI to reflect on its capabilities

This means every heartbeat cycle is also a self-audit opportunity.

### Database

Table: `limitations` (migration 006)

- Deduplication via case-insensitive unique index on title
- Status: 'open' (default) or 'resolved'
- Fields: `id`, `title`, `description`, `proposed_plan`, `status`, `created_at`, `resolved_at`

### Configuration

No new configuration needed. Limitation alerts use the existing heartbeat channel config (Telegram + reply_target).

## Design Decisions

### Why Deduplication?

Without dedup, the same limitation would be reported every time the AI encounters it, flooding the owner with alerts.

### Why Auto-Add to Heartbeat?

Ensures detected limitations are monitored every heartbeat cycle until resolved. The owner sees progress (or lack thereof) automatically.

### Why Immediate Alert?

New limitations are significant events. The owner should know immediately, not wait for the next heartbeat.

## Resolving Limitations

When a limitation has been resolved (e.g., a provider integration is added), either:

1. **Manual resolution** — Mark it as resolved in the database (sets `status='resolved'` and `resolved_at=now()`)
2. **Automatic resolution** — The AI detects and reports the resolution during a heartbeat cycle

Resolved limitations remain in the database for audit purposes but are no longer actively monitored.

## Examples

### Example 1: Missing Email Provider

During conversation:
```
User: Can you send me an email reminder next week?
Omega: I don't have email capabilities yet. LIMITATION: No email | Cannot send emails directly | Add SMTP provider integration
```

The gateway:
- Stores the limitation with title "No email"
- Alerts the owner: "New limitation detected: No email — Cannot send emails directly"
- Adds to heartbeat checklist: `- [ ] Email integration (detected: No email)`

### Example 2: Self-Audit During Heartbeat

Heartbeat prompt includes:
```
## Self-Audit

Reflect on what you cannot do:
1. What integrations or tools are missing?
2. What would add the most value?
3. Any error patterns repeating?

Use LIMITATION: format to report gaps you detect.
```

If Omega realizes it can't handle PDF editing:
```
LIMITATION: No PDF editing | Cannot modify PDF documents | Integrate PDF manipulation library
```

The limitation is stored and monitored from that point forward.

## Self-Audit

Beyond capability gaps, OMEGA monitors its own behavior for anomalies. The self-audit instruction in the system prompt tells OMEGA to flag immediately when:

- Output doesn't match expectations
- Claims can't be backed up with evidence
- Tools fail silently
- Results don't add up

OMEGA has read access to its own audit trail at `~/.omega/memory.db`:
- `audit_log` — every exchange with model used, processing time, status
- `conversations` — conversation history
- `facts` — user profile data

When something doesn't add up, OMEGA can query these tables to verify its own behavior before reporting.

## Self-Healing Protocol

When OMEGA detects a genuine infrastructure or code bug, it emits a `SELF_HEAL: description` marker. The **gateway** (not the AI) manages the entire lifecycle:

### Markers

| Marker | Trigger | Gateway Action |
|--------|---------|---------------|
| `SELF_HEAL: description` | AI detects anomaly | Create/update state, notify owner, schedule follow-up |
| `SELF_HEAL_RESOLVED` | AI confirms fix | Delete state file, notify owner |

### Gateway-Managed Lifecycle

1. **AI emits** `SELF_HEAL: description` on its own line
2. **Gateway reads** `~/.omega/self-healing.json` (or creates it with iteration 1)
3. **Gateway increments** the iteration counter
4. **If iteration ≤ 10**: writes state, notifies owner ("🔧 SELF-HEALING (N/10): ..."), schedules a `SCHEDULE_ACTION` verification task (2 min delay)
5. **If iteration > 10**: sends escalation alert ("🚨 SELF-HEALING ESCALATION"), preserves state file for owner review, does **not** schedule further actions
6. **On resolution**: AI emits `SELF_HEAL_RESOLVED`, gateway deletes state file and notifies owner ("✅ Self-healing complete")

The AI's responsibility during healing tasks is: read `~/.omega/self-healing.json` for context, diagnose, fix, build+clippy until clean, restart service, update the attempts array. The gateway handles everything else (iteration tracking, scheduling, escalation).

### State Tracking

All self-healing state is persisted in `~/.omega/self-healing.json`:

```json
{
  "anomaly": "audit_log not recording model field",
  "iteration": 3,
  "max_iterations": 10,
  "started_at": "2026-02-18T19:00:00Z",
  "attempts": [
    "1: Added model fallback in provider — build passed, still not recording",
    "2: Fixed audit entry to pass model from response — clippy failed, fixed, deployed",
    "3: Verifying..."
  ]
}
```

The file is created on first `SELF_HEAL:` detection, updated after each iteration, and deleted on `SELF_HEAL_RESOLVED`. If max iterations are reached, the file is preserved for owner review.

### Processing Locations

Both `handle_message` (Stages 5i/5j) and `scheduler_loop` process these markers, ensuring self-healing works whether triggered by a direct message response or an action task response.

### Safety Guardrails

- **Max 10 iterations** — enforced in gateway code, then human escalation
- **Build + clippy gate** — the AI must build+clippy before deploying (prompt instruction)
- **State file** — `~/.omega/self-healing.json` tracks iteration count, anomaly, and attempt history across restarts
- **Scope limit** — only for genuine infrastructure/code bugs, not feature requests or user tasks
- **Code-enforced** — iteration limits and escalation are in gateway code, not dependent on AI compliance
