//! Gateway — the main event loop connecting channels, memory, and providers.
//!
//! Includes: auth enforcement, prompt sanitization, audit logging,
//! background conversation summarization, and graceful shutdown.

use crate::commands;
use omega_channels::whatsapp;
use omega_core::{
    config::{AuthConfig, ChannelConfig, HeartbeatConfig, Prompts, SchedulerConfig},
    context::Context,
    message::{IncomingMessage, MessageMetadata, OutgoingMessage},
    sanitize,
    traits::{Channel, Provider},
};
use omega_memory::{
    audit::{AuditEntry, AuditLogger, AuditStatus},
    detect_language, Store,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

/// The central gateway that routes messages between channels and providers.
pub struct Gateway {
    provider: Arc<dyn Provider>,
    channels: HashMap<String, Arc<dyn Channel>>,
    memory: Store,
    audit: AuditLogger,
    auth_config: AuthConfig,
    channel_config: ChannelConfig,
    heartbeat_config: HeartbeatConfig,
    scheduler_config: SchedulerConfig,
    prompts: Prompts,
    data_dir: String,
    skills: Vec<omega_skills::Skill>,
    projects: Vec<omega_skills::Project>,
    uptime: Instant,
    sandbox_mode: String,
    sandbox_prompt: Option<String>,
}

impl Gateway {
    /// Create a new gateway.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        provider: Arc<dyn Provider>,
        channels: HashMap<String, Arc<dyn Channel>>,
        memory: Store,
        auth_config: AuthConfig,
        channel_config: ChannelConfig,
        heartbeat_config: HeartbeatConfig,
        scheduler_config: SchedulerConfig,
        prompts: Prompts,
        data_dir: String,
        skills: Vec<omega_skills::Skill>,
        projects: Vec<omega_skills::Project>,
        sandbox_mode: String,
        sandbox_prompt: Option<String>,
    ) -> Self {
        let audit = AuditLogger::new(memory.pool().clone());
        Self {
            provider,
            channels,
            memory,
            audit,
            auth_config,
            channel_config,
            heartbeat_config,
            scheduler_config,
            prompts,
            data_dir,
            skills,
            projects,
            uptime: Instant::now(),
            sandbox_mode,
            sandbox_prompt,
        }
    }

    /// Run the main event loop.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!(
            "Omega gateway running | provider: {} | channels: {} | auth: {} | sandbox: {}",
            self.provider.name(),
            self.channels.keys().cloned().collect::<Vec<_>>().join(", "),
            if self.auth_config.enabled {
                "enforced"
            } else {
                "disabled"
            },
            self.sandbox_mode
        );

        let (tx, mut rx) = mpsc::channel::<IncomingMessage>(256);

        for (name, channel) in &self.channels {
            let mut channel_rx = channel
                .start()
                .await
                .map_err(|e| anyhow::anyhow!("failed to start channel {name}: {e}"))?;
            let tx = tx.clone();
            let channel_name = name.clone();

            tokio::spawn(async move {
                while let Some(msg) = channel_rx.recv().await {
                    if tx.send(msg).await.is_err() {
                        info!("gateway receiver dropped, stopping {channel_name} forwarder");
                        break;
                    }
                }
            });

            info!("Channel started: {name}");
        }

        drop(tx);

        // Spawn background summarization task.
        let bg_store = self.memory.clone();
        let bg_provider = self.provider.clone();
        let bg_summarize = self.prompts.summarize.clone();
        let bg_facts = self.prompts.facts.clone();
        let bg_handle = tokio::spawn(async move {
            Self::background_summarizer(bg_store, bg_provider, bg_summarize, bg_facts).await;
        });

        // Spawn scheduler loop.
        let sched_handle = if self.scheduler_config.enabled {
            let sched_store = self.memory.clone();
            let sched_channels = self.channels.clone();
            let poll_secs = self.scheduler_config.poll_interval_secs;
            Some(tokio::spawn(async move {
                Self::scheduler_loop(sched_store, sched_channels, poll_secs).await;
            }))
        } else {
            None
        };

        // Spawn heartbeat loop.
        let hb_handle = if self.heartbeat_config.enabled {
            let hb_provider = self.provider.clone();
            let hb_channels = self.channels.clone();
            let hb_config = self.heartbeat_config.clone();
            let hb_prompt_checklist = self.prompts.heartbeat_checklist.clone();
            let hb_memory = self.memory.clone();
            Some(tokio::spawn(async move {
                Self::heartbeat_loop(
                    hb_provider,
                    hb_channels,
                    hb_config,
                    hb_prompt_checklist,
                    hb_memory,
                )
                .await;
            }))
        } else {
            None
        };

        // Main event loop with graceful shutdown.
        loop {
            tokio::select! {
                Some(incoming) = rx.recv() => {
                    self.handle_message(incoming).await;
                }
                _ = tokio::signal::ctrl_c() => {
                    info!("Received shutdown signal");
                    break;
                }
            }
        }

        // Graceful shutdown.
        self.shutdown(&bg_handle, &sched_handle, &hb_handle).await;
        Ok(())
    }

    /// Background task: periodically find and summarize idle conversations.
    async fn background_summarizer(
        store: Store,
        provider: Arc<dyn Provider>,
        summarize_prompt: String,
        facts_prompt: String,
    ) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;

            match store.find_idle_conversations().await {
                Ok(convos) => {
                    for (conv_id, _channel, _sender_id) in &convos {
                        if let Err(e) = Self::summarize_conversation(
                            &store,
                            &provider,
                            conv_id,
                            &summarize_prompt,
                            &facts_prompt,
                        )
                        .await
                        {
                            error!("failed to summarize conversation {conv_id}: {e}");
                        }
                    }
                }
                Err(e) => {
                    error!("failed to find idle conversations: {e}");
                }
            }
        }
    }

    /// Background task: deliver due scheduled tasks.
    async fn scheduler_loop(
        store: Store,
        channels: HashMap<String, Arc<dyn Channel>>,
        poll_secs: u64,
    ) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(poll_secs)).await;

            match store.get_due_tasks().await {
                Ok(tasks) => {
                    for (id, channel_name, reply_target, description, repeat) in &tasks {
                        let msg = OutgoingMessage {
                            text: format!("Reminder: {description}"),
                            metadata: MessageMetadata::default(),
                            reply_target: Some(reply_target.clone()),
                        };

                        if let Some(ch) = channels.get(channel_name) {
                            if let Err(e) = ch.send(msg).await {
                                error!("failed to deliver task {id}: {e}");
                                continue;
                            }
                        } else {
                            warn!("scheduler: no channel '{channel_name}' for task {id}");
                            continue;
                        }

                        if let Err(e) = store.complete_task(id, repeat.as_deref()).await {
                            error!("failed to complete task {id}: {e}");
                        } else {
                            info!("delivered scheduled task {id}: {description}");
                        }
                    }
                }
                Err(e) => {
                    error!("scheduler: failed to get due tasks: {e}");
                }
            }
        }
    }

    /// Background task: periodic heartbeat check-in.
    ///
    /// Skips the provider call entirely when no checklist is configured.
    /// When a checklist exists, enriches the prompt with recent memory context.
    async fn heartbeat_loop(
        provider: Arc<dyn Provider>,
        channels: HashMap<String, Arc<dyn Channel>>,
        config: HeartbeatConfig,
        heartbeat_checklist_prompt: String,
        memory: Store,
    ) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(config.interval_minutes * 60)).await;

            // Check active hours.
            if !config.active_start.is_empty()
                && !config.active_end.is_empty()
                && !is_within_active_hours(&config.active_start, &config.active_end)
            {
                info!("heartbeat: outside active hours, skipping");
                continue;
            }

            // Read optional checklist — skip API call if none configured.
            let checklist = match read_heartbeat_file() {
                Some(cl) => cl,
                None => {
                    info!("heartbeat: no checklist configured, skipping");
                    continue;
                }
            };

            let mut prompt = heartbeat_checklist_prompt.replace("{checklist}", &checklist);

            // Enrich heartbeat context with recent memory.
            if let Ok(facts) = memory.get_all_facts().await {
                if !facts.is_empty() {
                    prompt.push_str("\n\nKnown about the user:");
                    for (key, value) in &facts {
                        prompt.push_str(&format!("\n- {key}: {value}"));
                    }
                }
            }
            if let Ok(summaries) = memory.get_all_recent_summaries(3).await {
                if !summaries.is_empty() {
                    prompt.push_str("\n\nRecent activity:");
                    for (summary, timestamp) in &summaries {
                        prompt.push_str(&format!("\n- [{timestamp}] {summary}"));
                    }
                }
            }

            let ctx = Context::new(&prompt);
            match provider.complete(&ctx).await {
                Ok(resp) => {
                    let cleaned: String = resp
                        .text
                        .chars()
                        .filter(|c| *c != '*' && *c != '`')
                        .collect();
                    if cleaned.trim().contains("HEARTBEAT_OK") {
                        info!("heartbeat: OK");
                    } else if let Some(ch) = channels.get(&config.channel) {
                        let msg = OutgoingMessage {
                            text: resp.text,
                            metadata: MessageMetadata::default(),
                            reply_target: Some(config.reply_target.clone()),
                        };
                        if let Err(e) = ch.send(msg).await {
                            error!("heartbeat: failed to send alert: {e}");
                        }
                    } else {
                        warn!(
                            "heartbeat: channel '{}' not found, alert dropped",
                            config.channel
                        );
                    }
                }
                Err(e) => {
                    error!("heartbeat: provider error: {e}");
                }
            }
        }
    }

    /// Summarize a conversation using the provider, extract facts, then close it.
    pub async fn summarize_conversation(
        store: &Store,
        provider: &Arc<dyn Provider>,
        conversation_id: &str,
        summarize_prompt: &str,
        facts_prompt_template: &str,
    ) -> Result<(), anyhow::Error> {
        let messages = store.get_conversation_messages(conversation_id).await?;
        if messages.is_empty() {
            store
                .close_conversation(conversation_id, "(empty conversation)")
                .await?;
            return Ok(());
        }

        // Build a transcript for summarization.
        let mut transcript = String::new();
        for (role, content) in &messages {
            let label = if role == "user" { "User" } else { "Assistant" };
            transcript.push_str(&format!("{label}: {content}\n"));
        }

        // Ask provider to summarize.
        let full_summary_prompt = format!("{summarize_prompt}\n\n{transcript}");
        let summary_ctx = Context::new(&full_summary_prompt);
        let summary = match provider.complete(&summary_ctx).await {
            Ok(resp) => resp.text,
            Err(e) => {
                warn!("summarization failed, using fallback: {e}");
                format!("({} messages, summary unavailable)", messages.len())
            }
        };

        // Ask provider to extract facts.
        let facts_prompt = format!("{facts_prompt_template}\n\n{transcript}");
        let facts_ctx = Context::new(&facts_prompt);
        if let Ok(facts_resp) = provider.complete(&facts_ctx).await {
            let text = facts_resp.text.trim().to_string();
            if text.to_lowercase() != "none" {
                // Find sender_id from the conversation messages context.
                // We need the sender_id — extract from the conversation row.
                let conv_info: Option<(String,)> =
                    sqlx::query_as("SELECT sender_id FROM conversations WHERE id = ?")
                        .bind(conversation_id)
                        .fetch_optional(store.pool())
                        .await
                        .ok()
                        .flatten();

                if let Some((sender_id,)) = conv_info {
                    for line in text.lines() {
                        if let Some((key, value)) = line.split_once(':') {
                            let key = key.trim().trim_start_matches("- ").to_lowercase();
                            let value = value.trim().to_string();
                            if !key.is_empty() && !value.is_empty() {
                                let _ = store.store_fact(&sender_id, &key, &value).await;
                            }
                        }
                    }
                }
            }
        }

        store.close_conversation(conversation_id, &summary).await?;
        info!("Conversation {conversation_id} summarized and closed");
        Ok(())
    }

    /// Graceful shutdown: summarize active conversations, stop channels.
    async fn shutdown(
        &self,
        bg_handle: &tokio::task::JoinHandle<()>,
        sched_handle: &Option<tokio::task::JoinHandle<()>>,
        hb_handle: &Option<tokio::task::JoinHandle<()>>,
    ) {
        info!("Shutting down...");

        // Abort background tasks.
        bg_handle.abort();
        if let Some(h) = sched_handle {
            h.abort();
        }
        if let Some(h) = hb_handle {
            h.abort();
        }

        // Summarize all active conversations.
        match self.memory.find_all_active_conversations().await {
            Ok(convos) => {
                for (conv_id, _channel, _sender_id) in &convos {
                    if let Err(e) = Self::summarize_conversation(
                        &self.memory,
                        &self.provider,
                        conv_id,
                        &self.prompts.summarize,
                        &self.prompts.facts,
                    )
                    .await
                    {
                        warn!("shutdown summarization failed for {conv_id}: {e}");
                    }
                }
            }
            Err(e) => {
                warn!("failed to find active conversations for shutdown: {e}");
            }
        }

        // Stop all channels.
        for (name, channel) in &self.channels {
            if let Err(e) = channel.stop().await {
                warn!("failed to stop channel {name}: {e}");
            }
        }

        info!("Shutdown complete.");
    }

    /// Process a single incoming message through the full pipeline.
    async fn handle_message(&self, incoming: IncomingMessage) {
        let preview = if incoming.text.len() > 60 {
            format!("{}...", &incoming.text[..60])
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

                // Audit the denial.
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

                // Send denial message back.
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

        // Use sanitized text for the rest of the pipeline.
        let mut clean_incoming = incoming.clone();
        clean_incoming.text = sanitized.text;

        // --- 2b. WELCOME CHECK (first-time users) ---
        if let Ok(true) = self.memory.is_new_user(&incoming.sender_id).await {
            let lang = detect_language(&clean_incoming.text);
            let default_welcome = self
                .prompts
                .welcome
                .get("English")
                .cloned()
                .unwrap_or_default();
            let welcome_msg = self.prompts.welcome.get(lang).unwrap_or(&default_welcome);
            self.send_text(&incoming, welcome_msg).await;
            // Store welcomed fact and detected language preference.
            let _ = self
                .memory
                .store_fact(&incoming.sender_id, "welcomed", "true")
                .await;
            let _ = self
                .memory
                .store_fact(&incoming.sender_id, "preferred_language", lang)
                .await;
            info!("welcomed new user {} ({})", incoming.sender_id, lang);
        }

        // --- 3. COMMAND DISPATCH ---
        if let Some(cmd) = commands::Command::parse(&clean_incoming.text) {
            let ctx = commands::CommandContext {
                store: &self.memory,
                channel: &incoming.channel,
                sender_id: &incoming.sender_id,
                text: &clean_incoming.text,
                uptime: &self.uptime,
                provider_name: self.provider.name(),
                skills: &self.skills,
                projects: &self.projects,
                sandbox_mode: &self.sandbox_mode,
            };
            let response = commands::handle(cmd, &ctx).await;

            // Intercept WHATSAPP_QR marker from /whatsapp command.
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
            // Send initial typing action.
            let _ = ch.send_typing(&target).await;
            // Spawn repeater.
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
        // Inject active project instructions, platform hint, and group chat rules.
        let system_prompt = {
            let mut prompt = format!(
                "{}\n\n{}\n\n{}",
                self.prompts.identity, self.prompts.soul, self.prompts.system
            );

            // Platform formatting hint.
            match incoming.channel.as_str() {
                "whatsapp" => prompt.push_str(
                    "\n\nPlatform: WhatsApp. Avoid markdown tables and headers — use bold (*text*) and bullet lists instead.",
                ),
                "telegram" => prompt.push_str(
                    "\n\nPlatform: Telegram. Markdown is supported (bold, italic, code blocks).",
                ),
                _ => {}
            }

            // Group chat awareness.
            if incoming.is_group {
                prompt.push_str(
                    "\n\nThis is a GROUP CHAT. Only respond when directly mentioned by name, \
                     asked a question, or you can add genuine value. Do not leak personal facts \
                     from private conversations. If the message does not warrant a response, \
                     reply with exactly SILENT on its own line.",
                );
            }

            if let Ok(Some(project_name)) = self
                .memory
                .get_fact(&incoming.sender_id, "active_project")
                .await
            {
                if let Some(instructions) =
                    omega_skills::get_project_instructions(&self.projects, &project_name)
                {
                    prompt = format!("{instructions}\n\n---\n\n{prompt}");
                }
            }

            // Heartbeat awareness: show current checklist items so Claude knows
            // what is already monitored.
            if let Some(checklist) = read_heartbeat_file() {
                prompt
                    .push_str("\n\nCurrent heartbeat checklist (items monitored periodically):\n");
                prompt.push_str(&checklist);
            }

            // Sandbox mode constraint.
            if let Some(ref constraint) = self.sandbox_prompt {
                prompt.push_str("\n\n");
                prompt.push_str(constraint);
            }

            prompt
        };

        let context = match self
            .memory
            .build_context(&clean_incoming, &system_prompt)
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

        // --- 5. GET RESPONSE FROM PROVIDER (async with status updates) ---

        // Spawn provider call as background task.
        let provider = self.provider.clone();
        let ctx = context.clone();
        let provider_task = tokio::spawn(async move { provider.complete(&ctx).await });

        // Resolve user language for status messages.
        let user_lang = self
            .memory
            .get_fact(&incoming.sender_id, "preferred_language")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "English".to_string());
        let (nudge_msg, still_msg) = status_messages(&user_lang);

        // Spawn delayed status updater: first nudge after 15s, then every 120s.
        // If the provider responds quickly, this gets aborted and the user sees nothing extra.
        let status_channel = self.channels.get(&incoming.channel).cloned();
        let status_target = incoming.reply_target.clone();
        let status_handle = tokio::spawn(async move {
            // First nudge: wait 15 seconds before telling the user.
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            if let (Some(ref ch), Some(ref target)) = (&status_channel, &status_target) {
                let msg = OutgoingMessage {
                    text: nudge_msg.to_string(),
                    metadata: MessageMetadata::default(),
                    reply_target: Some(target.clone()),
                };
                let _ = ch.send(msg).await;
            }
            // Subsequent nudges every 120 seconds.
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(120)).await;
                if let (Some(ref ch), Some(ref target)) = (&status_channel, &status_target) {
                    let msg = OutgoingMessage {
                        text: still_msg.to_string(),
                        metadata: MessageMetadata::default(),
                        reply_target: Some(target.clone()),
                    };
                    let _ = ch.send(msg).await;
                }
            }
        });

        // Wait for the provider result.
        let response = match provider_task.await {
            Ok(Ok(mut resp)) => {
                status_handle.abort();
                resp.reply_target = incoming.reply_target.clone();
                resp
            }
            Ok(Err(e)) => {
                status_handle.abort();
                error!("provider error: {e}");
                if let Some(h) = typing_handle {
                    h.abort();
                }

                // Audit the error.
                let _ = self
                    .audit
                    .log(&AuditEntry {
                        channel: incoming.channel.clone(),
                        sender_id: incoming.sender_id.clone(),
                        sender_name: incoming.sender_name.clone(),
                        input_text: incoming.text.clone(),
                        output_text: Some(format!("ERROR: {e}")),
                        provider_used: Some(self.provider.name().to_string()),
                        model: None,
                        processing_ms: None,
                        status: AuditStatus::Error,
                        denial_reason: None,
                    })
                    .await;

                let friendly = friendly_provider_error(&e.to_string());
                self.send_text(&incoming, &friendly).await;
                return;
            }
            Err(join_err) => {
                status_handle.abort();
                error!("provider task panicked: {join_err}");
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(&incoming, "Something went wrong. Please try again.")
                    .await;
                return;
            }
        };

        // Stop typing indicator.
        if let Some(h) = typing_handle {
            h.abort();
        }

        // --- 5a. SUPPRESS SILENT RESPONSES (group chats) ---
        if incoming.is_group && response.text.trim() == "SILENT" {
            info!(
                "[{}] group chat: suppressing SILENT response",
                incoming.channel
            );
            return;
        }

        // --- 5b. EXTRACT SCHEDULE MARKER ---
        let mut response = response;
        if let Some(schedule_line) = extract_schedule_marker(&response.text) {
            if let Some((desc, due_at, repeat)) = parse_schedule_line(&schedule_line) {
                let reply_target = incoming.reply_target.as_deref().unwrap_or("");
                let repeat_opt = if repeat == "once" {
                    None
                } else {
                    Some(repeat.as_str())
                };
                match self
                    .memory
                    .create_task(
                        &incoming.channel,
                        &incoming.sender_id,
                        reply_target,
                        &desc,
                        &due_at,
                        repeat_opt,
                    )
                    .await
                {
                    Ok(id) => {
                        info!("scheduled task {id}: {desc} at {due_at}");
                    }
                    Err(e) => {
                        error!("failed to create scheduled task: {e}");
                    }
                }
            }
            // Strip the SCHEDULE: line from the response.
            response.text = strip_schedule_marker(&response.text);
        }

        // --- 5c. EXTRACT WHATSAPP_QR MARKER ---
        if has_whatsapp_qr_marker(&response.text) {
            response.text = strip_whatsapp_qr_marker(&response.text);
            self.handle_whatsapp_qr(&incoming).await;
        }

        // --- 5d. EXTRACT LANG_SWITCH MARKER ---
        if let Some(lang) = extract_lang_switch(&response.text) {
            if let Err(e) = self
                .memory
                .store_fact(&incoming.sender_id, "preferred_language", &lang)
                .await
            {
                error!("failed to store language preference: {e}");
            } else {
                info!("language switched to '{lang}' for {}", incoming.sender_id);
            }
            response.text = strip_lang_switch(&response.text);
        }

        // --- 5e. EXTRACT HEARTBEAT_ADD / HEARTBEAT_REMOVE MARKERS ---
        let heartbeat_actions = extract_heartbeat_markers(&response.text);
        if !heartbeat_actions.is_empty() {
            apply_heartbeat_changes(&heartbeat_actions);
            for action in &heartbeat_actions {
                match action {
                    HeartbeatAction::Add(item) => {
                        info!("heartbeat: added '{item}' to checklist");
                    }
                    HeartbeatAction::Remove(item) => {
                        info!("heartbeat: removed '{item}' from checklist");
                    }
                }
            }
            response.text = strip_heartbeat_markers(&response.text);
        }

        // --- 6. STORE IN MEMORY ---
        if let Err(e) = self.memory.store_exchange(&incoming, &response).await {
            error!("failed to store exchange: {e}");
        }

        // --- 7. AUDIT LOG ---
        let _ = self
            .audit
            .log(&AuditEntry {
                channel: incoming.channel.clone(),
                sender_id: incoming.sender_id.clone(),
                sender_name: incoming.sender_name.clone(),
                input_text: incoming.text.clone(),
                output_text: Some(response.text.clone()),
                provider_used: Some(response.metadata.provider_used.clone()),
                model: response.metadata.model.clone(),
                processing_ms: Some(response.metadata.processing_time_ms as i64),
                status: AuditStatus::Ok,
                denial_reason: None,
            })
            .await;

        // --- 8. SEND RESPONSE ---
        if let Some(channel) = self.channels.get(&incoming.channel) {
            if let Err(e) = channel.send(response).await {
                error!("failed to send response via {}: {e}", incoming.channel);
            }
        } else {
            error!("no channel found for '{}'", incoming.channel);
        }
    }

    /// Check if an incoming message is authorized.
    /// Returns `None` if allowed, `Some(reason)` if denied.
    fn check_auth(&self, incoming: &IncomingMessage) -> Option<String> {
        match incoming.channel.as_str() {
            "telegram" => {
                let allowed = self
                    .channel_config
                    .telegram
                    .as_ref()
                    .map(|tg| &tg.allowed_users);

                match allowed {
                    Some(users) if users.is_empty() => {
                        // Empty list = allow all (for easy testing).
                        None
                    }
                    Some(users) => {
                        let sender_id: i64 = incoming.sender_id.parse().unwrap_or(-1);
                        if users.contains(&sender_id) {
                            None
                        } else {
                            Some(format!(
                                "telegram user {} not in allowed_users",
                                incoming.sender_id
                            ))
                        }
                    }
                    None => Some("telegram channel not configured".to_string()),
                }
            }
            "whatsapp" => {
                let allowed = self
                    .channel_config
                    .whatsapp
                    .as_ref()
                    .map(|wa| &wa.allowed_users);

                match allowed {
                    Some(users) if users.is_empty() => None,
                    Some(users) => {
                        if users.contains(&incoming.sender_id) {
                            None
                        } else {
                            Some(format!(
                                "whatsapp user {} not in allowed_users",
                                incoming.sender_id
                            ))
                        }
                    }
                    None => Some("whatsapp channel not configured".to_string()),
                }
            }
            other => Some(format!("unknown channel: {other}")),
        }
    }

    /// Handle the WHATSAPP_QR flow: start pairing, send QR image, wait for result.
    async fn handle_whatsapp_qr(&self, incoming: &IncomingMessage) {
        self.send_text(incoming, "Starting WhatsApp pairing...")
            .await;

        match whatsapp::start_pairing(&self.data_dir).await {
            Ok((mut qr_rx, mut done_rx)) => {
                // Wait for the first QR code (with timeout).
                let qr_timeout =
                    tokio::time::timeout(std::time::Duration::from_secs(30), qr_rx.recv());

                match qr_timeout.await {
                    Ok(Some(qr_data)) => {
                        // Generate QR image and send it.
                        match whatsapp::generate_qr_image(&qr_data) {
                            Ok(png_bytes) => {
                                if let Some(channel) = self.channels.get(&incoming.channel) {
                                    let target = incoming.reply_target.as_deref().unwrap_or("");
                                    if let Err(e) = channel
                                        .send_photo(
                                            target,
                                            &png_bytes,
                                            "Scan with WhatsApp (Link a Device > QR Code)",
                                        )
                                        .await
                                    {
                                        warn!("failed to send QR image: {e}");
                                        self.send_text(
                                            incoming,
                                            &format!("Failed to send QR image: {e}"),
                                        )
                                        .await;
                                        return;
                                    }
                                }
                            }
                            Err(e) => {
                                self.send_text(incoming, &format!("QR generation failed: {e}"))
                                    .await;
                                return;
                            }
                        }

                        // Wait for pairing confirmation (up to 60s).
                        let pair_timeout = tokio::time::timeout(
                            std::time::Duration::from_secs(60),
                            done_rx.recv(),
                        );

                        match pair_timeout.await {
                            Ok(Some(true)) => {
                                self.send_text(incoming, "WhatsApp connected!").await;
                            }
                            _ => {
                                self.send_text(
                                    incoming,
                                    "WhatsApp pairing timed out. Try /whatsapp again.",
                                )
                                .await;
                            }
                        }
                    }
                    _ => {
                        self.send_text(incoming, "Failed to generate QR code. Try again.")
                            .await;
                    }
                }
            }
            Err(e) => {
                self.send_text(incoming, &format!("WhatsApp pairing failed: {e}"))
                    .await;
            }
        }
    }

    /// Send a plain text message back to the sender.
    async fn send_text(&self, incoming: &IncomingMessage, text: &str) {
        let msg = OutgoingMessage {
            text: text.to_string(),
            metadata: MessageMetadata::default(),
            reply_target: incoming.reply_target.clone(),
        };

        if let Some(channel) = self.channels.get(&incoming.channel) {
            if let Err(e) = channel.send(msg).await {
                error!("failed to send message: {e}");
            }
        }
    }
}

