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

When OMEGA detects a genuine infrastructure or code bug (not a user request or cosmetic issue), it triggers the self-healing protocol:

1. **Diagnose**: Query `~/.omega/memory.db`, read logs at `~/.omega/omega.log`, inspect source code. Understand the root cause.
2. **Fix**: Write the code fix. Build with `cargo build --release` via Nix. If compilation fails, read the errors and fix them — repeat until the build is clean. Run `cargo clippy -- -D warnings` — fix lint errors until clippy passes. Never deploy code that doesn't compile or has warnings.
3. **Deploy & Verify**: Restart the service. Schedule a verification action via `SCHEDULE_ACTION` with the anomaly description, what was tried, and the current iteration count (e.g., "iteration 3/10").
4. **Iterate**: The scheduled verification fires, checks if the anomaly is resolved. If not, repeat steps 1-3.
5. **Escalate**: At iteration 10, STOP. Send the owner a detailed message: what the anomaly is, what was tried across all iterations, why it's not fixed, and what needs human intervention.

### Safety Guardrails

- **Max 10 iterations** — hard limit, then human escalation
- **Build + clippy gate** — broken code is never deployed; compilation errors are fixed within the same iteration
- **Context continuity** — each SCHEDULE_ACTION carries the full context (anomaly, attempts, iteration count) so the next instance can continue without losing information
- **Scope limit** — only for genuine infrastructure/code bugs, not feature requests or user tasks
