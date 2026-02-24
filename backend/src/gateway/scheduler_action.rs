//! Action task execution — provider-based scheduled task processing with project awareness.

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

use super::keywords::MAX_ACTION_RETRIES;

/// Execute a single action task with full provider access and project awareness.
///
/// When `project` is non-empty, loads ROLE.md instructions and uses project-scoped
/// lessons/outcomes for enrichment.
#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_action_task(
    id: &str,
    channel_name: &str,
    sender_id: &str,
    reply_target: &str,
    description: &str,
    repeat: Option<&str>,
    project: &str,
    store: &Store,
    channels: &HashMap<String, Arc<dyn Channel>>,
    provider: &dyn Provider,
    skills: &[omega_skills::Skill],
    prompts: &Prompts,
    model_complex: &str,
    heartbeat_interval: &Arc<AtomicU64>,
    audit: &AuditLogger,
    provider_name: &str,
    data_dir: &str,
) {
    info!("scheduler: executing action task {id}: {description}");
    let started = Instant::now();

    let mut system = format!(
        "{}\n\n{}\n\n{}",
        prompts.identity, prompts.soul, prompts.system
    );

    // Current time — so the AI always knows when it is.
    system.push_str(&format!(
        "\n\nCurrent time: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M %Z")
    ));

    // Inject project ROLE.md when this is a project-scoped action task.
    if !project.is_empty() {
        let data_path = omega_core::config::shellexpand(data_dir);
        let projects = omega_skills::load_projects(&data_path);
        if let Some(instructions) = omega_skills::get_project_instructions(&projects, project) {
            system.push_str(&format!(
                "\n\n---\n\n[Active project: {project}]\n{instructions}"
            ));
        }
    }

    // Enrich with user profile so the AI knows who the owner is.
    let facts = store.get_facts(sender_id).await.unwrap_or_default();
    let profile = omega_memory::store::format_user_profile(&facts);
    if !profile.is_empty() {
        system.push_str("\n\n");
        system.push_str(&profile);
    }

    // Inject learned lessons + recent outcomes for self-learning.
    // When project-scoped, use project-specific + general (layered).
    let project_filter = if project.is_empty() {
        None
    } else {
        Some(project)
    };
    let lessons = store
        .get_lessons(sender_id, project_filter)
        .await
        .unwrap_or_default();
    if !lessons.is_empty() {
        system.push_str("\n\nLearned behavioral rules (MUST follow):");
        for (domain, rule, proj) in &lessons {
            if proj.is_empty() {
                system.push_str(&format!("\n- [{domain}] {rule}"));
            } else {
                system.push_str(&format!("\n- [{domain}] ({proj}) {rule}"));
            }
        }
    }
    let outcomes = store
        .get_recent_outcomes(sender_id, 10, project_filter)
        .await
        .unwrap_or_default();
    if !outcomes.is_empty() {
        system.push_str("\n\nRecent outcomes:");
        for (score, domain, lesson, _ts) in &outcomes {
            let sign = if *score > 0 {
                "+"
            } else if *score < 0 {
                "-"
            } else {
                "~"
            };
            system.push_str(&format!("\n- [{sign}] {domain}: {lesson}"));
        }
    }

    // Resolve language preference.
    let language = facts
        .iter()
        .find(|(k, _)| k == "preferred_language")
        .map(|(_, v)| v.as_str())
        .unwrap_or("English");
    system.push_str(&format!("\n\nIMPORTANT: Always respond in {language}."));

    // Critical: tell the AI how action task delivery works.
    let owner = facts
        .iter()
        .find(|(k, _)| k == "name")
        .map(|(_, v)| v.as_str())
        .unwrap_or("the user");
    system.push_str(&format!(
        "\n\nIMPORTANT — Action Task Delivery:\n\
         You are executing a scheduled action task for {owner}. \
         Your text response will be delivered DIRECTLY to {owner} \
         via {channel_name}.\n\
         - To communicate with {owner}: just write the message as \
         your response. The system delivers it automatically.\n\
         - To perform external actions (send email, call APIs, run \
         commands): use your tools normally, then report the result \
         as your response to {owner}.\n\
         Never search contacts or guess how to reach {owner} — your \
         response IS the delivery channel.",
    ));

    // Builds are user-initiated only — action tasks must never start them.
    system.push_str(
        "\n\nIMPORTANT — No Builds:\n\
         You must NEVER initiate, scaffold, or create software projects. \
         Builds can only be triggered by the user through a dedicated keyword gate. \
         If the task description sounds like a build request, reply to the user \
         suggesting they ask for it directly (e.g. \"build me a ...\").",
    );

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
    ctx.model = Some(model_complex.to_string());

    // Match skill triggers on description to inject MCP servers.
    let matched_servers = omega_skills::match_skill_triggers(skills, description);
    ctx.mcp_servers = matched_servers;

    match provider.complete(&ctx).await {
        Ok(resp) => {
            let elapsed_ms = started.elapsed().as_millis() as i64;
            let mut text = resp.text.clone();

            // Parse ACTION_OUTCOME before stripping other markers.
            let outcome = extract_action_outcome(&text);
            text = strip_action_outcome(&text);

            // Process markers from action response (project-tagged).
            process_action_markers(
                &mut text,
                store,
                channel_name,
                sender_id,
                reply_target,
                project,
                heartbeat_interval,
            )
            .await;

            // Determine audit status and handle outcome.
            let (audit_status, action_ok) = match &outcome {
                Some(ActionOutcome::Success) => (AuditStatus::Ok, true),
                Some(ActionOutcome::Failed(reason)) => {
                    warn!("action task {id} reported failure: {reason}");
                    (AuditStatus::Error, false)
                }
                None => {
                    warn!("action task {id}: no ACTION_OUTCOME marker, assuming success");
                    (AuditStatus::Ok, true)
                }
            };

            // Audit log the action execution.
            let audit_entry = AuditEntry {
                channel: channel_name.to_string(),
                sender_id: sender_id.to_string(),
                sender_name: None,
                input_text: format!("[ACTION] {description}"),
                output_text: Some(resp.text.clone()),
                provider_used: Some(provider_name.to_string()),
                model: Some(model_complex.to_string()),
                processing_ms: Some(elapsed_ms),
                status: audit_status,
                denial_reason: None,
            };
            if let Err(e) = audit.log(&audit_entry).await {
                error!("action task {id}: audit log failed: {e}");
            }

            if action_ok {
                if let Err(e) = store.complete_task(id, repeat).await {
                    error!("failed to complete action task {id}: {e}");
                } else {
                    info!("completed action task {id}: {description}");
                }
            } else {
                let reason = match &outcome {
                    Some(ActionOutcome::Failed(r)) if !r.is_empty() => r.clone(),
                    _ => "action reported failure".to_string(),
                };
                match store.fail_task(id, &reason, MAX_ACTION_RETRIES).await {
                    Ok(will_retry) => {
                        if will_retry {
                            info!("action task {id} will retry in 2 minutes");
                            if let Some(ch) = channels.get(channel_name) {
                                let msg = OutgoingMessage {
                                    text: format!(
                                        "Action failed: {description}\nRetrying in 2 minutes..."
                                    ),
                                    metadata: MessageMetadata::default(),
                                    reply_target: Some(reply_target.to_string()),
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
                                    reply_target: Some(reply_target.to_string()),
                                };
                                let _ = ch.send(msg).await;
                            }
                        }
                    }
                    Err(e) => error!("action task {id}: fail_task error: {e}"),
                }
                return; // Skip sending the response for failed actions.
            }

            // Send response to channel (if non-empty after stripping markers).
            let cleaned = text.trim();
            if !cleaned.is_empty() && cleaned != "HEARTBEAT_OK" {
                if let Some(ch) = channels.get(channel_name) {
                    let msg = OutgoingMessage {
                        text: cleaned.to_string(),
                        metadata: MessageMetadata::default(),
                        reply_target: Some(reply_target.to_string()),
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

            let audit_entry = AuditEntry {
                channel: channel_name.to_string(),
                sender_id: sender_id.to_string(),
                sender_name: None,
                input_text: format!("[ACTION] {description}"),
                output_text: None,
                provider_used: Some(provider_name.to_string()),
                model: Some(model_complex.to_string()),
                processing_ms: Some(elapsed_ms),
                status: AuditStatus::Error,
                denial_reason: Some(err_str.clone()),
            };
            if let Err(ae) = audit.log(&audit_entry).await {
                error!("action task {id}: audit log failed: {ae}");
            }

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
                            reply_target: Some(reply_target.to_string()),
                        };
                        let _ = ch.send(msg).await;
                    }
                }
                Err(fe) => error!("action task {id}: fail_task error: {fe}"),
            }
        }
    }
}

