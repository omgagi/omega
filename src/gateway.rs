//! Gateway — the main event loop connecting channels, memory, and providers.
//!
//! Includes: auth enforcement, prompt sanitization, audit logging,
//! background conversation summarization, and graceful shutdown.

use crate::commands;
use omega_core::{
    config::{AuthConfig, ChannelConfig, HeartbeatConfig, SchedulerConfig},
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
    uptime: Instant,
}

impl Gateway {
    /// Create a new gateway.
    pub fn new(
        provider: Arc<dyn Provider>,
        channels: HashMap<String, Arc<dyn Channel>>,
        memory: Store,
        auth_config: AuthConfig,
        channel_config: ChannelConfig,
        heartbeat_config: HeartbeatConfig,
        scheduler_config: SchedulerConfig,
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
            uptime: Instant::now(),
        }
    }

    /// Run the main event loop.
    pub async fn run(&mut self) -> anyhow::Result<()> {
        info!(
            "Omega gateway running | provider: {} | channels: {} | auth: {}",
            self.provider.name(),
            self.channels.keys().cloned().collect::<Vec<_>>().join(", "),
            if self.auth_config.enabled {
                "enforced"
            } else {
                "disabled"
            }
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
        let bg_handle = tokio::spawn(async move {
            Self::background_summarizer(bg_store, bg_provider).await;
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
            Some(tokio::spawn(async move {
                Self::heartbeat_loop(hb_provider, hb_channels, hb_config).await;
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
    async fn background_summarizer(store: Store, provider: Arc<dyn Provider>) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;

            match store.find_idle_conversations().await {
                Ok(convos) => {
                    for (conv_id, _channel, _sender_id) in &convos {
                        if let Err(e) =
                            Self::summarize_conversation(&store, &provider, conv_id).await
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
    async fn heartbeat_loop(
        provider: Arc<dyn Provider>,
        channels: HashMap<String, Arc<dyn Channel>>,
        config: HeartbeatConfig,
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

            // Read optional checklist.
            let checklist = read_heartbeat_file().unwrap_or_default();

            let prompt = if checklist.is_empty() {
                "You are Omega performing a periodic heartbeat check. \
                 If everything is fine, respond with exactly HEARTBEAT_OK. \
                 Otherwise, respond with a brief alert."
                    .to_string()
            } else {
                format!(
                    "You are Omega performing a periodic heartbeat check.\n\
                     Review this checklist and report anything that needs attention.\n\
                     If everything is fine, respond with exactly HEARTBEAT_OK.\n\n\
                     {checklist}"
                )
            };

            let ctx = Context::new(&prompt);
            match provider.complete(&ctx).await {
                Ok(resp) => {
                    if resp.text.trim().contains("HEARTBEAT_OK") {
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
        let summary_prompt = format!(
            "Summarize this conversation in 1-2 sentences. Be factual and concise. \
             Do not add commentary.\n\n{transcript}"
        );
        let summary_ctx = Context::new(&summary_prompt);
        let summary = match provider.complete(&summary_ctx).await {
            Ok(resp) => resp.text,
            Err(e) => {
                warn!("summarization failed, using fallback: {e}");
                format!("({} messages, summary unavailable)", messages.len())
            }
        };

        // Ask provider to extract facts.
        let facts_prompt = format!(
            "Extract key facts about the user from this conversation. \
             Return each fact as 'key: value' on its own line. \
             Only include concrete, personal facts (name, preferences, location, etc.). \
             If no facts are apparent, respond with 'none'.\n\n{transcript}"
        );
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
                    if let Err(e) =
                        Self::summarize_conversation(&self.memory, &self.provider, conv_id).await
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
            self.send_text(&incoming, welcome_message(lang)).await;
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
            let response = commands::handle(
                cmd,
                &self.memory,
                &incoming.channel,
                &incoming.sender_id,
                &clean_incoming.text,
                &self.uptime,
                self.provider.name(),
            )
            .await;
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
        let context = match self.memory.build_context(&clean_incoming).await {
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

        // --- 5. GET RESPONSE FROM PROVIDER ---
        let response = match self.provider.complete(&context).await {
            Ok(mut resp) => {
                resp.reply_target = incoming.reply_target.clone();
                resp
            }
            Err(e) => {
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

                self.send_text(&incoming, &format!("Provider error: {e}"))
                    .await;
                return;
            }
        };

        // Stop typing indicator.
        if let Some(h) = typing_handle {
            h.abort();
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

        // --- 5c. EXTRACT LANG_SWITCH MARKER ---
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
            other => Some(format!("unknown channel: {other}")),
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

/// Return a hardcoded welcome message for the given language.
fn welcome_message(language: &str) -> &'static str {
    match language {
        "Spanish" => "Soy Omega, tu agente personal de inteligencia artificial. Corro sobre tu propia infraestructura, construido en Rust \u{01f4aa}, conectado a Telegram y con Claude como cerebro.\n\nEs un honor estar a tu servicio \u{01fae1} \u{00bf}Qu\u{00e9} necesitas de m\u{00ed}?",
        "Portuguese" => "Sou o Omega, seu agente pessoal de intelig\u{00ea}ncia artificial. Rodo na sua pr\u{00f3}pria infraestrutura, constru\u{00ed}do em Rust \u{01f4aa}, conectado ao Telegram e com Claude como c\u{00e9}rebro.\n\n\u{00c9} uma honra estar ao seu servi\u{00e7}o \u{01fae1} Do que voc\u{00ea} precisa?",
        "French" => "Je suis Omega, votre agent personnel d'intelligence artificielle. Je tourne sur votre propre infrastructure, construit en Rust \u{01f4aa}, connect\u{00e9} \u{00e0} Telegram et avec Claude comme cerveau.\n\nC'est un honneur d'\u{00ea}tre \u{00e0} votre service \u{01fae1} De quoi avez-vous besoin\u{00a0}?",
        "German" => "Ich bin Omega, dein pers\u{00f6}nlicher KI-Agent. Ich laufe auf deiner eigenen Infrastruktur, gebaut in Rust \u{01f4aa}, verbunden mit Telegram und mit Claude als Gehirn.\n\nEs ist mir eine Ehre, dir zu dienen \u{01fae1} Was brauchst du von mir?",
        "Italian" => "Sono Omega, il tuo agente personale di intelligenza artificiale. Giro sulla tua infrastruttura, costruito in Rust \u{01f4aa}, connesso a Telegram e con Claude come cervello.\n\n\u{00c8} un onore essere al tuo servizio \u{01fae1} Di cosa hai bisogno?",
        "Dutch" => "Ik ben Omega, je persoonlijke AI-agent. Ik draai op je eigen infrastructuur, gebouwd in Rust \u{01f4aa}, verbonden met Telegram en met Claude als brein.\n\nHet is een eer om je van dienst te zijn \u{01fae1} Wat heb je nodig?",
        "Russian" => "\u{042f} Omega, \u{0432}\u{0430}\u{0448} \u{043f}\u{0435}\u{0440}\u{0441}\u{043e}\u{043d}\u{0430}\u{043b}\u{044c}\u{043d}\u{044b}\u{0439} \u{0430}\u{0433}\u{0435}\u{043d}\u{0442} \u{0438}\u{0441}\u{043a}\u{0443}\u{0441}\u{0441}\u{0442}\u{0432}\u{0435}\u{043d}\u{043d}\u{043e}\u{0433}\u{043e} \u{0438}\u{043d}\u{0442}\u{0435}\u{043b}\u{043b}\u{0435}\u{043a}\u{0442}\u{0430}. \u{042f} \u{0440}\u{0430}\u{0431}\u{043e}\u{0442}\u{0430}\u{044e} \u{043d}\u{0430} \u{0432}\u{0430}\u{0448}\u{0435}\u{0439} \u{0441}\u{043e}\u{0431}\u{0441}\u{0442}\u{0432}\u{0435}\u{043d}\u{043d}\u{043e}\u{0439} \u{0438}\u{043d}\u{0444}\u{0440}\u{0430}\u{0441}\u{0442}\u{0440}\u{0443}\u{043a}\u{0442}\u{0443}\u{0440}\u{0435}, \u{043d}\u{0430}\u{043f}\u{0438}\u{0441}\u{0430}\u{043d} \u{043d}\u{0430} Rust \u{01f4aa}, \u{043f}\u{043e}\u{0434}\u{043a}\u{043b}\u{044e}\u{0447}\u{0451}\u{043d} \u{043a} Telegram \u{0438} \u{0438}\u{0441}\u{043f}\u{043e}\u{043b}\u{044c}\u{0437}\u{0443}\u{044e} Claude \u{043a}\u{0430}\u{043a} \u{043c}\u{043e}\u{0437}\u{0433}.\n\n\u{0414}\u{043b}\u{044f} \u{043c}\u{0435}\u{043d}\u{044f} \u{0447}\u{0435}\u{0441}\u{0442}\u{044c} \u{0441}\u{043b}\u{0443}\u{0436}\u{0438}\u{0442}\u{044c} \u{0432}\u{0430}\u{043c} \u{01fae1} \u{0427}\u{0442}\u{043e} \u{0432}\u{0430}\u{043c} \u{043d}\u{0443}\u{0436}\u{043d}\u{043e}?",
        _ => "I'm Omega, your personal artificial intelligence agent. I run on your own infrastructure, built in Rust \u{01f4aa}, connected to Telegram and with Claude as my brain.\n\nIt's an honor to be at your service \u{01fae1} What do you need from me?",
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
    fn test_welcome_message_all_languages() {
        let languages = [
            "English", "Spanish", "Portuguese", "French", "German", "Italian", "Dutch", "Russian",
        ];
        for lang in &languages {
            let msg = welcome_message(lang);
            assert!(!msg.is_empty(), "welcome for {lang} should not be empty");
            assert!(
                msg.contains("Omega"),
                "welcome for {lang} should mention Omega"
            );
        }
    }

    #[test]
    fn test_welcome_message_unknown_falls_back_to_english() {
        let msg = welcome_message("Klingon");
        assert!(msg.contains("Omega"));
        assert!(msg.contains("Rust"));
    }
}