/// Extract the first `SCHEDULE:` line from response text.
fn extract_schedule_marker(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("SCHEDULE:"))
        .map(|line| line.trim().to_string())
}

/// Parse a schedule line: `SCHEDULE: desc | ISO datetime | repeat`
fn parse_schedule_line(line: &str) -> Option<(String, String, String)> {
    let content = line.strip_prefix("SCHEDULE:")?.trim();
    let parts: Vec<&str> = content.splitn(3, '|').collect();
    if parts.len() != 3 {
        return None;
    }
    let desc = parts[0].trim().to_string();
    let due_at = parts[1].trim().to_string();
    let repeat = parts[2].trim().to_lowercase();
    if desc.is_empty() || due_at.is_empty() {
        return None;
    }
    Some((desc, due_at, repeat))
}

/// Extract the language from a `LANG_SWITCH:` line in response text.
fn extract_lang_switch(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("LANG_SWITCH:"))
        .and_then(|line| {
            let lang = line.trim().strip_prefix("LANG_SWITCH:")?.trim().to_string();
            if lang.is_empty() {
                None
            } else {
                Some(lang)
            }
        })
}

/// Strip all `LANG_SWITCH:` lines from response text.
fn strip_lang_switch(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("LANG_SWITCH:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Strip all `SCHEDULE:` lines from response text.
fn strip_schedule_marker(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("SCHEDULE:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Check if response text contains a `WHATSAPP_QR` marker line.
fn has_whatsapp_qr_marker(text: &str) -> bool {
    text.lines().any(|line| line.trim() == "WHATSAPP_QR")
}

/// Strip all `WHATSAPP_QR` lines from response text.
fn strip_whatsapp_qr_marker(text: &str) -> String {
    text.lines()
        .filter(|line| line.trim() != "WHATSAPP_QR")
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Read `~/.omega/HEARTBEAT.md` if it exists.
fn read_heartbeat_file() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = format!("{home}/.omega/HEARTBEAT.md");
    let content = std::fs::read_to_string(path).ok()?;
    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}

/// Action extracted from a `HEARTBEAT_ADD:` or `HEARTBEAT_REMOVE:` marker.
#[derive(Debug, Clone, PartialEq)]
enum HeartbeatAction {
    Add(String),
    Remove(String),
}

/// Extract all `HEARTBEAT_ADD:` and `HEARTBEAT_REMOVE:` markers from response text.
fn extract_heartbeat_markers(text: &str) -> Vec<HeartbeatAction> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if let Some(item) = trimmed.strip_prefix("HEARTBEAT_ADD:") {
                let item = item.trim();
                if item.is_empty() {
                    None
                } else {
                    Some(HeartbeatAction::Add(item.to_string()))
                }
            } else if let Some(item) = trimmed.strip_prefix("HEARTBEAT_REMOVE:") {
                let item = item.trim();
                if item.is_empty() {
                    None
                } else {
                    Some(HeartbeatAction::Remove(item.to_string()))
                }
            } else {
                None
            }
        })
        .collect()
}

/// Strip all `HEARTBEAT_ADD:` and `HEARTBEAT_REMOVE:` lines from response text.
fn strip_heartbeat_markers(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("HEARTBEAT_ADD:") && !trimmed.starts_with("HEARTBEAT_REMOVE:")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Apply heartbeat add/remove actions to `~/.omega/HEARTBEAT.md`.
///
/// Creates the file if missing. Prevents duplicate adds. Uses case-insensitive
/// partial matching for removes. Skips comment lines (`#`) during removal.
fn apply_heartbeat_changes(actions: &[HeartbeatAction]) {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return,
    };
    let path = format!("{home}/.omega/HEARTBEAT.md");

    // Read existing lines (or start empty).
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect();

    for action in actions {
        match action {
            HeartbeatAction::Add(item) => {
                // Prevent duplicates (case-insensitive).
                let already_exists = lines.iter().any(|l| {
                    let trimmed = l.trim();
                    !trimmed.starts_with('#')
                        && trimmed.trim_start_matches("- ").eq_ignore_ascii_case(item)
                });
                if !already_exists {
                    lines.push(format!("- {item}"));
                }
            }
            HeartbeatAction::Remove(item) => {
                let needle = item.to_lowercase();
                lines.retain(|l| {
                    let trimmed = l.trim();
                    // Never remove comment lines.
                    if trimmed.starts_with('#') {
                        return true;
                    }
                    let content = trimmed.trim_start_matches("- ").to_lowercase();
                    // Remove if content contains the needle (partial match).
                    !content.contains(&needle)
                });
            }
        }
    }

    // Write back.
    let content = lines.join("\n");
    // Ensure parent directory exists.
    let dir = format!("{home}/.omega");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        &path,
        if content.is_empty() {
            content
        } else {
            content + "\n"
        },
    );
}

