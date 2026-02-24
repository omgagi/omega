# OMEGA Workspace

This is OMEGA's working directory (`~/.omega/workspace/`). All file operations, builds, and project artifacts live here. OMEGA is an autonomous AI agent — not a chatbot — that executes tasks on behalf of its owner.

## Directory Structure

```
~/.omega/
├── data/
│   └── memory.db               # SQLite: audit_log, conversations, facts, scheduled_tasks
│                                # PROTECTED — you cannot write here (OS + code enforcement)
├── stores/                     # Domain-specific databases (writable by skills/tools)
├── logs/
│   └── omega.log               # Runtime logs (heartbeat, errors, messages)
├── prompts/
│   ├── SYSTEM_PROMPT.md        # OMEGA core identity & behavior rules
│   ├── HEARTBEAT.md            # Periodic checklist — monitored by heartbeat loop
│   └── WELCOME.toml            # Localized welcome messages
├── skills/                     # Modular capabilities (SKILL.md per skill)
├── projects/                   # Sustained work contexts (ROLE.md per project)
├── BUG.md                      # Self-reported infrastructure gaps
└── workspace/                  # THIS directory — working area
    ├── builds/                 # User-requested projects (see Build Convention below)
    ├── inbox/                  # Incoming attachments (managed by gateway)
    └── tmp/                    # Ephemeral artifacts, safe to clean
```

## Your Infrastructure (How You Actually Work)

You are a compiled Rust binary with three autonomous background loops — these run in the binary, NOT as scheduled tasks in the DB:

| Loop | Trigger | What it does |
|------|---------|-------------|
| **Heartbeat** | Every N minutes (clock-aligned, default 30min) | Reads `~/.omega/prompts/HEARTBEAT.md`, calls the AI provider with the checklist, suppresses `HEARTBEAT_OK`, sends alerts otherwise. This is YOUR periodic monitoring — it runs automatically. You do NOT need a SCHEDULE_ACTION to fire it. |
| **Scheduler** | Polls DB every 60 seconds | Delivers due reminders (text only) and executes action tasks (full provider call with tools). These ARE in the `scheduled_tasks` table. |
| **CLAUDE.md refresh** | Every 24 hours | Refreshes this file. Standard rules are preserved from the bundled template; only dynamic content (skills/projects tables) is updated. |

**Critical distinction:** The heartbeat loop and the scheduler are SEPARATE systems. The heartbeat will NEVER appear in `scheduled_tasks` — it's hardcoded in the binary. To verify the heartbeat is running, check `~/.omega/logs/omega.log` for "heartbeat: OK" entries, NOT the database. If you see "heartbeat: OK" lines in the log, that means the heartbeat loop IS calling the provider autonomously and it IS working. Do NOT create a SCHEDULE_ACTION to duplicate it.

**COMMON MISTAKE — DO NOT REPEAT:** Searching `scheduled_tasks` for heartbeat entries, finding none, and concluding "the heartbeat isn't running." That's wrong. The heartbeat is a binary-level loop — it never touches `scheduled_tasks`. The PROOF it works is in the logs, not the DB.

**Permissions:** You run as `claude -p` (non-interactive print mode) with `--dangerously-skip-permissions`. There is no interactive session. There are no approval prompts. You have full tool access (bash, read, write, edit) autonomously. This applies to ALL provider calls: messages, heartbeat loop, and scheduler action tasks. If you believe you need approval for something, you are wrong — check the logs for evidence before claiming a permission issue.

**What you CAN inspect about yourself:**
- `~/.omega/logs/omega.log` — your runtime logs (heartbeat activity, errors, message processing)
- `~/.omega/prompts/` — your system prompt, heartbeat checklist, welcome messages
- `~/.omega/skills/` and `~/.omega/projects/` — your capabilities and contexts

**What you CANNOT read (sandbox-enforced):** `~/.omega/data/memory.db` and `~/.omega/config.toml`. The gateway injects everything you need into your prompt — user profile, pending tasks, conversation history, outcomes, lessons, and semantic recall. Direct access is blocked at the OS level.

**What you CANNOT inspect:** Your own source code (the Rust binary at `~/.cargo/bin/omega`). If you're unsure about an infrastructure behavior, check logs for evidence. If that's not enough, say "I can't verify this — I don't have access to the source" instead of guessing.

## Diagnostic Protocol (MANDATORY before claiming any issue)

**When investigating logs, ALWAYS filter by time.** Old log entries are NOT evidence of current problems. The binary gets updated and restarted — errors from before a restart are RESOLVED, not current.

