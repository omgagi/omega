# Improvement Requirements: Build Agent Pipeline

> Replace the hardcoded 5-phase build orchestrator with a 7-phase agent pipeline
> that uses Claude Code CLI `--agent` flag and build-specific agent definitions
> embedded in the binary.

## Scope

**Domains affected:** gateway (builds), omega-providers (claude_code), omega-core (context)

**Files affected:**
- `backend/src/gateway/builds.rs` — rewrite orchestrator (7 phases, agent lifecycle)
- `backend/src/gateway/builds_parse.rs` — replace prompt templates with agent content, extend phase_message, keep parse functions
- `backend/src/gateway/builds_agents.rs` — NEW: embedded agent content + temp file lifecycle
- `backend/crates/omega-core/src/context.rs` — add `agent_name` field to Context
- `backend/crates/omega-providers/src/claude_code/command.rs` — add `--agent` flag support to `run_cli()`
- `backend/crates/omega-providers/src/claude_code/provider.rs` — wire `agent_name` through `complete()`
- `backend/src/gateway/mod.rs` — add `mod builds_agents;`

**Files NOT affected:**
- `backend/src/gateway/pipeline.rs` — no changes (build branch already calls `handle_build_request` at line 225)
- `backend/src/gateway/keywords.rs` — no changes (detection + confirmation logic stays)
- `backend/src/gateway/process_markers.rs` — no changes
- All omega-memory, omega-channels, omega-skills, omega-sandbox code — no changes

## Summary (plain language)

OMEGA currently builds software projects using 5 hardcoded phases, each with a system prompt
embedded as a Rust string constant. The Claude Code CLI is invoked via `claude -p "prompt"` with
the system prompt injected into the prompt text through `Context.to_prompt_string()`.

The Claude Code CLI supports a `--agent` flag that loads agent definitions from `.claude/agents/<name>.md`
files in the working directory. These agent files define the system prompt, model, tool restrictions,
max turns, and permission mode in YAML frontmatter — exactly what each build phase needs.

This improvement replaces the 5-phase pipeline with a 7-phase pipeline that mirrors the
multi-agent workflow used interactively (analyst → architect → test-writer → developer →
QA → reviewer → delivery). Each phase invokes `claude --agent <name> -p "..."` instead of
embedding the prompt in the CLI argument.

The agent definitions are compiled into the Rust binary using string constants or `include_str!()`.
At runtime, OMEGA writes them as temporary files to the build workspace's `.claude/agents/`
directory, invokes the CLI, then cleans up the files. This keeps the agent content proprietary —
no `.md` files are shipped or permanently stored on disk.

## User Stories

- As OMEGA, I want to invoke Claude Code with `--agent build-analyst` so that each build phase
  gets a purpose-built agent with correct model, tools, and permissions, rather than a raw prompt
  embedded in the CLI argument.

- As the OMEGA developer, I want build agent definitions embedded in the binary so that they are
  not visible as files on the production system, protecting the prompt engineering.

- As a user requesting a build, I want the build to follow TDD (tests written before code) so that
  the resulting project has verified, working code with test coverage.

- As a user requesting a build, I want to see progress messages as each agent phase completes so
  that I know the build is progressing and can estimate remaining time.

## Requirements

