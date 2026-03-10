# Functionalities: Gateway (Core + Pipeline + Routing)

## Overview

The central gateway orchestrates the full message pipeline from arrival through authentication, context building, prompt composition (all sections always injected), model routing, provider call, marker processing, and response delivery. It also manages background tasks (summarizer, scheduler, heartbeat, CLAUDE.md maintenance).

## Functionalities

| # | Name | Type | Location | Description | Dependencies |
|---|------|------|----------|-------------|--------------|
| 1 | Gateway struct | Model | `backend/src/gateway/mod.rs:80` | Central gateway holding all runtime state: provider, channels, memory, audit, configs, prompts, skills, projects, model_fast/complex, active_senders mutex, heartbeat_interval atomic, heartbeat_notify | All subsystems |
| 2 | Gateway::new() | Service | `backend/src/gateway/mod.rs:112` | Constructs gateway from GatewayConfig bundle, initializes AuditLogger and loads projects | GatewayConfig |
| 3 | Gateway::run() | Service | `backend/src/gateway/mod.rs:142` | Main event loop: starts channels, spawns API server, summarizer, scheduler, heartbeat, CLAUDE.md loops; dispatches messages via tokio::select; graceful shutdown on SIGINT | All background tasks |
| 4 | Gateway::dispatch_message() | Service | `backend/src/gateway/mod.rs:342` | Message dispatcher with sender buffering: if sender has active call, buffers message; otherwise processes and drains buffered messages | handle_message |
| 5 | Gateway::shutdown() | Service | `backend/src/gateway/mod.rs:391` | Graceful shutdown: aborts background tasks, summarizes all active conversations, stops all channels | summarize_conversation |
| 6 | Gateway::send_text() | Utility | `backend/src/gateway/mod.rs:449` | Sends a plain text message back to the sender via their channel | Channel trait |
| 7 | Gateway::handle_message() | Pipeline | `backend/src/gateway/pipeline.rs:20` | Full message pipeline: auth check, sanitize, save attachments, cross-channel identity, active project lookup, command dispatch, typing indicator, pending session checks, keyword matching, prompt building, context building, MCP matching, session persistence, model routing, provider call | All subsystems |
| 8 | Auth check | Pipeline Stage | `backend/src/gateway/pipeline.rs:35` | Checks authorization and logs denied access to audit | check_auth |
| 9 | Input sanitization | Pipeline Stage | `backend/src/gateway/pipeline.rs:65` | Sanitizes user input against prompt injection | sanitize::sanitize |
| 10 | Attachment saving | Pipeline Stage | `backend/src/gateway/pipeline.rs:77` | Saves incoming image attachments to inbox directory with RAII cleanup guard | ensure_inbox_dir, save_attachments_to_inbox, InboxGuard |
| 11 | Cross-channel identity | Pipeline Stage | `backend/src/gateway/pipeline.rs:93` | Resolves cross-channel user identity via alias system; detects new users and sets language | Store::is_new_user, find_canonical_user, create_alias, resolve_sender_id |
| 12 | Command dispatch | Pipeline Stage | `backend/src/gateway/pipeline.rs:137` | Parses /commands and dispatches to command handlers; intercepts /forget and /setup specially | commands::Command::parse, commands::handle |
| 13 | Keyword matching | Pipeline Stage | `backend/src/gateway/pipeline.rs:268` | Matches 9 keyword categories against message text to gate prompt sections: scheduling, recall, tasks, projects, builds, meta, profile, summaries, outcomes | kw_match, keyword arrays |
| 14 | Build request early return | Pipeline Stage | `backend/src/gateway/pipeline.rs:296` | When build keywords detected, runs discovery agent before confirmation instead of normal pipeline | handle_build_keyword_discovery |
| 15 | Context building | Pipeline Stage | `backend/src/gateway/pipeline.rs:328` | Builds conversation context from memory with selective loading based on ContextNeeds | Store::build_context |
| 16 | MCP server matching | Pipeline Stage | `backend/src/gateway/pipeline.rs:351` | Matches skill triggers against message text to activate MCP servers | omega_skills::match_skill_triggers |
| 17 | Session-based prompt persistence | Pipeline Stage | `backend/src/gateway/pipeline.rs:356` | For Claude Code CLI: reuses session_id across messages, sends minimal context update (time + conditional sections) instead of full system prompt | Store::get_session |
| 18 | Model routing | Pipeline Stage | `backend/src/gateway/pipeline.rs:419` | Routes non-build messages to fast model (Sonnet); build requests handled separately | -- |
| 19 | check_auth_inner() | Service | `backend/src/gateway/auth.rs:10` | Pure function: Telegram (allowed_users list, empty=deny), WhatsApp (allowed_users list, empty=allow), unknown channel=deny | ChannelConfig |
| 20 | handle_whatsapp_qr() | Service | `backend/src/gateway/auth.rs:68` | WhatsApp QR pairing flow: restarts bot for fresh QR, generates PNG, sends via photo, waits for pairing confirmation | WhatsAppChannel |
| 21 | handle_direct_response() | Service | `backend/src/gateway/routing.rs:18` | Direct response path: workspace image snapshot, provider call as background task, delayed status updater (15s/120s), session retry on failure, process markers, store exchange, audit log, send response, deliver workspace images | Provider, process_markers, AuditLogger |
| 22 | build_system_prompt() | Service | `backend/src/gateway/prompt_builder.rs:13` | Builds system prompt with conditional sections: identity+soul+system, provider/model info, time, platform hints, project awareness, scheduling/projects/builds/meta sections, active project ROLE.md + skills, heartbeat checklist | Prompts, projects, skills |
| 23 | handle_pending_discovery() | Service | `backend/src/gateway/pipeline_builds.rs:24` | Handles active discovery session: TTL check, cancellation, multi-round Q&A with discovery agent, produces enriched brief | parse_discovery_output, run_build_phase |
| 24 | handle_pending_build_confirmation() | Service | `backend/src/gateway/pipeline_builds.rs:249` | Handles pending build confirmation: TTL check, confirmation/cancellation/fallthrough | handle_build_request, is_build_confirmed |
| 25 | handle_build_keyword_discovery() | Service | `backend/src/gateway/pipeline_builds.rs:325` | Runs discovery agent when build keywords detected: either direct confirmation (specific request) or multi-round discovery (vague request) | run_build_phase, parse_discovery_output |

## Internal Dependencies

- handle_message() calls auth -> sanitize -> identity -> commands -> keyword matching -> prompt building -> context building -> MCP matching -> session -> model routing -> handle_direct_response()
- handle_direct_response() calls provider.complete() -> process_markers() -> store_exchange() -> audit.log() -> channel.send()
- dispatch_message() wraps handle_message() with sender buffering
- run() spawns all background tasks and the main event loop

## Dead Code / Unused

- None detected. All gateway submodules are actively used.
