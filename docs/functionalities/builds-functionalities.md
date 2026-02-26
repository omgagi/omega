# Functionalities: Builds

## Overview
A multi-phase software build pipeline that goes through discovery (3 rounds max), user confirmation, and then 7 sequential agent phases: Analyst, Architect, Test Writer, Developer, QA (with retry), Reviewer, and Delivery. Each phase runs as a separate Claude Code CLI agent.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | handle_build_request() | Method | backend/src/gateway/builds.rs:31 | Main 7-phase build orchestrator: Analyst -> Architect -> Test Writer -> Developer -> QA -> Reviewer -> Delivery; creates project dir at ~/.omega/workspace/builds/<name> | Provider, agents |
| 2 | run_build_phase() | Method | backend/src/gateway/builds.rs:381 | Generic phase runner: 3 attempts with 2s delay; creates Context with agent_name, model, max_turns | Provider |
| 3 | audit_build() | Method | backend/src/gateway/builds.rs:410 | Logs audit entry for build operation with status (success/failed/partial) | Audit |
| 4 | Discovery session | Logic | backend/src/gateway/pipeline.rs:~120 | 3-round max, 30-minute TTL, persisted to filesystem as pending_discovery fact; routes through BUILD_DISCOVERY_AGENT | Provider, filesystem |
| 5 | Build confirmation gate | Logic | backend/src/gateway/pipeline.rs:~140 | pending_build_request fact with 120s TTL; multilingual confirm/cancel keywords | Memory, keywords |
| 6 | AgentFilesGuard | Struct | backend/src/gateway/builds_agents.rs:~10 | RAII struct: writes 8 agent .md files to .claude/agents/ on creation, removes on drop | Filesystem |
| 7 | 8 Embedded Agents | Constants | backend/src/gateway/builds_agents.rs:~30 | BUILD_DISCOVERY_AGENT, BUILD_ANALYST_AGENT, BUILD_ARCHITECT_AGENT, BUILD_TEST_WRITER_AGENT, BUILD_DEVELOPER_AGENT, BUILD_QA_AGENT, BUILD_REVIEWER_AGENT, BUILD_DELIVERY_AGENT | -- |
| 8 | ProjectBrief struct | Struct | backend/src/gateway/builds_parse.rs:17 | Parsed Phase 1 output: name, language, database, frontend, scope, components | -- |
| 9 | parse_project_brief() | Function | backend/src/gateway/builds_parse.rs:~100 | Parses analyst output into ProjectBrief | -- |
| 10 | VerificationResult enum | Enum | backend/src/gateway/builds_parse.rs:27 | Pass or Fail(String); parsed from QA output | -- |
| 11 | parse_verification_result() | Function | backend/src/gateway/builds_parse.rs:~120 | Parses QA output for VERIFICATION: PASS/FAIL | -- |
| 12 | BuildSummary struct | Struct | backend/src/gateway/builds_parse.rs:33 | Parsed delivery output: project, location, language, summary, usage, skill | -- |
| 13 | parse_build_summary() | Function | backend/src/gateway/builds_parse.rs:~140 | Parses delivery output for BUILD_SUMMARY markers | -- |
| 14 | DiscoveryOutput enum | Enum | backend/src/gateway/builds_parse.rs:43 | Questions(String) or Complete(String); determines if discovery needs more rounds | -- |
| 15 | parse_discovery_output() | Function | backend/src/gateway/builds_parse.rs:~160 | Parses discovery agent output to determine if complete or needs more questions | -- |
| 16 | phase_message() | Function | backend/src/gateway/builds_parse.rs:~100 | Localized build phase status messages for all 8 languages | -- |

## Internal Dependencies
- Discovery session -> BUILD_DISCOVERY_AGENT -> parse_discovery_output()
- Confirmation gate -> handle_build_request()
- handle_build_request() -> AgentFilesGuard::write() -> run_build_phase() x 7
- Phase 1 -> parse_project_brief() -> ProjectBrief
- Phase 5 -> parse_verification_result() -> retry loop (developer + QA)
- Phase 7 -> parse_build_summary() -> final message

## Dead Code / Unused
- **ProjectBrief fields (language, database, frontend, components)**: `backend/src/gateway/builds_parse.rs:16` -- struct marked `#[allow(dead_code)]`; fields parsed but only `name` and `scope` are used by the orchestrator; kept for future conditional phase logic