| ID | Requirement | Priority | Acceptance Criteria |
|----|------------|----------|-------------------|
| REQ-BAP-001 | Agent file lifecycle: write temp agent .md files to workspace before each phase, clean up after build completes or fails | Must | Agent files written to `<project_dir>/.claude/agents/` before phase invocation; cleaned up after build completes (success or failure); cleanup runs even on panic (RAII guard pattern) |
| REQ-BAP-002 | Embedded agent content: all 7 build agent definitions compiled into the binary | Must | No .md files shipped on disk; content accessible via `include_str!()` or const strings; each agent has YAML frontmatter (name, description, tools, model, permissionMode) |
| REQ-BAP-003 | Context.agent_name field: add optional agent name to Context struct | Must | `agent_name: Option<String>` added to Context; field is `serde(default, skip_serializing_if = "Option::is_none")`; backward compatible (existing callers unaffected) |
| REQ-BAP-004 | ClaudeCodeProvider --agent support: pass `--agent <name>` when Context.agent_name is set | Must | `run_cli()` emits `--agent <name>` when agent_name is present; when --agent is used, only current_message is passed as -p prompt (no [System] block); --model, --max-turns, --dangerously-skip-permissions still applied as overrides |
| REQ-BAP-005 | 7-phase pipeline: analyst → architect → test-writer → developer → QA → reviewer → delivery | Must | Each phase invokes the correct agent and produces expected output |
| REQ-BAP-006 | Phase isolation: each phase gets a fresh Context with no session_id | Must | No context window carryover between phases; phases communicate exclusively via filesystem artifacts |
| REQ-BAP-007 | Per-phase retry: each phase retries up to 3 times on failure | Must | 3 attempts per phase with 2s delay between; after 3 failures, pipeline stops with error message naming the failed phase |
| REQ-BAP-008 | Verification retry loop: QA failure triggers one retry cycle (developer fix → QA re-check) | Must | Maximum one retry cycle; if retry QA also fails, pipeline stops |
| REQ-BAP-009 | Localized progress messages for all 7 phases | Must | Each phase transition sends a localized message to the user; all 8 languages supported |
| REQ-BAP-010 | Preserve existing parse functions: parse_project_brief, parse_verification_result, parse_build_summary | Must | All 3 parse functions remain functional; existing unit tests pass unchanged |
| REQ-BAP-011 | Build-specific agent content: non-interactive adaptations of the interactive agent philosophy | Must | "Do NOT ask questions" in every agent; "Make reasonable defaults for anything ambiguous"; no "present questions to user and wait" patterns |
| REQ-BAP-012 | Analyst agent output format: structured brief + requirements in parseable format | Must | Output includes PROJECT_NAME, LANGUAGE, DATABASE, FRONTEND, SCOPE, COMPONENTS; compatible with existing parse_project_brief() |
| REQ-BAP-013 | Correct working directory: CLI subprocess cwd set to project_dir | Must | `.claude/agents/` directory is relative to this cwd |
| REQ-BAP-014 | Permission bypass: build agents use bypassPermissions or --dangerously-skip-permissions | Must | Build agents execute all tools without permission prompts |
| REQ-BAP-015 | Model selection per phase: all phases use model_complex (Opus) for deep reasoning quality. Changed from split model routing in commit `4a471b0`. | Should | All 7 phases use model_complex. Rationale: QA and review require deep analysis; consistency simplifies debugging. |
| REQ-BAP-016 | Architect creates TDD-ready specs with testable acceptance criteria | Should | specs/requirements.md with numbered requirements; specs/architecture.md with module descriptions |
| REQ-BAP-017 | Test-writer references specs: reads specs/ and writes tests covering each requirement | Should | Tests reference requirement numbers; Must requirements covered exhaustively; all tests fail initially |
| REQ-BAP-018 | Developer reads tests first: implements minimum code to pass all tests | Should | Module-by-module implementation; 500-line file limit enforced |
| REQ-BAP-019 | QA checks acceptance criteria and outputs VERIFICATION: PASS/FAIL | Should | Structured output parseable by parse_verification_result() |
| REQ-BAP-020 | Reviewer audits quality and outputs REVIEW: PASS/FAIL | Should | Reviews for bugs, security, performance; structured output with specific findings |
| REQ-BAP-021 | Agent frontmatter uses correct tool restrictions per role | Should | Analyst: Read, Grep, Glob; Architect: Read, Write, Bash, Glob, Grep; Test-writer/Developer/QA/Delivery: Read, Write, Edit, Bash, Glob, Grep; Reviewer: Read, Grep, Glob, Bash |
| REQ-BAP-022 | Audit logging per phase: each phase completion/failure logged | Should | Phase name in audit entry; success/failure status recorded |
| REQ-BAP-023 | Reviewer failure triggers fix cycle: reviewer FAIL → developer fix → reviewer re-check | Could | One retry cycle maximum; if retry fails, pipeline continues to delivery with warnings |
| REQ-BAP-024 | Phase timing: log elapsed time per phase | Could | Start/end time logged; total build time logged |
| REQ-BAP-025 | Agent maxTurns in frontmatter to prevent runaway phases | Could | Analyst: 25, Reviewer: 50, others: no limit |
| REQ-BAP-026 | Parallel builds prevention | Won't | Already handled by active_senders mutex |
| REQ-BAP-027 | Build history / management commands | Won't | Deferred |
| REQ-BAP-028 | Mid-build cancellation | Won't | Deferred |
| REQ-BAP-029 | Incremental builds / resume from failed phase | Won't | Deferred |

