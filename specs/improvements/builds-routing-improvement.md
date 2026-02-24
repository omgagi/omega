# Improvement: Builds Pipeline — Multi-Phase Orchestration

## Problem Statement

When users ask OMEGA to build programming projects (e.g., "build me a price tracker"), the system
pretends to execute a multi-step workflow but actually runs everything in a single `provider.complete()`
call via `handle_direct_response()`. The AI receives a prompt instructing it to "confirm first, create
structure, write specs, implement, test, create skill" but has no mechanism to enforce phase separation.
The result is hallucinated progress — OMEGA says "Done" without delivering verified artifacts.

The root cause is architectural: `classify_and_route()` and `execute_steps()` in `routing.rs` are
dead code (`#[allow(dead_code)]`), never called. All messages, including builds, go through the
single-shot `handle_direct_response()` path. The `## Builds` section in `SYSTEM_PROMPT.md` describes
a 7-step workflow that the runtime cannot enforce.

## Current Behavior (Preserved)

The following components work correctly and MUST NOT be modified in behavior:

1. **Keyword detection** — `kw_match(&msg_lower, BUILDS_KW)` at `pipeline.rs:191` correctly
   identifies build requests. The `BUILDS_KW` constant in `keywords.rs:145` has 24 trigger patterns
   (English, Spanish, Portuguese). This detection stays as-is.

2. **`handle_direct_response()` for non-build messages** — The direct response path at
   `routing.rs:179-434` handles all non-build messages (chat, scheduling, actions, etc.) and must
   remain the default for everything except builds.

3. **`execute_steps()` scaffolding** — The dead code at `routing.rs:69-174` has useful patterns:
   progress messages to user, per-step retry (3 attempts), marker processing per step, audit logging
   per step, completion summary. These patterns should inform the new implementation but the function
   itself will NOT be reactivated — its flat loop with identical config per step is inadequate for
   builds.

4. **Gateway struct fields** — `model_fast` (Sonnet), `model_complex` (Opus), `provider`,
   `channels`, `memory`, `audit`, `prompts`, `data_dir`, `skills` all stay unchanged.

5. **Context struct** — `omega_core::context::Context` already supports per-call overrides for
   `model`, `allowed_tools`, `max_turns`, and `mcp_servers`. No changes to omega-core needed.

6. **All other routing paths** — scheduler, heartbeat, summarizer, commands, auth — untouched.

## Specs Drift Detected

| Spec/Doc File | What Is Outdated |
|---|---|
| `specs/src-gateway-rs.md` line 42 | Flow diagram says "Classify & Route (always, model selection)" but classification is disabled. All messages go DIRECT. |
| `specs/src-gateway-rs.md` line 17 | Lists `keywords.rs` missing `BUILDS_KW` in description. |
| `prompts/SYSTEM_PROMPT.md` `## Builds` | Describes 7-step workflow that has zero runtime enforcement. |
| `prompts/WORKSPACE_CLAUDE.md` `## Build Convention` | Same disconnect — describes structure without enforcement. |

These will be updated as part of this improvement.

## Improvement Specification

### Architecture Overview

When `needs_builds` is true, `pipeline.rs` will branch to a new `handle_build_request()` method
instead of falling through to `handle_direct_response()`. This method lives in the new
`gateway/builds.rs` module and orchestrates 5 sequential phases, each as an isolated
`provider.complete()` call with phase-specific configuration.

```
User message (BUILDS_KW detected)
  |
  v
Phase 1: Clarification (Opus, no tools, text-only)
  |-- output: project brief (name, language, scope, architecture direction)
  |-- sends: brief summary to user via Telegram
  v
Phase 2: Architecture (Opus, full tools)
  |-- input: project brief from Phase 1
  |-- output: specs/ directory, directory structure, architecture doc
  |-- sends: "Architecture ready" progress to user
  v
Phase 3: Implementation (Sonnet, full tools)
  |-- input: specs/ and architecture from Phase 2
  |-- output: backend/ (and frontend/ if needed) with code
  |-- sends: "Implementation complete" progress to user
  v
Phase 4: Verification (Sonnet, full tools)
  |-- input: the entire project directory
  |-- output: test results, lint results, build results
  |-- on failure: loops back to Phase 3 ONCE, then stops
  |-- sends: "Verification passed" or failure report to user
  v
Phase 5: Delivery (Sonnet, full tools)
  |-- input: the verified project
  |-- output: docs/, SKILL.md, final summary
  |-- sends: complete build report to user
```

