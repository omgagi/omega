# Functionalities: Core & Sandbox

## Overview
omega-core provides shared types, configuration, error handling, and prompt sanitization. omega-sandbox provides OS-level protection (Seatbelt on macOS, Landlock on Linux) and code-level path enforcement for all AI-executed file operations.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | Config struct | Struct | backend/crates/omega-core/src/config/mod.rs:21 | Top-level: omega, auth, provider, channel, memory, heartbeat, scheduler, api sections | -- |
| 2 | load() | Function | backend/crates/omega-core/src/config/mod.rs:320 | Loads config from TOML file; falls back to defaults if file missing | -- |
| 3 | shellexpand() | Function | backend/crates/omega-core/src/config/mod.rs:186 | Expands ~ to $HOME in paths | -- |
| 4 | migrate_layout() | Function | backend/crates/omega-core/src/config/mod.rs:201 | Migrates flat ~/.omega/ to structured subdirectory layout; patches config.toml db_path | Filesystem |
| 5 | patch_heartbeat_interval() | Function | backend/crates/omega-core/src/config/mod.rs:259 | Text-based patching of interval_minutes in config.toml; preserves comments | Filesystem |
| 6 | SYSTEM_FACT_KEYS | Constant | backend/crates/omega-core/src/config/mod.rs:175 | 7 protected keys: welcomed, preferred_language, active_project, personality, onboarding_stage, pending_build_request, pending_discovery | -- |
| 7 | Context struct | Struct | backend/crates/omega-core/src/context.rs:54 | system_prompt, history, current_message, mcp_servers, max_turns, allowed_tools, model, session_id, agent_name | -- |
| 8 | ContextNeeds struct | Struct | backend/crates/omega-core/src/context.rs:7 | recall, pending_tasks, profile, summaries, outcomes -- controls which optional context blocks are loaded | -- |
| 9 | IncomingMessage | Struct | backend/crates/omega-core/src/message.rs:6 | id, channel, sender_id, sender_name, text, timestamp, reply_to, attachments, reply_target, is_group | -- |
| 10 | OutgoingMessage | Struct | backend/crates/omega-core/src/message.rs:30 | text, metadata (provider, tokens, processing_time, model, session_id), reply_target | -- |
| 11 | OmegaError | Enum | backend/crates/omega-core/src/error.rs:4 | Provider, Channel, Config, Memory, Sandbox, Io, Serialization variants | thiserror |
| 12 | sanitize() | Function | backend/crates/omega-core/src/sanitize.rs:24 | Neutralizes prompt injection: role tags (zero-width space insertion), override phrase detection, code block flagging | -- |
| 13 | protected_command() | Function | backend/crates/omega-sandbox/src/lib.rs:44 | OS-level sandboxed Command: macOS Seatbelt, Linux Landlock, fallback plain | -- |
| 14 | is_write_blocked() | Function | backend/crates/omega-sandbox/src/lib.rs:55 | Code-level write enforcement: blocks {data_dir}/data/ and OS system directories | -- |
| 15 | is_read_blocked() | Function | backend/crates/omega-sandbox/src/lib.rs:105 | Code-level read enforcement: blocks {data_dir}/data/ and {data_dir}/config.toml | -- |

## Internal Dependencies
- Gateway -> load() -> Config -> all subsystems
- All providers -> protected_command() for CLI subprocess sandboxing
- ToolExecutor -> is_write_blocked() / is_read_blocked() for HTTP provider tools
- sanitize() called in handle_message() pipeline step
- SYSTEM_FACT_KEYS used by /purge, process_purge_facts(), is_valid_fact()

## Dead Code / Unused
- **ChatChoice::finish_reason**: `backend/crates/omega-providers/src/openai.rs:134` -- field parsed but marked `#[allow(dead_code)]`; available for future use
