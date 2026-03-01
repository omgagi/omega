# Functionalities Index

> Auto-generated inventory of all functionalities in the Omega codebase.
> Generated from code only -- specs and docs were not consulted.

## Summary
- **Total modules analyzed**: 11
- **Total functionalities found**: 187
- **Dead code items flagged**: 6

## Modules

| Module | File | Functionalities | Dead Code |
|--------|------|-----------------|-----------|
| Gateway Core | [gateway-core-functionalities.md](gateway-core-functionalities.md) | 28 | 2 |
| Projects & Skills | [projects-skills-functionalities.md](projects-skills-functionalities.md) | 14 | 0 |
| Heartbeat | [heartbeat-functionalities.md](heartbeat-functionalities.md) | 12 | 0 |
| Scheduler | [scheduler-functionalities.md](scheduler-functionalities.md) | 10 | 0 |
| Memory | [memory-functionalities.md](memory-functionalities.md) | 32 | 0 |
| Markers | [markers-functionalities.md](markers-functionalities.md) | 26 | 2 |
| Providers | [providers-functionalities.md](providers-functionalities.md) | 24 | 0 |
| Channels | [channels-functionalities.md](channels-functionalities.md) | 8 | 0 |
| Commands & i18n | [commands-functionalities.md](commands-functionalities.md) | 22 | 0 |
| Builds | [builds-functionalities.md](builds-functionalities.md) | 16 | 1 |
| Core & Sandbox | [core-sandbox-functionalities.md](core-sandbox-functionalities.md) | 15 | 1 |

## Cross-Module Dependencies

### Main Pipeline Flow
```
Channel (Telegram/WhatsApp)
  -> mpsc channel
  -> Gateway::dispatch_message()
  -> Gateway::handle_message() [pipeline.rs]
    -> auth.rs (check_auth)
    -> sanitize.rs (neutralize injection)
    -> keywords.rs (detect context needs)
    -> memory/context.rs (build_context)
    -> skills.rs (match_skill_triggers)
    -> routing.rs (handle_direct_response)
      -> provider.complete() (ClaudeCode/OpenAI/etc.)
      -> process_markers.rs (extract + act on markers)
      -> memory (store exchange, session, audit)
    -> channel.send() (deliver response)
```

### Background Systems
```
Gateway::run() spawns:
  -> summarizer_loop()   [summarizer.rs]    -- idle conversation summarization
  -> scheduler_loop()    [scheduler.rs]     -- task polling + execution
  -> heartbeat_loop()    [heartbeat.rs]     -- periodic AI check-ins
  -> claudemd_loop()     [claudemd.rs]      -- workspace CLAUDE.md maintenance
  -> api_server()        [api.rs]           -- optional HTTP API
```

## Dead Code Inventory

| # | Item | Location | Reason |
|---|------|----------|--------|
| 1 | `classify_and_route()` | `gateway/routing.rs:20` | Intentionally kept for future multi-step routing |
| 2 | `execute_steps()` | `gateway/routing.rs:68` | Intentionally kept for future multi-step execution |
| 3 | `extract_schedule_marker()` | `markers/schedule.rs:4` | Superseded by `extract_all_schedule_markers()` |
| 4 | `extract_schedule_action_marker()` | `markers/schedule.rs:50` | Superseded by `extract_all_schedule_action_markers()` |
| 5 | `ProjectBrief` fields | `gateway/builds_parse.rs:16` | Parsed but unused (language, database, frontend, components) |
| 6 | `ChatChoice::finish_reason` | `omega-providers/openai.rs:134` | Parsed but unused |

---

## Answers: Projects, Heartbeats, Scheduling, and Autonomy

### 1. How Do Projects Work?

A **project** is a named scope defined by a `ROLE.md` file at `~/.omega/projects/<name>/ROLE.md`. The file has TOML/YAML frontmatter with optional skill declarations and a markdown body with the project's role instructions.

**Activation**: A project is activated by setting the `active_project` user fact (via `/project <name>` command or `PROJECT_ACTIVATE:` marker). Deactivation deletes that fact and creates a `.disabled` marker file to stop its heartbeat.

**Switching**: Use `/project <name>` to switch to a different project. This activates the new project (removes `.disabled` marker) and stores the `active_project` fact.

**What project scoping affects**:
- **Conversations**: `build_context()` filters conversations by `project` column. Active project conversations are separate from "no project" conversations.
- **Sessions**: Claude Code `--resume` sessions are stored as `(sender_id, project)` pairs. Each project gets its own session continuity.
- **System prompt**: When a project is active, its `ROLE.md` instructions are appended to the system prompt in `build_system_prompt()`.
- **Skills**: Projects can declare skills in their frontmatter (`skills: [skill-name]`), which are auto-activated when the project is active.
- **Outcomes & Lessons**: `outcomes` and `lessons` are stored with a `project` column. `build_context()` filters them by active project. Learning is project-scoped.
- **Tasks**: Scheduled tasks have a `project` column. Tasks created while a project is active are tagged with that project name. Action task execution loads the project's ROLE.md into the system prompt.
- **Heartbeat**: Each project can have its own `HEARTBEAT.md` at `~/.omega/projects/<name>/HEARTBEAT.md` with project-specific checklist items.