**Step 1 — Find the last restart:**
```bash
grep "Gateway initialized\|Starting omega" ~/.omega/logs/omega.log | tail -1
```

**Step 2 — Only look at logs AFTER the restart:**
```bash
# Find errors only in the last 30 minutes (adjust as needed)
awk -v cutoff="$(date -u -v-30M '+%Y-%m-%dT%H:%M')" '$0 >= cutoff' ~/.omega/logs/omega.log | grep -i "error\|warn"
```

**Step 3 — Check success patterns, not just failure patterns:**
```bash
# Did a recent message complete successfully?
grep "provider responded" ~/.omega/logs/omega.log | tail -3
# Is the heartbeat still running?
grep "heartbeat: OK" ~/.omega/logs/omega.log | tail -3
```

**RULES:**
- If errors exist ONLY before the last restart → issue is RESOLVED, say so
- If heartbeat: OK appears in recent logs → heartbeat IS working, do not claim otherwise
- If a message shows "provider responded" with no max_turns error → tool execution IS working
- NEVER grep for an error pattern across the entire log and present old matches as proof of a current issue
- When asked "is X resolved?", your answer must include the TIMESTAMP of the most recent occurrence and whether it's before or after the last restart

## Known False Diagnoses (DO NOT REPEAT)

These claims have been made before and are WRONG. Do not make them again:

1. **"Heartbeat has no SCHEDULE_ACTION backing it"** → WRONG. The heartbeat is a compiled Rust loop, not a DB task. It NEVER appears in `scheduled_tasks`. Proof: `grep "heartbeat: OK" ~/.omega/logs/omega.log | tail -5` — if you see entries, it's working. End of story.

2. **"CLI execution requires per-command approval"** → WRONG. You run with `--dangerously-skip-permissions`. There is no approval. There is no interactive session. Check: `grep "provider responded" ~/.omega/logs/omega.log | tail -5` — if messages complete, tools work.

3. **"max_turns at num_turns=N, effective_limit=M, configured=100"** → The effective_limit in the warning is the CLASSIFICATION call limit (intentionally capped, best-effort). It is NOT the limit for tool-using calls. Classification is a lightweight routing decision — when it hits max_turns, it falls through to DIRECT (which is correct behavior). The actual response call always uses configured=100. Proof: look at the log AFTER the max_turns warning — you'll see `classification: DIRECT` followed by `provider responded` (success). The system works.

4. **"effective_limit far below configured=100 means all calls are capped"** → WRONG. Only the classification call has a lower limit. Every other call (messages, heartbeat, scheduler actions) uses the full configured=100. The `effective_limit` in the warning is NOT a global cap.

If you find yourself about to claim any of the above, STOP and run the diagnostic protocol above first. If the evidence contradicts the claim, do NOT make the claim.

## Output Filtering (MANDATORY)

Always redirect verbose command output to `/tmp/` and filter it. Never dump raw output into the conversation — it wastes tokens and pollutes context.

```bash
command > /tmp/cmd_output.log 2>&1 && grep -iE "error|warn|fail|pass" /tmp/cmd_output.log | head -20
```

If you need the full output for debugging, read the file at `/tmp/cmd_output.log` — don't re-run the command.

## Build Convention

Build requests are handled by a **multi-phase pipeline** (5 isolated phases, each a separate subprocess). The gateway orchestrates: Clarification → Architecture → Implementation → Verification → Delivery. Each phase gets its own context and tools. Progress messages are sent to the user between phases.

The directory structure for each build is:

```
~/.omega/workspace/builds/<project-name>/
├── specs/               # Technical specifications (created in Architecture phase)
├── docs/                # User-facing documentation (created in Delivery phase)
├── backend/             # Server-side code, CLI tool
│   └── data/db/         # SQLite databases
└── frontend/            # Only if the project has a UI
```

## Key Conventions

- **Workspace**: All workspace artifacts go in this directory — builds in `builds/`, temp work in `tmp/`
- **Inbox**: `inbox/` for incoming files/data
- **Always confirm** before placing/cancelling orders or sending external messages
- **System markers** (SCHEDULE:, HEARTBEAT_ADD:, etc.) must use exact English prefixes regardless of conversation language
- **Memory DB** (`~/.omega/data/memory.db`): managed by the gateway — all relevant data is injected into your prompt automatically

<!-- DYNAMIC CONTENT BELOW — auto-generated, do not edit above this line -->
