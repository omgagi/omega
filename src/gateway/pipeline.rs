//! Message processing pipeline — the main handle_message flow.

use super::keywords::*;
use super::Gateway;
use crate::commands;
use crate::markers::*;
use omega_core::{context::ContextNeeds, message::IncomingMessage, sanitize};
use omega_memory::{
    audit::{AuditEntry, AuditStatus},
    detect_language,
};
use std::sync::atomic::Ordering;
use tracing::{error, info, warn};

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

        let msg_lower = clean_incoming.text.to_lowercase();
        let needs_scheduling = kw_match(&msg_lower, SCHEDULING_KW);
        let needs_recall = kw_match(&msg_lower, RECALL_KW);
        let needs_tasks = needs_scheduling || kw_match(&msg_lower, TASKS_KW);
        let needs_projects = kw_match(&msg_lower, PROJECTS_KW);
        let needs_meta = kw_match(&msg_lower, META_KW);
        let needs_profile = kw_match(&msg_lower, PROFILE_KW)
            || needs_scheduling // timezone/location needed
            || needs_recall // past context needs identity
            || needs_tasks; // task context needs identity
        let needs_summaries = needs_recall;
        let needs_outcomes = kw_match(&msg_lower, OUTCOMES_KW);

        info!(
            "[{}] prompt needs: scheduling={} recall={} tasks={} projects={} meta={} profile={} summaries={} outcomes={}",
            incoming.channel,
            needs_scheduling,
            needs_recall,
            needs_tasks,
            needs_projects,
            needs_meta,
            needs_profile,
            needs_summaries,
            needs_outcomes,
        );

        let system_prompt = self.build_system_prompt(
            &incoming,
            &msg_lower,
            active_project.as_deref(),
            &projects,
            needs_scheduling,
            needs_projects,
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
            .build_context(&clean_incoming, &system_prompt, &context_needs)
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
        let sender_key = format!("{}:{}", incoming.channel, incoming.sender_id);
        let full_system_prompt = context.system_prompt.clone();
        let full_history = context.history.clone();

        if self.provider.name() == "claude-code" {
            if let Ok(sessions) = self.cli_sessions.lock() {
                if let Some(sid) = sessions.get(&sender_key) {
                    context.session_id = Some(sid.clone());

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
                    if needs_meta {
                        minimal.push_str("\n\n");
                        minimal.push_str(&self.prompts.meta);
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
        }

        // --- 5. AUTONOMOUS CLASSIFICATION & MODEL ROUTING ---
        // Always pass full_history to the classifier — context.history may be empty
        // during session continuation (cleared at step 4c), leaving the classifier
        // blind to conversation context and causing misroutes.
        let skill_names: Vec<&str> = self.skills.iter().map(|s| s.name.as_str()).collect();
        if let Some(steps) = self
            .classify_and_route(
                &clean_incoming.text,
                active_project.as_deref(),
                &full_history,
                &skill_names,
            )
            .await
        {
            info!(
                "[{}] classification: {} steps → model {}",
                incoming.channel,
                steps.len(),
                self.model_complex
            );
            context.model = Some(self.model_complex.clone());
            self.execute_steps(&incoming, &clean_incoming.text, &context, &steps)
                .await;

            if let Some(h) = typing_handle {
                h.abort();
            }
            return;
        }

        // Direct response → Sonnet handles it.
        info!(
            "[{}] classification: DIRECT → model {}",
            incoming.channel, self.model_fast
        );
        context.model = Some(self.model_fast.clone());

        self.handle_direct_response(
            &incoming,
            context,
            &sender_key,
            full_system_prompt,
            full_history,
            typing_handle,
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

        if needs_scheduling {
            prompt.push_str("\n\n");
            prompt.push_str(&self.prompts.scheduling);
        }
        if needs_projects {
            prompt.push_str("\n\n");
            prompt.push_str(&self.prompts.projects_rules);
        }
        if needs_meta {
            prompt.push_str("\n\n");
            prompt.push_str(&self.prompts.meta);
        }

        if needs_projects {
            if let Some(project_name) = active_project {
                if let Some(instructions) =
                    omega_skills::get_project_instructions(projects, project_name)
                {
                    prompt.push_str(&format!(
                        "\n\n---\n\n[Active project: {project_name}]\n{instructions}"
                    ));
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
