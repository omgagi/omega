# Functionalities: Memory

## Overview
SQLite-backed persistent storage for conversations, messages, facts, tasks, outcomes, lessons, sessions, and audit. Uses FTS5 for semantic recall. Includes progressive onboarding and conditional context building.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | Store struct | Struct | backend/crates/omega-memory/src/store/mod.rs:~10 | SqlitePool + max_context_messages; 13 SQL migrations; WAL mode; 4 max connections | SQLite |
| 2 | bootstrap() | Method | backend/crates/omega-memory/src/store/mod.rs:~30 | Runs all 13 migrations via migration_tracking table; creates tables for conversations, messages, facts, tasks, audit, outcomes, lessons, sessions | SQLite |
| 3 | build_context() | Method | backend/crates/omega-memory/src/store/context.rs:~10 | Loads conversation history, facts, summaries, recall (FTS5), pending tasks, outcomes, lessons -- all conditionally based on ContextNeeds | Store |
| 4 | Progressive onboarding | Logic | backend/crates/omega-memory/src/store/context.rs:~50 | 5 stages: first_contact -> teach_help -> teach_personality -> teach_tasks -> teach_projects -> done; triggered by fact count and task creation | Memory |
| 5 | detect_language() | Function | backend/crates/omega-memory/src/store/context.rs:~100 | Stop-word heuristic for 7 languages + English default | -- |
| 6 | format_user_profile() | Function | backend/crates/omega-memory/src/store/context.rs:~120 | Groups facts: identity facts first, context facts second, then rest | -- |
| 7 | get_or_create_conversation() | Method | backend/crates/omega-memory/src/store/conversations.rs:~10 | 120-minute timeout for conversation reuse; project-scoped | SQLite |
| 8 | find_idle_conversations() | Method | backend/crates/omega-memory/src/store/conversations.rs:~40 | Finds conversations idle for 120+ minutes | SQLite |
| 9 | close_current_conversation() | Method | backend/crates/omega-memory/src/store/conversations.rs:~60 | Sets conversation status to 'closed' | SQLite |
| 10 | get_history() | Method | backend/crates/omega-memory/src/store/conversations.rs:~80 | Returns last N conversation summaries with timestamps | SQLite |
| 11 | get_memory_stats() | Method | backend/crates/omega-memory/src/store/conversations.rs:~100 | Returns (conversations_count, messages_count, facts_count) | SQLite |
| 12 | store_exchange() | Method | backend/crates/omega-memory/src/store/messages.rs:~10 | Stores user message + assistant response in current conversation | SQLite |
| 13 | search_messages() | Method | backend/crates/omega-memory/src/store/messages.rs:~54 | FTS5 full-text search across past conversations; excludes current; sanitizes query to prevent operator injection | SQLite |
| 14 | store_fact() | Method | backend/crates/omega-memory/src/store/facts.rs:~10 | Upsert a key-value fact for a sender | SQLite |
| 15 | get_fact() | Method | backend/crates/omega-memory/src/store/facts.rs:~25 | Get a single fact by key | SQLite |
| 16 | get_facts() | Method | backend/crates/omega-memory/src/store/facts.rs:~35 | Get all facts for a sender | SQLite |
| 17 | delete_fact() | Method | backend/crates/omega-memory/src/store/facts.rs:~45 | Delete a single fact; returns whether it existed | SQLite |
| 18 | delete_facts() | Method | backend/crates/omega-memory/src/store/facts.rs:~55 | Delete all facts or filtered set; returns count deleted | SQLite |
| 19 | get_all_facts_by_key() | Method | backend/crates/omega-memory/src/store/facts.rs:~65 | Cross-user lookup by key (used by heartbeat to find all active_project users) | SQLite |
| 20 | store_alias() | Method | backend/crates/omega-memory/src/store/facts.rs:~75 | Cross-channel identity: maps (channel, sender_id) pairs to same user | SQLite |
| 21 | resolve_alias() | Method | backend/crates/omega-memory/src/store/facts.rs:~85 | Resolves canonical sender_id from any known alias | SQLite |
| 22 | create_task() | Method | backend/crates/omega-memory/src/store/tasks.rs:~10 | Task creation with 2-level dedup | SQLite |
| 23 | complete_task() | Method | backend/crates/omega-memory/src/store/tasks.rs:~50 | Task completion with recurring schedule advancement | SQLite |
| 24 | fail_task() | Method | backend/crates/omega-memory/src/store/tasks.rs:~90 | Task failure with retry logic | SQLite |
| 25 | cancel_task() | Method | backend/crates/omega-memory/src/store/tasks.rs:~120 | Cancel task by ID prefix matching | SQLite |
| 26 | store_outcome() | Method | backend/crates/omega-memory/src/store/outcomes.rs:~10 | Stores raw reward outcomes with project tagging | SQLite |
| 27 | store_lesson() | Method | backend/crates/omega-memory/src/store/outcomes.rs:~30 | Content-based dedup (bumps occurrences); cap of 10 per (sender, domain, project) | SQLite |
| 28 | get_recent_outcomes() | Method | backend/crates/omega-memory/src/store/outcomes.rs:~55 | Returns recent outcomes with optional project filter | SQLite |
| 29 | get_lessons() | Method | backend/crates/omega-memory/src/store/outcomes.rs:~70 | Project-specific first, then general; layered retrieval | SQLite |
| 30 | get_or_create_session() | Method | backend/crates/omega-memory/src/store/sessions.rs:~10 | CLI session persistence per (channel, sender_id, project); survives restarts | SQLite |
| 31 | update_session() | Method | backend/crates/omega-memory/src/store/sessions.rs:~30 | Updates session_id for existing session record | SQLite |
| 32 | AuditLogger::log() | Method | backend/crates/omega-memory/src/audit.rs:~51 | Writes audit entry: channel, sender, input, output, provider, model, processing_ms, status, denial_reason | SQLite |

## Internal Dependencies
- build_context() -> get_or_create_conversation() -> recent messages + facts + summaries + recall + tasks + outcomes + lessons
- store_exchange() -> get_or_create_conversation() (120min timeout)
- search_messages() -> FTS5 index on messages table
- create_task() -> 2-level dedup check
- complete_task() -> recurring schedule advancement
- store_lesson() -> content dedup + cap enforcement

## Dead Code / Unused
None detected.