## Implementation Guide

### 1. Context Extension (REQ-BAP-003)

**File:** `backend/crates/omega-core/src/context.rs`

Add field after `session_id`:

```rust
/// Agent name for Claude Code CLI `--agent` flag. When set, the CLI
/// loads the agent definition from `.claude/agents/<name>.md` in the
/// working directory. The agent file provides the system prompt, so
/// `to_prompt_string()` emits only the current_message.
#[serde(default, skip_serializing_if = "Option::is_none")]
pub agent_name: Option<String>,
```

Update `Context::new()`: add `agent_name: None`.

Update `to_prompt_string()`: when `self.agent_name.is_some()`, return only `self.current_message` (no [System] block, no history — the agent file provides the system prompt).

### 2. Provider --agent Support (REQ-BAP-004)

**File:** `backend/crates/omega-providers/src/claude_code/command.rs`

Add `agent_name: Option<&str>` parameter to `run_cli()`. When `Some(name)`:
- Emit `--agent name` before `-p`
- The `-p` argument receives only the user message (not the full prompt string)

When `None`: behavior identical to current implementation.

**File:** `backend/crates/omega-providers/src/claude_code/provider.rs`

In `complete()`, extract `context.agent_name.as_deref()` and pass to `run_cli()`. When agent_name is set, pass `context.current_message` directly as the prompt.

### 3. Embedded Agent Content (REQ-BAP-002, REQ-BAP-011)

**File:** `backend/src/gateway/builds_agents.rs` (NEW)

Eight agent definitions as `const &str` (7 pipeline + 1 discovery). Example structure:

```rust
pub(super) const BUILD_ANALYST_AGENT: &str = "\
---
name: build-analyst
description: Analyzes build requests and produces structured project briefs with requirements
tools: Read, Grep, Glob
model: opus
permissionMode: bypassPermissions
maxTurns: 25
---

You are a build analyst...
";
```

All 7: `BUILD_ANALYST_AGENT`, `BUILD_ARCHITECT_AGENT`, `BUILD_TEST_WRITER_AGENT`,
`BUILD_DEVELOPER_AGENT`, `BUILD_QA_AGENT`, `BUILD_REVIEWER_AGENT`, `BUILD_DELIVERY_AGENT`.

Name-to-content mapping:

```rust
pub(super) const BUILD_AGENTS: &[(&str, &str)] = &[
    ("build-analyst", BUILD_ANALYST_AGENT),
    ("build-architect", BUILD_ARCHITECT_AGENT),
    ("build-test-writer", BUILD_TEST_WRITER_AGENT),
    ("build-developer", BUILD_DEVELOPER_AGENT),
    ("build-qa", BUILD_QA_AGENT),
    ("build-reviewer", BUILD_REVIEWER_AGENT),
    ("build-delivery", BUILD_DELIVERY_AGENT),
];
```

### 4. Agent File Lifecycle (REQ-BAP-001)

**File:** `backend/src/gateway/builds_agents.rs`

```rust
use std::path::{Path, PathBuf};

/// RAII guard that writes agent files on creation and removes them on drop.
pub(super) struct AgentFilesGuard {
    agents_dir: PathBuf,
}

impl AgentFilesGuard {
    pub async fn write(project_dir: &Path) -> std::io::Result<Self> {
        let agents_dir = project_dir.join(".claude").join("agents");
        tokio::fs::create_dir_all(&agents_dir).await?;
        for (name, content) in BUILD_AGENTS {
            let path = agents_dir.join(format!("{name}.md"));
            tokio::fs::write(&path, content).await?;
        }
        Ok(Self { agents_dir })
    }
}

impl Drop for AgentFilesGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.agents_dir);
        if let Some(claude_dir) = self.agents_dir.parent() {
            let _ = std::fs::remove_dir(claude_dir);
        }
    }
}
```