/// Return localized status messages for the delayed provider nudge.
/// Returns `(first_nudge, still_working)`.
fn status_messages(lang: &str) -> (&'static str, &'static str) {
    match lang {
        "Spanish" => (
            "Esto va a tomar un momento — te mantendré informado.",
            "Sigo trabajando en tu solicitud...",
        ),
        "Portuguese" => (
            "Isso vai levar um momento — vou te manter informado.",
            "Ainda estou trabalhando no seu pedido...",
        ),
        "French" => (
            "Cela prend un moment — je vous tiendrai informé.",
            "Je travaille encore sur votre demande...",
        ),
        "German" => (
            "Das dauert einen Moment — ich halte dich auf dem Laufenden.",
            "Ich arbeite noch an deiner Anfrage...",
        ),
        "Italian" => (
            "Ci vorrà un momento — ti terrò aggiornato.",
            "Sto ancora lavorando alla tua richiesta...",
        ),
        "Dutch" => (
            "Dit duurt even — ik hou je op de hoogte.",
            "Ik werk nog aan je verzoek...",
        ),
        "Russian" => (
            "Это займёт немного времени — я буду держать вас в курсе.",
            "Всё ещё работаю над вашим запросом...",
        ),
        _ => (
            "This is taking a moment — I'll keep you updated.",
            "Still working on your request...",
        ),
    }
}

