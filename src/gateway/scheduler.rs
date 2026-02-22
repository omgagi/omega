//! Scheduled task delivery — reminders and action tasks.

use super::keywords::MAX_ACTION_RETRIES;
use super::Gateway;
use crate::markers::*;
use omega_core::{
    config::Prompts,
    context::Context,
    message::{MessageMetadata, OutgoingMessage},
    traits::{Channel, Provider},
};
use omega_memory::{
    audit::{AuditEntry, AuditLogger, AuditStatus},
    Store,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};

impl Gateway {
    /// Background task: deliver due scheduled tasks.
    ///
    /// Reminder tasks send a text message. Action tasks invoke the provider
    /// with full tool access and process response markers.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn scheduler_loop(
        store: Store,
        channels: HashMap<String, Arc<dyn Channel>>,
        poll_secs: u64,
        provider: Arc<dyn Provider>,
        skills: Vec<omega_skills::Skill>,
        prompts: Prompts,
        model_complex: String,
        sandbox_prompt: Option<String>,
        heartbeat_interval: Arc<AtomicU64>,
        audit: AuditLogger,
        provider_name: String,
    ) {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(poll_secs)).await;

            match store.get_due_tasks().await {
                Ok(tasks) => {
                    for (
                        id,
                        channel_name,
                        sender_id,
                        reply_target,
                        description,
                        repeat,
                        task_type,
                    ) in &tasks
                    {
                        if task_type == "action" {
                            // --- Action task: invoke provider ---
                            info!("scheduler: executing action task {id}: {description}");
                            let started = Instant::now();

                            let mut system = format!(
                                "{}\n\n{}\n\n{}",
                                prompts.identity, prompts.soul, prompts.system
                            );
                            if let Some(ref sp) = sandbox_prompt {
                                system.push_str("\n\n");
                                system.push_str(sp);
                            }

                            // Current time — so the AI always knows when it is.
                            system.push_str(&format!(
                                "\n\nCurrent time: {}",
                                chrono::Local::now().format("%Y-%m-%d %H:%M %Z")
                            ));

                            // Enrich with user profile so the AI knows who the owner is.
                            let facts = store.get_facts(sender_id).await.unwrap_or_default();
                            let profile = omega_memory::store::format_user_profile(&facts);
                            if !profile.is_empty() {
                                system.push_str("\n\n");
                                system.push_str(&profile);
                            }

                            // Resolve language preference.
                            let language = facts
                                .iter()
                                .find(|(k, _)| k == "preferred_language")
                                .map(|(_, v)| v.as_str())
                                .unwrap_or("English");
                            system
                                .push_str(&format!("\n\nIMPORTANT: Always respond in {language}."));

                            // Critical: tell the AI how action task delivery works.
                            system.push_str(&format!(
                                "\n\nIMPORTANT — Action Task Delivery:\n\
                                 You are executing a scheduled action task for {owner}. \
                                 Your text response will be delivered DIRECTLY to {owner} \
                                 via {channel}.\n\
                                 - To communicate with {owner}: just write the message as \
                                 your response. The system delivers it automatically.\n\
                                 - To perform external actions (send email, call APIs, run \
                                 commands): use your tools normally, then report the result \
                                 as your response to {owner}.\n\
                                 Never search contacts or guess how to reach {owner} — your \
                                 response IS the delivery channel.",
                                owner = facts
                                    .iter()
                                    .find(|(k, _)| k == "name")
                                    .map(|(_, v)| v.as_str())
                                    .unwrap_or("the user"),
                                channel = channel_name,
                            ));

                            // Inject verification instruction for outcome tracking.
                            system.push_str(concat!(
                                "\n\nIMPORTANT — Action Task Verification:\n",
                                "After completing this action, end your response with exactly ",
                                "one of these markers on its own line:\n",
                                "- ACTION_OUTCOME: success\n",
                                "- ACTION_OUTCOME: failed | <brief reason>\n",
                                "The gateway strips this marker before delivering to the user."
                            ));

                            let mut ctx = Context::new(description);
                            ctx.system_prompt = system;
                            ctx.model = Some(model_complex.clone());

                            // Match skill triggers on description to inject MCP servers.
                            let matched_servers =
                                omega_skills::match_skill_triggers(&skills, description);
                            ctx.mcp_servers = matched_servers;

                            match provider.complete(&ctx).await {
                                Ok(resp) => {
                                    let elapsed_ms = started.elapsed().as_millis() as i64;
                                    let mut text = resp.text.clone();

                                    // Parse ACTION_OUTCOME before stripping other markers.
                                    let outcome = extract_action_outcome(&text);
                                    text = strip_action_outcome(&text);

                                    // Process SCHEDULE markers from action response.
                                    for sched_line in extract_all_schedule_markers(&text) {
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
                                                    sender_id,
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
                                    }
                                    text = strip_schedule_marker(&text);

                                    // Process SCHEDULE_ACTION markers from action response.
                                    for sched_line in extract_all_schedule_action_markers(&text) {
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
                                                    sender_id,
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
                                    }
                                    text = strip_schedule_action_markers(&text);

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

                                    // Process CANCEL_TASK markers from action response.
                                    for id_prefix in extract_all_cancel_tasks(&text) {
                                        match store
                                            .cancel_task(&id_prefix, sender_id)
                                            .await
                                        {
                                            Ok(true) => info!(
                                                "action task cancelled task: {id_prefix}"
                                            ),
                                            Ok(false) => warn!(
                                                "action task: no matching task to cancel: {id_prefix}"
                                            ),
                                            Err(e) => error!(
                                                "action task: failed to cancel task: {e}"
                                            ),
                                        }
                                    }
                                    text = strip_cancel_task(&text);

                                    // Process UPDATE_TASK markers from action response.
                                    for update_line in extract_all_update_tasks(&text) {
                                        if let Some((id_prefix, desc, due_at, repeat)) =
                                            parse_update_task_line(&update_line)
                                        {
                                            match store
                                                .update_task(
                                                    &id_prefix,
                                                    sender_id,
                                                    desc.as_deref(),
                                                    due_at.as_deref(),
                                                    repeat.as_deref(),
                                                )
                                                .await
                                            {
                                                Ok(true) => info!(
                                                    "action task updated task: {id_prefix}"
                                                ),
                                                Ok(false) => warn!(
                                                    "action task: no matching task to update: {id_prefix}"
                                                ),
                                                Err(e) => error!(
                                                    "action task: failed to update task: {e}"
                                                ),
                                            }
                                        }
                                    }
                                    text = strip_update_task(&text);

                                    // Determine audit status and handle outcome.
                                    let (audit_status, action_ok) = match &outcome {
                                        Some(ActionOutcome::Success) => (AuditStatus::Ok, true),
                                        Some(ActionOutcome::Failed(reason)) => {
                                            warn!("action task {id} reported failure: {reason}");
                                            (AuditStatus::Error, false)
                                        }
                                        None => {
                                            // No marker — backward compat, assume success.
                                            warn!("action task {id}: no ACTION_OUTCOME marker, assuming success");
                                            (AuditStatus::Ok, true)
                                        }
                                    };

                                    // Audit log the action execution.
                                    let audit_entry = AuditEntry {
                                        channel: channel_name.clone(),
                                        sender_id: sender_id.clone(),
                                        sender_name: None,
                                        input_text: format!("[ACTION] {description}"),
                                        output_text: Some(resp.text.clone()),
                                        provider_used: Some(provider_name.clone()),
                                        model: Some(model_complex.clone()),
                                        processing_ms: Some(elapsed_ms),
                                        status: audit_status,
                                        denial_reason: None,
                                    };
                                    if let Err(e) = audit.log(&audit_entry).await {
                                        error!("action task {id}: audit log failed: {e}");
                                    }

                                    if action_ok {
                                        // Success: complete the task.
                                        if let Err(e) =
                                            store.complete_task(id, repeat.as_deref()).await
                                        {
                                            error!("failed to complete action task {id}: {e}");
                                        } else {
                                            info!("completed action task {id}: {description}");
                                        }
                                    } else {
                                        // Failure: retry or permanently fail.
                                        let reason = match &outcome {
                                            Some(ActionOutcome::Failed(r)) if !r.is_empty() => {
                                                r.clone()
                                            }
                                            _ => "action reported failure".to_string(),
                                        };
                                        match store.fail_task(id, &reason, MAX_ACTION_RETRIES).await
                                        {
                                            Ok(will_retry) => {
                                                if will_retry {
                                                    info!(
                                                        "action task {id} will retry in 2 minutes"
                                                    );
                                                    // Notify user about retry.
                                                    if let Some(ch) = channels.get(channel_name) {
                                                        let msg = OutgoingMessage {
                                                            text: format!(
                                                                "Action failed: {description}\nRetrying in 2 minutes..."
                                                            ),
                                                            metadata: MessageMetadata::default(),
                                                            reply_target: Some(reply_target.clone()),
                                                        };
                                                        let _ = ch.send(msg).await;
                                                    }
                                                } else {
                                                    error!("action task {id} permanently failed after {MAX_ACTION_RETRIES} retries");
                                                    if let Some(ch) = channels.get(channel_name) {
                                                        let msg = OutgoingMessage {
                                                            text: format!(
                                                                "Action permanently failed after {} retries: {description}\nReason: {reason}",
                                                                MAX_ACTION_RETRIES
                                                            ),
                                                            metadata: MessageMetadata::default(),
                                                            reply_target: Some(reply_target.clone()),
                                                        };
                                                        let _ = ch.send(msg).await;
                                                    }
                                                }
                                            }
                                            Err(e) => {
                                                error!("action task {id}: fail_task error: {e}")
                                            }
                                        }
                                        continue; // Skip sending the response for failed actions.
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
                                }
                                Err(e) => {
                                    let elapsed_ms = started.elapsed().as_millis() as i64;
                                    let err_str = e.to_string();
                                    error!("action task {id} provider error: {err_str}");

                                    // Audit log the provider error.
                                    let audit_entry = AuditEntry {
                                        channel: channel_name.clone(),
                                        sender_id: sender_id.clone(),
                                        sender_name: None,
                                        input_text: format!("[ACTION] {description}"),
                                        output_text: None,
                                        provider_used: Some(provider_name.clone()),
                                        model: Some(model_complex.clone()),
                                        processing_ms: Some(elapsed_ms),
                                        status: AuditStatus::Error,
                                        denial_reason: Some(err_str.clone()),
                                    };
                                    if let Err(ae) = audit.log(&audit_entry).await {
                                        error!("action task {id}: audit log failed: {ae}");
                                    }

                                    // Retry or permanently fail.
                                    match store.fail_task(id, &err_str, MAX_ACTION_RETRIES).await {
                                        Ok(will_retry) => {
                                            if let Some(ch) = channels.get(channel_name) {
                                                let msg_text = if will_retry {
                                                    format!("Action failed: {description}\nRetrying in 2 minutes...")
                                                } else {
                                                    format!(
                                                        "Action permanently failed after {} retries: {description}",
                                                        MAX_ACTION_RETRIES
                                                    )
                                                };
                                                let msg = OutgoingMessage {
                                                    text: msg_text,
                                                    metadata: MessageMetadata::default(),
                                                    reply_target: Some(reply_target.clone()),
                                                };
                                                let _ = ch.send(msg).await;
                                            }
                                        }
                                        Err(fe) => {
                                            error!("action task {id}: fail_task error: {fe}")
                                        }
                                    }
                                }
                            }
                            continue; // Action tasks handle their own completion above.
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
}
