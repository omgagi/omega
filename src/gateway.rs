//! Gateway — the main event loop connecting channels, memory, and providers.
//!
//! Includes: auth enforcement, prompt sanitization, audit logging,
//! background conversation summarization, and graceful shutdown.

use crate::commands;
use omega_channels::whatsapp;
use omega_core::{
    config::{shellexpand, AuthConfig, ChannelConfig, HeartbeatConfig, Prompts, SchedulerConfig},
    context::{Context, ContextEntry},
    message::{AttachmentType, IncomingMessage, MessageMetadata, OutgoingMessage},
    sanitize,
    traits::{Channel, Provider},
};
use omega_memory::{
    audit::{AuditEntry, AuditLogger, AuditStatus},
    detect_language, Store,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
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
    uptime: Instant,
    sandbox_mode: String,
    sandbox_prompt: Option<String>,
    /// Fast model for classification and direct responses (Sonnet).
    model_fast: String,
    /// Complex model for multi-step autonomous execution (Opus).
    model_complex: String,
    /// Tracks senders with active provider calls. New messages are buffered here.
    active_senders: Mutex<HashMap<String, Vec<IncomingMessage>>>,
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
        sandbox_mode: String,
        sandbox_prompt: Option<String>,
        model_fast: String,
        model_complex: String,
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
            uptime: Instant::now(),
            sandbox_mode,
            sandbox_prompt,
            model_fast,
            model_complex,
            active_senders: Mutex::new(HashMap::new()),
        }
    }

    /// Run the main event loop.
    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
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
            let sched_provider = self.provider.clone();
            let sched_skills = self.skills.clone();
            let sched_prompts = self.prompts.clone();
            let sched_model = self.model_complex.clone();
            let sched_sandbox = self.sandbox_prompt.clone();
            let sched_hb_config = self.heartbeat_config.clone();
            Some(tokio::spawn(async move {
                Self::scheduler_loop(
                    sched_store,
                    sched_channels,
                    poll_secs,
                    sched_provider,
                    sched_skills,
                    sched_prompts,
                    sched_model,
                    sched_sandbox,
                    sched_hb_config,
                )
                .await;
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
                    let gw = self.clone();
                    tokio::spawn(async move {
                        gw.dispatch_message(incoming).await;
                    });
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

    /// Dispatch a message: buffer if sender is busy, otherwise process.
    async fn dispatch_message(self: Arc<Self>, incoming: IncomingMessage) {
        let sender_key = format!("{}:{}", incoming.channel, incoming.sender_id);

        {
            let mut active = self.active_senders.lock().await;
            if active.contains_key(&sender_key) {
                // Sender already has an active call — buffer this message.
                active.get_mut(&sender_key).unwrap().push(incoming.clone());
                info!(
                    "buffered message from {} (active call in progress)",
                    sender_key
                );
                self.send_text(&incoming, "Got it, I'll get to this next.")
                    .await;
                return;
            }
            // Mark sender as active (empty buffer).
            active.insert(sender_key.clone(), Vec::new());
        }

        // Process the message.
        self.handle_message(incoming).await;

        // Drain any buffered messages for this sender.
        loop {
            let next = {
                let mut active = self.active_senders.lock().await;
                let buffer = active.get_mut(&sender_key);
                match buffer {
                    Some(buf) if !buf.is_empty() => Some(buf.remove(0)),
                    _ => {
                        // No more buffered messages — remove sender from active.
                        active.remove(&sender_key);
                        None
                    }
                }
            };

            match next {
                Some(buffered_msg) => {
                    info!("processing buffered message from {}", sender_key);
                    self.handle_message(buffered_msg).await;
                }
                None => break,
            }
        }
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
    ///
    /// Reminder tasks send a text message. Action tasks invoke the provider
    /// with full tool access and process response markers.
    #[allow(clippy::too_many_arguments)]
    async fn scheduler_loop(
        store: Store,
        channels: HashMap<String, Arc<dyn Channel>>,
        poll_secs: u64,
        provider: Arc<dyn Provider>,
        skills: Vec<omega_skills::Skill>,
        prompts: Prompts,
        model_complex: String,
        sandbox_prompt: Option<String>,
        heartbeat_config: HeartbeatConfig,
    ) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(poll_secs)).await;

            match store.get_due_tasks().await {
                Ok(tasks) => {
                    for (id, channel_name, reply_target, description, repeat, task_type) in &tasks {
                        if task_type == "action" {
                            // --- Action task: invoke provider ---
                            info!("scheduler: executing action task {id}: {description}");

                            let mut system = format!(
                                "{}\n\n{}\n\n{}",
                                prompts.identity, prompts.soul, prompts.system
                            );
                            if let Some(ref sp) = sandbox_prompt {
                                system.push_str("\n\n");
                                system.push_str(sp);
                            }

                            let mut ctx = Context::new(description);
                            ctx.system_prompt = system;
                            ctx.model = Some(model_complex.clone());

                            // Match skill triggers on description to inject MCP servers.
                            let matched_servers =
                                omega_skills::match_skill_triggers(&skills, description);
                            ctx.mcp_servers = matched_servers;

                            match provider.complete(&ctx).await {
                                Ok(resp) => {
                                    let mut text = resp.text.clone();

                                    // Process SCHEDULE markers from action response.
                                    if let Some(sched_line) = extract_schedule_marker(&text) {
                                        if let Some((desc, due, rep)) =
                                            parse_schedule_line(&sched_line)
                                        {
                                            let rep_opt = if rep == "once" {
                                                None
                                            } else {
                                                Some(rep.as_str())
                                            };
                                            match store
                                                .create_task(
                                                    channel_name,
                                                    "",
                                                    reply_target,
                                                    &desc,
                                                    &due,
                                                    rep_opt,
                                                    "reminder",
                                                )
                                                .await
                                            {
                                                Ok(new_id) => info!(
                                                    "action task spawned reminder {new_id}: {desc}"
                                                ),
                                                Err(e) => error!(
                                                    "action task: failed to create reminder: {e}"
                                                ),
                                            }
                                        }
                                        text = strip_schedule_marker(&text);
                                    }

                                    // Process SCHEDULE_ACTION markers from action response.
                                    if let Some(sched_line) = extract_schedule_action_marker(&text)
                                    {
                                        if let Some((desc, due, rep)) =
                                            parse_schedule_action_line(&sched_line)
                                        {
                                            let rep_opt = if rep == "once" {
                                                None
                                            } else {
                                                Some(rep.as_str())
                                            };
                                            match store
                                                .create_task(
                                                    channel_name,
                                                    "",
                                                    reply_target,
                                                    &desc,
                                                    &due,
                                                    rep_opt,
                                                    "action",
                                                )
                                                .await
                                            {
                                                Ok(new_id) => info!(
                                                    "action task spawned action {new_id}: {desc}"
                                                ),
                                                Err(e) => error!(
                                                    "action task: failed to create action: {e}"
                                                ),
                                            }
                                        }
                                        text = strip_schedule_action_markers(&text);
                                    }

                                    // Process HEARTBEAT markers.
                                    let hb_actions = extract_heartbeat_markers(&text);
                                    if !hb_actions.is_empty() {
                                        apply_heartbeat_changes(&hb_actions);
                                        text = strip_heartbeat_markers(&text);
                                    }

                                    // Process LIMITATION markers.
                                    if let Some(lim_line) = extract_limitation_marker(&text) {
                                        if let Some((title, desc, plan)) =
                                            parse_limitation_line(&lim_line)
                                        {
                                            match store.store_limitation(&title, &desc, &plan).await
                                            {
                                                Ok(true) => {
                                                    info!("action task: new limitation: {title}");
                                                    if let Some(ch) =
                                                        channels.get(&heartbeat_config.channel)
                                                    {
                                                        let alert = format!(
                                                            "LIMITATION DETECTED: {title}\n{desc}\n\nProposed fix: {plan}"
                                                        );
                                                        let msg = OutgoingMessage {
                                                            text: alert,
                                                            metadata: MessageMetadata::default(),
                                                            reply_target: Some(
                                                                heartbeat_config
                                                                    .reply_target
                                                                    .clone(),
                                                            ),
                                                        };
                                                        let _ = ch.send(msg).await;
                                                    }
                                                    apply_heartbeat_changes(&[
                                                        HeartbeatAction::Add(format!(
                                                            "CRITICAL: {title} — {desc}"
                                                        )),
                                                    ]);
                                                }
                                                Ok(false) => info!(
                                                    "action task: duplicate limitation: {title}"
                                                ),
                                                Err(e) => error!(
                                                    "action task: failed to store limitation: {e}"
                                                ),
                                            }
                                        }
                                        text = strip_limitation_markers(&text);
                                    }

                                    // Process SELF_HEAL markers.
                                    if let Some(heal_line) = extract_self_heal_marker(&text) {
                                        if let Some(heal_desc) = parse_self_heal_line(&heal_line) {
                                            let mut state = read_self_healing_state()
                                                .unwrap_or_else(|| SelfHealingState {
                                                    anomaly: heal_desc.clone(),
                                                    iteration: 0,
                                                    max_iterations: 10,
                                                    started_at: chrono::Utc::now().to_rfc3339(),
                                                    attempts: Vec::new(),
                                                });
                                            state.iteration += 1;

                                            if state.iteration > state.max_iterations {
                                                let alert = format!(
                                                    "🚨 SELF-HEALING ESCALATION\n\n\
                                                     Anomaly: {}\n\
                                                     Iterations: {}/{}\n\
                                                     Started: {}\n\n\
                                                     Attempts:\n{}\n\n\
                                                     Human intervention required.",
                                                    state.anomaly,
                                                    state.iteration,
                                                    state.max_iterations,
                                                    state.started_at,
                                                    state
                                                        .attempts
                                                        .iter()
                                                        .enumerate()
                                                        .map(|(i, a)| format!("{}. {a}", i + 1))
                                                        .collect::<Vec<_>>()
                                                        .join("\n")
                                                );
                                                if let Some(ch) =
                                                    channels.get(&heartbeat_config.channel)
                                                {
                                                    let msg = OutgoingMessage {
                                                        text: alert,
                                                        metadata: MessageMetadata::default(),
                                                        reply_target: Some(
                                                            heartbeat_config.reply_target.clone(),
                                                        ),
                                                    };
                                                    let _ = ch.send(msg).await;
                                                }
                                                if let Err(e) = write_self_healing_state(&state) {
                                                    error!("self-heal: failed to write state: {e}");
                                                }
                                                info!(
                                                    "self-heal: escalated after {} iterations",
                                                    state.iteration
                                                );
                                            } else {
                                                if let Err(e) = write_self_healing_state(&state) {
                                                    error!("self-heal: failed to write state: {e}");
                                                }
                                                let alert = format!(
                                                    "🔧 SELF-HEALING ({}/{}): {}",
                                                    state.iteration,
                                                    state.max_iterations,
                                                    state.anomaly
                                                );
                                                if let Some(ch) =
                                                    channels.get(&heartbeat_config.channel)
                                                {
                                                    let msg = OutgoingMessage {
                                                        text: alert,
                                                        metadata: MessageMetadata::default(),
                                                        reply_target: Some(
                                                            heartbeat_config.reply_target.clone(),
                                                        ),
                                                    };
                                                    let _ = ch.send(msg).await;
                                                }
                                                let due_at = (chrono::Utc::now()
                                                    + chrono::Duration::minutes(2))
                                                .to_rfc3339();
                                                let next_desc = format!(
                                                    "Self-healing verification — read \
                                                     ~/.omega/self-healing.json for context, \
                                                     check if the anomaly is resolved. \
                                                     If resolved, emit SELF_HEAL_RESOLVED. \
                                                     If not, diagnose, fix, build+clippy until \
                                                     clean, restart service, update the attempts \
                                                     array in self-healing.json, and emit \
                                                     SELF_HEAL: {} to continue.",
                                                    state.anomaly
                                                );
                                                match store
                                                    .create_task(
                                                        channel_name,
                                                        "",
                                                        reply_target,
                                                        &next_desc,
                                                        &due_at,
                                                        None,
                                                        "action",
                                                    )
                                                    .await
                                                {
                                                    Ok(new_id) => info!(
                                                        "self-heal: scheduled verification \
                                                         task {new_id} (iteration {})",
                                                        state.iteration
                                                    ),
                                                    Err(e) => error!(
                                                        "self-heal: failed to schedule \
                                                         verification: {e}"
                                                    ),
                                                }
                                            }
                                        }
                                        text = strip_self_heal_markers(&text);
                                    }

                                    // Process SELF_HEAL_RESOLVED marker.
                                    if has_self_heal_resolved_marker(&text) {
                                        match delete_self_healing_state() {
                                            Ok(()) => {
                                                info!(
                                                    "self-heal: anomaly resolved (via scheduler)"
                                                );
                                                if let Some(ch) =
                                                    channels.get(&heartbeat_config.channel)
                                                {
                                                    let msg = OutgoingMessage {
                                                        text: "✅ Self-healing complete — anomaly resolved.".to_string(),
                                                        metadata: MessageMetadata::default(),
                                                        reply_target: Some(
                                                            heartbeat_config.reply_target.clone(),
                                                        ),
                                                    };
                                                    let _ = ch.send(msg).await;
                                                }
                                            }
                                            Err(e) => {
                                                error!(
                                                    "self-heal: failed to delete state file: {e}"
                                                );
                                            }
                                        }
                                        text = strip_self_heal_markers(&text);
                                    }

                                    // Send response to channel (if non-empty after stripping markers).
                                    let cleaned = text.trim();
                                    if !cleaned.is_empty() && cleaned != "HEARTBEAT_OK" {
                                        if let Some(ch) = channels.get(channel_name) {
                                            let msg = OutgoingMessage {
                                                text: cleaned.to_string(),
                                                metadata: MessageMetadata::default(),
                                                reply_target: Some(reply_target.clone()),
                                            };
                                            if let Err(e) = ch.send(msg).await {
                                                error!("action task {id}: failed to send response: {e}");
                                            }
                                        }
                                    }

                                    info!("completed action task {id}: {description}");
                                }
                                Err(e) => {
                                    error!("action task {id} provider error: {e}");
                                    // Send error notification.
                                    if let Some(ch) = channels.get(channel_name) {
                                        let msg = OutgoingMessage {
                                            text: format!("Action task failed: {description}\n(will retry next cycle)"),
                                            metadata: MessageMetadata::default(),
                                            reply_target: Some(reply_target.clone()),
                                        };
                                        let _ = ch.send(msg).await;
                                    }
                                }
                            }
                        } else {
                            // --- Reminder task: send text ---
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

            // Inject known open limitations.
            if let Ok(limitations) = memory.get_open_limitations().await {
                if !limitations.is_empty() {
                    prompt.push_str("\n\nKnown open limitations (previously detected):");
                    for (title, desc, plan) in &limitations {
                        prompt.push_str(&format!("\n- {title}: {desc} (plan: {plan})"));
                    }
                }
            }

            // Self-audit instruction.
            prompt.push_str(
                "\n\nBeyond the checklist, reflect on your own capabilities. \
                 If you detect a NEW limitation (something you CANNOT do but SHOULD be able to), \
                 include LIMITATION: <title> | <description> | <plan> on its own line.",
            );

            let ctx = Context::new(&prompt);
            match provider.complete(&ctx).await {
                Ok(resp) => {
                    // Process limitation markers from heartbeat response.
                    if let Some(lim_line) = extract_limitation_marker(&resp.text) {
                        if let Some((title, desc, plan)) = parse_limitation_line(&lim_line) {
                            match memory.store_limitation(&title, &desc, &plan).await {
                                Ok(true) => {
                                    info!("heartbeat: new limitation detected: {title}");
                                    if let Some(ch) = channels.get(&config.channel) {
                                        let alert = format!(
                                            "LIMITATION DETECTED: {title}\n{desc}\n\nProposed fix: {plan}"
                                        );
                                        let msg = OutgoingMessage {
                                            text: alert,
                                            metadata: MessageMetadata::default(),
                                            reply_target: Some(config.reply_target.clone()),
                                        };
                                        if let Err(e) = ch.send(msg).await {
                                            error!(
                                                "heartbeat: failed to send limitation alert: {e}"
                                            );
                                        }
                                    }
                                    apply_heartbeat_changes(&[HeartbeatAction::Add(format!(
                                        "CRITICAL: {title} — {desc}"
                                    ))]);
                                }
                                Ok(false) => {
                                    info!("heartbeat: duplicate limitation: {title}");
                                }
                                Err(e) => {
                                    error!("heartbeat: failed to store limitation: {e}");
                                }
                            }
                        }
                    }

                    let cleaned: String = resp
                        .text
                        .chars()
                        .filter(|c| *c != '*' && *c != '`')
                        .collect();
                    let cleaned = strip_limitation_markers(&cleaned);
                    if cleaned.trim().contains("HEARTBEAT_OK") {
                        info!("heartbeat: OK");
                    } else if let Some(ch) = channels.get(&config.channel) {
                        let text = strip_limitation_markers(&resp.text);
                        let msg = OutgoingMessage {
                            text,
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

        // --- 2a. SAVE INCOMING IMAGE ATTACHMENTS ---
        let inbox_images = if !incoming.attachments.is_empty() {
            let inbox = ensure_inbox_dir(&self.data_dir);
            let paths = save_attachments_to_inbox(&inbox, &incoming.attachments);
            // Prepend image paths to the message text so the provider can read them.
            for path in &paths {
                clean_incoming.text = format!(
                    "[Attached image: {}]\n{}",
                    path.display(),
                    clean_incoming.text
                );
            }
            paths
        } else {
            Vec::new()
        };

        // --- 2b. FIRST-TIME USER DETECTION ---
        // No separate welcome message — the AI handles introduction via onboarding hint.
        // We still detect language and mark the user as welcomed for onboarding tracking.
        if let Ok(true) = self.memory.is_new_user(&incoming.sender_id).await {
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

        // --- 3. COMMAND DISPATCH ---
        // Hot-reload projects from disk so newly created ones are available immediately.
        let projects = omega_skills::load_projects(&self.data_dir);
        if let Some(cmd) = commands::Command::parse(&clean_incoming.text) {
            let ctx = commands::CommandContext {
                store: &self.memory,
                channel: &incoming.channel,
                sender_id: &incoming.sender_id,
                text: &clean_incoming.text,
                uptime: &self.uptime,
                provider_name: self.provider.name(),
                skills: &self.skills,
                projects: &projects,
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
        let active_project: Option<String> = self
            .memory
            .get_fact(&incoming.sender_id, "active_project")
            .await
            .ok()
            .flatten();

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

            if let Some(ref project_name) = active_project {
                if let Some(instructions) =
                    omega_skills::get_project_instructions(&projects, project_name)
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

        // --- 5. AUTONOMOUS CLASSIFICATION & MODEL ROUTING ---
        // Every message gets a fast Sonnet classification call that determines
        // whether to handle directly or decompose into steps.
        let skill_names: Vec<&str> = self.skills.iter().map(|s| s.name.as_str()).collect();
        if let Some(steps) = self
            .classify_and_route(
                &clean_incoming.text,
                active_project.as_deref(),
                &context.history,
                &skill_names,
            )
            .await
        {
            // Complex task → Opus executes each step.
            info!(
                "[{}] classification: {} steps → model {}",
                incoming.channel,
                steps.len(),
                self.model_complex
            );
            context.model = Some(self.model_complex.clone());
            self.execute_steps(
                &incoming,
                &clean_incoming.text,
                &context,
                &steps,
                &inbox_images,
            )
            .await;

            // Stop typing indicator and return — skip normal send flow.
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

        // --- 5b. GET RESPONSE FROM PROVIDER (async with status updates) ---

        // Snapshot workspace images before provider call.
        let workspace_path = PathBuf::from(shellexpand(&self.data_dir)).join("workspace");
        let images_before = snapshot_workspace_images(&workspace_path);

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
                info!(
                    "[{}] provider responded | model: {} | {}ms",
                    incoming.channel,
                    resp.metadata.model.as_deref().unwrap_or("unknown"),
                    resp.metadata.processing_time_ms
                );
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
                        "reminder",
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

        // --- 5b2. EXTRACT SCHEDULE_ACTION MARKER ---
        if let Some(sched_line) = extract_schedule_action_marker(&response.text) {
            if let Some((desc, due_at, repeat)) = parse_schedule_action_line(&sched_line) {
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
                        "action",
                    )
                    .await
                {
                    Ok(id) => {
                        info!("scheduled action task {id}: {desc} at {due_at}");
                    }
                    Err(e) => {
                        error!("failed to create action task: {e}");
                    }
                }
            }
            response.text = strip_schedule_action_markers(&response.text);
        }

        // --- 5c. EXTRACT PROJECT MARKER ---
        if let Some(project_name) = extract_project_activate(&response.text) {
            // Verify project exists on disk before activating.
            let fresh_projects = omega_skills::load_projects(&self.data_dir);
            if omega_skills::get_project_instructions(&fresh_projects, &project_name).is_some() {
                if let Err(e) = self
                    .memory
                    .store_fact(&incoming.sender_id, "active_project", &project_name)
                    .await
                {
                    error!("failed to activate project {project_name}: {e}");
                } else {
                    info!("project activated: {project_name}");
                }
            } else {
                warn!("project activate marker for unknown project: {project_name}");
            }
            response.text = strip_project_markers(&response.text);
        }
        if has_project_deactivate(&response.text) {
            if let Err(e) = self
                .memory
                .delete_fact(&incoming.sender_id, "active_project")
                .await
            {
                error!("failed to deactivate project: {e}");
            } else {
                info!("project deactivated");
            }
            response.text = strip_project_markers(&response.text);
        }

        // --- 5e. EXTRACT WHATSAPP_QR MARKER ---
        if has_whatsapp_qr_marker(&response.text) {
            response.text = strip_whatsapp_qr_marker(&response.text);
            self.handle_whatsapp_qr(&incoming).await;
        }

        // --- 5f. EXTRACT LANG_SWITCH MARKER ---
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

        // --- 5g. EXTRACT HEARTBEAT_ADD / HEARTBEAT_REMOVE MARKERS ---
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

        // --- 5h. EXTRACT LIMITATION MARKER ---
        if let Some(limitation_line) = extract_limitation_marker(&response.text) {
            if let Some((title, description, plan)) = parse_limitation_line(&limitation_line) {
                match self
                    .memory
                    .store_limitation(&title, &description, &plan)
                    .await
                {
                    Ok(true) => {
                        info!("limitation detected (new): {title}");
                        // Send Telegram alert via heartbeat channel.
                        let alert = format!(
                            "LIMITATION DETECTED: {title}\n{description}\n\nProposed fix: {plan}"
                        );
                        if let Some(ch) = self.channels.get(&self.heartbeat_config.channel) {
                            let msg = OutgoingMessage {
                                text: alert,
                                metadata: MessageMetadata::default(),
                                reply_target: Some(self.heartbeat_config.reply_target.clone()),
                            };
                            if let Err(e) = ch.send(msg).await {
                                error!("limitation: failed to send alert: {e}");
                            }
                        }
                        // Auto-add to heartbeat checklist.
                        apply_heartbeat_changes(&[HeartbeatAction::Add(format!(
                            "CRITICAL: {title} — {description}"
                        ))]);
                    }
                    Ok(false) => {
                        info!("limitation detected (duplicate): {title}");
                    }
                    Err(e) => {
                        error!("failed to store limitation: {e}");
                    }
                }
            }
            response.text = strip_limitation_markers(&response.text);
        }

        // --- 5i. EXTRACT SELF_HEAL MARKER ---
        if let Some(heal_line) = extract_self_heal_marker(&response.text) {
            if let Some(description) = parse_self_heal_line(&heal_line) {
                let mut state = read_self_healing_state().unwrap_or_else(|| SelfHealingState {
                    anomaly: description.clone(),
                    iteration: 0,
                    max_iterations: 10,
                    started_at: chrono::Utc::now().to_rfc3339(),
                    attempts: Vec::new(),
                });
                state.iteration += 1;

                if state.iteration > state.max_iterations {
                    // Escalate to owner — max iterations reached.
                    let alert = format!(
                        "🚨 SELF-HEALING ESCALATION\n\n\
                         Anomaly: {}\n\
                         Iterations: {}/{}\n\
                         Started: {}\n\n\
                         Attempts:\n{}\n\n\
                         Human intervention required.",
                        state.anomaly,
                        state.iteration,
                        state.max_iterations,
                        state.started_at,
                        state
                            .attempts
                            .iter()
                            .enumerate()
                            .map(|(i, a)| format!("{}. {a}", i + 1))
                            .collect::<Vec<_>>()
                            .join("\n")
                    );
                    if let Some(ch) = self.channels.get(&self.heartbeat_config.channel) {
                        let msg = OutgoingMessage {
                            text: alert,
                            metadata: MessageMetadata::default(),
                            reply_target: Some(self.heartbeat_config.reply_target.clone()),
                        };
                        if let Err(e) = ch.send(msg).await {
                            error!("self-heal: failed to send escalation: {e}");
                        }
                    }
                    // Keep file for owner review.
                    if let Err(e) = write_self_healing_state(&state) {
                        error!("self-heal: failed to write state: {e}");
                    }
                    info!(
                        "self-heal: escalated after {} iterations: {}",
                        state.iteration, state.anomaly
                    );
                } else {
                    // Normal iteration — notify owner and schedule follow-up.
                    if let Err(e) = write_self_healing_state(&state) {
                        error!("self-heal: failed to write state: {e}");
                    }
                    let alert = format!(
                        "🔧 SELF-HEALING ({}/{}): {}",
                        state.iteration, state.max_iterations, state.anomaly
                    );
                    if let Some(ch) = self.channels.get(&self.heartbeat_config.channel) {
                        let msg = OutgoingMessage {
                            text: alert,
                            metadata: MessageMetadata::default(),
                            reply_target: Some(self.heartbeat_config.reply_target.clone()),
                        };
                        if let Err(e) = ch.send(msg).await {
                            error!("self-heal: failed to send notification: {e}");
                        }
                    }
                    // Schedule follow-up verification in 2 minutes.
                    let due_at = (chrono::Utc::now() + chrono::Duration::minutes(2)).to_rfc3339();
                    let heal_desc = format!(
                        "Self-healing verification — read ~/.omega/self-healing.json for context, \
                         check if the anomaly is resolved. If resolved, emit SELF_HEAL_RESOLVED. \
                         If not, diagnose, fix, build+clippy until clean, restart service, \
                         update the attempts array in self-healing.json, \
                         and emit SELF_HEAL: {} to continue.",
                        state.anomaly
                    );
                    let reply_target = incoming.reply_target.as_deref().unwrap_or("");
                    match self
                        .memory
                        .create_task(
                            &incoming.channel,
                            &incoming.sender_id,
                            reply_target,
                            &heal_desc,
                            &due_at,
                            None,
                            "action",
                        )
                        .await
                    {
                        Ok(id) => {
                            info!(
                                "self-heal: scheduled verification task {id} (iteration {})",
                                state.iteration
                            );
                        }
                        Err(e) => {
                            error!("self-heal: failed to schedule verification: {e}");
                        }
                    }
                }
            }
            response.text = strip_self_heal_markers(&response.text);
        }

        // --- 5j. EXTRACT SELF_HEAL_RESOLVED MARKER ---
        if has_self_heal_resolved_marker(&response.text) {
            match delete_self_healing_state() {
                Ok(()) => {
                    info!("self-heal: anomaly resolved, state file deleted");
                    let alert = "✅ Self-healing complete — anomaly resolved.".to_string();
                    if let Some(ch) = self.channels.get(&self.heartbeat_config.channel) {
                        let msg = OutgoingMessage {
                            text: alert,
                            metadata: MessageMetadata::default(),
                            reply_target: Some(self.heartbeat_config.reply_target.clone()),
                        };
                        if let Err(e) = ch.send(msg).await {
                            error!("self-heal: failed to send resolution notice: {e}");
                        }
                    }
                }
                Err(e) => {
                    error!("self-heal: failed to delete state file: {e}");
                }
            }
            response.text = strip_self_heal_markers(&response.text);
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

        info!(
            "[{}] audit logged | sender: {}",
            incoming.channel, incoming.sender_id
        );

        // --- 8. SEND RESPONSE ---
        if let Some(channel) = self.channels.get(&incoming.channel) {
            if let Err(e) = channel.send(response).await {
                error!("failed to send response via {}: {e}", incoming.channel);
            }

            // --- 8b. SEND NEW WORKSPACE IMAGES ---
            // Detect files that are new OR were modified (overwritten) since the snapshot.
            let images_after = snapshot_workspace_images(&workspace_path);
            let new_images: Vec<PathBuf> = images_after
                .iter()
                .filter(|(path, mtime)| {
                    match images_before.get(path.as_path()) {
                        None => true,                          // new file
                        Some(old_mtime) => mtime > &old_mtime, // overwritten file
                    }
                })
                .map(|(path, _)| path.clone())
                .collect();
            let target = incoming.reply_target.as_deref().unwrap_or("");
            for image_path in &new_images {
                let filename = image_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("image.png");
                match std::fs::read(image_path) {
                    Ok(bytes) => {
                        if let Err(e) = channel.send_photo(target, &bytes, filename).await {
                            warn!("failed to send workspace image {filename}: {e}");
                        } else {
                            info!("sent workspace image: {filename}");
                        }
                    }
                    Err(e) => {
                        warn!("failed to read workspace image {filename}: {e}");
                    }
                }
                // Clean up the file after sending.
                if let Err(e) = std::fs::remove_file(image_path) {
                    warn!("failed to remove workspace image {filename}: {e}");
                }
            }
        } else {
            error!("no channel found for '{}'", incoming.channel);
        }

        // --- 9. CLEANUP INBOX IMAGES ---
        if !inbox_images.is_empty() {
            cleanup_inbox_images(&inbox_images);
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

    /// Classify a message and route to the appropriate model.
    ///
    /// Always runs a fast Sonnet classification call. Returns parsed steps for
    /// complex tasks or `None` for simple/direct responses.
    async fn classify_and_route(
        &self,
        message: &str,
        active_project: Option<&str>,
        recent_history: &[ContextEntry],
        skill_names: &[&str],
    ) -> Option<Vec<String>> {
        let context_block =
            build_classification_context(active_project, recent_history, skill_names);
        let context_section = if context_block.is_empty() {
            String::new()
        } else {
            format!("Context:\n{context_block}\n\n")
        };

        let planning_prompt = format!(
            "You are a task classifier. Do NOT use any tools — respond with text only.\n\n\
             If this request is a simple question, greeting, or single-action task, respond \
             with exactly: DIRECT\n\
             If it requires multiple independent steps, respond with ONLY a numbered list of \
             small self-contained steps. Nothing else.\n\n\
             {context_section}\
             Request: {message}"
        );

        let mut ctx = Context::new(&planning_prompt);
        ctx.max_turns = Some(5);
        ctx.model = Some(self.model_fast.clone());
        match self.provider.complete(&ctx).await {
            Ok(resp) => parse_plan_response(&resp.text),
            Err(e) => {
                warn!("classification call failed, falling back to direct: {e}");
                None
            }
        }
    }

    /// Execute a list of steps autonomously, with progress updates and retry.
    async fn execute_steps(
        &self,
        incoming: &IncomingMessage,
        original_task: &str,
        context: &Context,
        steps: &[String],
        inbox_images: &[PathBuf],
    ) {
        let total = steps.len();
        info!("pre-flight planning: decomposed into {total} steps");

        // Announce the plan.
        let announcement = format!("On it — {total} things to work through. I'll keep you posted.");
        self.send_text(incoming, &announcement).await;

        let mut completed_summary = String::new();

        for (i, step) in steps.iter().enumerate() {
            let step_num = i + 1;
            info!("planning: executing step {step_num}/{total}: {step}");

            // Build step prompt with context.
            let step_prompt = if completed_summary.is_empty() {
                format!(
                    "Original task: {original_task}\n\n\
                     Execute step {step_num}/{total}: {step}"
                )
            } else {
                format!(
                    "Original task: {original_task}\n\n\
                     Completed so far:\n{completed_summary}\n\n\
                     Now execute step {step_num}/{total}: {step}"
                )
            };

            let mut step_ctx = Context::new(&step_prompt);
            step_ctx.system_prompt = context.system_prompt.clone();
            step_ctx.mcp_servers = context.mcp_servers.clone();
            step_ctx.model = context.model.clone();

            // Retry loop for each step (up to 3 attempts).
            let mut step_result = None;
            for attempt in 1..=3u32 {
                match self.provider.complete(&step_ctx).await {
                    Ok(resp) => {
                        step_result = Some(resp);
                        break;
                    }
                    Err(e) => {
                        warn!("planning: step {step_num} attempt {attempt}/3 failed: {e}");
                        if attempt < 3 {
                            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        }
                    }
                }
            }

            match step_result {
                Some(step_resp) => {
                    completed_summary.push_str(&format!("- Step {step_num}: {step} (done)\n"));

                    // Send progress update.
                    let progress = format!("✓ {step} ({step_num}/{total})");
                    self.send_text(incoming, &progress).await;

                    // Audit each step.
                    let _ = self
                        .audit
                        .log(&AuditEntry {
                            channel: incoming.channel.clone(),
                            sender_id: incoming.sender_id.clone(),
                            sender_name: incoming.sender_name.clone(),
                            input_text: format!("[Step {step_num}/{total}] {step}"),
                            output_text: Some(step_resp.text.clone()),
                            provider_used: Some(step_resp.metadata.provider_used.clone()),
                            model: step_resp.metadata.model.clone(),
                            processing_ms: Some(step_resp.metadata.processing_time_ms as i64),
                            status: AuditStatus::Ok,
                            denial_reason: None,
                        })
                        .await;
                }
                None => {
                    completed_summary.push_str(&format!("- Step {step_num}: {step} (FAILED)\n"));
                    let fail_msg = format!("✗ Couldn't complete: {step} ({step_num}/{total})");
                    self.send_text(incoming, &fail_msg).await;
                }
            }
        }

        // Send final summary.
        let final_msg = format!("Done — all {total} wrapped up ✓");
        self.send_text(incoming, &final_msg).await;

        // Cleanup inbox images.
        if !inbox_images.is_empty() {
            cleanup_inbox_images(inbox_images);
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

/// Ensure the inbox directory exists under `{data_dir}/workspace/inbox/`.
fn ensure_inbox_dir(data_dir: &str) -> PathBuf {
    let dir = PathBuf::from(shellexpand(data_dir))
        .join("workspace")
        .join("inbox");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Save image attachments to the inbox directory. Returns the paths of saved files.
fn save_attachments_to_inbox(
    inbox: &Path,
    attachments: &[omega_core::message::Attachment],
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for att in attachments {
        if !matches!(att.file_type, AttachmentType::Image) {
            continue;
        }
        if let Some(ref data) = att.data {
            let filename = att.filename.as_deref().unwrap_or("image.jpg");
            let path = inbox.join(filename);
            if let Err(e) = std::fs::write(&path, data) {
                warn!("failed to save inbox image {filename}: {e}");
            } else {
                info!("saved inbox image: {}", path.display());
                paths.push(path);
            }
        }
    }
    paths
}

/// Remove previously saved inbox image files.
fn cleanup_inbox_images(paths: &[PathBuf]) {
    for path in paths {
        if let Err(e) = std::fs::remove_file(path) {
            warn!("failed to remove inbox image {}: {e}", path.display());
        }
    }
}

/// Build a lightweight context block for the classification prompt.
///
/// Includes active project name, last 3 messages (truncated to 80 chars),
/// and available skill names. Empty sections are omitted. Returns an empty
/// string when all inputs are empty, preserving identical prompt behavior.
fn build_classification_context(
    active_project: Option<&str>,
    recent_history: &[ContextEntry],
    skill_names: &[&str],
) -> String {
    let mut parts = Vec::new();

    if let Some(project) = active_project {
        parts.push(format!("Active project: {project}"));
    }

    let recent: Vec<&ContextEntry> = recent_history.iter().rev().take(3).collect();
    if !recent.is_empty() {
        let mut history_block = String::from("Recent conversation:");
        for entry in recent.iter().rev() {
            let role = if entry.role == "user" {
                "User"
            } else {
                "Assistant"
            };
            let truncated = if entry.content.len() > 80 {
                format!("{}...", &entry.content[..80])
            } else {
                entry.content.clone()
            };
            history_block.push_str(&format!("\n  {role}: {truncated}"));
        }
        parts.push(history_block);
    }

    if !skill_names.is_empty() {
        parts.push(format!("Available skills: {}", skill_names.join(", ")));
    }

    if parts.is_empty() {
        String::new()
    } else {
        parts.join("\n")
    }
}

/// Parse a planning response into a list of steps.
///
/// Returns `None` for:
/// - "DIRECT" responses (simple task, no decomposition needed)
/// - Single-step plans (no benefit to decomposition)
/// - Unparseable responses (fall back to direct execution)
///
/// Returns `Some(steps)` for multi-step numbered lists.
fn parse_plan_response(text: &str) -> Option<Vec<String>> {
    let trimmed = text.trim();

    // Check for DIRECT marker (case-insensitive, may have surrounding text).
    if trimmed
        .lines()
        .any(|line| line.trim().eq_ignore_ascii_case("DIRECT"))
    {
        return None;
    }

    // Extract numbered steps: "1. Step description", "2) Step description", etc.
    let steps: Vec<String> = trimmed
        .lines()
        .filter_map(|line| {
            let t = line.trim();
            t.strip_prefix(|c: char| c.is_ascii_digit())
                .and_then(|s| s.strip_prefix(". ").or_else(|| s.strip_prefix(") ")))
                .map(|rest| rest.trim().to_string())
                .filter(|s| !s.is_empty())
        })
        .collect();

    // Single-step plans have no benefit — treat as direct.
    if steps.len() <= 1 {
        return None;
    }

    Some(steps)
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

/// Extract project name from a `PROJECT_ACTIVATE: <name>` marker line.
fn extract_project_activate(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("PROJECT_ACTIVATE:"))
        .and_then(|line| {
            let name = line
                .trim()
                .strip_prefix("PROJECT_ACTIVATE:")?
                .trim()
                .to_string();
            if name.is_empty() {
                None
            } else {
                Some(name)
            }
        })
}

/// Check if response text contains a `PROJECT_DEACTIVATE` marker line.
fn has_project_deactivate(text: &str) -> bool {
    text.lines().any(|line| line.trim() == "PROJECT_DEACTIVATE")
}

/// Strip all `PROJECT_ACTIVATE:` and `PROJECT_DEACTIVATE` lines from response text.
fn strip_project_markers(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let t = line.trim();
            !t.starts_with("PROJECT_ACTIVATE:") && t != "PROJECT_DEACTIVATE"
        })
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

/// Extract the first `SCHEDULE_ACTION:` line from response text.
fn extract_schedule_action_marker(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("SCHEDULE_ACTION:"))
        .map(|line| line.trim().to_string())
}

/// Parse a schedule action line: `SCHEDULE_ACTION: desc | ISO datetime | repeat`
fn parse_schedule_action_line(line: &str) -> Option<(String, String, String)> {
    let content = line.strip_prefix("SCHEDULE_ACTION:")?.trim();
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

/// Strip all `SCHEDULE_ACTION:` lines from response text.
fn strip_schedule_action_markers(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("SCHEDULE_ACTION:"))
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

/// Extract the first `LIMITATION:` line from response text.
fn extract_limitation_marker(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("LIMITATION:"))
        .map(|line| line.trim().to_string())
}

/// Parse a limitation line: `LIMITATION: title | description | proposed plan`
fn parse_limitation_line(line: &str) -> Option<(String, String, String)> {
    let content = line.strip_prefix("LIMITATION:")?.trim();
    let parts: Vec<&str> = content.splitn(3, '|').collect();
    if parts.len() != 3 {
        return None;
    }
    let title = parts[0].trim().to_string();
    let description = parts[1].trim().to_string();
    let plan = parts[2].trim().to_string();
    if title.is_empty() || description.is_empty() {
        return None;
    }
    Some((title, description, plan))
}

/// Strip all `LIMITATION:` lines from response text.
fn strip_limitation_markers(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("LIMITATION:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// State tracked in `~/.omega/self-healing.json` during active self-healing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SelfHealingState {
    /// Description of the anomaly being healed.
    pub anomaly: String,
    /// Current iteration (1-based).
    pub iteration: u32,
    /// Maximum iterations before escalation.
    pub max_iterations: u32,
    /// ISO 8601 timestamp when self-healing started.
    pub started_at: String,
    /// History of what was tried in each iteration.
    pub attempts: Vec<String>,
}

/// Extract the first `SELF_HEAL:` line from response text.
fn extract_self_heal_marker(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("SELF_HEAL:"))
        .map(|line| line.trim().to_string())
}

/// Parse the description from a `SELF_HEAL: description` line.
fn parse_self_heal_line(line: &str) -> Option<String> {
    let content = line.strip_prefix("SELF_HEAL:")?.trim();
    if content.is_empty() {
        return None;
    }
    Some(content.to_string())
}

/// Check if response text contains a `SELF_HEAL_RESOLVED` marker line.
fn has_self_heal_resolved_marker(text: &str) -> bool {
    text.lines().any(|line| line.trim() == "SELF_HEAL_RESOLVED")
}

/// Strip all `SELF_HEAL:` and `SELF_HEAL_RESOLVED` lines from response text.
fn strip_self_heal_markers(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("SELF_HEAL:") && trimmed != "SELF_HEAL_RESOLVED"
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Return the path to `~/.omega/self-healing.json`.
fn self_healing_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(format!("{home}/.omega/self-healing.json")))
}

/// Read the current self-healing state from disk.
fn read_self_healing_state() -> Option<SelfHealingState> {
    let path = self_healing_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Write the self-healing state to disk.
fn write_self_healing_state(state: &SelfHealingState) -> anyhow::Result<()> {
    let path = self_healing_path().ok_or_else(|| anyhow::anyhow!("HOME not set"))?;
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Delete the self-healing state file.
fn delete_self_healing_state() -> anyhow::Result<()> {
    let path = self_healing_path().ok_or_else(|| anyhow::anyhow!("HOME not set"))?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Return localized status messages for the delayed provider nudge.
/// Returns `(first_nudge, still_working)`.
fn status_messages(lang: &str) -> (&'static str, &'static str) {
    match lang {
        "Spanish" => ("Déjame pensar en esto... 🧠", "Sigo en ello ⏳"),
        "Portuguese" => ("Deixa eu pensar nisso... 🧠", "Ainda estou nessa ⏳"),
        "French" => ("Laisse-moi réfléchir... 🧠", "J'y suis encore ⏳"),
        "German" => ("Lass mich kurz nachdenken... 🧠", "Bin noch dran ⏳"),
        "Italian" => ("Fammi pensare... 🧠", "Ci sto ancora lavorando ⏳"),
        "Dutch" => ("Even nadenken... 🧠", "Nog mee bezig ⏳"),
        "Russian" => ("Дай подумать... 🧠", "Ещё работаю ⏳"),
        _ => ("Let me think about this... 🧠", "Still on it ⏳"),
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

/// Image file extensions recognized for workspace diff.
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// Snapshot top-level image files in the workspace directory.
///
/// Returns a map of path → modification time. Returns an empty map on any
/// error (non-existent dir, permission issues). Tracks mtime so we can detect
/// both new files and overwritten files (same name, newer mtime).
fn snapshot_workspace_images(workspace: &Path) -> HashMap<PathBuf, std::time::SystemTime> {
    let entries = match std::fs::read_dir(workspace) {
        Ok(e) => e,
        Err(_) => return HashMap::new(),
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().map(|ft| ft.is_file()).unwrap_or(false)
                && entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
                    .unwrap_or(false)
        })
        .filter_map(|entry| {
            let mtime = entry.metadata().ok()?.modified().ok()?;
            Some((entry.path(), mtime))
        })
        .collect()
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
    fn test_extract_project_activate() {
        let text = "I've created a project for you.\nPROJECT_ACTIVATE: real-estate";
        assert_eq!(
            extract_project_activate(text),
            Some("real-estate".to_string())
        );
    }

    #[test]
    fn test_extract_project_activate_none() {
        assert!(extract_project_activate("Just a normal response.").is_none());
    }

    #[test]
    fn test_extract_project_activate_empty_name() {
        assert!(extract_project_activate("PROJECT_ACTIVATE: ").is_none());
    }

    #[test]
    fn test_has_project_deactivate() {
        let text = "Project deactivated.\nPROJECT_DEACTIVATE";
        assert!(has_project_deactivate(text));
    }

    #[test]
    fn test_has_project_deactivate_false() {
        assert!(!has_project_deactivate("No marker here."));
    }

    #[test]
    fn test_strip_project_markers() {
        let text =
            "I've set up the project.\nPROJECT_ACTIVATE: stocks\nLet me know if you need more.";
        let result = strip_project_markers(text);
        assert_eq!(
            result,
            "I've set up the project.\nLet me know if you need more."
        );
    }

    #[test]
    fn test_strip_project_markers_deactivate() {
        let text = "Done, project deactivated.\nPROJECT_DEACTIVATE";
        let result = strip_project_markers(text);
        assert_eq!(result, "Done, project deactivated.");
    }

    #[test]
    fn test_strip_project_markers_both() {
        let text = "Switching.\nPROJECT_DEACTIVATE\nPROJECT_ACTIVATE: new-proj\nEnjoy!";
        let result = strip_project_markers(text);
        assert_eq!(result, "Switching.\nEnjoy!");
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
        assert!(msg.contains("honor"));
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
        assert!(nudge.contains("think about this"));
        assert!(still.contains("Still on it"));
    }

    #[test]
    fn test_status_messages_spanish() {
        let (nudge, still) = status_messages("Spanish");
        assert!(nudge.contains("pensar"));
        assert!(still.contains("ello"));
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
            content.contains("quietly confident"),
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

    // --- Workspace image snapshot tests ---

    #[test]
    fn test_snapshot_workspace_images_finds_images() {
        let dir = std::env::temp_dir().join("omega_test_snap_images");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("screenshot.png"), b"fake png").unwrap();
        std::fs::write(dir.join("photo.jpg"), b"fake jpg").unwrap();
        std::fs::write(dir.join("readme.txt"), b"not an image").unwrap();

        let result = snapshot_workspace_images(&dir);
        assert_eq!(result.len(), 2);
        assert!(result.contains_key(&dir.join("screenshot.png")));
        assert!(result.contains_key(&dir.join("photo.jpg")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_snapshot_workspace_images_empty_dir() {
        let dir = std::env::temp_dir().join("omega_test_snap_empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let result = snapshot_workspace_images(&dir);
        assert!(result.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_snapshot_workspace_images_nonexistent_dir() {
        let dir = std::env::temp_dir().join("omega_test_snap_nonexistent");
        let _ = std::fs::remove_dir_all(&dir);

        let result = snapshot_workspace_images(&dir);
        assert!(result.is_empty());
    }

    #[test]
    fn test_ensure_inbox_dir() {
        let tmp = std::env::temp_dir().join("omega_test_inbox_dir");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        // ensure_inbox_dir expects a data_dir and creates workspace/inbox/ under it.
        let inbox = ensure_inbox_dir(tmp.to_str().unwrap());
        assert!(inbox.exists());
        assert!(inbox.is_dir());
        assert!(inbox.ends_with("workspace/inbox"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_save_and_cleanup_inbox_images() {
        use omega_core::message::{Attachment, AttachmentType};

        let tmp = std::env::temp_dir().join("omega_test_save_inbox");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let attachments = vec![Attachment {
            file_type: AttachmentType::Image,
            url: None,
            data: Some(b"fake image data".to_vec()),
            filename: Some("test_photo.jpg".to_string()),
        }];

        let paths = save_attachments_to_inbox(&tmp, &attachments);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].exists());
        assert_eq!(std::fs::read(&paths[0]).unwrap(), b"fake image data");

        cleanup_inbox_images(&paths);
        assert!(!paths[0].exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_save_attachments_skips_non_images() {
        use omega_core::message::{Attachment, AttachmentType};

        let tmp = std::env::temp_dir().join("omega_test_skip_non_img");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let attachments = vec![
            Attachment {
                file_type: AttachmentType::Document,
                url: None,
                data: Some(b"some doc".to_vec()),
                filename: Some("doc.pdf".to_string()),
            },
            Attachment {
                file_type: AttachmentType::Audio,
                url: None,
                data: Some(b"some audio".to_vec()),
                filename: Some("audio.mp3".to_string()),
            },
        ];

        let paths = save_attachments_to_inbox(&tmp, &attachments);
        assert!(paths.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_snapshot_workspace_images_all_extensions() {
        let dir = std::env::temp_dir().join("omega_test_snap_all_ext");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for ext in IMAGE_EXTENSIONS {
            std::fs::write(dir.join(format!("test.{ext}")), b"fake").unwrap();
        }

        let result = snapshot_workspace_images(&dir);
        assert_eq!(result.len(), IMAGE_EXTENSIONS.len());

        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Classification & planning tests ---

    #[test]
    fn test_parse_plan_response_direct() {
        assert!(parse_plan_response("DIRECT").is_none());
        assert!(parse_plan_response("  DIRECT  ").is_none());
        assert!(parse_plan_response("direct").is_none());
    }

    #[test]
    fn test_parse_plan_response_numbered_list() {
        let text = "1. Set up the database schema\n\
                    2. Create the API endpoint\n\
                    3. Write integration tests";
        let steps = parse_plan_response(text).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], "Set up the database schema");
        assert_eq!(steps[1], "Create the API endpoint");
        assert_eq!(steps[2], "Write integration tests");
    }

    #[test]
    fn test_parse_plan_response_single_step() {
        let text = "1. Just do the thing";
        assert!(parse_plan_response(text).is_none());
    }

    #[test]
    fn test_parse_plan_response_with_preamble() {
        let text = "Here are the steps:\n\
                    1. First step\n\
                    2. Second step\n\
                    3. Third step";
        let steps = parse_plan_response(text).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], "First step");
    }

    // --- Limitation marker tests ---

    #[test]
    fn test_extract_limitation_marker() {
        let text =
            "I noticed an issue.\nLIMITATION: No email | Cannot send emails | Add SMTP provider";
        let result = extract_limitation_marker(text);
        assert_eq!(
            result,
            Some("LIMITATION: No email | Cannot send emails | Add SMTP provider".to_string())
        );
    }

    #[test]
    fn test_extract_limitation_marker_none() {
        let text = "Everything is working fine.";
        assert!(extract_limitation_marker(text).is_none());
    }

    #[test]
    fn test_parse_limitation_line() {
        let line = "LIMITATION: No email | Cannot send emails | Add SMTP provider";
        let result = parse_limitation_line(line).unwrap();
        assert_eq!(result.0, "No email");
        assert_eq!(result.1, "Cannot send emails");
        assert_eq!(result.2, "Add SMTP provider");
    }

    #[test]
    fn test_parse_limitation_line_invalid() {
        assert!(parse_limitation_line("LIMITATION: only one part").is_none());
        assert!(parse_limitation_line("not a limitation line").is_none());
        assert!(parse_limitation_line("LIMITATION:  | desc | plan").is_none());
    }

    #[test]
    fn test_strip_limitation_markers() {
        let text =
            "I found a gap.\nLIMITATION: No email | Cannot send | Add SMTP\nHope this helps.";
        let result = strip_limitation_markers(text);
        assert_eq!(result, "I found a gap.\nHope this helps.");
    }

    #[test]
    fn test_strip_limitation_markers_multiple() {
        let text = "Response.\nLIMITATION: A | B | C\nMore text.\nLIMITATION: D | E | F\nEnd.";
        let result = strip_limitation_markers(text);
        assert_eq!(result, "Response.\nMore text.\nEnd.");
    }

    // --- SCHEDULE_ACTION marker tests ---

    #[test]
    fn test_extract_schedule_action_marker() {
        let text =
            "I'll handle that.\nSCHEDULE_ACTION: Check BTC price | 2026-02-18T14:00:00 | daily";
        let result = extract_schedule_action_marker(text);
        assert_eq!(
            result,
            Some("SCHEDULE_ACTION: Check BTC price | 2026-02-18T14:00:00 | daily".to_string())
        );
    }

    #[test]
    fn test_extract_schedule_action_marker_none() {
        let text = "No action scheduled here.";
        assert!(extract_schedule_action_marker(text).is_none());
    }

    #[test]
    fn test_parse_schedule_action_line() {
        let line = "SCHEDULE_ACTION: Check BTC price | 2026-02-18T14:00:00 | daily";
        let result = parse_schedule_action_line(line).unwrap();
        assert_eq!(result.0, "Check BTC price");
        assert_eq!(result.1, "2026-02-18T14:00:00");
        assert_eq!(result.2, "daily");
    }

    #[test]
    fn test_parse_schedule_action_line_once() {
        let line = "SCHEDULE_ACTION: Run scraper | 2026-02-18T22:00:00 | once";
        let result = parse_schedule_action_line(line).unwrap();
        assert_eq!(result.0, "Run scraper");
        assert_eq!(result.2, "once");
    }

    #[test]
    fn test_parse_schedule_action_line_invalid() {
        assert!(parse_schedule_action_line("SCHEDULE_ACTION: missing parts").is_none());
        assert!(parse_schedule_action_line("not an action line").is_none());
        assert!(parse_schedule_action_line("SCHEDULE_ACTION:  | time | once").is_none());
    }

    #[test]
    fn test_strip_schedule_action_markers() {
        let text = "I'll do that.\nSCHEDULE_ACTION: Check BTC | 2026-02-18T14:00:00 | daily\nDone.";
        let result = strip_schedule_action_markers(text);
        assert_eq!(result, "I'll do that.\nDone.");
    }

    #[test]
    fn test_strip_schedule_action_preserves_schedule() {
        let text = "Response.\nSCHEDULE: Remind me | 2026-02-18T09:00:00 | once\nSCHEDULE_ACTION: Check prices | 2026-02-18T14:00:00 | daily\nEnd.";
        let result = strip_schedule_action_markers(text);
        assert!(
            result.contains("SCHEDULE: Remind me"),
            "should keep SCHEDULE lines"
        );
        assert!(
            !result.contains("SCHEDULE_ACTION:"),
            "should strip SCHEDULE_ACTION lines"
        );
    }

    #[test]
    fn test_classification_context_full() {
        let history = vec![
            ContextEntry {
                role: "user".into(),
                content: "Check BTC price".into(),
            },
            ContextEntry {
                role: "assistant".into(),
                content: "BTC is at $45,000".into(),
            },
            ContextEntry {
                role: "user".into(),
                content: "Set up a trailing stop".into(),
            },
        ];
        let result = build_classification_context(
            Some("trader"),
            &history,
            &["claude-code", "playwright-mcp"],
        );
        assert!(result.contains("Active project: trader"));
        assert!(result.contains("Recent conversation:"));
        assert!(result.contains("User: Check BTC price"));
        assert!(result.contains("Assistant: BTC is at $45,000"));
        assert!(result.contains("User: Set up a trailing stop"));
        assert!(result.contains("Available skills: claude-code, playwright-mcp"));
    }

    #[test]
    fn test_classification_context_empty() {
        let result = build_classification_context(None, &[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_classification_context_truncation() {
        let long_msg = "a".repeat(120);
        let history = vec![ContextEntry {
            role: "user".into(),
            content: long_msg,
        }];
        let result = build_classification_context(None, &history, &[]);
        assert!(result.contains("..."));
        // 80 chars + "..." = line should be truncated
        assert!(!result.contains(&"a".repeat(120)));
        assert!(result.contains(&"a".repeat(80)));
    }

    #[test]
    fn test_classification_context_partial() {
        let result = build_classification_context(Some("trader"), &[], &[]);
        assert!(result.contains("Active project: trader"));
        assert!(!result.contains("Recent conversation:"));
        assert!(!result.contains("Available skills:"));
    }

    // --- SELF_HEAL marker tests ---

    #[test]
    fn test_extract_self_heal_marker() {
        let text = "Something is wrong.\nSELF_HEAL: Build pipeline broken\nLet me fix it.";
        let result = extract_self_heal_marker(text);
        assert_eq!(result, Some("SELF_HEAL: Build pipeline broken".to_string()));
    }

    #[test]
    fn test_extract_self_heal_marker_none() {
        let text = "Everything is working fine.";
        assert!(extract_self_heal_marker(text).is_none());
    }

    #[test]
    fn test_parse_self_heal_line() {
        let line = "SELF_HEAL: Build pipeline broken";
        let result = parse_self_heal_line(line).unwrap();
        assert_eq!(result, "Build pipeline broken");
    }

    #[test]
    fn test_parse_self_heal_line_empty() {
        assert!(parse_self_heal_line("SELF_HEAL:").is_none());
        assert!(parse_self_heal_line("SELF_HEAL:   ").is_none());
        assert!(parse_self_heal_line("not a self-heal line").is_none());
    }

    #[test]
    fn test_has_self_heal_resolved_marker() {
        let text = "Fixed the issue.\nSELF_HEAL_RESOLVED\nAll good now.";
        assert!(has_self_heal_resolved_marker(text));
    }

    #[test]
    fn test_has_self_heal_resolved_marker_none() {
        let text = "No resolved marker here.";
        assert!(!has_self_heal_resolved_marker(text));
    }

    #[test]
    fn test_strip_self_heal_markers() {
        let text = "Detected issue.\nSELF_HEAL: Build broken\nFixing now.";
        let result = strip_self_heal_markers(text);
        assert_eq!(result, "Detected issue.\nFixing now.");
    }

    #[test]
    fn test_strip_self_heal_markers_resolved() {
        let text = "Fixed it.\nSELF_HEAL_RESOLVED\nAll done.";
        let result = strip_self_heal_markers(text);
        assert_eq!(result, "Fixed it.\nAll done.");
    }

    #[test]
    fn test_strip_self_heal_markers_both() {
        let text = "Start.\nSELF_HEAL: Bug found\nMiddle.\nSELF_HEAL_RESOLVED\nEnd.";
        let result = strip_self_heal_markers(text);
        assert_eq!(result, "Start.\nMiddle.\nEnd.");
    }

    #[test]
    fn test_self_healing_state_serde_roundtrip() {
        let state = SelfHealingState {
            anomaly: "Build broken".to_string(),
            iteration: 3,
            max_iterations: 10,
            started_at: "2026-02-18T12:00:00Z".to_string(),
            attempts: vec![
                "1: Tried restarting service".to_string(),
                "2: Fixed import path".to_string(),
                "3: Rebuilt binary".to_string(),
            ],
        };
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SelfHealingState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.anomaly, "Build broken");
        assert_eq!(deserialized.iteration, 3);
        assert_eq!(deserialized.max_iterations, 10);
        assert_eq!(deserialized.attempts.len(), 3);
    }
}
