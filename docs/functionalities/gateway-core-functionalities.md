# Functionalities: Gateway Core

## Overview
The gateway is the central orchestrator. It owns the event loop, dispatches messages through a 15-step pipeline, routes to providers, processes markers, manages sessions, and coordinates all background tasks.

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | Gateway struct | Struct | backend/src/gateway/mod.rs:~30 | Central state holder: provider, channels, memory, audit, config, prompts, skills, models, active_senders, heartbeat_interval | All crates |
| 2 | Gateway::run() | Method | backend/src/gateway/mod.rs:~60 | Main event loop: starts channels, spawns 5 background tasks (summarizer, scheduler, heartbeat, claudemd, API), select loop on messages + shutdown | All background tasks |
| 3 | Gateway::dispatch_message() | Method | backend/src/gateway/mod.rs:~100 | Per-sender buffering: if sender has active call, buffers messages and sends "Got it, I'll get to this next"; otherwise calls handle_message | handle_message |
| 4 | Gateway::shutdown() | Method | backend/src/gateway/mod.rs:~140 | Graceful shutdown: summarizes all active conversations, stops channels | summarizer, channels |
| 5 | handle_message() | Method | backend/src/gateway/pipeline.rs:~10 | 15-step pipeline: auth, sanitize, attachments, cross-channel identity, commands, typing, context, discovery, build confirmation, keywords, builds, system prompt, sessions, model routing, direct response | All gateway submodules |
| 6 | build_system_prompt() | Method | backend/src/gateway/pipeline.rs:~200 | Conditional prompt assembly: always Identity+Soul+System+time; conditionally injects scheduling/projects/builds/meta sections based on keyword detection; always injects project awareness and active ROLE.md | Prompts, keywords |
| 7 | check_auth() | Function | backend/src/gateway/auth.rs:~5 | Per-channel auth: telegram by user ID, whatsapp by sender string; returns deny message if unauthorized | Config |
| 8 | handle_whatsapp_qr() | Function | backend/src/gateway/auth.rs:~40 | QR pairing flow: gets WhatsApp channel, calls pairing_channels(), waits for QR/done events, sends QR as base64 PNG | WhatsApp channel |
| 9 | handle_direct_response() | Method | backend/src/gateway/routing.rs:~100 | Spawns provider call as background task; delayed status messages (15s first, 120s intervals); session retry on failure; marker processing; memory storage; audit; workspace image delivery | Provider, markers, memory, audit |
| 10 | process_markers() | Function | backend/src/gateway/process_markers.rs:~10 | Processes all markers from provider responses: SCHEDULE, SCHEDULE_ACTION, LANG_SWITCH, PERSONALITY, FORGET_CONVERSATION, CANCEL_TASK, UPDATE_TASK, PURGE_FACTS, PROJECT_ACTIVATE/DEACTIVATE, BUILD_PROPOSAL, WHATSAPP_QR, HEARTBEAT markers | All marker modules |
| 11 | send_task_confirmation() | Function | backend/src/gateway/process_markers.rs:~100 | Anti-hallucination: after scheduling, queries DB for actual results; detects similar existing tasks; sends localized confirmation | Memory, i18n |
| 12 | process_improvement_markers() | Function | backend/src/gateway/process_markers.rs:~150 | Handles SKILL_IMPROVE and BUG_REPORT markers: appends to skill files or BUG.md | Skills, filesystem |
| 13 | process_purge_facts() | Function | backend/src/gateway/process_markers.rs:~180 | Handles PURGE_FACTS marker: deletes all non-system facts, preserves SYSTEM_FACT_KEYS | Memory, config |
| 14 | background_summarizer() | Function | backend/src/gateway/summarizer.rs:~5 | Background loop every 60s: finds idle conversations (120min timeout), summarizes, extracts facts | Memory, provider |
| 15 | summarize_conversation() | Function | backend/src/gateway/summarizer.rs:~30 | Builds transcript, calls provider for summary + facts extraction, stores both | Memory, provider |
| 16 | handle_forget() | Method | backend/src/gateway/summarizer.rs:~80 | Closes conversation immediately, summarizes in background, clears session | Memory, sessions |
| 17 | ensure_claudemd() | Function | backend/src/claudemd.rs:~10 | Startup: writes bundled CLAUDE.md template to workspace, enriches with dynamic content via claude -p | Filesystem, provider |
| 18 | claudemd_loop() | Function | backend/src/claudemd.rs:~50 | Every 24h: refreshes dynamic content section of workspace CLAUDE.md | Provider, filesystem |
| 19 | start_api_server() | Function | backend/src/api.rs:~10 | Axum HTTP server: GET /api/health, POST /api/pair, GET /api/pair/status; Bearer token auth | WhatsApp channel |
| 20 | kw_match() | Function | backend/src/gateway/keywords.rs:~10 | Keyword matching: checks if message contains any keyword from a list (case-insensitive substring) | -- |
| 21 | SCHEDULING_KW / RECALL_KW / etc. | Constants | backend/src/gateway/keywords.rs:~20 | 9 keyword categories for conditional prompt injection, multilingual (8 languages), with typo tolerance | -- |
| 22 | is_valid_fact() | Function | backend/src/gateway/keywords.rs:~150 | Fact validation: rejects system keys, numeric keys, price values, pipe-delimited, oversized facts | config::SYSTEM_FACT_KEYS |
| 23 | phase_message() | Function | backend/src/gateway/builds_parse.rs:~100 | Localized build phase messages for all 8 languages | -- |
| 24 | send_text() | Method | backend/src/gateway/mod.rs | Sends a text message to the channel the incoming message came from | Channel |
| 25 | active_senders buffering | Feature | backend/src/gateway/mod.rs | DashMap<String, Vec<IncomingMessage>> prevents concurrent processing of same sender | -- |
| 26 | is_active_hours() | Function | backend/src/markers/helpers.rs | Checks if current UTC time is within configured active_start..active_end range | Config |
| 27 | workspace_image_snapshot/diff | Functions | backend/src/markers/helpers.rs | Captures before/after snapshots of workspace image files; sends new images to user | Filesystem, channel |
| 28 | InboxGuard | Struct | backend/src/markers/helpers.rs | RAII guard that removes saved attachment files on drop | Filesystem |

## Internal Dependencies
- handle_message() -> check_auth() -> send_text()
- handle_message() -> build_system_prompt() -> keyword detection
- handle_message() -> handle_direct_response() -> process_markers()
- handle_direct_response() -> store_exchange() -> memory
- background_summarizer() -> summarize_conversation() -> memory + provider
- Gateway::run() -> spawns all background loops

## Dead Code / Unused
- **classify_and_route()**: `backend/src/gateway/routing.rs:20` -- marked `#[allow(dead_code)]`, intentionally kept for future multi-step routing
- **execute_steps()**: `backend/src/gateway/routing.rs:68` -- marked `#[allow(dead_code)]`, intentionally kept for future multi-step execution
