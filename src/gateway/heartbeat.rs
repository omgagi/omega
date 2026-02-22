//! Periodic heartbeat check-in loop.
//!
//! A fast Sonnet classification groups related checklist items by domain.
//! Each group gets its own focused Opus session **in parallel**.
//! Falls back to a single call when all items belong to the same domain.

use super::Gateway;
use crate::markers::*;
use omega_core::{
    config::{HeartbeatConfig, Prompts},
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
    /// Background task: periodic heartbeat check-in.
    ///
    /// Skips the provider call entirely when no checklist is configured.
    /// When a checklist exists, a fast Sonnet classification groups related items
    /// by domain. Each group gets its own focused Opus session in parallel.
    /// Falls back to a single call when all items belong to the same domain.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn heartbeat_loop(
        provider: Arc<dyn Provider>,
        channels: HashMap<String, Arc<dyn Channel>>,
        config: HeartbeatConfig,
        prompts: Prompts,
        sandbox_prompt: Option<String>,
        memory: Store,
        interval: Arc<AtomicU64>,
        model_complex: String,
        model_fast: String,
        skills: Vec<omega_skills::Skill>,
        audit: AuditLogger,
        provider_name: String,
    ) {
        loop {
            // Clock-aligned sleep: fire at clean boundaries (e.g. :00, :30).
            let mins = interval.load(Ordering::Relaxed);
            let now = chrono::Local::now();
            use chrono::Timelike;
            let current_minute = u64::from(now.hour()) * 60 + u64::from(now.minute());
            let next_boundary = ((current_minute / mins) + 1) * mins;
            let wait_secs = (next_boundary - current_minute) * 60 - u64::from(now.second());
            tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;

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

            // Build enrichment and system prompt once (shared across all groups).
            let enrichment = build_enrichment(&memory).await;
            let system = build_system_prompt(&prompts, &sandbox_prompt);
            let sender_id = &config.reply_target;
            let channel_name = &config.channel;

            // Classify: group related items or DIRECT for single call.
            let groups = classify_heartbeat_groups(&*provider, &model_fast, &checklist).await;

            match groups {
                None => {
                    info!("heartbeat: DIRECT (single call)");
                    let result = execute_heartbeat_group(
                        provider.clone(),
                        model_complex.clone(),
                        checklist.clone(),
                        prompts.heartbeat_checklist.clone(),
                        enrichment,
                        system,
                        skills.clone(),
                        memory.clone(),
                        sender_id.clone(),
                        channel_name.clone(),
                        interval.clone(),
                    )
                    .await;
                    send_heartbeat_result(
                        result,
                        channel_name,
                        sender_id,
                        &channels,
                        &config,
                        &audit,
                        &provider_name,
                        &model_complex,
                    )
                    .await;
                }
                Some(groups) => {
                    let group_count = groups.len();
                    info!("heartbeat: classified into {group_count} groups");

                    let mut handles = Vec::new();
                    for group in groups {
                        handles.push(tokio::spawn(execute_heartbeat_group(
                            provider.clone(),
                            model_complex.clone(),
                            group,
                            prompts.heartbeat_checklist.clone(),
                            enrichment.clone(),
                            system.clone(),
                            skills.clone(),
                            memory.clone(),
                            sender_id.clone(),
                            channel_name.clone(),
                            interval.clone(),
                        )));
                    }

                    let mut texts = Vec::new();
                    let mut max_ms: i64 = 0;
                    for (i, handle) in handles.into_iter().enumerate() {
                        match handle.await {
                            Ok(Some((text, ms))) => {
                                texts.push(text);
                                if ms > max_ms {
                                    max_ms = ms;
                                }
                            }
                            Ok(None) => info!("heartbeat: group {} OK", i + 1),
                            Err(e) => error!("heartbeat: group {} panicked: {e}", i + 1),
                        }
                    }

                    if texts.is_empty() {
                        info!("heartbeat: all {group_count} groups OK");
                    } else {
                        let combined = texts.join("\n\n---\n\n");
                        send_heartbeat_result(
                            Some((combined, max_ms)),
                            channel_name,
                            sender_id,
                            &channels,
                            &config,
                            &audit,
                            &provider_name,
                            &model_complex,
                        )
                        .await;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Heartbeat helper functions
// ---------------------------------------------------------------------------

/// Fast Sonnet classification: group heartbeat items by domain.
///
/// Returns `None` for DIRECT (all items related, or ≤3 items).
/// Returns `Some(groups)` when items span different domains.
async fn classify_heartbeat_groups(
    provider: &dyn Provider,
    model_fast: &str,
    checklist: &str,
) -> Option<Vec<String>> {
    let prompt = format!(
        "You are a heartbeat checklist organizer. Do NOT use any tools — respond with text only.\n\n\
         Given this checklist, decide how to group items for focused execution.\n\n\
         Respond DIRECT if:\n\
         - All items are closely related (same domain, same tools)\n\
         - There are 3 or fewer items total\n\n\
         Otherwise, group related items together. Each group becomes one focused \
         execution session. Items in the same domain (e.g., all trading tasks, all \
         personal reminders, all system monitoring) belong in the same group.\n\n\
         Format each group as a single numbered line:\n\
         1. First group item, second group item, third group item\n\
         2. Fourth group item, fifth group item\n\n\
         Checklist:\n{checklist}"
    );

    let mut ctx = Context::new(&prompt);
    ctx.max_turns = Some(25);
    ctx.allowed_tools = Some(vec![]);
    ctx.model = Some(model_fast.to_string());

    match provider.complete(&ctx).await {
        Ok(resp) => parse_plan_response(&resp.text),
        Err(e) => {
            warn!("heartbeat classification failed, falling back to single call: {e}");
            None
        }
    }
}

/// Execute a single heartbeat group via Opus.
///
/// Returns `None` if HEARTBEAT_OK (nothing to report).
/// Returns `Some((text, elapsed_ms))` if content should be sent to the user.
#[allow(clippy::too_many_arguments)]
async fn execute_heartbeat_group(
    provider: Arc<dyn Provider>,
    model_complex: String,
    group_items: String,
    heartbeat_template: String,
    enrichment: String,
    system_prompt: String,
    skills: Vec<omega_skills::Skill>,
    memory: Store,
    sender_id: String,
    channel_name: String,
    interval: Arc<AtomicU64>,
) -> Option<(String, i64)> {
    let mut prompt = heartbeat_template.replace("{checklist}", &group_items);
    prompt.push_str(&enrichment);

    let mut ctx = Context::new(&prompt);
    ctx.system_prompt = system_prompt;
    ctx.model = Some(model_complex);
    ctx.mcp_servers = omega_skills::match_skill_triggers(&skills, &group_items);

    let started = Instant::now();
    let resp = match provider.complete(&ctx).await {
        Ok(r) => r,
        Err(e) => {
            error!("heartbeat: group execution failed: {e}");
            return None;
        }
    };
    let elapsed_ms = started.elapsed().as_millis() as i64;

    let text =
        process_heartbeat_markers(resp.text, &memory, &sender_id, &channel_name, &interval).await;

    // Evaluate HEARTBEAT_OK: strip formatting, check if only HEARTBEAT_OK remains.
    let cleaned: String = text.chars().filter(|c| *c != '*' && *c != '`').collect();
    let without_ok = cleaned.replace("HEARTBEAT_OK", "");
    if without_ok.trim().is_empty() {
        None
    } else {
        let text = text
            .replace("**HEARTBEAT_OK**", "")
            .replace("HEARTBEAT_OK", "");
        Some((text.trim().to_string(), elapsed_ms))
    }
}

/// Process all markers in a heartbeat response.
///
/// Handles: SCHEDULE, SCHEDULE_ACTION, heartbeat markers (interval, add/remove),
/// CANCEL_TASK, UPDATE_TASK, REWARD, LESSON. Returns the text with all markers stripped.
async fn process_heartbeat_markers(
    mut text: String,
    memory: &Store,
    sender_id: &str,
    channel_name: &str,
    interval: &Arc<AtomicU64>,
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
        apply_heartbeat_changes(&hb_actions);
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

    for reward_line in extract_all_rewards(&text) {
        if let Some((score, domain, lesson)) = parse_reward_line(&reward_line) {
            match memory
                .store_outcome(sender_id, &domain, score, &lesson, "heartbeat")
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
            match memory.store_lesson(sender_id, &domain, &rule).await {
                Ok(()) => info!("heartbeat lesson: {domain} | {rule}"),
                Err(e) => error!("heartbeat: failed to store lesson: {e}"),
            }
        }
    }
    text = strip_lesson_markers(&text);

    text
}

/// Build enrichment context from user facts and recent conversation summaries.
async fn build_enrichment(memory: &Store) -> String {
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
    if let Ok(lessons) = memory.get_all_lessons().await {
        if !lessons.is_empty() {
            enrichment.push_str("\n\nLearned behavioral rules:");
            for (domain, rule) in &lessons {
                enrichment.push_str(&format!("\n- [{domain}] {rule}"));
            }
        }
    }
    if let Ok(outcomes) = memory.get_all_recent_outcomes(24, 20).await {
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

/// Build the heartbeat system prompt (Identity + Soul + System + sandbox + time).
fn build_system_prompt(prompts: &Prompts, sandbox_prompt: &Option<String>) -> String {
    let mut system = format!(
        "{}\n\n{}\n\n{}",
        prompts.identity, prompts.soul, prompts.system
    );
    if let Some(ref sp) = sandbox_prompt {
        system.push_str("\n\n");
        system.push_str(sp);
    }
    system.push_str(&format!(
        "\n\nCurrent time: {}",
        chrono::Local::now().format("%Y-%m-%d %H:%M %Z")
    ));
    system
}

/// Audit and send a heartbeat result to the user.
#[allow(clippy::too_many_arguments)]
async fn send_heartbeat_result(
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
