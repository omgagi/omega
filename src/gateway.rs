//! Gateway — the main event loop connecting channels, memory, and providers.
//!
//! Includes: auth enforcement, prompt sanitization, audit logging,
//! background conversation summarization, and graceful shutdown.

use crate::commands;
use crate::markers::*;
use omega_channels::whatsapp;
use omega_core::{
    config::{shellexpand, AuthConfig, ChannelConfig, HeartbeatConfig, Prompts, SchedulerConfig},
    context::{Context, ContextEntry},
    message::{IncomingMessage, MessageMetadata, OutgoingMessage},
    sanitize,
    traits::{Channel, Provider},
};
use omega_memory::{
    audit::{AuditEntry, AuditLogger, AuditStatus},
    detect_language, Store,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, error, info, warn};

/// Validate a fact key/value before storing. Rejects junk patterns.
/// System-managed fact keys that only bot commands may write.
const SYSTEM_FACT_KEYS: &[&str] = &[
    "welcomed",
    "preferred_language",
    "active_project",
    "personality",
    "onboarding_stage",
];

fn is_valid_fact(key: &str, value: &str) -> bool {
    // Reject system-managed keys — only bot commands may set these.
    if SYSTEM_FACT_KEYS.contains(&key) {
        return false;
    }

    // Length limits.
    if key.len() > 50 || value.len() > 200 {
        return false;
    }

    // Key must not be numeric-only or start with a digit.
    if key.chars().next().is_none_or(|c| c.is_ascii_digit()) {
        return false;
    }

    // Value must not start with '$' (price patterns).
    if value.starts_with('$') {
        return false;
    }

    // Reject pipe-delimited table rows.
    if value.contains('|') && value.matches('|').count() >= 2 {
        return false;
    }

    // Reject values that look like prices (e.g., "0.00123", "45,678.90").
    let price_like = value
        .trim()
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == ',' || c == '-');
    if price_like && !value.trim().is_empty() {
        return false;
    }

    true
}

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
    /// Shared heartbeat interval (minutes) — updated at runtime via `HEARTBEAT_INTERVAL:` marker.
    heartbeat_interval: Arc<AtomicU64>,
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
        let heartbeat_interval = Arc::new(AtomicU64::new(heartbeat_config.interval_minutes));
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
            heartbeat_interval,
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
            let sched_hb_interval = self.heartbeat_interval.clone();
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
                    sched_hb_interval,
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
            let hb_interval = self.heartbeat_interval.clone();
            Some(tokio::spawn(async move {
                Self::heartbeat_loop(
                    hb_provider,
                    hb_channels,
                    hb_config,
                    hb_prompt_checklist,
                    hb_memory,
                    hb_interval,
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
        heartbeat_interval: Arc<AtomicU64>,
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
                                        for action in &hb_actions {
                                            if let HeartbeatAction::SetInterval(mins) = action {
                                                heartbeat_interval.store(*mins, Ordering::Relaxed);
                                                info!(
                                                    "heartbeat: interval changed to {mins} minutes (via scheduler)"
                                                );
                                            }
                                        }
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
                                        if let Some((heal_desc, heal_verif)) =
                                            parse_self_heal_line(&heal_line)
                                        {
                                            let mut state = read_self_healing_state()
                                                .unwrap_or_else(|| SelfHealingState {
                                                    anomaly: heal_desc.clone(),
                                                    verification: heal_verif.clone(),
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
                                                let next_desc = self_heal_follow_up(
                                                    &state.anomaly,
                                                    &state.verification,
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
        interval: Arc<AtomicU64>,
    ) {
        loop {
            let mins = interval.load(Ordering::Relaxed);
            tokio::time::sleep(std::time::Duration::from_secs(mins * 60)).await;

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
                                if is_valid_fact(&key, &value) {
                                    let _ = store.store_fact(&sender_id, &key, &value).await;
                                } else {
                                    debug!("rejected invalid fact: {key}: {value}");
                                }
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

    /// Handle /forget: find active conversation, summarize + extract facts, close.
    /// Falls back to a plain close if no active conversation or summarization fails.
    async fn handle_forget(&self, channel: &str, sender_id: &str) -> String {
        // Find the active conversation for this sender.
        let conv: Option<(String,)> = sqlx::query_as(
            "SELECT id FROM conversations \
             WHERE channel = ? AND sender_id = ? AND status = 'active' \
             ORDER BY last_activity DESC LIMIT 1",
        )
        .bind(channel)
        .bind(sender_id)
        .fetch_optional(self.memory.pool())
        .await
        .ok()
        .flatten();

        match conv {
            Some((conversation_id,)) => {
                // Summarize (extracts facts + closes the conversation).
                if let Err(e) = Self::summarize_conversation(
                    &self.memory,
                    &self.provider,
                    &conversation_id,
                    &self.prompts.summarize,
                    &self.prompts.facts,
                )
                .await
                {
                    warn!("summarization during /forget failed: {e}, closing directly");
                    let _ = self
                        .memory
                        .close_current_conversation(channel, sender_id)
                        .await;
                }
                "Conversation saved and cleared. Starting fresh.".to_string()
            }
            None => "No active conversation to clear.".to_string(),
        }
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
            // Intercept /forget: summarize + extract facts before closing.
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

            // Model identity — so the AI knows which model it is running on.
            prompt.push_str(&format!(
                "\n\nYou are running on provider '{}', model '{}'.",
                self.provider.name(),
                self.model_fast
            ));

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

        // --- 5. PROCESS MARKERS ---
        let mut response = response;
        self.process_markers(&incoming, &mut response.text).await;

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
             Respond DIRECT if the request is:\n\
             - A simple question, greeting, or conversation\n\
             - One or more routine actions (reminders, scheduling, sending messages, storing \
             information, short lookups)\n\n\
             Respond with a numbered step list ONLY if the request requires genuinely complex \
             work that benefits from decomposition — such as multi-file code changes, deep \
             research and synthesis, building something new, or tasks where each step produces \
             context the next step needs.\n\n\
             When in doubt, prefer DIRECT.\n\n\
             {context_section}\
             Request: {message}"
        );

        let mut ctx = Context::new(&planning_prompt);
        ctx.max_turns = Some(1);
        ctx.allowed_tools = Some(vec![]); // No tools during classification.
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
                Some(mut step_resp) => {
                    // Process markers on each step result.
                    self.process_markers(incoming, &mut step_resp.text).await;

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

    /// Extract and process all markers from a provider response text.
    ///
    /// Handles: SCHEDULE, SCHEDULE_ACTION, PROJECT_ACTIVATE/DEACTIVATE,
    /// WHATSAPP_QR, LANG_SWITCH, HEARTBEAT_ADD/REMOVE, LIMITATION,
    /// SELF_HEAL, SELF_HEAL_RESOLVED. Strips processed markers from the text.
    async fn process_markers(&self, incoming: &IncomingMessage, text: &mut String) {
        // SCHEDULE
        if let Some(schedule_line) = extract_schedule_marker(text) {
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
                    Ok(id) => info!("scheduled task {id}: {desc} at {due_at}"),
                    Err(e) => error!("failed to create scheduled task: {e}"),
                }
            }
            *text = strip_schedule_marker(text);
        }

        // SCHEDULE_ACTION
        if let Some(sched_line) = extract_schedule_action_marker(text) {
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
                    Ok(id) => info!("scheduled action task {id}: {desc} at {due_at}"),
                    Err(e) => error!("failed to create action task: {e}"),
                }
            }
            *text = strip_schedule_action_markers(text);
        }

        // PROJECT_ACTIVATE / PROJECT_DEACTIVATE
        if let Some(project_name) = extract_project_activate(text) {
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
            *text = strip_project_markers(text);
        }
        if has_project_deactivate(text) {
            if let Err(e) = self
                .memory
                .delete_fact(&incoming.sender_id, "active_project")
                .await
            {
                error!("failed to deactivate project: {e}");
            } else {
                info!("project deactivated");
            }
            *text = strip_project_markers(text);
        }

        // WHATSAPP_QR
        if has_whatsapp_qr_marker(text) {
            *text = strip_whatsapp_qr_marker(text);
            self.handle_whatsapp_qr(incoming).await;
        }

        // LANG_SWITCH
        if let Some(lang) = extract_lang_switch(text) {
            if let Err(e) = self
                .memory
                .store_fact(&incoming.sender_id, "preferred_language", &lang)
                .await
            {
                error!("failed to store language preference: {e}");
            } else {
                info!("language switched to '{lang}' for {}", incoming.sender_id);
            }
            *text = strip_lang_switch(text);
        }

        // PERSONALITY
        if let Some(value) = extract_personality(text) {
            if value.eq_ignore_ascii_case("reset") {
                match self
                    .memory
                    .delete_fact(&incoming.sender_id, "personality")
                    .await
                {
                    Ok(_) => info!("personality reset for {}", incoming.sender_id),
                    Err(e) => error!("failed to reset personality: {e}"),
                }
            } else {
                match self
                    .memory
                    .store_fact(&incoming.sender_id, "personality", &value)
                    .await
                {
                    Ok(()) => info!("personality set to '{value}' for {}", incoming.sender_id),
                    Err(e) => error!("failed to store personality: {e}"),
                }
            }
            *text = strip_personality(text);
        }

        // FORGET_CONVERSATION
        if has_forget_marker(text) {
            match self
                .memory
                .close_current_conversation(&incoming.channel, &incoming.sender_id)
                .await
            {
                Ok(true) => info!("conversation cleared via marker for {}", incoming.sender_id),
                Ok(false) => {
                    info!("no active conversation to clear for {}", incoming.sender_id)
                }
                Err(e) => error!("failed to clear conversation via marker: {e}"),
            }
            *text = strip_forget_marker(text);
        }

        // CANCEL_TASK
        if let Some(id_prefix) = extract_cancel_task(text) {
            match self
                .memory
                .cancel_task(&id_prefix, &incoming.sender_id)
                .await
            {
                Ok(true) => info!("task cancelled via marker: {id_prefix}"),
                Ok(false) => warn!("no matching task for cancel marker: {id_prefix}"),
                Err(e) => error!("failed to cancel task via marker: {e}"),
            }
            *text = strip_cancel_task(text);
        }

        // PURGE_FACTS
        if has_purge_marker(text) {
            // Save system facts first.
            let preserved: Vec<(String, String)> =
                match self.memory.get_facts(&incoming.sender_id).await {
                    Ok(facts) => facts
                        .into_iter()
                        .filter(|(k, _)| SYSTEM_FACT_KEYS.contains(&k.as_str()))
                        .collect(),
                    Err(e) => {
                        error!("purge marker: failed to read facts: {e}");
                        Vec::new()
                    }
                };
            // Delete all facts.
            match self.memory.delete_facts(&incoming.sender_id, None).await {
                Ok(n) => {
                    // Restore system facts.
                    for (key, value) in &preserved {
                        let _ = self
                            .memory
                            .store_fact(&incoming.sender_id, key, value)
                            .await;
                    }
                    let purged = n as usize - preserved.len();
                    info!(
                        "purged {purged} facts via marker for {}",
                        incoming.sender_id
                    );
                }
                Err(e) => error!("purge marker: failed to delete facts: {e}"),
            }
            *text = strip_purge_marker(text);
        }

        // HEARTBEAT_ADD / HEARTBEAT_REMOVE / HEARTBEAT_INTERVAL
        let heartbeat_actions = extract_heartbeat_markers(text);
        if !heartbeat_actions.is_empty() {
            apply_heartbeat_changes(&heartbeat_actions);
            for action in &heartbeat_actions {
                match action {
                    HeartbeatAction::Add(item) => info!("heartbeat: added '{item}' to checklist"),
                    HeartbeatAction::Remove(item) => {
                        info!("heartbeat: removed '{item}' from checklist")
                    }
                    HeartbeatAction::SetInterval(mins) => {
                        self.heartbeat_interval.store(*mins, Ordering::Relaxed);
                        info!("heartbeat: interval changed to {mins} minutes");
                        // Notify owner via heartbeat channel.
                        if let Some(ch) = self.channels.get(&self.heartbeat_config.channel) {
                            let msg = OutgoingMessage {
                                text: format!("⏱️ Heartbeat interval updated to {mins} minutes."),
                                metadata: MessageMetadata::default(),
                                reply_target: Some(self.heartbeat_config.reply_target.clone()),
                            };
                            if let Err(e) = ch.send(msg).await {
                                warn!("heartbeat interval notify failed: {e}");
                            }
                        }
                    }
                }
            }
            *text = strip_heartbeat_markers(text);
        }

        // LIMITATION
        if let Some(limitation_line) = extract_limitation_marker(text) {
            if let Some((title, description, plan)) = parse_limitation_line(&limitation_line) {
                match self
                    .memory
                    .store_limitation(&title, &description, &plan)
                    .await
                {
                    Ok(true) => {
                        info!("limitation detected (new): {title}");
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
                        apply_heartbeat_changes(&[HeartbeatAction::Add(format!(
                            "CRITICAL: {title} — {description}"
                        ))]);
                    }
                    Ok(false) => info!("limitation detected (duplicate): {title}"),
                    Err(e) => error!("failed to store limitation: {e}"),
                }
            }
            *text = strip_limitation_markers(text);
        }

        // SELF_HEAL
        if let Some(heal_line) = extract_self_heal_marker(text) {
            if let Some((description, verification)) = parse_self_heal_line(&heal_line) {
                let mut state = read_self_healing_state().unwrap_or_else(|| SelfHealingState {
                    anomaly: description.clone(),
                    verification: verification.clone(),
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
                    if let Err(e) = write_self_healing_state(&state) {
                        error!("self-heal: failed to write state: {e}");
                    }
                    info!(
                        "self-heal: escalated after {} iterations: {}",
                        state.iteration, state.anomaly
                    );
                } else {
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
                    let due_at = (chrono::Utc::now() + chrono::Duration::minutes(2)).to_rfc3339();
                    let heal_desc = self_heal_follow_up(&state.anomaly, &state.verification);
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
                        Err(e) => error!("self-heal: failed to schedule verification: {e}"),
                    }
                }
            }
            *text = strip_self_heal_markers(text);
        }

        // SELF_HEAL_RESOLVED
        if has_self_heal_resolved_marker(text) {
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
                Err(e) => error!("self-heal: failed to delete state file: {e}"),
            }
            *text = strip_self_heal_markers(text);
        }

        // Safety net: strip any markers still remaining (catches inline markers
        // from small models that don't put them on their own line).
        *text = strip_all_remaining_markers(text);
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

#[cfg(test)]
mod tests {
    use super::*;

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
            content.contains("PERSON"),
            "bundled facts section should emphasize personal facts"
        );
    }

    // --- Fact validation tests ---

    #[test]
    fn test_is_valid_fact_accepts_good_facts() {
        assert!(is_valid_fact("name", "Juan"));
        assert!(is_valid_fact("occupation", "software engineer"));
        assert!(is_valid_fact("timezone", "Europe/Madrid"));
        assert!(is_valid_fact("interests", "trading, hiking, Rust"));
        assert!(is_valid_fact("communication_style", "direct and concise"));
    }

    #[test]
    fn test_is_valid_fact_rejects_numeric_keys() {
        assert!(!is_valid_fact("1", "some value"));
        assert!(!is_valid_fact("42", "another value"));
        assert!(!is_valid_fact("3. step three", "do something"));
    }

    #[test]
    fn test_is_valid_fact_rejects_price_values() {
        assert!(!is_valid_fact("target", "$150.00"));
        assert!(!is_valid_fact("price", "0.00123"));
        assert!(!is_valid_fact("level", "45,678.90"));
    }

    #[test]
    fn test_is_valid_fact_rejects_pipe_delimited() {
        assert!(!is_valid_fact("data", "BTC | 45000 | bullish"));
        assert!(!is_valid_fact("row", "col1 | col2 | col3"));
    }

    #[test]
    fn test_is_valid_fact_rejects_oversized() {
        let long_key = "a".repeat(51);
        assert!(!is_valid_fact(&long_key, "value"));
        let long_value = "b".repeat(201);
        assert!(!is_valid_fact("key", &long_value));
    }

    #[test]
    fn test_is_valid_fact_rejects_system_keys() {
        assert!(!is_valid_fact("welcomed", "true"));
        assert!(!is_valid_fact("preferred_language", "en"));
        assert!(!is_valid_fact("active_project", "trader"));
        assert!(!is_valid_fact("personality", "direct, results-oriented"));
    }
}
