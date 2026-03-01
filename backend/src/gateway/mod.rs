//! Gateway — the main event loop connecting channels, memory, and providers.
//!
//! Includes: auth enforcement, prompt sanitization, audit logging,
//! background conversation summarization, and graceful shutdown.

mod auth;
mod builds;
mod builds_agents;
mod builds_i18n;
mod builds_loop;
mod builds_parse;
mod builds_topology;
mod heartbeat;
mod heartbeat_helpers;
mod keywords;
mod keywords_data;
mod pipeline;
mod pipeline_builds;
mod process_markers;
mod prompt_builder;
mod routing;
mod scheduler;
mod scheduler_action;
mod setup;
mod setup_response;
mod shared_markers;
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
use tokio::sync::{mpsc, Mutex, Notify};
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
    /// Wakes the heartbeat loop when `HEARTBEAT_INTERVAL:` changes so it re-sleeps with the new value.
    pub(super) heartbeat_notify: Arc<Notify>,
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
        let heartbeat_notify = Arc::new(Notify::new());
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
            heartbeat_notify,
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

        // Spawn HTTP API server (BEFORE drop(tx) so we can clone the sender).
        let api_handle = if self.api_config.enabled {
            let api_cfg = self.api_config.clone();
            let api_channels = self.channels.clone();
            let api_uptime = self.uptime;
            let api_tx = tx.clone();
            let api_audit = AuditLogger::new(self.memory.pool().clone());
            let api_channel_config = self.channel_config.clone();
            Some(tokio::spawn(async move {
                crate::api::serve(
                    api_cfg,
                    api_channels,
                    api_uptime,
                    api_tx,
                    api_audit,
                    api_channel_config,
                )
                .await;
            }))
        } else {
            None
        };

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
            let sched_hb_interval = self.heartbeat_interval.clone();
            let sched_hb_notify = self.heartbeat_notify.clone();
            let sched_audit = AuditLogger::new(self.memory.pool().clone());
            let sched_provider_name = self.provider.name().to_string();
            let sched_data_dir = self.data_dir.clone();
            let sched_config_path = self.config_path.clone();
            let sched_active_start = self.heartbeat_config.active_start.clone();
            let sched_active_end = self.heartbeat_config.active_end.clone();
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
                    sched_hb_notify,
                    sched_audit,
                    sched_provider_name,
                    sched_data_dir,
                    sched_config_path,
                    sched_active_start,
                    sched_active_end,
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
            let hb_notify = self.heartbeat_notify.clone();
            let hb_model = self.model_complex.clone();
            let hb_model_fast = self.model_fast.clone();
            let hb_skills = self.skills.clone();
            let hb_audit = AuditLogger::new(self.memory.pool().clone());
            let hb_provider_name = self.provider.name().to_string();
            let hb_data_dir = self.data_dir.clone();
            let hb_config_path = self.config_path.clone();
            Some(tokio::spawn(async move {
                Self::heartbeat_loop(
                    hb_provider,
                    hb_channels,
                    hb_config,
                    hb_prompts,
                    hb_memory,
                    hb_interval,
                    hb_notify,
                    hb_model,
                    hb_model_fast,
                    hb_skills,
                    hb_audit,
                    hb_provider_name,
                    hb_data_dir,
                    hb_config_path,
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
            if let Some(buf) = active.get_mut(&sender_key) {
                // Sender already has an active call — buffer this message.
                buf.push(incoming.clone());
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
                for (conv_id, _channel, _sender_id, _project) in &convos {
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
mod tests;
