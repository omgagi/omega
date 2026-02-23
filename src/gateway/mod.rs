//! Gateway — the main event loop connecting channels, memory, and providers.
//!
//! Includes: auth enforcement, prompt sanitization, audit logging,
//! background conversation summarization, and graceful shutdown.

mod auth;
mod heartbeat;
mod heartbeat_helpers;
mod keywords;
mod pipeline;
mod process_markers;
mod routing;
mod scheduler;
mod scheduler_action;
mod summarizer;

use crate::markers::*;
use omega_core::{
    config::{
        shellexpand, ApiConfig, AuthConfig, ChannelConfig, HeartbeatConfig, Prompts,
        SchedulerConfig,
    },
    message::{IncomingMessage, MessageMetadata, OutgoingMessage},
    traits::{Channel, Provider},
};
use omega_memory::{audit::AuditLogger, Store};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info, warn};

/// The central gateway that routes messages between channels and providers.
pub struct Gateway {
    pub(super) provider: Arc<dyn Provider>,
    pub(super) channels: HashMap<String, Arc<dyn Channel>>,
    pub(super) memory: Store,
    pub(super) audit: AuditLogger,
    pub(super) auth_config: AuthConfig,
    pub(super) channel_config: ChannelConfig,
    pub(super) heartbeat_config: HeartbeatConfig,
    pub(super) scheduler_config: SchedulerConfig,
    pub(super) api_config: ApiConfig,
    pub(super) prompts: Prompts,
    pub(super) data_dir: String,
    pub(super) skills: Vec<omega_skills::Skill>,
    pub(super) uptime: Instant,
    /// Fast model for classification and direct responses (Sonnet).
    pub(super) model_fast: String,
    /// Complex model for multi-step autonomous execution (Opus).
    pub(super) model_complex: String,
    /// Tracks senders with active provider calls. New messages are buffered here.
    pub(super) active_senders: Mutex<HashMap<String, Vec<IncomingMessage>>>,
    /// Shared heartbeat interval (minutes) — updated at runtime via `HEARTBEAT_INTERVAL:` marker.
    pub(super) heartbeat_interval: Arc<AtomicU64>,
    /// Active CLI sessions per sender (channel:sender_id → session_id).
    /// Used for session-based prompt persistence with Claude Code CLI.
    pub(super) cli_sessions: Arc<std::sync::Mutex<HashMap<String, String>>>,
    /// Path to config.toml — used for persisting runtime changes (e.g. heartbeat interval).
    pub(super) config_path: String,
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
        api_config: ApiConfig,
        prompts: Prompts,
        data_dir: String,
        skills: Vec<omega_skills::Skill>,
        model_fast: String,
        model_complex: String,
        config_path: String,
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
            api_config,
            prompts,
            data_dir,
            skills,
            uptime: Instant::now(),
            model_fast,
            model_complex,
            active_senders: Mutex::new(HashMap::new()),
            heartbeat_interval,
            cli_sessions: Arc::new(std::sync::Mutex::new(HashMap::new())),
            config_path,
        }
    }

    /// Run the main event loop.
    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        info!(
            "Omega gateway running | provider: {} | channels: {} | auth: {}",
            self.provider.name(),
            self.channels.keys().cloned().collect::<Vec<_>>().join(", "),
            if self.auth_config.enabled {
                "enforced"
            } else {
                "disabled"
            },
        );

        // Purge orphaned inbox files from previous runs.
        purge_inbox(&self.data_dir);

        // Ensure workspace CLAUDE.md exists (Claude Code provider only).
        if self.provider.name() == "claude-code" {
            let workspace = PathBuf::from(shellexpand(&self.data_dir)).join("workspace");
            let data_dir = PathBuf::from(shellexpand(&self.data_dir));
            tokio::spawn(async move {
                crate::claudemd::ensure_claudemd(&workspace, &data_dir).await;
            });
        }

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
        let bg_sessions = self.cli_sessions.clone();
        let bg_handle = tokio::spawn(async move {
            Self::background_summarizer(bg_store, bg_provider, bg_summarize, bg_facts, bg_sessions)
                .await;
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
            let sched_hb_interval = self.heartbeat_interval.clone();
            let sched_audit = AuditLogger::new(self.memory.pool().clone());
            let sched_provider_name = self.provider.name().to_string();
            let sched_data_dir = self.data_dir.clone();
            Some(tokio::spawn(async move {
                Self::scheduler_loop(
                    sched_store,
                    sched_channels,
                    poll_secs,
                    sched_provider,
                    sched_skills,
                    sched_prompts,
                    sched_model,
                    sched_hb_interval,
                    sched_audit,
                    sched_provider_name,
                    sched_data_dir,
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
            let hb_prompts = self.prompts.clone();
            let hb_memory = self.memory.clone();
            let hb_interval = self.heartbeat_interval.clone();
            let hb_model = self.model_complex.clone();
            let hb_model_fast = self.model_fast.clone();
            let hb_skills = self.skills.clone();
            let hb_audit = AuditLogger::new(self.memory.pool().clone());
            let hb_provider_name = self.provider.name().to_string();
            let hb_data_dir = self.data_dir.clone();
            Some(tokio::spawn(async move {
                Self::heartbeat_loop(
                    hb_provider,
                    hb_channels,
                    hb_config,
                    hb_prompts,
                    hb_memory,
                    hb_interval,
                    hb_model,
                    hb_model_fast,
                    hb_skills,
                    hb_audit,
                    hb_provider_name,
                    hb_data_dir,
                )
                .await;
            }))
        } else {
            None
        };

        // Spawn CLAUDE.md maintenance loop (Claude Code provider only, 24h interval).
        let claudemd_handle = if self.provider.name() == "claude-code" {
            let ws = PathBuf::from(shellexpand(&self.data_dir)).join("workspace");
            let dd = PathBuf::from(shellexpand(&self.data_dir));
            Some(tokio::spawn(async move {
                crate::claudemd::claudemd_loop(ws, dd, 24).await;
            }))
        } else {
            None
        };

        // Spawn HTTP API server.
        let api_handle = if self.api_config.enabled {
            let api_cfg = self.api_config.clone();
            let api_channels = self.channels.clone();
            let api_uptime = self.uptime;
            Some(tokio::spawn(async move {
                crate::api::serve(api_cfg, api_channels, api_uptime).await;
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
        self.shutdown(
            &bg_handle,
            &sched_handle,
            &hb_handle,
            &claudemd_handle,
            &api_handle,
        )
        .await;
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

    /// Graceful shutdown: summarize active conversations, stop channels.
    async fn shutdown(
        &self,
        bg_handle: &tokio::task::JoinHandle<()>,
        sched_handle: &Option<tokio::task::JoinHandle<()>>,
        hb_handle: &Option<tokio::task::JoinHandle<()>>,
        claudemd_handle: &Option<tokio::task::JoinHandle<()>>,
        api_handle: &Option<tokio::task::JoinHandle<()>>,
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
        if let Some(h) = claudemd_handle {
            h.abort();
        }
        if let Some(h) = api_handle {
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
    use super::keywords::*;
    use super::*;
    use omega_core::context::ContextNeeds;

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
        let content = include_str!("../../prompts/SYSTEM_PROMPT.md");
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
        let content = include_str!("../../prompts/SYSTEM_PROMPT.md");
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

    // --- Prompt injection integration tests ---

    /// Simulate the gateway's keyword detection + prompt assembly logic.
    fn assemble_test_prompt(
        prompts: &Prompts,
        msg: &str,
        _has_active_project: bool,
    ) -> (String, ContextNeeds) {
        let msg_lower = msg.to_lowercase();
        let needs_scheduling = kw_match(&msg_lower, SCHEDULING_KW);
        let needs_recall = kw_match(&msg_lower, RECALL_KW);
        let needs_tasks = needs_scheduling || kw_match(&msg_lower, TASKS_KW);
        let needs_projects = kw_match(&msg_lower, PROJECTS_KW);
        let needs_meta = kw_match(&msg_lower, META_KW);
        let needs_profile =
            kw_match(&msg_lower, PROFILE_KW) || needs_scheduling || needs_recall || needs_tasks;
        let needs_summaries = needs_recall;
        let needs_outcomes = kw_match(&msg_lower, OUTCOMES_KW);

        let mut prompt = format!(
            "{}\n\n{}\n\n{}",
            prompts.identity, prompts.soul, prompts.system
        );

        if needs_scheduling {
            prompt.push_str("\n\n");
            prompt.push_str(&prompts.scheduling);
        }
        if needs_projects {
            prompt.push_str("\n\n");
            prompt.push_str(&prompts.projects_rules);
        }
        if needs_meta {
            prompt.push_str("\n\n");
            prompt.push_str(&prompts.meta);
        }

        let context_needs = ContextNeeds {
            recall: needs_recall,
            pending_tasks: needs_tasks,
            profile: needs_profile,
            summaries: needs_summaries,
            outcomes: needs_outcomes,
        };

        (prompt, context_needs)
    }

    #[test]
    fn test_prompt_injection_simple_greeting() {
        let prompts = Prompts::default();
        let (prompt, needs) = assemble_test_prompt(&prompts, "good morning", false);

        // Core sections always present
        assert!(prompt.contains("OMEGA"));
        assert!(prompt.contains("precise, warm"));

        // Conditional sections NOT injected
        assert!(
            !prompt.contains("scheduler"),
            "scheduling should not be in greeting prompt"
        );
        assert!(
            !prompt.contains("Projects path"),
            "projects should not be in greeting prompt"
        );
        assert!(
            !prompt.contains("SKILL_IMPROVE"),
            "meta should not be in greeting prompt"
        );

        // ContextNeeds: skip both expensive queries
        assert!(!needs.recall);
        assert!(!needs.pending_tasks);
    }

    #[test]
    fn test_prompt_injection_scheduling_keyword() {
        let prompts = Prompts::default();
        let (prompt, needs) =
            assemble_test_prompt(&prompts, "remind me to call mom tomorrow at 5pm", false);

        // Scheduling section injected
        assert!(
            prompt.contains("scheduler"),
            "scheduling section should be injected for 'remind'"
        );

        // Other conditional sections NOT injected
        assert!(!prompt.contains("Projects path"));
        assert!(!prompt.contains("SKILL_IMPROVE"));

        // ContextNeeds: scheduling implies pending_tasks
        assert!(!needs.recall);
        assert!(needs.pending_tasks, "scheduling should imply pending_tasks");
    }

    #[test]
    fn test_prompt_injection_recall_keyword() {
        let prompts = Prompts::default();
        let (prompt, needs) = assemble_test_prompt(
            &prompts,
            "do you remember what we discussed yesterday?",
            false,
        );

        // No conditional prompt sections injected (recall only affects ContextNeeds)
        assert!(!prompt.contains("scheduler"));
        assert!(!prompt.contains("Projects path"));
        assert!(!prompt.contains("SKILL_IMPROVE"));

        // ContextNeeds: recall enabled
        assert!(
            needs.recall,
            "recall should be enabled for 'remember' + 'yesterday'"
        );
        assert!(!needs.pending_tasks);
    }

    #[test]
    fn test_prompt_injection_tasks_keyword() {
        let prompts = Prompts::default();
        let (prompt, needs) = assemble_test_prompt(&prompts, "show me my pending tasks", false);

        // No conditional prompt sections (tasks only affects ContextNeeds)
        assert!(!prompt.contains("scheduler"));
        assert!(!prompt.contains("Projects path"));
        assert!(!prompt.contains("SKILL_IMPROVE"));

        // ContextNeeds: pending_tasks enabled
        assert!(!needs.recall);
        assert!(
            needs.pending_tasks,
            "pending_tasks should be enabled for 'task' + 'pending'"
        );
    }

    #[test]
    fn test_prompt_injection_scheduling_implies_tasks() {
        let prompts = Prompts::default();
        let (_, needs) = assemble_test_prompt(&prompts, "schedule a daily alarm for 7am", false);

        // Scheduling always implies pending_tasks (need task awareness)
        assert!(
            needs.pending_tasks,
            "scheduling keyword should always enable pending_tasks"
        );
    }

    #[test]
    fn test_prompt_injection_project_keyword() {
        let prompts = Prompts::default();
        let (prompt, needs) = assemble_test_prompt(&prompts, "activate project trader", false);

        // Projects section injected
        assert!(
            prompt.contains("Projects path"),
            "projects section should be injected for 'project' + 'activate'"
        );

        // Others not injected
        assert!(!prompt.contains("scheduler"));
        assert!(!prompt.contains("SKILL_IMPROVE"));

        // ContextNeeds: neither recall nor tasks
        assert!(!needs.recall);
        assert!(!needs.pending_tasks);
    }

    #[test]
    fn test_prompt_injection_active_project_no_keyword() {
        let prompts = Prompts::default();
        let (prompt, _) = assemble_test_prompt(&prompts, "how is the weather today", true);

        // Projects section NOT injected without keyword — keyword-gated since contextual injection
        assert!(
            !prompt.contains("Projects path"),
            "projects section should NOT be injected without project keywords"
        );

        // Others not injected
        assert!(!prompt.contains("scheduler"));
        assert!(!prompt.contains("SKILL_IMPROVE"));
    }

    #[test]
    fn test_prompt_injection_meta_keyword() {
        let prompts = Prompts::default();
        let (prompt, needs) = assemble_test_prompt(&prompts, "improve this skill please", false);

        // Meta section injected
        assert!(
            prompt.contains("SKILL_IMPROVE"),
            "meta section should be injected for 'improve' + 'skill'"
        );

        // Others not injected
        assert!(!prompt.contains("scheduler"));
        assert!(!prompt.contains("Projects path"));

        // ContextNeeds: neither recall nor tasks
        assert!(!needs.recall);
        assert!(!needs.pending_tasks);
    }

    #[test]
    fn test_prompt_injection_combined_scheduling_and_meta() {
        let prompts = Prompts::default();
        let (prompt, needs) =
            assemble_test_prompt(&prompts, "remind me to improve my skill tomorrow", false);

        // Both scheduling and meta injected
        assert!(
            prompt.contains("scheduler"),
            "scheduling should be injected"
        );
        assert!(prompt.contains("SKILL_IMPROVE"), "meta should be injected");

        // Projects NOT injected
        assert!(!prompt.contains("Projects path"));

        // ContextNeeds: pending_tasks from scheduling, no recall
        assert!(!needs.recall);
        assert!(needs.pending_tasks);
    }

    #[test]
    fn test_prompt_injection_all_sections() {
        let prompts = Prompts::default();
        let (prompt, needs) = assemble_test_prompt(
            &prompts,
            "remember to schedule a project skill improvement for tomorrow",
            true,
        );

        // All conditional sections injected
        assert!(
            prompt.contains("scheduler"),
            "scheduling should be injected"
        );
        assert!(
            prompt.contains("Projects path"),
            "projects should be injected"
        );
        assert!(prompt.contains("SKILL_IMPROVE"), "meta should be injected");

        // ContextNeeds: both enabled
        assert!(needs.recall, "recall should be enabled for 'remember'");
        assert!(
            needs.pending_tasks,
            "pending_tasks should be enabled for scheduling"
        );
    }

    #[test]
    fn test_prompt_injection_token_reduction() {
        let prompts = Prompts::default();
        let (lean_prompt, _) = assemble_test_prompt(&prompts, "hello", false);
        let (full_prompt, _) = assemble_test_prompt(
            &prompts,
            "remind me about the project skill improvement tomorrow",
            true,
        );

        // Full prompt should be significantly larger than lean prompt
        assert!(
            full_prompt.len() > lean_prompt.len(),
            "full prompt ({}) should be larger than lean prompt ({})",
            full_prompt.len(),
            lean_prompt.len()
        );

        // Difference should be at least the size of the conditional sections
        let conditional_size =
            prompts.scheduling.len() + prompts.projects_rules.len() + prompts.meta.len();
        let diff = full_prompt.len() - lean_prompt.len();
        assert!(
            diff >= conditional_size,
            "prompt size difference ({diff}) should be >= conditional sections ({conditional_size})"
        );
    }

    #[test]
    fn test_prompt_injection_multilingual_spanish() {
        let prompts = Prompts::default();
        let (prompt, needs) =
            assemble_test_prompt(&prompts, "recuérdame agendar una cita mañana", false);

        // Spanish scheduling keywords should trigger scheduling
        assert!(
            prompt.contains("scheduler"),
            "scheduling should be injected for Spanish keywords"
        );
        assert!(
            needs.pending_tasks,
            "pending_tasks should be enabled for Spanish scheduling"
        );
    }

    #[test]
    fn test_prompt_injection_multilingual_portuguese() {
        let prompts = Prompts::default();
        let (prompt, needs) =
            assemble_test_prompt(&prompts, "lembre-me de verificar o projeto amanhã", false);

        // Portuguese keywords trigger scheduling + recall + projects
        assert!(
            prompt.contains("scheduler"),
            "scheduling should be injected for Portuguese 'lembr'"
        );
        assert!(
            prompt.contains("Projects path"),
            "projects should be injected for 'projeto'"
        );
        assert!(
            needs.recall,
            "recall should be enabled for Portuguese 'lembr'"
        );
        assert!(needs.pending_tasks);
    }

    #[test]
    fn test_bundled_prompt_has_conditional_sections() {
        let content = include_str!("../../prompts/SYSTEM_PROMPT.md");
        assert!(
            content.contains("## Scheduling"),
            "bundled prompt should have ## Scheduling section"
        );
        assert!(
            content.contains("## Projects"),
            "bundled prompt should have ## Projects section"
        );
        assert!(
            content.contains("## Meta"),
            "bundled prompt should have ## Meta section"
        );
    }
}