/// Process markers in an action task response, tagging with project.
#[allow(clippy::too_many_arguments)]
async fn process_action_markers(
    text: &mut String,
    store: &Store,
    channel_name: &str,
    sender_id: &str,
    reply_target: &str,
    project: &str,
    heartbeat_interval: &Arc<AtomicU64>,
) {
    // SCHEDULE markers.
    for sched_line in extract_all_schedule_markers(text) {
        if let Some((desc, due, rep)) = parse_schedule_line(&sched_line) {
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
                    project,
                )
                .await
            {
                Ok(new_id) => info!("action task spawned reminder {new_id}: {desc}"),
                Err(e) => error!("action task: failed to create reminder: {e}"),
            }
        }
    }
    *text = strip_schedule_marker(text);

    // SCHEDULE_ACTION markers.
    for sched_line in extract_all_schedule_action_markers(text) {
        if let Some((desc, due, rep)) = parse_schedule_action_line(&sched_line) {
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
                    project,
                )
                .await
            {
                Ok(new_id) => info!("action task spawned action {new_id}: {desc}"),
                Err(e) => error!("action task: failed to create action: {e}"),
            }
        }
    }
    *text = strip_schedule_action_markers(text);

    // HEARTBEAT markers.
    let hb_actions = extract_heartbeat_markers(text);
    if !hb_actions.is_empty() {
        apply_heartbeat_changes(&hb_actions, None);
        for action in &hb_actions {
            if let HeartbeatAction::SetInterval(mins) = action {
                heartbeat_interval.store(*mins, Ordering::Relaxed);
                info!("heartbeat: interval changed to {mins} minutes (via scheduler)");
            }
        }
        *text = strip_heartbeat_markers(text);
    }

    // CANCEL_TASK markers.
    for id_prefix in extract_all_cancel_tasks(text) {
        match store.cancel_task(&id_prefix, sender_id).await {
            Ok(true) => info!("action task cancelled task: {id_prefix}"),
            Ok(false) => warn!("action task: no matching task to cancel: {id_prefix}"),
            Err(e) => error!("action task: failed to cancel task: {e}"),
        }
    }
    *text = strip_cancel_task(text);

    // UPDATE_TASK markers.
    for update_line in extract_all_update_tasks(text) {
        if let Some((id_prefix, desc, due_at, repeat)) = parse_update_task_line(&update_line) {
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
                Ok(true) => info!("action task updated task: {id_prefix}"),
                Ok(false) => warn!("action task: no matching task to update: {id_prefix}"),
                Err(e) => error!("action task: failed to update task: {e}"),
            }
        }
    }
    *text = strip_update_task(text);

    // PROJECT_ACTIVATE / PROJECT_DEACTIVATE markers.
    if let Some(project_name) = extract_project_activate(text) {
        let data_dir = omega_core::config::shellexpand("~/.omega");
        let fresh_projects = omega_skills::load_projects(&data_dir);
        if omega_skills::get_project_instructions(&fresh_projects, &project_name).is_some() {
            if let Err(e) = store
                .store_fact(sender_id, "active_project", &project_name)
                .await
            {
                error!("action task: failed to activate project {project_name}: {e}");
            } else {
                info!("action task: activated project {project_name}");
            }
        } else {
            warn!("action task: project {project_name} not found, ignoring activate");
        }
    } else if has_project_deactivate(text) {
        if let Err(e) = store.delete_fact(sender_id, "active_project").await {
            error!("action task: failed to deactivate project: {e}");
        } else {
            info!("action task: deactivated project");
        }
    }
    *text = strip_project_markers(text);

    // FORGET_CONVERSATION marker.
    if has_forget_marker(text) {
        let _ = store
            .close_current_conversation(channel_name, sender_id, project)
            .await;
        let _ = store.clear_session(channel_name, sender_id, project).await;
        *text = strip_forget_marker(text);
        info!("action task: forgot conversation for project '{project}'");
    }

    // REWARD + LESSON markers (project-tagged).
    for rl in extract_all_rewards(text) {
        if let Some((score, domain, lesson)) = parse_reward_line(&rl) {
            if let Err(e) = store
                .store_outcome(sender_id, &domain, score, &lesson, "action", project)
                .await
            {
                error!("action task: store outcome: {e}");
            } else {
                info!("action task outcome: {score:+} | {domain} | {lesson}");
            }
        }
    }
    *text = strip_reward_markers(text);
    for ll in extract_all_lessons(text) {
        if let Some((domain, rule)) = parse_lesson_line(&ll) {
            if let Err(e) = store.store_lesson(sender_id, &domain, &rule, project).await {
                error!("action task: store lesson: {e}");
            } else {
                info!("action task lesson: {domain} | {rule}");
            }
        }
    }
    *text = strip_lesson_markers(text);
}
