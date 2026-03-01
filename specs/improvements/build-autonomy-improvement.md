# Improvement: Build Pipeline Autonomy (BUILD_PROPOSAL Activation)

> Enable OMEGA's AI to autonomously detect build-intent from natural language and
> propose builds via the `BUILD_PROPOSAL` marker, eliminating the dependency on
> keyword matching as the sole entry point to the build pipeline.

## Problem

The `BUILD_PROPOSAL` marker is fully implemented in Rust (`process_markers.rs:171-187`,
`markers/protocol.rs:203-209`) but **never documented in the system prompt**. The AI
doesn't know it can emit this marker. The only way to trigger builds is through keyword
matching (`BUILDS_KW`, 75 entries) — which fails for natural language requests like:

- "Create a hello world program"
- "I need an app that tracks my expenses"
- "Escríbeme un programa que calcule facturas"

## Solution

Two additions to `prompts/SYSTEM_PROMPT.md` (no Rust changes):
1. Add `BUILD_PROPOSAL: description` to the marker quick-reference (always visible)
2. Add a concise build-awareness instruction to the `## System` section

## Scope

**Files changed:** `prompts/SYSTEM_PROMPT.md` only
**Files NOT changed:** All `.rs` files, all other prompts, topology files

## Requirements

| ID | Requirement | Priority |
|----|------------|----------|
| REQ-BLDAU-001 | Add BUILD_PROPOSAL to marker quick-reference | Must |
| REQ-BLDAU-002 | Add build-awareness instruction to `## System` section | Must |
| REQ-BLDAU-003 | Clear boundary: build-intent vs code-help requests | Must |
| REQ-BLDAU-004 | Language-agnostic (works in all 8 languages) | Should |
| REQ-BLDAU-005 | Keyword fast-path remains unchanged | Must |
| REQ-BLDAU-006 | `## Builds` section stays conditionally injected | Must |
| REQ-BLDAU-007 | No Rust code changes | Must |
| REQ-BLDAU-008 | Added text under 150 words total | Should |
| REQ-BLDAU-009 | Runtime prompt cache cleared after change | Must |

## Acceptance Criteria

### REQ-BLDAU-001
- `BUILD_PROPOSAL: description` appears in marker quick-reference block
- Format matches existing marker style

### REQ-BLDAU-002
- Instruction tells AI when to emit BUILD_PROPOSAL
- States it triggers a confirmation step (user must approve)
- Specifies marker format on its own line

### REQ-BLDAU-003
- EMIT: user wants a new standalone application/tool/service/library built from scratch
- DO NOT EMIT: help with existing code, code snippets, debugging, code review, one-off scripts

### REQ-BLDAU-005
- `BUILDS_KW` array unchanged
- `kw_match` in `pipeline.rs:272` unchanged
- Early-return at `pipeline.rs:296-299` unchanged

## Two Entry Paths (After Change)

| Path | Trigger | Flow |
|------|---------|------|
| Keyword (fast-path) | `BUILDS_KW` match | Gateway intercepts → discovery → confirmation → build |
| BUILD_PROPOSAL (autonomous) | AI emits marker | AI responds → gateway processes marker → confirmation → build |

Both converge at `pending_build_request` fact and the same confirmation flow (`pipeline_builds.rs:249-319`).

## Risk Assessment

- **False positives**: Mitigated by clear boundary criteria + user confirmation step + 120s TTL
- **Discovery bypass**: BUILD_PROPOSAL skips multi-round discovery. Phase 1 (clarification) handles refinement
- **Prompt size**: ~100-150 words added to always-visible prompt (~2.5% increase)