### Phase 1: Clarification

| Property | Value |
|---|---|
| Model | `self.model_complex` (Opus) |
| Tools | `allowed_tools: Some(vec![])` — no tools, text-only |
| Max turns | `Some(25)` — lightweight classification, no tool use |
| Session | None (fresh Context) |
| Working directory | N/A (no file operations) |

**System prompt theme:**
You are analyzing a build request. Extract and decide:
- Project name (kebab-case, max 3 words)
- Programming language (default: Rust unless user specifies otherwise)
- Database (default: SQLite unless user specifies otherwise)
- Scope summary (1-3 sentences of what the project does)
- Key components (list of modules/features to build)
- Whether a frontend is needed

Do NOT ask questions. Make reasonable defaults for anything ambiguous. Output ONLY a structured
brief in this exact format (parseable by the next phase):

```
PROJECT_NAME: <name>
LANGUAGE: <language>
DATABASE: <database>
FRONTEND: yes/no
SCOPE: <1-3 sentence description>
COMPONENTS:
- <component 1>
- <component 2>
...
```

**Input:** The user's original message text plus user profile facts (language preference, name).

**Output:** Structured brief text. Parsed by the orchestrator to extract `PROJECT_NAME` for
directory creation.

**Success criteria:** Output contains a valid `PROJECT_NAME:` line. If parsing fails after 3
retries, pipeline stops with error.

**User notification:** Send the parsed brief as a Telegram message:
"Building `<name>` — <scope summary>. I'll keep you posted."

### Phase 2: Architecture

| Property | Value |
|---|---|
| Model | `self.model_complex` (Opus) |
| Tools | Full access (no `allowed_tools` restriction) |
| Max turns | None (use provider default) |
| Session | None (fresh Context) |
| Working directory | `~/.omega/workspace/builds/<project-name>/` |

**System prompt theme:**
You are designing the architecture for a software project. You have full tool access.

Project brief:
```
<entire Phase 1 output>
```

Your tasks:
1. Create the directory structure at the current working directory
2. Write `specs/architecture.md` with module descriptions, data flow, API design
3. Write `specs/requirements.md` with functional requirements
4. If Rust: initialize with `cargo init` and set up Cargo.toml with dependencies
5. Create stub files for each module (empty files with doc comments)

Do NOT implement any logic. Only create structure and specifications.

**Input:** Phase 1 brief text injected into the system prompt.

**Output:** Project directory with `specs/` populated. Verified by checking that
`specs/architecture.md` exists after the call.

**Success criteria:** `specs/architecture.md` exists and is non-empty.

**User notification:** "Architecture defined — <N> spec files created."

### Phase 3: Implementation

| Property | Value |
|---|---|
| Model | `self.model_fast` (Sonnet) |
| Tools | Full access |
| Max turns | None (use provider default) |
| Session | None (fresh Context) |
| Working directory | `~/.omega/workspace/builds/<project-name>/` |

**System prompt theme:**
You are implementing a software project. You have full tool access.

Read the specifications in `specs/` to understand what to build. Implement the project
module by module:
1. Read `specs/architecture.md` and `specs/requirements.md`
2. Implement each module described in the architecture
3. Write tests alongside the code
4. Ensure all code compiles

Do NOT write documentation. Do NOT create skills. Focus only on working code.

**Input:** The specs/ directory written by Phase 2 (read from filesystem).

**Output:** Working code in `backend/` (and `frontend/` if applicable).

**Success criteria:** For Rust projects: `cargo build` exits 0. For other languages: the
primary build command succeeds. Checked by Phase 4, not by the orchestrator directly.

**User notification:** "Implementation complete — moving to verification."

### Phase 4: Verification

| Property | Value |
|---|---|
| Model | `self.model_fast` (Sonnet) |
| Tools | Full access |
| Max turns | None (use provider default) |
| Session | None (fresh Context) |
| Working directory | `~/.omega/workspace/builds/<project-name>/` |

