//! Helper functions for heartbeat execution — enrichment, system prompt, markers, delivery.

use crate::markers::*;
use omega_core::{
    config::{HeartbeatConfig, Prompts},
    message::{MessageMetadata, OutgoingMessage},
    traits::Channel,
};
use omega_memory::{
    audit::{AuditEntry, AuditLogger, AuditStatus},
    Store,
};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tracing::{error, info, warn};

/// Build enrichment context from user facts and recent conversation summaries.
///
/// When `project` is Some, uses project-scoped lessons/outcomes.
pub async fn build_enrichment(memory: &Store, project: Option<&str>) -> String {
    let mut enrichment = String::new();
    if let Ok(facts) = memory.get_all_facts().await {
        if !facts.is_empty() {
            enrichment.push_str("\n\nKnown about the user:");
            for (key, value) in &facts {
                enrichment.push_str(&format!("\n- {key}: {value}"));
            }
        }
    }
    if let Ok(summaries) = memory.get_all_recent_summaries(3).await {
        if !summaries.is_empty() {
            enrichment.push_str("\n\nRecent activity:");
            for (summary, timestamp) in &summaries {
                enrichment.push_str(&format!("\n- [{timestamp}] {summary}"));
            }
        }
    }
    if let Ok(lessons) = memory.get_all_lessons(project).await {
        if !lessons.is_empty() {
            enrichment.push_str("\n\nLearned behavioral rules:");
            for (domain, rule, proj) in &lessons {
                if proj.is_empty() {
                    enrichment.push_str(&format!("\n- [{domain}] {rule}"));
                } else {
                    enrichment.push_str(&format!("\n- [{domain}] ({proj}) {rule}"));
                }
            }
        }
    }
    if let Ok(outcomes) = memory.get_all_recent_outcomes(24, 20, project).await {
        if !outcomes.is_empty() {
            enrichment.push_str("\n\nRecent outcomes (last 24h):");
            for (score, domain, lesson, timestamp) in &outcomes {
                let sign = match score.cmp(&0) {
                    std::cmp::Ordering::Greater => "+",
                    std::cmp::Ordering::Less => "-",
                    std::cmp::Ordering::Equal => "~",
                };
                enrichment.push_str(&format!("\n- [{sign}] {domain}: {lesson} ({timestamp})"));
            }
        }
    }
    enrichment
}

/// Build the heartbeat system prompt (Identity + Soul + System + time).
///
/// When `project` is Some and has ROLE.md, appends project instructions.
pub fn build_system_prompt(
    prompts: &Prompts,
    project: Option<&str>,
    data_dir: Option<&str>,
) -> String {
    let mut system = format!(
        "{}\n\n{}\n\n{}",
        prompts.identity, prompts.soul, prompts.system
    );
    system.push_str(&format!(
        "\n\nCurrent time: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M %Z")
    ));

    // Inject project ROLE.md for project-scoped heartbeats.
    if let (Some(proj), Some(dd)) = (project, data_dir) {
        let data_path = omega_core::config::shellexpand(dd);
        let projects = omega_skills::load_projects(&data_path);
        if let Some(instructions) = omega_skills::get_project_instructions(&projects, proj) {
            system.push_str(&format!(
                "\n\n---\n\n[Active project: {proj}]\n{instructions}"
            ));
        }
    }

    system
}