/// Map raw provider errors to user-friendly messages.
fn friendly_provider_error(raw: &str) -> String {
    if raw.contains("timed out") {
        "I took too long to respond. Please try again — sometimes complex requests need a second attempt.".to_string()
    } else {
        "Something went wrong. Please try again.".to_string()
    }
}

/// Check if the current local time is within the active hours window.
fn is_within_active_hours(start: &str, end: &str) -> bool {
    let now = chrono::Local::now().format("%H:%M").to_string();
    if start <= end {
        // Normal range: e.g. 08:00 to 22:00
        now.as_str() >= start && now.as_str() < end
    } else {
        // Midnight wrap: e.g. 22:00 to 06:00
        now.as_str() >= start || now.as_str() < end
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_schedule_marker() {
        let text = "Sure, I'll remind you.\nSCHEDULE: Call John | 2026-02-17T15:00:00 | once";
        let result = extract_schedule_marker(text);
        assert_eq!(
            result,
            Some("SCHEDULE: Call John | 2026-02-17T15:00:00 | once".to_string())
        );
    }

    #[test]
    fn test_extract_schedule_marker_none() {
        let text = "No schedule here, just a normal response.";
        assert!(extract_schedule_marker(text).is_none());
    }

    #[test]
    fn test_parse_schedule_line() {
        let line = "SCHEDULE: Call John | 2026-02-17T15:00:00 | once";
        let result = parse_schedule_line(line).unwrap();
        assert_eq!(result.0, "Call John");
        assert_eq!(result.1, "2026-02-17T15:00:00");
        assert_eq!(result.2, "once");
    }

    #[test]
    fn test_parse_schedule_line_daily() {
        let line = "SCHEDULE: Stand-up meeting | 2026-02-18T09:00:00 | daily";
        let result = parse_schedule_line(line).unwrap();
        assert_eq!(result.0, "Stand-up meeting");
        assert_eq!(result.2, "daily");
    }

    #[test]
    fn test_parse_schedule_line_invalid() {
        assert!(parse_schedule_line("SCHEDULE: missing parts").is_none());
        assert!(parse_schedule_line("not a schedule line").is_none());
    }

    #[test]
    fn test_strip_schedule_marker() {
        let text = "Sure, I'll remind you.\nSCHEDULE: Call John | 2026-02-17T15:00:00 | once";
        let result = strip_schedule_marker(text);
        assert_eq!(result, "Sure, I'll remind you.");
    }

    #[test]
    fn test_strip_schedule_marker_preserves_other_lines() {
        let text = "Line 1\nLine 2\nSCHEDULE: test | 2026-01-01T00:00:00 | once\nLine 3";
        let result = strip_schedule_marker(text);
        assert_eq!(result, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_extract_lang_switch() {
        let text = "Sure, I'll speak French now.\nLANG_SWITCH: French";
        assert_eq!(extract_lang_switch(text), Some("French".to_string()));
    }

    #[test]
    fn test_extract_lang_switch_none() {
        assert!(extract_lang_switch("Just a normal response.").is_none());
    }

    #[test]
    fn test_strip_lang_switch() {
        let text = "Sure, I'll speak French now.\nLANG_SWITCH: French";
        assert_eq!(strip_lang_switch(text), "Sure, I'll speak French now.");
    }

    #[test]
    fn test_is_within_active_hours_normal_range() {
        // This test checks the logic, not the current time.
        // Normal range: 00:00 to 23:59 should always be true.
        assert!(is_within_active_hours("00:00", "23:59"));
    }

    #[test]
    fn test_is_within_active_hours_narrow_miss() {
        // Range 00:00 to 00:00 is empty (start == end, so start <= end is true,
        // and now >= "00:00" && now < "00:00" is always false).
        assert!(!is_within_active_hours("00:00", "00:00"));
    }

    #[test]
    fn test_prompts_default_welcome_all_languages() {
        let prompts = Prompts::default();
        let languages = [
            "English",
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ];
        for lang in &languages {
            let msg = prompts.welcome.get(*lang);
            assert!(msg.is_some(), "welcome for {lang} should exist");
            assert!(
                msg.unwrap().contains("*OMEGA*"),
                "welcome for {lang} should mention *OMEGA*"
            );
        }
    }

    #[test]
    fn test_prompts_default_welcome_fallback() {
        let prompts = Prompts::default();
        let default = prompts.welcome.get("English").cloned().unwrap_or_default();
        let msg = prompts.welcome.get("Klingon").unwrap_or(&default);
        assert!(msg.contains("*OMEGA*"));
        assert!(msg.contains("private"));
    }

    #[test]
    fn test_friendly_provider_error_timeout() {
        let msg = friendly_provider_error("claude CLI timed out after 600s");
        assert!(msg.contains("too long"));
        assert!(!msg.contains("timed out"));
    }

    #[test]
    fn test_friendly_provider_error_generic() {
        let msg = friendly_provider_error("failed to run claude CLI: No such file");
        assert_eq!(msg, "Something went wrong. Please try again.");
    }

    #[test]
    fn test_status_messages_all_languages() {
        let languages = [
            "English",
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ];
        for lang in &languages {
            let (nudge, still) = status_messages(lang);
            assert!(!nudge.is_empty(), "nudge for {lang} should not be empty");
            assert!(!still.is_empty(), "still for {lang} should not be empty");
        }
    }

    #[test]
    fn test_status_messages_unknown_falls_back_to_english() {
        let (nudge, still) = status_messages("Klingon");
        assert!(nudge.contains("taking a moment"));
        assert!(still.contains("Still working"));
    }

    #[test]
    fn test_status_messages_spanish() {
        let (nudge, still) = status_messages("Spanish");
        assert!(nudge.contains("tomar un momento"));
        assert!(still.contains("trabajando"));
    }

    #[test]
    fn test_read_heartbeat_file_returns_none_when_missing() {
        // When the file does not exist, read_heartbeat_file returns None.
        // This test relies on HOME being set, which it always is in CI/dev.
        // We cannot easily control the file, so just verify the function is callable.
        let result = read_heartbeat_file();
        // Result depends on whether the file exists; just check it doesn't panic.
        let _ = result;
    }

    #[test]
    fn test_bundled_system_prompt_contains_identity_soul_system() {
        let content = include_str!("../prompts/SYSTEM_PROMPT.md");
        assert!(
            content.contains("## Identity"),
            "bundled system prompt should contain Identity section"
        );
        assert!(
            content.contains("## Soul"),
            "bundled system prompt should contain Soul section"
        );
        assert!(
            content.contains("## System"),
            "bundled system prompt should contain System section"
        );
        assert!(
            content.contains("genuinely helpful"),
            "bundled system prompt should contain personality principles"
        );
    }

    #[test]
    fn test_bundled_facts_prompt_guided_schema() {
        // Verify the bundled SYSTEM_PROMPT.md has guided fact-extraction fields.
        let content = include_str!("../prompts/SYSTEM_PROMPT.md");
        assert!(
            content.contains("preferred_name"),
            "bundled facts section should list preferred_name"
        );
        assert!(
            content.contains("timezone"),
            "bundled facts section should list timezone"
        );
        assert!(
            content.contains("pronouns"),
            "bundled facts section should list pronouns"
        );
        assert!(
            content.contains("dossier"),
            "bundled facts section should include privacy framing"
        );
    }

    // --- Heartbeat marker tests ---

    #[test]
    fn test_extract_heartbeat_add() {
        let text = "Sure, I'll monitor that.\nHEARTBEAT_ADD: Check exercise habits";
        let actions = extract_heartbeat_markers(text);
        assert_eq!(
            actions,
            vec![HeartbeatAction::Add("Check exercise habits".to_string())]
        );
    }

    #[test]
    fn test_extract_heartbeat_remove() {
        let text = "I'll stop monitoring that.\nHEARTBEAT_REMOVE: exercise";
        let actions = extract_heartbeat_markers(text);
        assert_eq!(
            actions,
            vec![HeartbeatAction::Remove("exercise".to_string())]
        );
    }

    #[test]
    fn test_extract_heartbeat_multiple() {
        let text =
            "Updating your checklist.\nHEARTBEAT_ADD: Water plants\nHEARTBEAT_REMOVE: old task";
        let actions = extract_heartbeat_markers(text);
        assert_eq!(
            actions,
            vec![
                HeartbeatAction::Add("Water plants".to_string()),
                HeartbeatAction::Remove("old task".to_string()),
            ]
        );
    }

    #[test]
    fn test_extract_heartbeat_empty_ignored() {
        let text = "HEARTBEAT_ADD: \nHEARTBEAT_REMOVE:   \nSome response.";
        let actions = extract_heartbeat_markers(text);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_strip_heartbeat_markers() {
        let text = "Sure, I'll monitor that.\nHEARTBEAT_ADD: Check exercise habits\nDone!";
        let result = strip_heartbeat_markers(text);
        assert_eq!(result, "Sure, I'll monitor that.\nDone!");
    }

    #[test]
    fn test_strip_heartbeat_both_types() {
        let text = "Response.\nHEARTBEAT_ADD: new item\nHEARTBEAT_REMOVE: old item\nEnd.";
        let result = strip_heartbeat_markers(text);
        assert_eq!(result, "Response.\nEnd.");
    }

    #[test]
    fn test_apply_heartbeat_add() {
        // Use a temp dir to avoid touching real ~/.omega/HEARTBEAT.md.
        let tmp = std::env::temp_dir().join("omega_test_hb_add");
        let _ = std::fs::create_dir_all(&tmp);
        let path = tmp.join("HEARTBEAT.md");
        std::fs::write(&path, "# My checklist\n- Existing item\n").unwrap();

        // Temporarily override HOME for the test.
        let original_home = std::env::var("HOME").unwrap();
        let fake_home = tmp.parent().unwrap().join("omega_test_hb_add_home");
        let _ = std::fs::create_dir_all(fake_home.join(".omega"));
        std::fs::write(
            fake_home.join(".omega/HEARTBEAT.md"),
            "# My checklist\n- Existing item\n",
        )
        .unwrap();
        std::env::set_var("HOME", &fake_home);

        apply_heartbeat_changes(&[HeartbeatAction::Add("New item".to_string())]);

        let content = std::fs::read_to_string(fake_home.join(".omega/HEARTBEAT.md")).unwrap();
        assert!(content.contains("- Existing item"), "should keep existing");
        assert!(content.contains("- New item"), "should add new item");

        // Duplicate add should not create a second entry.
        apply_heartbeat_changes(&[HeartbeatAction::Add("New item".to_string())]);
        let content = std::fs::read_to_string(fake_home.join(".omega/HEARTBEAT.md")).unwrap();
        assert_eq!(
            content.matches("New item").count(),
            1,
            "should not duplicate"
        );

        // Restore HOME and clean up.
        std::env::set_var("HOME", &original_home);
        let _ = std::fs::remove_dir_all(&tmp);
        let _ = std::fs::remove_dir_all(&fake_home);
    }

    #[test]
    fn test_apply_heartbeat_remove() {
        let fake_home = std::env::temp_dir().join("omega_test_hb_remove_home");
        let _ = std::fs::create_dir_all(fake_home.join(".omega"));
        std::fs::write(
            fake_home.join(".omega/HEARTBEAT.md"),
            "# My checklist\n- Check exercise habits\n- Water the plants\n",
        )
        .unwrap();

        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", &fake_home);

        apply_heartbeat_changes(&[HeartbeatAction::Remove("exercise".to_string())]);

        let content = std::fs::read_to_string(fake_home.join(".omega/HEARTBEAT.md")).unwrap();
        assert!(!content.contains("exercise"), "should remove exercise line");
        assert!(
            content.contains("Water the plants"),
            "should keep other items"
        );
        assert!(content.contains("# My checklist"), "should keep comments");

        // Restore HOME and clean up.
        std::env::set_var("HOME", &original_home);
        let _ = std::fs::remove_dir_all(&fake_home);
    }
}