### 2. Do Projects Have Their Own Heartbeat?

**Yes.** The heartbeat system runs in two phases:

1. **Global heartbeat**: Reads `~/.omega/prompts/HEARTBEAT.md`, classifies items into groups, executes each group.
2. **Per-project heartbeats**: `run_project_heartbeats()` scans the `~/.omega/projects/` directory for all projects that have a `HEARTBEAT.md` file and do NOT have a `.disabled` marker. `/project off` creates `.disabled` (stops both heartbeat and conversation context). `/project <name>` activates a project by removing its `.disabled` marker.

Per-project heartbeats get:
- The project's `ROLE.md` instructions in the system prompt
- Project-scoped outcomes and lessons as enrichment context
- Results delivered to the user who has that project active
- Markers processed with project tagging (tasks, lessons created under that project)

The heartbeat can also **self-modify** via markers:
- `HEARTBEAT_ADD:` adds checklist items (to global or project heartbeat)
- `HEARTBEAT_REMOVE:` removes items
- `HEARTBEAT_INTERVAL:` changes the poll interval (persisted to `config.toml`)

### 3. Do Projects Have Their Own Schedule System?

**The scheduler is global, but tasks are project-tagged.** There is a single `scheduler_loop()` that polls all due tasks regardless of project. However:

- Tasks created while a project is active have `project = Some("project-name")` in the database.
- When an action task executes, `execute_action_task()` loads the project's ROLE.md into the system prompt if the task has a project tag.
- Task dedup (`create_task()`) checks for fuzzy matches within a 30-minute window with >50% word overlap to prevent duplicates.
- Quiet hours are global (configured in `config.toml`), not per-project.
- Repeat schedules (daily, weekly, monthly, weekdays) are task-level, not project-level.

**Bottom line**: One scheduler, but tasks carry project context into execution.

### 4. How Autonomous Is the System?

The system has **five autonomous capabilities** that operate without any user input:

| Capability | How It Works | Autonomy Level |
|------------|-------------|----------------|
| **Heartbeat** | Periodic AI check-ins with checklist items. Can create tasks, store lessons, modify its own checklist. | **High** -- fully autonomous, self-modifying |
| **Action Tasks** | Scheduled tasks with `task_type='action'` invoke the AI with full context and tools. Can chain new tasks, store rewards/lessons. | **High** -- autonomous execution with retry |
| **Summarizer** | After 2h idle, summarizes conversations and extracts user facts automatically. | **Medium** -- automatic but simple |
| **CLAUDE.md Maintenance** | Every 24h, regenerates the workspace CLAUDE.md file. | **Low** -- maintenance only |
| **Reminder Delivery** | Sends scheduled reminder text at due time. | **Low** -- fire and forget |

**Self-improvement loop**: The heartbeat and action systems form a feedback loop. The AI can:
1. Observe patterns via heartbeat checklist execution
2. Create `REWARD:` markers to record what worked/failed
3. Create `LESSON:` markers to modify future behavior
4. Create `SCHEDULE_ACTION:` markers to schedule follow-up autonomous actions
5. Modify its own heartbeat checklist (`HEARTBEAT_ADD:/REMOVE:`)

**What limits autonomy**:
- All autonomous actions use `model_complex` (Opus) -- the most capable but expensive model
- Action tasks are capped at 3 retries with 2-minute backoff
- Quiet hours block execution outside configured active windows
- The sandbox prevents AI subprocesses from accessing `memory.db` or `config.toml`
- No rate limiting on how many tasks/heartbeats can chain -- this is a potential runaway risk

### 5. Efficiency Assessment

| Mechanism | Token Savings | How |
|-----------|--------------|-----|
| Keyword-gated prompts | ~55-70% per turn | Only sends relevant prompt sections based on detected keywords |
| Session resume | ~90-99% per continuation | Claude Code `--resume` reuses existing conversation context |
| Conversation summarization | Bounded history | Old conversations are summarized, reducing context window |
| Per-sender serialization | N/A (latency cost) | Prevents concurrent provider calls per user, saves provider quota |

**Current efficiency gaps**:
- Model routing is disabled -- all messages use `model_fast` (Sonnet). The Sonnet/Opus classification code exists but is dead code.
- No rate limiting on incoming messages or provider calls.
- Background summarizer processes conversations sequentially -- can bottleneck if many conversations timeout simultaneously.
- Build pipeline runs 7 sequential AI calls with no session reuse between phases.