**System prompt theme:**
You are verifying a software project. You have full tool access.

Run the validation pipeline:
1. `cargo build` (or equivalent) — must compile with zero errors
2. `cargo clippy --workspace` (or equivalent linter) — fix ALL warnings
3. `cargo test --workspace` (or equivalent) — all tests must pass

If any step fails, fix the issue and re-run. After all checks pass, output exactly:
```
VERIFICATION: PASS
```

If you cannot fix the issues, output:
```
VERIFICATION: FAIL
REASON: <brief description of what failed>
```

**Input:** The full project directory.

**Output:** Either `VERIFICATION: PASS` or `VERIFICATION: FAIL` with reason.

**Success criteria:** Response text contains `VERIFICATION: PASS`.

**Retry loop:** If FAIL, the orchestrator runs Phase 3 again (re-implementation) with the failure
reason prepended to the system prompt, then runs Phase 4 again. This loop happens at most ONCE.
If the second Phase 4 also fails, the pipeline stops and reports the failure to the user.

**User notification on success:** "All checks passed."
**User notification on failure:** "Build verification failed: <reason>. Partial results at
`~/.omega/workspace/builds/<project-name>/`."

### Phase 5: Delivery

| Property | Value |
|---|---|
| Model | `self.model_fast` (Sonnet) |
| Tools | Full access |
| Max turns | None (use provider default) |
| Session | None (fresh Context) |
| Working directory | `~/.omega/workspace/builds/<project-name>/` |

**System prompt theme:**
You are delivering a completed software project. You have full tool access.

Tasks:
1. Write user documentation in `docs/` (README, usage guide, API reference if applicable)
2. Create a skill file at `~/.omega/skills/<project-name>/SKILL.md` with:
   - YAML frontmatter: name, description, trigger keywords
   - Body: CLI subcommands/flags documentation
3. Write a final summary of what was built

Output the summary in this format:
```
BUILD_COMPLETE
PROJECT: <name>
LOCATION: <full path>
LANGUAGE: <language>
SUMMARY: <2-3 sentence description of what was built>
USAGE: <primary CLI command or entry point>
SKILL: <skill name if created>
```

**Input:** The verified project directory.

**Output:** docs/ populated, SKILL.md created, summary text.

**Success criteria:** Response contains `BUILD_COMPLETE`. The summary is parsed and delivered
to the user.

**User notification:** Final formatted message to the user with project name, location,
what it does, and how to use it.

## Pipeline Orchestrator Logic (Pseudocode)

```
fn handle_build_request(incoming, typing_handle):
    // Phase 1: Clarification
    send_text(incoming, "Analyzing your build request...")
    brief = run_phase(Phase1Config, incoming.text)
    if brief.is_err():
        send_text(incoming, "Could not understand the build request. <error>")
        return
    project_name = parse_project_name(brief)
    project_dir = ~/.omega/workspace/builds/{project_name}/
    create_dir_all(project_dir)
    send_text(incoming, "Building `{project_name}` — {scope}. I'll keep you posted.")

    // Phase 2: Architecture
    result = run_phase(Phase2Config, brief, project_dir)
    if result.is_err():
        send_text(incoming, "Architecture phase failed: <error>")
        return
    send_text(incoming, "Architecture defined.")

    // Phase 3: Implementation
    result = run_phase(Phase3Config, project_dir)
    if result.is_err():
        send_text(incoming, "Implementation phase failed: <error>")
        return
    send_text(incoming, "Implementation complete — verifying...")

    // Phase 4: Verification (with retry loop)
    verification = run_phase(Phase4Config, project_dir)
    if verification == FAIL:
        send_text(incoming, "Verification found issues — fixing...")
        run_phase(Phase3RetryConfig, project_dir, verification.reason)
        verification = run_phase(Phase4Config, project_dir)
        if verification == FAIL:
            send_text(incoming, "Build verification failed: <reason>")
            return
    send_text(incoming, "All checks passed.")

    // Phase 5: Delivery
    result = run_phase(Phase5Config, project_dir, project_name)
    summary = parse_build_summary(result)
    send_text(incoming, format_final_message(summary))

    // Audit + cleanup
    audit.log(...)
    typing_handle.abort()
```

## What Will NOT Change