/// Process all markers in a heartbeat response.
///
/// Handles: SCHEDULE, SCHEDULE_ACTION, heartbeat markers (interval, add/remove),
/// CANCEL_TASK, UPDATE_TASK, REWARD, LESSON. Returns the text with all markers stripped.
/// Tags REWARD/LESSON markers with `project`.
pub async fn process_heartbeat_markers(
    mut text: String,
    memory: &Store,
    sender_id: &str,
    channel_name: &str,
    interval: &Arc<AtomicU64>,
    project: &str,
) -> String {
    for sched_line in extract_all_schedule_markers(&text) {
        if let Some((desc, due, rep)) = parse_schedule_line(&sched_line) {
            let rep_opt = if rep == "once" {
                None
            } else {
                Some(rep.as_str())
            };
            match memory
                .create_task(
                    channel_name,
                    sender_id,
                    sender_id,
                    &desc,
                    &due,
                    rep_opt,
                    "reminder",
                    project,
                )
                .await
            {
                Ok(new_id) => info!("heartbeat spawned reminder {new_id}: {desc}"),
                Err(e) => error!("heartbeat: failed to create reminder: {e}"),
            }
        }
    }
    text = strip_schedule_marker(&text);

    for sched_line in extract_all_schedule_action_markers(&text) {
        if let Some((desc, due, rep)) = parse_schedule_action_line(&sched_line) {
            let rep_opt = if rep == "once" {
                None
            } else {
                Some(rep.as_str())
            };
            match memory
                .create_task(
                    channel_name,
                    sender_id,
                    sender_id,
                    &desc,
                    &due,
                    rep_opt,
                    "action",
                    project,
                )
                .await
            {
                Ok(new_id) => info!("heartbeat spawned action {new_id}: {desc}"),
                Err(e) => error!("heartbeat: failed to create action: {e}"),
            }
        }
    }
    text = strip_schedule_action_markers(&text);

    let hb_actions = extract_heartbeat_markers(&text);
    if !hb_actions.is_empty() {
        let hb_project = if project.is_empty() {
            None
        } else {
            Some(project)
        };
        apply_heartbeat_changes(&hb_actions, hb_project);
        for action in &hb_actions {
            if let HeartbeatAction::SetInterval(mins) = action {
                interval.store(*mins, Ordering::Relaxed);
                info!("heartbeat: interval changed to {mins} minutes (via heartbeat loop)");
            }
        }
        text = strip_heartbeat_markers(&text);
    }

    for id_prefix in extract_all_cancel_tasks(&text) {
        match memory.cancel_task(&id_prefix, sender_id).await {
            Ok(true) => info!("heartbeat cancelled task: {id_prefix}"),
            Ok(false) => warn!("heartbeat: no matching task to cancel: {id_prefix}"),
            Err(e) => error!("heartbeat: failed to cancel task: {e}"),
        }
    }
    text = strip_cancel_task(&text);

    for update_line in extract_all_update_tasks(&text) {
        if let Some((id_prefix, desc, due_at, repeat)) = parse_update_task_line(&update_line) {
            match memory
                .update_task(
                    &id_prefix,
                    sender_id,
                    desc.as_deref(),
                    due_at.as_deref(),
                    repeat.as_deref(),
                )
                .await
            {
                Ok(true) => info!("heartbeat updated task: {id_prefix}"),
                Ok(false) => warn!("heartbeat: no matching task to update: {id_prefix}"),
                Err(e) => error!("heartbeat: failed to update task: {e}"),
            }
        }
    }
    text = strip_update_task(&text);

    // REWARD + LESSON markers (project-tagged).
    for reward_line in extract_all_rewards(&text) {
        if let Some((score, domain, lesson)) = parse_reward_line(&reward_line) {
            match memory
                .store_outcome(sender_id, &domain, score, &lesson, "heartbeat", project)
                .await
            {
                Ok(()) => info!("heartbeat outcome: {score:+} | {domain} | {lesson}"),
                Err(e) => error!("heartbeat: failed to store outcome: {e}"),
            }
        }
    }
    text = strip_reward_markers(&text);

    for lesson_line in extract_all_lessons(&text) {
        if let Some((domain, rule)) = parse_lesson_line(&lesson_line) {
            match memory
                .store_lesson(sender_id, &domain, &rule, project)
                .await
            {
                Ok(()) => info!("heartbeat lesson: {domain} | {rule}"),
                Err(e) => error!("heartbeat: failed to store lesson: {e}"),
            }
        }
    }
    text = strip_lesson_markers(&text);

    // HEARTBEAT_SUPPRESS_SECTION / HEARTBEAT_UNSUPPRESS_SECTION
    let suppress_actions = extract_suppress_section_markers(&text);
    if !suppress_actions.is_empty() {
        let hb_project = if project.is_empty() {
            None
        } else {
            Some(project)
        };
        apply_suppress_actions(&suppress_actions, hb_project);
        text = strip_suppress_section_markers(&text);
    }

    text
}

/// Audit and send a heartbeat result to the user.
#[allow(clippy::too_many_arguments)]
pub async fn send_heartbeat_result(
    result: Option<(String, i64)>,
    channel_name: &str,
    sender_id: &str,
    channels: &HashMap<String, Arc<dyn Channel>>,
    config: &HeartbeatConfig,
    audit: &AuditLogger,
    provider_name: &str,
    model: &str,
) {
    let (text, elapsed_ms) = match result {
        Some(r) => r,
        None => {
            info!("heartbeat: OK");
            return;
        }
    };

    let audit_entry = AuditEntry {
        channel: channel_name.to_string(),
        sender_id: sender_id.to_string(),
        sender_name: None,
        input_text: "[HEARTBEAT]".to_string(),
        output_text: Some(text.clone()),
        provider_used: Some(provider_name.to_string()),
        model: Some(model.to_string()),
        processing_ms: Some(elapsed_ms),
        status: AuditStatus::Ok,
        denial_reason: None,
    };
    if let Err(e) = audit.log(&audit_entry).await {
        error!("heartbeat: audit log failed: {e}");
    }

    if let Some(ch) = channels.get(channel_name) {
        let msg = OutgoingMessage {
            text,
            metadata: MessageMetadata::default(),
            reply_target: Some(config.reply_target.clone()),
        };
        if let Err(e) = ch.send(msg).await {
            error!("heartbeat: failed to send alert: {e}");
        }
    } else {
        warn!("heartbeat: channel '{channel_name}' not found, alert dropped");
    }
}
