//! Message processing pipeline — the main handle_message flow.

use std::sync::atomic::Ordering;

use tracing::{error, info, warn};

use omega_core::{context::ContextNeeds, message::IncomingMessage, sanitize};
use omega_memory::{
    audit::{AuditEntry, AuditStatus},
    detect_language,
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

        // --- 3. ACTIVE PROJECT (needed by commands + pipeline) ---
        let active_project: Option<String> = self
            .memory
            .get_fact(&incoming.sender_id, "active_project")
            .await
            .ok()
            .flatten();

        // --- 3a. COMMAND DISPATCH ---
        let projects = &self.projects;
        if let Some(cmd) = commands::Command::parse(&clean_incoming.text) {
            if matches!(cmd, commands::Command::Forget) {
                let response = self
                    .handle_forget(&incoming.channel, &incoming.sender_id)
                    .await;
                self.send_text(&incoming, &response).await;
                return;
            }

            // --- /setup intercept (REQ-BRAIN-011) ---
            if matches!(cmd, commands::Command::Setup) {
                let first_word = clean_incoming.text.split_whitespace().next().unwrap_or("");
                let description = if first_word.starts_with("/setup") {
                    clean_incoming.text[first_word.len()..].trim()
                } else {
                    ""
                };

                if description.is_empty() {
                    let user_lang = self
                        .memory
                        .get_fact(&incoming.sender_id, "preferred_language")
                        .await
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| "English".to_string());
                    self.send_text(&incoming, setup_help_message(&user_lang))
                        .await;
                } else {
                    // Start typing indicator before long-running Brain call.
                    let typing_channel = self.channels.get(&incoming.channel).cloned();
                    let typing_target = incoming.reply_target.clone();
                    let typing_handle =
                        if let (Some(ch), Some(ref target)) = (&typing_channel, &typing_target) {
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
                    self.start_setup_session(&incoming, description, typing_handle)
                        .await;
                }
                return;
            }

            // --- /google intercept (REQ-GAUTH-002) ---
            if matches!(cmd, commands::Command::Google) {
                self.start_google_session(&incoming).await;
                return;
            }

            // --- /context intercept ---
            if matches!(cmd, commands::Command::Context) {
                self.handle_context_command(&incoming, active_project.as_deref())
                    .await;
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
                projects,
                heartbeat_enabled: self.heartbeat_config.enabled,
                heartbeat_interval_mins: self.heartbeat_interval.load(Ordering::Relaxed),
                active_project: active_project.as_deref(),
                base_prompt_chars: self.prompts.identity.len()
                    + self.prompts.soul.len()
                    + self.prompts.system.len(),
            };
            let response = commands::handle(cmd, &ctx).await;

            if response.trim() == "WHATSAPP_QR" {
                self.handle_whatsapp_qr(&incoming).await;
                return;
            }

            self.send_text(&incoming, &response).await;
            return;
        }

        // --- 3b. WHATSAPP HELP INTERCEPT ---
        // WhatsApp has no command autocomplete menu. When a user asks what
        // OMEGA can do (in any language), return the /help output directly
        // instead of letting the AI improvise an incomplete capabilities list.
        if incoming.channel == "whatsapp" && kw_match(&clean_incoming.text.to_lowercase(), HELP_KW)
        {
            let user_lang = self
                .memory
                .get_fact(&incoming.sender_id, "preferred_language")
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "English".to_string());
            let response = commands::handle_help_text(&user_lang);
            self.send_text(&incoming, &response).await;
            return;
        }

        // --- 4. TYPING INDICATOR ---
        let typing_channel = self.channels.get(&incoming.channel).cloned();
        let typing_target = incoming.reply_target.clone();
        let mut typing_handle =
            if let (Some(ch), Some(ref target)) = (&typing_channel, &typing_target) {
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

        // --- 4a-SETUP. PENDING SETUP SESSION CHECK (REQ-BRAIN-012) ---
        let pending_setup: Option<String> = self
            .memory
            .get_fact(&incoming.sender_id, "pending_setup")
            .await
            .ok()
            .flatten();

        if let Some(setup_value) = pending_setup {
            self.handle_setup_response(&incoming, &setup_value, typing_handle)
                .await;
            return;
        }

        // --- 4a-GOOGLE. PENDING GOOGLE AUTH SESSION CHECK (REQ-GAUTH-011) ---
        // SECURITY: This check MUST remain BEFORE context building and provider calls.
        // Credentials in pending google sessions must NEVER reach the AI provider.
        let pending_google: Option<String> = self
            .memory
            .get_fact(&incoming.sender_id, "pending_google")
            .await
            .ok()
            .flatten();

        if let Some(google_value) = pending_google {
            if let Some(h) = typing_handle {
                h.abort();
            }
            self.handle_google_response(&incoming, &google_value).await;
            return;
        }

        // --- 4a. PENDING BUILD CONFIRMATION CHECK ---
        if self
            .handle_pending_build_confirmation(&incoming, &clean_incoming.text, &mut typing_handle)
            .await
        {
            return;
        }

        // All context sections are always injected — no keyword gating.
        // This eliminates false negatives (missed intent) from fragile keyword
        // matching. The token cost is small; reliability wins.
        let context_needs = ContextNeeds::default();

        let system_prompt =
            self.build_system_prompt(&incoming, active_project.as_deref(), projects);

        info!(
            "[{}] system prompt: ~{} tokens ({} chars)",
            incoming.channel,
            system_prompt.len() / 4,
            system_prompt.len()
        );

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

        // --- 4b. ACTIVATE MCP SERVERS ---
        // Claude Code CLI: always activate all MCP servers (cheap — just a config write).
        // HTTP providers: use keyword-based trigger matching (real per-message cost).
        let mcp_servers = if self.provider.name() == "claude-code" {
            omega_skills::collect_all_mcp_servers(&self.skills)
        } else {
            omega_skills::match_skill_triggers(&self.skills, &clean_incoming.text)
        };
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

                // Session continuation: inject all sections (lightweight refresh).
                let mut minimal = format!(
                    "Current time: {}",
                    chrono::Local::now().format("%Y-%m-%d %H:%M %Z")
                );
                minimal.push_str("\n\n");
                minimal.push_str(&self.prompts.scheduling);
                minimal.push_str("\n\n");
                minimal.push_str(&self.prompts.projects_rules);
                minimal.push_str("\n\n");
                minimal.push_str(&self.prompts.builds);
                minimal.push_str("\n\n");
                minimal.push_str(&self.prompts.meta);

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
}
