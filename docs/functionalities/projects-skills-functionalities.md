# Functionalities: Projects & Skills

## Overview
Projects provide per-user workspace isolation with custom instructions (ROLE.md) and skill declarations. Skills are plugin-like extensions with trigger keywords, MCP server declarations, and self-improvement capabilities.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | Project struct | Struct | backend/crates/omega-skills/src/projects.rs:~10 | name, instructions, path, skills (from frontmatter) | -- |
| 2 | load_projects() | Function | backend/crates/omega-skills/src/projects.rs:~30 | Scans {data_dir}/projects/*/ROLE.md, parses TOML/YAML frontmatter for skill declarations | Filesystem |
| 3 | get_project_instructions() | Function | backend/crates/omega-skills/src/projects.rs:~60 | Lookup project ROLE.md content by name | -- |
| 4 | Skill struct | Struct | backend/crates/omega-skills/src/skills.rs:~10 | name, description, requires, homepage, available, path, trigger, mcp_servers | -- |
| 5 | load_skills() | Function | backend/crates/omega-skills/src/skills.rs:~40 | Scans {data_dir}/skills/*/SKILL.md + 5 bundled skills; checks availability via `requires` binaries | Filesystem |
| 6 | match_skill_triggers() | Function | backend/crates/omega-skills/src/skills.rs:~100 | Matches message keywords against skill triggers; returns MCP servers to activate for this request | -- |
| 7 | build_skill_prompt() | Function | backend/crates/omega-skills/src/skills.rs:~130 | Formats skill list for system prompt injection | -- |
| 8 | parse_frontmatter() | Function | backend/crates/omega-skills/src/parse.rs:~5 | Parses TOML (+++...+++) or YAML (---...---) frontmatter from Markdown files | -- |
| 9 | 5 Bundled Skills | Constants | backend/crates/omega-skills/src/skills.rs:~50 | claude-code, google-workspace, playwright-mcp, skill-creator, ibkr-trader | -- |
| 10 | /projects command | Handler | backend/src/commands/settings.rs:97 | Lists available projects, marking the active one (from active_project fact) | Memory, i18n |
| 11 | /project command | Handler | backend/src/commands/settings.rs:125 | Activate/deactivate/show project; stores/deletes active_project fact | Memory, i18n |
| 12 | /skills command | Handler | backend/src/commands/settings.rs:80 | Lists installed skills with availability status | Skills, i18n |
| 13 | PROJECT_ACTIVATE marker | Marker | backend/src/markers/protocol.rs | AI-initiated project activation via marker in response | Memory |
| 14 | PROJECT_DEACTIVATE marker | Marker | backend/src/markers/protocol.rs | AI-initiated project deactivation via marker in response | Memory |

## Internal Dependencies
- load_projects() -> parse_frontmatter() -> Project struct
- load_skills() -> bundled skill definitions -> Skill struct
- handle_message() -> match_skill_triggers() -> MCP server activation
- /project command -> store_fact("active_project") -> session isolation
- handle_message() -> build_system_prompt() -> get_project_instructions()

## Dead Code / Unused
None detected.