### 5. 7-Phase Pipeline (REQ-BAP-005)

**File:** `backend/src/gateway/builds.rs` (REWRITE)

`run_build_phase` signature changes to:

```rust
async fn run_build_phase(
    &self,
    agent_name: &str,
    user_message: &str,
    model: &str,
    max_turns: Option<u32>,
) -> Result<String, String>
```

Creates `Context` with `agent_name = Some(agent_name.to_string())`, no session_id.

### 6. Phase-to-Agent Mapping

| Phase | Agent Name | Model | Tools (frontmatter) | User Message (-p argument) |
|-------|-----------|-------|---------------------|---------------------------|
| 1 | build-analyst | model_complex | Read, Grep, Glob | The user's original build request text |
| 2 | build-architect | model_complex | Read, Write, Bash, Glob, Grep | "Project brief:\n{brief_text}\nBegin architecture design in {project_dir}." |
| 3 | build-test-writer | model_complex | Read, Write, Edit, Bash, Glob, Grep | "Read specs/ in {project_dir} and write failing tests. Begin." |
| 4 | build-developer | model_complex | Read, Write, Edit, Bash, Glob, Grep | "Read the tests and specs/ in {project_dir}. Implement until all tests pass. Begin." |
| 5 | build-qa | model_complex | Read, Write, Edit, Bash, Glob, Grep | "Validate the project in {project_dir}. Run build, lint, tests. Report VERIFICATION: PASS or FAIL." |
| 6 | build-reviewer | model_complex | Read, Write, Grep, Glob, Bash | "Review the code in {project_dir} for bugs, security, quality. Report REVIEW: PASS or FAIL." |
| 7 | build-delivery | model_complex | Read, Write, Edit, Bash, Glob, Grep | "Create docs and skill file for {project_name} in {project_dir}. Skills dir: {skills_dir}." |

### 7. Phase Interaction with Existing Functions

| Existing Function | Used By Phase | Change Needed |
|-------------------|--------------|---------------|
| `parse_project_brief()` (builds_parse.rs:275) | Phase 1 output | None — format preserved in analyst agent instructions |
| `parse_verification_result()` (builds_parse.rs:318) | Phase 5 output | None — format preserved in QA agent instructions |
| `parse_build_summary()` (builds_parse.rs:332) | Phase 7 output | None — format preserved in delivery agent instructions |
| `phase_message()` (builds_parse.rs:354) | All 7 phases | EXTEND for 7 phases |
| `audit_build()` (builds.rs:373) | Final | None — signature unchanged |

## Current vs New Phase Comparison

| Current Phase | Current Prompt Source | New Phase(s) | New Agent | Key Difference |
|--------------|----------------------|-------------|-----------|----------------|
| Phase 1: Clarification | `PHASE_1_PROMPT` (builds_parse.rs:42) | Phase 1: Analyst | build-analyst | Same brief extraction + produces requirements with acceptance criteria |
| Phase 2: Architecture | `PHASE_2_TEMPLATE` (builds_parse.rs:66) | Phase 2: Architect | build-architect | Informed by analyst requirements, creates testable specs |
| Phase 3: Implementation | `PHASE_3_TEMPLATE` (builds_parse.rs:116) | Phase 3: Test-Writer + Phase 4: Developer | build-test-writer, build-developer | SPLIT: tests first (TDD red), then implementation (TDD green) |
| Phase 4: Verification | `PHASE_4_TEMPLATE` (builds_parse.rs:158) | Phase 5: QA + Phase 6: Reviewer | build-qa, build-reviewer | SPLIT: QA validates functionally, reviewer audits quality |
| Phase 5: Delivery | `PHASE_5_TEMPLATE` (builds_parse.rs:202) | Phase 7: Delivery | build-delivery | Same function (docs, SKILL.md, summary) |
| Phase 3 Retry | `PHASE_3_RETRY_TEMPLATE` (builds_parse.rs:143) | Developer re-invocation after QA fail | build-developer (re-invoked) | Triggered by Phase 5 QA instead of Phase 4 Verification |