| Component | Why |
|---|---|
| `keywords.rs` BUILDS_KW constant | Detection logic works correctly |
| `handle_direct_response()` | Non-build messages use this path |
| Gateway struct definition | All needed fields already exist |
| Prompts struct definition | Already has `builds` field |
| omega-core Context struct | Already supports per-call overrides |
| omega-providers ClaudeCodeProvider | `complete()` handles model/tools/max_turns via Context |
| scheduler.rs, scheduler_action.rs | Unrelated subsystem |
| heartbeat.rs, heartbeat_helpers.rs | Unrelated subsystem |
| summarizer.rs | Unrelated subsystem |
| auth.rs | Unrelated subsystem |
| process_markers.rs | May be called per-phase but not modified |
| All omega-channels code | Unrelated subsystem |
| All omega-memory code | Unrelated subsystem |

## Design Constraints

1. **FULLY non-interactive.** OMEGA runs on Telegram/WhatsApp. No user prompts between phases.
   Phase 1 makes all decisions autonomously. Each subsequent phase proceeds without user input.

2. **Progress visible.** Each phase transition sends a progress message to the user via the
   channel. Users see: analyzing -> brief -> architecture -> implementation -> verification ->
   delivery.

3. **Fail gracefully.** If any phase fails after 3 internal retries, the pipeline stops. The
   user receives a clear message stating what phase failed, what was completed, and where partial
   results live. No silent "success."

4. **Context isolation.** Each phase is a separate `provider.complete()` call with a fresh
   `Context` (no `session_id`). Phases communicate exclusively via the filesystem — specs/,
   code files, docs/ written to the project directory. This prevents context window overflow.

5. **Result delivery.** The final user message includes: project name, filesystem location,
   what was built, how to use it (primary CLI command), and skill name if created.

6. **Per-phase retry.** Each phase internally retries up to 3 times (matching `execute_steps`
   pattern). The Phase 3 <-> Phase 4 verification loop is separate from internal retries.

7. **One verification loop.** Phase 4 failure triggers ONE re-run of Phase 3 + Phase 4. If
   the second attempt fails, the pipeline stops. No infinite loops.

8. **500-line rule.** If `builds.rs` exceeds 500 lines, split into sub-files.

## Affected Files

| File | Change Type | Description |
|---|---|---|
| `backend/src/gateway/builds.rs` | NEW | Multi-phase build orchestrator |
| `backend/src/gateway/mod.rs` | MODIFY | Add `mod builds;` |
| `backend/src/gateway/pipeline.rs` | MODIFY | Branch to `handle_build_request()` when `needs_builds` |
| `prompts/SYSTEM_PROMPT.md` | MODIFY | Update `## Builds` section |
| `prompts/WORKSPACE_CLAUDE.md` | MODIFY | Update `## Build Convention` section |
| `specs/src-gateway-rs.md` | MODIFY | Add builds.rs to module table, fix classification description |

## Assumptions

| # | Assumption | Confirmed |
|---|---|---|
| 1 | `allowed_tools: Some(vec![])` disables all tools in provider | YES |
| 2 | `Context.model` override is respected by the provider | YES |
| 3 | Fresh Context per phase prevents context window overflow | YES (by design) |
| 4 | Builds directory may not exist on first build | YES — orchestrator must `create_dir_all` |
| 5 | Phase output parsing is reliable with structured prompts | YES (same pattern as ACTION_OUTCOME) |
| 6 | `active_senders` mutex prevents concurrent builds for same user | YES |

## Risks

| Risk | Severity | Mitigation |
|---|---|---|
| Phase 1 brief parsing fails | Medium | 3 retries + fallback to default project name |
| Total build time exceeds patience | Low | Progress messages keep user informed |
| Phase 3 code fails Phase 4 verification | Medium | One retry loop, then report partial results |
| Disk space from accumulated builds | Low | Out of scope for v1 |
| Skill name conflicts | Low | Check before creating, skip if exists |

## Out of Scope

- Mid-build cancellation
- Build history / management commands
- Incremental builds / resume from previous session
- Frontend-specific tooling (verification is Rust-focused)
- Reactivating `classify_and_route()` or `execute_steps()`
- Changes to omega-core, omega-providers, omega-channels, omega-memory
