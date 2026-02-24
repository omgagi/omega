//! Classification-based routing, multi-step execution, and direct response handling.

use super::Gateway;
use crate::markers::*;
use omega_core::{
    config::shellexpand,
    context::{Context, ContextEntry},
    message::{IncomingMessage, MessageMetadata, OutgoingMessage},
};
use omega_memory::audit::{AuditEntry, AuditStatus};
use std::path::PathBuf;
use tracing::{error, info, warn};

impl Gateway {
    /// Classify a message and route to the appropriate model.
    ///
    /// Intentionally kept but currently unused: all conversations route DIRECT.
    /// Heartbeat uses a similar classifier pattern. Will be re-enabled when
    /// multi-step execution is restored. See `execute_steps()` below.
    #[allow(dead_code)]
    pub(super) async fn classify_and_route(
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
        ctx.max_turns = Some(25); // Classification is best-effort — generous limit avoids noisy warnings.
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
    ///
    /// Intentionally kept but currently unused: paired with `classify_and_route()`.
    /// Will be re-enabled when multi-step execution is restored.
    #[allow(dead_code)]
    pub(super) async fn execute_steps(
        &self,
        incoming: &IncomingMessage,
        original_task: &str,
        context: &Context,
        steps: &[String],
        active_project: Option<&str>,
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
                    let step_markers = self
                        .process_markers(incoming, &mut step_resp.text, active_project)
                        .await;

                    completed_summary.push_str(&format!("- Step {step_num}: {step} (done)\n"));

                    // Send progress update.
                    let progress = format!("✓ {step} ({step_num}/{total})");
                    self.send_text(incoming, &progress).await;

                    // Send task confirmation if any tasks were scheduled in this step.
                    if !step_markers.is_empty() {
                        self.send_task_confirmation(incoming, &step_markers).await;
                    }

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

        // Inbox images are cleaned up by InboxGuard in handle_message.
    }

    /// Handle the direct response path: provider call, session retry, markers,
    /// audit, send response, workspace image delivery.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn handle_direct_response(
        &self,
        incoming: &IncomingMessage,
        mut context: Context,
        full_system_prompt: String,
        full_history: Vec<ContextEntry>,
        typing_handle: Option<tokio::task::JoinHandle<()>>,
        active_project: Option<&str>,
        project_key: &str,
    ) {
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
        let status_channel = self.channels.get(&incoming.channel).cloned();
        let status_target = incoming.reply_target.clone();
        let status_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(15)).await;
            if let (Some(ref ch), Some(ref target)) = (&status_channel, &status_target) {
                let msg = OutgoingMessage {
                    text: nudge_msg.to_string(),
                    metadata: MessageMetadata::default(),
                    reply_target: Some(target.clone()),
                };
                let _ = ch.send(msg).await;
            }
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
        let provider_result = provider_task.await;

        // If session call failed, retry with full context (session may be stale).
        let response = match provider_result {
            Ok(Err(ref e)) if context.session_id.is_some() => {
                warn!("session call failed: {e}, retrying with full context");
                let _ = self
                    .memory
                    .clear_session(&incoming.channel, &incoming.sender_id, project_key)
                    .await;
                context.session_id = None;
                context.system_prompt = full_system_prompt;
                context.history = full_history;

                let provider = self.provider.clone();
                let retry_ctx = context.clone();
                match provider.complete(&retry_ctx).await {
                    Ok(mut resp) => {
                        status_handle.abort();
                        info!(
                            "[{}] provider responded (retry) | model: {} | {}ms",
                            incoming.channel,
                            resp.metadata.model.as_deref().unwrap_or("unknown"),
                            resp.metadata.processing_time_ms
                        );
                        resp.reply_target = incoming.reply_target.clone();
                        resp
                    }
                    Err(e) => {
                        status_handle.abort();
                        error!("provider retry error: {e}");
                        if let Some(h) = typing_handle {
                            h.abort();
                        }
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
                        self.send_text(incoming, &friendly).await;
                        return;
                    }
                }
            }
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
                self.send_text(incoming, &friendly).await;
                return;
            }
            Err(join_err) => {
                status_handle.abort();
                error!("provider task panicked: {join_err}");
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(incoming, "Something went wrong. Please try again.")
                    .await;
                return;
            }
        };

        // Capture session_id from provider response for future continuations.
        if let Some(ref sid) = response.metadata.session_id {
            let _ = self
                .memory
                .store_session(&incoming.channel, &incoming.sender_id, project_key, sid)
                .await;
        }

        // Stop typing indicator.
        if let Some(h) = typing_handle {
            h.abort();
        }

        // --- PROCESS MARKERS ---
        let mut response = response;
        let marker_results = self
            .process_markers(incoming, &mut response.text, active_project)
            .await;

        // --- STORE IN MEMORY ---
        if let Err(e) = self
            .memory
            .store_exchange(incoming, &response, project_key)
            .await
        {
            error!("failed to store exchange: {e}");
        }

        // --- AUDIT LOG ---
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

        // --- SEND RESPONSE ---
        if let Some(channel) = self.channels.get(&incoming.channel) {
            if let Err(e) = channel.send(response).await {
                error!("failed to send response via {}: {e}", incoming.channel);
            }

            // Send task confirmation.
            if !marker_results.is_empty() {
                self.send_task_confirmation(incoming, &marker_results).await;
            }

            // Send new workspace images.
            let images_after = snapshot_workspace_images(&workspace_path);
            let new_images: Vec<PathBuf> = images_after
                .iter()
                .filter(|(path, mtime)| match images_before.get(path.as_path()) {
                    None => true,
                    Some(old_mtime) => mtime > &old_mtime,
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
                if let Err(e) = std::fs::remove_file(image_path) {
                    warn!("failed to remove workspace image {filename}: {e}");
                }
            }
        } else {
            error!("no channel found for '{}'", incoming.channel);
        }
    }
}
