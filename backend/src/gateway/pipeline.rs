//! Message processing pipeline — the main handle_message flow.

use std::path::PathBuf;
use std::sync::atomic::Ordering;

use tracing::{error, info, warn};

use omega_core::config::shellexpand;
use omega_core::{context::ContextNeeds, message::IncomingMessage, sanitize};
use omega_memory::{
    audit::{AuditEntry, AuditStatus},
    detect_language,
};

use super::builds_agents::AgentFilesGuard;
use super::builds_parse::{
    discovery_file_path, parse_discovery_output, parse_discovery_round, truncate_brief_preview,
    DiscoveryOutput,
};
use super::keywords::*;
use super::Gateway;
use crate::commands;
use crate::markers::*;

impl Gateway {
    /// Process a single incoming message through the full pipeline.
    pub(super) async fn handle_message(&self, mut incoming: IncomingMessage) {
        let preview = if incoming.text.chars().count() > 60 {
            let truncated: String = incoming.text.chars().take(60).collect();
            format!("{truncated}...")
        } else {
            incoming.text.clone()
        };
        info!(
            "[{}] {} says: {}",
            incoming.channel,
            incoming.sender_name.as_deref().unwrap_or("unknown"),
            preview
        );

        // --- 1. AUTH CHECK ---
        if self.auth_config.enabled {
            if let Some(reason) = self.check_auth(&incoming) {
                warn!(
                    "auth denied for {} on {}: {reason}",
                    incoming.sender_id, incoming.channel
                );

                let _ = self
                    .audit
                    .log(&AuditEntry {
                        channel: incoming.channel.clone(),
                        sender_id: incoming.sender_id.clone(),
                        sender_name: incoming.sender_name.clone(),
                        input_text: incoming.text.clone(),
                        output_text: None,
                        provider_used: None,
                        model: None,
                        processing_ms: None,
                        status: AuditStatus::Denied,
                        denial_reason: Some(reason),
                    })
                    .await;

                self.send_text(&incoming, &self.auth_config.deny_message)
                    .await;
                return;
            }
        }

        // --- 2. SANITIZE INPUT ---
        let sanitized = sanitize::sanitize(&incoming.text);
        if sanitized.was_modified {
            warn!(
                "sanitized input from {}: {:?}",
                incoming.sender_id, sanitized.warnings
            );
        }

        let mut clean_incoming = incoming.clone();
        clean_incoming.text = sanitized.text;

        // --- 2a. SAVE INCOMING IMAGE ATTACHMENTS ---
        let _inbox_guard = if !incoming.attachments.is_empty() {
            let inbox = ensure_inbox_dir(&self.data_dir);
            let paths = save_attachments_to_inbox(&inbox, &incoming.attachments);
            for path in &paths {
                clean_incoming.text = format!(
                    "[Attached image: {}]\n{}",
                    path.display(),
                    clean_incoming.text
                );
            }
            InboxGuard::new(paths)
        } else {
            InboxGuard::new(Vec::new())
        };

        // --- 2b. CROSS-CHANNEL USER IDENTITY ---
        let original_sender_id = incoming.sender_id.clone();
        if let Ok(true) = self.memory.is_new_user(&incoming.sender_id).await {
            if let Ok(Some(canonical_id)) =
                self.memory.find_canonical_user(&incoming.sender_id).await
            {
                let _ = self
                    .memory
                    .create_alias(&incoming.sender_id, &canonical_id)
                    .await;
                incoming.sender_id = canonical_id.clone();
                clean_incoming.sender_id = canonical_id;
                info!(
                    "aliased {} → {} (cross-channel identity)",
                    original_sender_id, incoming.sender_id
                );
            } else {
                let lang = detect_language(&clean_incoming.text);
                let _ = self
                    .memory
                    .store_fact(&incoming.sender_id, "welcomed", "true")
                    .await;
                let _ = self
                    .memory
                    .store_fact(&incoming.sender_id, "preferred_language", lang)
                    .await;
                info!("new user detected {} ({})", incoming.sender_id, lang);
            }
        } else if let Ok(resolved) = self.memory.resolve_sender_id(&incoming.sender_id).await {
            if resolved != incoming.sender_id {
                incoming.sender_id = resolved.clone();
                clean_incoming.sender_id = resolved;
            }
        }

        // --- 3. COMMAND DISPATCH ---
        let projects = omega_skills::load_projects(&self.data_dir);
        if let Some(cmd) = commands::Command::parse(&clean_incoming.text) {
            if matches!(cmd, commands::Command::Forget) {
                let response = self
                    .handle_forget(&incoming.channel, &incoming.sender_id)
                    .await;
                self.send_text(&incoming, &response).await;
                return;
            }

            let ctx = commands::CommandContext {
                store: &self.memory,
                channel: &incoming.channel,
                sender_id: &incoming.sender_id,
                text: &clean_incoming.text,
                uptime: &self.uptime,
                provider_name: self.provider.name(),
                skills: &self.skills,
                projects: &projects,
                heartbeat_enabled: self.heartbeat_config.enabled,
                heartbeat_interval_mins: self.heartbeat_interval.load(Ordering::Relaxed),
            };
            let response = commands::handle(cmd, &ctx).await;

            if response.trim() == "WHATSAPP_QR" {
                self.handle_whatsapp_qr(&incoming).await;
                return;
            }

            self.send_text(&incoming, &response).await;
            return;
        }

        // --- 4. TYPING INDICATOR ---
        let typing_channel = self.channels.get(&incoming.channel).cloned();
        let typing_target = incoming.reply_target.clone();
        let typing_handle = if let (Some(ch), Some(ref target)) = (&typing_channel, &typing_target)
        {
            let ch = ch.clone();
            let target = target.clone();
            let _ = ch.send_typing(&target).await;
            Some(tokio::spawn(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    if ch.send_typing(&target).await.is_err() {
                        break;
                    }
                }
            }))
        } else {
            None
        };

        // --- 4. BUILD CONTEXT FROM MEMORY ---
        let active_project: Option<String> = self
            .memory
            .get_fact(&incoming.sender_id, "active_project")
            .await
            .ok()
            .flatten();

        // --- 4a-DISCOVERY. PENDING DISCOVERY SESSION CHECK ---
        let pending_discovery: Option<String> = self
            .memory
            .get_fact(&incoming.sender_id, "pending_discovery")
            .await
            .ok()
            .flatten();

        if let Some(discovery_value) = pending_discovery {
            // Parse timestamp from "timestamp|sender_id" format.
            let (stored_ts, _) = discovery_value
                .split_once('|')
                .unwrap_or(("0", &discovery_value));
            let created_at: i64 = stored_ts.parse().unwrap_or(0);
            let now = chrono::Utc::now().timestamp();
            let expired = (now - created_at) > DISCOVERY_TTL_SECS;

            let user_lang = self
                .memory
                .get_fact(&incoming.sender_id, "preferred_language")
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "English".to_string());

            if expired {
                // Clean up expired session.
                let _ = self
                    .memory
                    .delete_fact(&incoming.sender_id, "pending_discovery")
                    .await;
                let disc_file = discovery_file_path(&self.data_dir, &incoming.sender_id);
                let _ = tokio::fs::remove_file(&disc_file).await;
                info!("[{}] discovery session expired", incoming.channel);
                self.send_text(&incoming, discovery_expired_message(&user_lang))
                    .await;
                // Fall through — the current message might be a new build request or normal chat.
            } else if is_build_cancelled(&clean_incoming.text) {
                // User cancelled discovery.
                let _ = self
                    .memory
                    .delete_fact(&incoming.sender_id, "pending_discovery")
                    .await;
                let disc_file = discovery_file_path(&self.data_dir, &incoming.sender_id);
                let _ = tokio::fs::remove_file(&disc_file).await;
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(&incoming, discovery_cancelled_message(&user_lang))
                    .await;
                return;
            } else {
                // Active discovery session — process the user's answer.
                let disc_file = discovery_file_path(&self.data_dir, &incoming.sender_id);
                let mut discovery_context = tokio::fs::read_to_string(&disc_file)
                    .await
                    .unwrap_or_default();

                // Parse current round from file header.
                let current_round = parse_discovery_round(&discovery_context);

                // Append user's answer to the file.
                discovery_context
                    .push_str(&format!("\n### User Response\n{}\n", clean_incoming.text));

                let is_final_round = current_round >= 3;
                let next_round = current_round + 1;

                // Build prompt for discovery agent.
                let agent_prompt = if is_final_round {
                    format!(
                        "This is the FINAL round. You MUST output DISCOVERY_COMPLETE with an Idea Brief.\n\
                         Synthesize everything below into a brief.\n\n{discovery_context}"
                    )
                } else {
                    format!(
                        "Discovery round {next_round}/3. Read the accumulated context and either:\n\
                         - Output DISCOVERY_QUESTIONS if you need more info\n\
                         - Output DISCOVERY_COMPLETE if you have enough\n\n{discovery_context}"
                    )
                };

                // Write agent files and run discovery agent.
                let workspace_dir = PathBuf::from(shellexpand(&self.data_dir)).join("workspace");
                let _agent_guard = match AgentFilesGuard::write(&workspace_dir).await {
                    Ok(guard) => guard,
                    Err(e) => {
                        warn!("Failed to write agent files for discovery: {e}");
                        // Clean up discovery state and fall through.
                        let _ = self
                            .memory
                            .delete_fact(&incoming.sender_id, "pending_discovery")
                            .await;
                        let _ = tokio::fs::remove_file(&disc_file).await;
                        if let Some(h) = typing_handle {
                            h.abort();
                        }
                        self.send_text(&incoming, "Discovery failed (agent setup error).")
                            .await;
                        return;
                    }
                };

                let result = self
                    .run_build_phase(
                        "build-discovery",
                        &agent_prompt,
                        &self.model_complex,
                        Some(15),
                    )
                    .await;

                match result {
                    Ok(output) => {
                        let parsed = parse_discovery_output(&output);
                        // If final round, force Complete.
                        let parsed = if is_final_round {
                            match parsed {
                                DiscoveryOutput::Questions(q) => DiscoveryOutput::Complete(q),
                                other => other,
                            }
                        } else {
                            parsed
                        };

                        match parsed {
                            DiscoveryOutput::Questions(questions) => {
                                // Update discovery file with new round header.
                                let updated = format!(
                                    "{discovery_context}\n## Round {next_round}\n### Agent Questions\n{questions}\n"
                                );
                                // Update ROUND: header in file.
                                let updated = if updated.contains("ROUND:") {
                                    updated.replacen(
                                        &format!("ROUND: {current_round}"),
                                        &format!("ROUND: {next_round}"),
                                        1,
                                    )
                                } else {
                                    updated
                                };
                                let _ = tokio::fs::write(&disc_file, &updated).await;

                                // Send follow-up questions (next_round >= 2 in continuation path).
                                let msg =
                                    discovery_followup_message(&user_lang, &questions, next_round);
                                if let Some(h) = typing_handle {
                                    h.abort();
                                }
                                self.send_text(&incoming, &msg).await;
                                return;
                            }
                            DiscoveryOutput::Complete(brief) => {
                                // Discovery complete — clean up and hand off to confirmation.
                                let _ = self
                                    .memory
                                    .delete_fact(&incoming.sender_id, "pending_discovery")
                                    .await;
                                let _ = tokio::fs::remove_file(&disc_file).await;

                                // Store enriched brief as pending_build_request.
                                let stamped =
                                    format!("{}|{}", chrono::Utc::now().timestamp(), brief);
                                let _ = self
                                    .memory
                                    .store_fact(
                                        &incoming.sender_id,
                                        "pending_build_request",
                                        &stamped,
                                    )
                                    .await;

                                // Send discovery complete + confirmation message.
                                let preview = truncate_brief_preview(&brief, 300);
                                let msg = discovery_complete_message(&user_lang, &preview);
                                if let Some(h) = typing_handle {
                                    h.abort();
                                }
                                self.send_text(&incoming, &msg).await;
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        // Discovery agent failed — clean up, inform user.
                        let _ = self
                            .memory
                            .delete_fact(&incoming.sender_id, "pending_discovery")
                            .await;
                        let _ = tokio::fs::remove_file(&disc_file).await;
                        if let Some(h) = typing_handle {
                            h.abort();
                        }
                        self.send_text(&incoming, &format!("Discovery failed: {e}"))
                            .await;
                        return;
                    }
                }
            }
        }

        // --- 4a. PENDING BUILD CONFIRMATION CHECK ---
        // If user previously triggered a build keyword, we stored their request (with
        // a Unix timestamp prefix) and asked for an explicit confirmation phrase.
        // Format: "<unix_timestamp>|<original request text>"
        let pending_build: Option<String> = self
            .memory
            .get_fact(&incoming.sender_id, "pending_build_request")
            .await
            .ok()
            .flatten();

        if let Some(stored_value) = pending_build {
            // Always clear the pending state — one-shot.
            let _ = self
                .memory
                .delete_fact(&incoming.sender_id, "pending_build_request")
                .await;

            // Parse "timestamp|request_text" and check TTL.
            let (stored_ts, stored_request) =
                stored_value.split_once('|').unwrap_or(("0", &stored_value));
            let created_at: i64 = stored_ts.parse().unwrap_or(0);
            let now = chrono::Utc::now().timestamp();
            let expired = (now - created_at) > BUILD_CONFIRM_TTL_SECS;

            if expired {
                info!(
                    "[{}] pending build expired ({}s ago) — ignoring",
                    incoming.channel,
                    now - created_at
                );
            } else if is_build_confirmed(&clean_incoming.text) {
                info!(
                    "[{}] build CONFIRMED → multi-phase pipeline",
                    incoming.channel
                );
                let mut build_incoming = incoming.clone();
                build_incoming.text = stored_request.to_string();
                self.handle_build_request(&build_incoming, typing_handle)
                    .await;
                return;
            } else if is_build_cancelled(&clean_incoming.text) {
                info!("[{}] build explicitly CANCELLED by user", incoming.channel);
                let user_lang = self
                    .memory
                    .get_fact(&incoming.sender_id, "preferred_language")
                    .await
                    .ok()
                    .flatten()
                    .unwrap_or_else(|| "English".to_string());
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(&incoming, build_cancelled_message(&user_lang))
                    .await;
                return;
            } else {
                info!(
                    "[{}] build NOT confirmed — proceeding with normal pipeline",
                    incoming.channel
                );
            }
            // Fall through to normal message processing.
        }

        let msg_lower = clean_incoming.text.to_lowercase();
        let needs_scheduling = kw_match(&msg_lower, SCHEDULING_KW);
        let needs_recall = kw_match(&msg_lower, RECALL_KW);
        let needs_tasks = needs_scheduling || kw_match(&msg_lower, TASKS_KW);
        let needs_projects = kw_match(&msg_lower, PROJECTS_KW);
        let needs_builds = kw_match(&msg_lower, BUILDS_KW);
        let needs_meta = kw_match(&msg_lower, META_KW);
        let needs_profile = kw_match(&msg_lower, PROFILE_KW)
            || needs_scheduling // timezone/location needed
            || needs_recall // past context needs identity
            || needs_tasks; // task context needs identity
        let needs_summaries = needs_recall;
        let needs_outcomes = kw_match(&msg_lower, OUTCOMES_KW);

        info!(
            "[{}] prompt needs: scheduling={} recall={} tasks={} projects={} builds={} meta={} profile={} summaries={} outcomes={}",
            incoming.channel,
            needs_scheduling,
            needs_recall,
            needs_tasks,
            needs_projects,
            needs_builds,
            needs_meta,
            needs_profile,
            needs_summaries,
            needs_outcomes,
        );

        // --- 4b. BUILD REQUESTS — run discovery before confirmation ---
        // When a build keyword is detected, run the discovery agent first to
        // clarify the request. If the request is already specific, discovery
        // completes in one shot and flows into the confirmation gate.
        if needs_builds {
            info!(
                "[{}] build keyword detected \u{2192} starting discovery",
                incoming.channel
            );

            let user_lang = self
                .memory
                .get_fact(&incoming.sender_id, "preferred_language")
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "English".to_string());

            // Write agent files for discovery.
            let workspace_dir = PathBuf::from(shellexpand(&self.data_dir)).join("workspace");
            let _agent_guard = match AgentFilesGuard::write(&workspace_dir).await {
                Ok(guard) => guard,
                Err(e) => {
                    // Fall back to direct confirmation if agent files fail.
                    warn!("Failed to write agent files for discovery: {e}");
                    let stamped = format!("{}|{}", chrono::Utc::now().timestamp(), incoming.text);
                    let _ = self
                        .memory
                        .store_fact(&incoming.sender_id, "pending_build_request", &stamped)
                        .await;
                    let confirm_msg = build_confirm_message(&user_lang, &incoming.text);
                    if let Some(h) = typing_handle {
                        h.abort();
                    }
                    self.send_text(&incoming, &confirm_msg).await;
                    return;
                }
            };

            // Run first discovery round with raw request.
            let agent_prompt = format!(
                "Discovery round 1/3. Analyze this build request and decide:\n\
                 - If specific enough, output DISCOVERY_COMPLETE with an Idea Brief\n\
                 - If vague, output DISCOVERY_QUESTIONS with 3-5 clarifying questions\n\n\
                 User request: {}",
                incoming.text
            );

            let result = self
                .run_build_phase(
                    "build-discovery",
                    &agent_prompt,
                    &self.model_complex,
                    Some(15),
                )
                .await;

            match result {
                Ok(output) => {
                    let parsed = parse_discovery_output(&output);
                    match parsed {
                        DiscoveryOutput::Complete(brief) => {
                            // Request was specific — skip multi-round, go straight to confirmation.
                            let stamped = format!("{}|{}", chrono::Utc::now().timestamp(), brief);
                            let _ = self
                                .memory
                                .store_fact(&incoming.sender_id, "pending_build_request", &stamped)
                                .await;
                            let preview = truncate_brief_preview(&brief, 300);
                            let msg = discovery_complete_message(&user_lang, &preview);
                            if let Some(h) = typing_handle {
                                h.abort();
                            }
                            self.send_text(&incoming, &msg).await;
                            return;
                        }
                        DiscoveryOutput::Questions(questions) => {
                            // Request was vague — start multi-round discovery session.
                            let disc_file =
                                discovery_file_path(&self.data_dir, &incoming.sender_id);
                            let discovery_dir = disc_file
                                .parent()
                                .expect("discovery path always has parent");
                            let _ = tokio::fs::create_dir_all(discovery_dir).await;

                            // Create discovery file with round 1 content.
                            let file_content = format!(
                                "# Discovery Session\n\n\
                                 CREATED: {}\n\
                                 ROUND: 1\n\
                                 ORIGINAL_REQUEST: {}\n\n\
                                 ## Round 1\n\
                                 ### Agent Questions\n{}\n",
                                chrono::Utc::now().timestamp(),
                                incoming.text,
                                questions
                            );
                            let _ = tokio::fs::write(&disc_file, &file_content).await;

                            // Store pending_discovery fact.
                            let stamped = format!(
                                "{}|{}",
                                chrono::Utc::now().timestamp(),
                                incoming.sender_id
                            );
                            let _ = self
                                .memory
                                .store_fact(&incoming.sender_id, "pending_discovery", &stamped)
                                .await;

                            // Send questions to user.
                            let msg = discovery_intro_message(&user_lang, &questions);
                            if let Some(h) = typing_handle {
                                h.abort();
                            }
                            self.send_text(&incoming, &msg).await;
                            return;
                        }
                    }
                }
                Err(e) => {
                    // Discovery failed — fall back to old behavior (direct confirmation).
                    warn!("Discovery agent failed, falling back to direct confirmation: {e}");
                    let stamped = format!("{}|{}", chrono::Utc::now().timestamp(), incoming.text);
                    let _ = self
                        .memory
                        .store_fact(&incoming.sender_id, "pending_build_request", &stamped)
                        .await;
                    let msg = build_confirm_message(&user_lang, &incoming.text);
                    if let Some(h) = typing_handle {
                        h.abort();
                    }
                    self.send_text(&incoming, &msg).await;
                    return;
                }
            }
        }

        let system_prompt = self.build_system_prompt(
            &incoming,
            &msg_lower,
            active_project.as_deref(),
            &projects,
            needs_scheduling,
            needs_projects,
            needs_builds,
            needs_meta,
        );

        info!(
            "[{}] system prompt: ~{} tokens ({} chars)",
            incoming.channel,
            system_prompt.len() / 4,
            system_prompt.len()
        );

        let context_needs = ContextNeeds {
            recall: needs_recall,
            pending_tasks: needs_tasks,
            profile: needs_profile,
            summaries: needs_summaries,
            outcomes: needs_outcomes,
        };

        let context = match self
            .memory
            .build_context(
                &clean_incoming,
                &system_prompt,
                &context_needs,
                active_project.as_deref(),
            )
            .await
        {
            Ok(ctx) => ctx,
            Err(e) => {
                error!("failed to build context: {e}");
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(&incoming, &format!("Memory error: {e}"))
                    .await;
                return;
            }
        };

        // --- 4b. MATCH SKILL TRIGGERS FOR MCP SERVERS ---
        let mcp_servers = omega_skills::match_skill_triggers(&self.skills, &clean_incoming.text);
        let mut context = context;
        context.mcp_servers = mcp_servers;

        // --- 4c. SESSION-BASED PROMPT PERSISTENCE (Claude Code CLI only) ---
        let project_key = active_project.as_deref().unwrap_or("");
        let full_system_prompt = context.system_prompt.clone();
        let full_history = context.history.clone();

        if self.provider.name() == "claude-code" {
            if let Ok(Some(sid)) = self
                .memory
                .get_session(&incoming.channel, &incoming.sender_id, project_key)
                .await
            {
                context.session_id = Some(sid);

                let mut minimal = format!(
                    "Current time: {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M %Z")
                );
                if needs_scheduling {
                    minimal.push_str("\n\n");
                    minimal.push_str(&self.prompts.scheduling);
                }
                if needs_projects {
                    minimal.push_str("\n\n");
                    minimal.push_str(&self.prompts.projects_rules);
                }
                if needs_builds {
                    minimal.push_str("\n\n");
                    minimal.push_str(&self.prompts.builds);
                }
                if needs_meta {
                    minimal.push_str("\n\n");
                    minimal.push_str(&self.prompts.meta);
                }

                // Project awareness (lightweight — name only, not ROLE.md).
                // ROLE.md was injected in the first message of this session and
                // persists in the CLI's context. Re-injecting it on every
                // continuation wastes tokens. It will be re-injected when the
                // session expires (2h idle → summarize → new session).
                if !projects.is_empty() {
                    let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
                    let active_note = match active_project.as_deref() {
                        Some(ap) => format!(" (active: {ap})"),
                        None => String::new(),
                    };
                    minimal.push_str(&format!(
                        "\n\nAvailable projects: [{}]{}.",
                        names.join(", "),
                        active_note,
                    ));
                }

                context.system_prompt = minimal;
                context.history.clear();

                info!(
                    "[{}] system prompt: ~{} tokens ({} chars) [session continuation]",
                    incoming.channel,
                    context.system_prompt.len() / 4,
                    context.system_prompt.len()
                );
            }
        }

        // --- 5. MODEL ROUTING ---
        // All non-build messages go DIRECT (single provider call).
        // Build requests were handled above via early return to handle_build_request().
        info!(
            "[{}] classification: DIRECT → model {}",
            incoming.channel, self.model_fast
        );
        context.model = Some(self.model_fast.clone());

        self.handle_direct_response(
            &incoming,
            context,
            full_system_prompt,
            full_history,
            typing_handle,
            active_project.as_deref(),
            project_key,
        )
        .await;

        // Inbox images are cleaned up automatically by _inbox_guard (RAII Drop).
    }

    /// Build the system prompt with conditional section injection.
    #[allow(clippy::too_many_arguments)]
    fn build_system_prompt(
        &self,
        incoming: &IncomingMessage,
        msg_lower: &str,
        active_project: Option<&str>,
        projects: &[omega_skills::Project],
        needs_scheduling: bool,
        needs_projects: bool,
        needs_builds: bool,
        needs_meta: bool,
    ) -> String {
        let mut prompt = format!(
            "{}\n\n{}\n\n{}",
            self.prompts.identity, self.prompts.soul, self.prompts.system
        );

        prompt.push_str(&format!(
            "\n\nYou are running on provider '{}', model '{}'.",
            self.provider.name(),
            self.model_fast
        ));
        prompt.push_str(&format!(
            "\nCurrent time: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M %Z")
        ));

        match incoming.channel.as_str() {
            "whatsapp" => prompt.push_str(
                "\n\nPlatform: WhatsApp. Avoid markdown tables and headers — use bold (*text*) and bullet lists instead.",
            ),
            "telegram" => prompt.push_str(
                "\n\nPlatform: Telegram. Markdown is supported (bold, italic, code blocks).",
            ),
            _ => {}
        }

        // Always-on project awareness (compact hint, ~40-50 tokens)
        if !projects.is_empty() {
            let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
            let active_note = match active_project {
                Some(ap) => format!(" (active: {ap})"),
                None => String::new(),
            };
            prompt.push_str(&format!(
                "\n\nAvailable projects: [{}]{}. When conversation aligns with a project domain, activate it. For new recurring domains, suggest creating a project (~/.omega/projects/<name>/ROLE.md). User commands: /projects, /project <name>, /project off.",
                names.join(", "),
                active_note,
            ));
        } else {
            prompt.push_str(
                "\n\nNo projects yet. When the user works in a recurring domain (trading, real estate, fitness...), suggest creating a project (~/.omega/projects/<name>/ROLE.md). User commands: /projects, /project <name>, /project off."
            );
        }

        if needs_scheduling {
            prompt.push_str("\n\n");
            prompt.push_str(&self.prompts.scheduling);
        }
        if needs_projects {
            prompt.push_str("\n\n");
            prompt.push_str(&self.prompts.projects_rules);
        }
        if needs_builds {
            prompt.push_str("\n\n");
            prompt.push_str(&self.prompts.builds);
        }
        if needs_meta {
            prompt.push_str("\n\n");
            prompt.push_str(&self.prompts.meta);
        }

        // Active project ROLE.md — always injected when a project is active
        // (not gated by needs_projects — that gates management rules only)
        if let Some(project_name) = active_project {
            if let Some(proj) = projects.iter().find(|p| p.name == project_name) {
                prompt.push_str(&format!(
                    "\n\n---\n\n[Active project: {project_name}]\n{}",
                    proj.instructions
                ));
                // Inject project-declared skills
                if !proj.skills.is_empty() {
                    let project_skills: Vec<_> = self
                        .skills
                        .iter()
                        .filter(|s| proj.skills.contains(&s.name))
                        .collect();
                    if !project_skills.is_empty() {
                        prompt.push_str("\n\n[Project skills]");
                        for s in &project_skills {
                            let status = if s.available {
                                "installed"
                            } else {
                                "not installed"
                            };
                            prompt.push_str(&format!(
                                "\n- {} [{}]: {} → Read {}",
                                s.name,
                                status,
                                s.description,
                                s.path.display()
                            ));
                        }
                    }
                }
            }
        }

        if self.heartbeat_config.enabled {
            let needs_heartbeat = ["heartbeat", "watchlist", "monitoring", "checklist"]
                .iter()
                .any(|kw| msg_lower.contains(kw));
            if needs_heartbeat {
                if let Some(checklist) = read_heartbeat_file() {
                    prompt.push_str(
                        "\n\nCurrent heartbeat checklist (items monitored periodically):\n",
                    );
                    prompt.push_str(&checklist);
                }
                let mins = self.heartbeat_interval.load(Ordering::Relaxed);
                prompt.push_str(&format!(
                    "\n\nHeartbeat pulse: every {mins} minutes. You can report this when asked and change it with HEARTBEAT_INTERVAL: <1-1440>."
                ));
            }
        }

        prompt
    }
}