## Impact Analysis

| File | Change | Risk |
|------|--------|------|
| `backend/src/gateway/builds.rs` | REWRITE: 7-phase pipeline with agent invocation. `audit_build()` unchanged. | Medium — core orchestration logic changes but external interface stays identical |
| `backend/src/gateway/builds_parse.rs` | REMOVE: PHASE_1-5 prompt constants. KEEP: all parse functions and tests. EXTEND: `phase_message()` for 7 phases. | Low — additive changes |
| `backend/src/gateway/builds_agents.rs` | NEW: agent content constants, AgentFilesGuard | None — new file |
| `backend/src/gateway/mod.rs` | ADD: `mod builds_agents;` | Low — one line |
| `backend/crates/omega-core/src/context.rs` | ADD: `agent_name: Option<String>` field | Low — backward compatible |
| `backend/crates/omega-providers/src/claude_code/command.rs` | MODIFY: `run_cli()` gains agent_name, emits `--agent` flag | Medium |
| `backend/crates/omega-providers/src/claude_code/provider.rs` | MODIFY: `complete()` passes agent_name through | Medium |

## Regression Risk Areas

| Area | Mitigation |
|------|------------|
| Normal message handling | `agent_name: None` by default — no behavior change |
| Auto-resume in provider | Builds never use session_id — no conflict |
| Heartbeat/scheduler action tasks | Also call `provider.complete()` with `agent_name: None` — safe |
| Existing builds_parse tests | Parse functions kept unchanged |
| `base_command()` cwd behavior | AgentFilesGuard writes before invocation |

## Specs Drift Detected

| Spec/Doc File | What Is Outdated |
|---|---|
| `specs/improvements/builds-routing-improvement.md` | Describes the current 5-phase pipeline. Mark as `[SUPERSEDED]` after implementation. |
| `specs/src-gateway-rs.md` | Lists `builds.rs` and `builds_parse.rs` but not `builds_agents.rs`. Update after implementation. |

## Assumptions

| # | Assumption | Confirmed |
|---|-----------|-----------|
| 1 | `claude --agent <name> -p "..."` loads agent from `.claude/agents/<name>.md` relative to cwd | Yes |
| 2 | `--agent` and `--model` coexist: `--model` overrides agent frontmatter model | Yes |
| 3 | `--agent` and `--dangerously-skip-permissions` coexist | Yes |
| 4 | Agent frontmatter `permissionMode: bypassPermissions` works in `-p` mode | Yes |
| 5 | `--agent` uses ONLY the agent's system prompt, not the full Claude Code prompt | Yes |
| 6 | Writing to `.claude/agents/` before CLI invocation makes the agent discoverable by `--agent` | Yes |
| 7 | `include_str!()` or const strings work for agent embedding | Yes |

## Traceability Matrix

| Requirement ID | Priority | Test IDs | Implementation Module |
|---------------|----------|----------|---------------------|
| REQ-BAP-001 | Must | TEST-BAP-001a (guard_writes_all), TEST-BAP-001b (content_matches), TEST-BAP-001c (drop_cleans_up), TEST-BAP-001d (creates_hierarchy), TEST-BAP-001e (overwrites_existing), TEST-BAP-001f (double_write), TEST-BAP-001g (drop_idempotent) | builds_agents.rs |
| REQ-BAP-002 | Must | TEST-BAP-002a (has_7_entries), TEST-BAP-002b (correct_names), TEST-BAP-002c (non_empty), TEST-BAP-002d (yaml_frontmatter), TEST-BAP-002e (required_keys), TEST-BAP-002f (name_matches_key) | builds_agents.rs |
| REQ-BAP-003 | Must | TEST-BAP-003a (new_has_none), TEST-BAP-003b (skip_when_none), TEST-BAP-003c (backward_compat), TEST-BAP-003d (serde_round_trip), TEST-BAP-003e (prompt_only_message), TEST-BAP-003f (precedence_over_session), TEST-BAP-003g (empty_message), TEST-BAP-003h (special_chars), TEST-BAP-003i (unchanged_without), TEST-BAP-003j (all_fields), TEST-BAP-003k (unicode) | omega-core/context.rs |
| REQ-BAP-004 | Must | TEST-BAP-004a (no_agent_flag), TEST-BAP-004b (with_agent), TEST-BAP-004c (model_override), TEST-BAP-004d (skip_permissions), TEST-BAP-004e (max_turns), TEST-BAP-004f (empty_name), TEST-BAP-004g (with_session_id), TEST-BAP-004h (path_traversal), TEST-BAP-004i (explicit_tools), TEST-BAP-004j (disabled_tools) | claude_code/command.rs, provider.rs |
| REQ-BAP-005 | Must | (deferred: requires integration test with full Gateway) | builds.rs |
| REQ-BAP-006 | Must | (deferred: requires integration test with full Gateway) | builds.rs |
| REQ-BAP-007 | Must | (deferred: requires integration test with full Gateway) | builds.rs |
| REQ-BAP-008 | Must | (deferred: requires integration test with full Gateway) | builds.rs |
| REQ-BAP-009 | Must | TEST-BAP-009a (7_phases_english), TEST-BAP-009b (all_languages_non_empty), TEST-BAP-009c (7_phases_spanish), TEST-BAP-009d (unknown_language), TEST-BAP-009e (out_of_range), TEST-BAP-009f (delivery_english) | builds_parse.rs |
| REQ-BAP-010 | Must | TEST-BAP-010a (whitespace), TEST-BAP-010b (no_false_positive), TEST-BAP-010c (multiple_pass), TEST-BAP-010d (reason_non_adjacent), TEST-BAP-010e (empty_input), TEST-BAP-010f (large_input), TEST-BAP-010g (unicode_scope), TEST-BAP-010h (partial_fields), TEST-BAP-010i (slash_injection), TEST-BAP-010j (special_chars) | builds_parse.rs |
| REQ-BAP-011 | Must | TEST-BAP-011a (non_interactive), TEST-BAP-011b (reasonable_defaults) | builds_agents.rs |
| REQ-BAP-012 | Must | TEST-BAP-012a (analyst_output_format) | builds_agents.rs |
| REQ-BAP-013 | Must | (covered by TEST-BAP-004b: --agent flag passes cwd-relative name) | builds.rs, command.rs |
| REQ-BAP-014 | Must | TEST-BAP-014a (permission_bypass) | builds_agents.rs, command.rs |
| REQ-BAP-015 | Should | (deferred: requires integration test with model config) | builds.rs, builds_agents.rs |
| REQ-BAP-016 | Should | TEST-BAP-016a (architect_tdd_specs) | builds_agents.rs |
| REQ-BAP-017 | Should | TEST-BAP-017a (test_writer_refs_specs) | builds_agents.rs |
| REQ-BAP-018 | Should | TEST-BAP-018a (developer_reads_tests), TEST-BAP-018b (500_line_limit) | builds_agents.rs |
| REQ-BAP-019 | Should | TEST-BAP-019a (qa_verification_output) | builds_agents.rs |
| REQ-BAP-020 | Should | TEST-BAP-020a (reviewer_review_output) | builds_agents.rs |
| REQ-BAP-021 | Should | TEST-BAP-021a (analyst_restricted), TEST-BAP-021b (reviewer_restricted), TEST-BAP-021c (developer_full_tools) | builds_agents.rs |
| REQ-BAP-022 | Should | (deferred: requires integration test with audit logger) | builds.rs |
| REQ-BAP-023 | Could | (deferred: requires integration test) | builds.rs |
| REQ-BAP-024 | Could | (deferred: requires integration test) | builds.rs |
| REQ-BAP-025 | Could | TEST-BAP-025a (analyst_max_turns) | builds_agents.rs |
