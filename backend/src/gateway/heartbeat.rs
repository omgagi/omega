//! Periodic heartbeat check-in loop.
//!
//! A fast Sonnet classification groups related checklist items by domain.
//! Each group gets its own focused Opus session **in parallel**.
//! Falls back to a single call when all items belong to the same domain.
//! After global heartbeat, runs project-specific heartbeats for active projects.

use super::heartbeat_helpers::{
    build_enrichment, build_system_prompt, process_heartbeat_markers, send_heartbeat_result,
};
use super::Gateway;
use crate::markers::*;
use omega_core::{
    config::{HeartbeatConfig, Prompts},
    context::Context,
    traits::{Channel, Provider},
};
use omega_memory::{audit::AuditLogger, Store};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};

/// Compute the next clock-aligned boundary in minutes-since-midnight.
///
/// Given `current_minute` (0..1439) and `interval` (e.g. 60), returns the
/// next boundary that is strictly after `current_minute`.
/// Example: current=541 (09:01), interval=60 → returns 600 (10:00).
pub(crate) fn next_clock_boundary(current_minute: u64, interval: u64) -> u64 {
    ((current_minute / interval) + 1) * interval
}

/// Compute seconds until the next clock-aligned boundary from a local timestamp.
fn secs_until_boundary(now: &chrono::DateTime<chrono::Local>, interval_mins: u64) -> u64 {
    use chrono::Timelike;
    let current_minute = u64::from(now.hour()) * 60 + u64::from(now.minute());
    let next = next_clock_boundary(current_minute, interval_mins);
    (next - current_minute) * 60 - u64::from(now.second())
}

/// Compute seconds until `active_start` (HH:MM) from the current local time.
///
/// If `active_start` is later today, returns the difference.
/// If it's already past, returns the duration until tomorrow's `active_start`.
fn secs_until_active_start(active_start: &str) -> u64 {
    use chrono::{Local, NaiveTime, Timelike};
    let now = Local::now();
    let start = NaiveTime::parse_from_str(active_start, "%H:%M")
        .unwrap_or_else(|_| NaiveTime::from_hms_opt(8, 0, 0).unwrap());

    let now_secs =
        u64::from(now.hour()) * 3600 + u64::from(now.minute()) * 60 + u64::from(now.second());
    let start_secs = u64::from(start.hour()) * 3600 + u64::from(start.minute()) * 60;

    if start_secs > now_secs {
        start_secs - now_secs
    } else {
        // Tomorrow's active_start.
        (24 * 3600 - now_secs) + start_secs
    }
}

impl Gateway {
    /// Background task: periodic heartbeat check-in.
    ///
    /// Skips the provider call entirely when no checklist is configured.
    /// After the global heartbeat, runs project-specific heartbeats for
    /// active projects that have their own HEARTBEAT.md.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn heartbeat_loop(
        provider: Arc<dyn Provider>,
        channels: HashMap<String, Arc<dyn Channel>>,
        config: HeartbeatConfig,
        prompts: Prompts,
        memory: Store,
        interval: Arc<AtomicU64>,
        model_complex: String,
        model_fast: String,
        skills: Vec<omega_skills::Skill>,
        audit: AuditLogger,
        provider_name: String,
        data_dir: String,
    ) {
        loop {
            let mins = interval.load(Ordering::Relaxed);

            // Quiet-hours jump-ahead: sleep directly to active_start instead
            // of waking every boundary just to check and skip.
            if !config.active_start.is_empty()
                && !config.active_end.is_empty()
                && !is_within_active_hours(&config.active_start, &config.active_end)
            {
                let secs = secs_until_active_start(&config.active_start);
                info!(
                    "heartbeat: quiet hours, sleeping until {} (~{}m)",
                    config.active_start,
                    secs / 60
                );
                tokio::time::sleep(std::time::Duration::from_secs(secs)).await;
                // After waking from a potentially long sleep (system sleep may
                // have delayed us further), re-check active hours from the top.
                continue;
            }

            // Clock-aligned sleep: fire at clean boundaries (e.g. :00, :30).
            let now = chrono::Local::now();
            let wait_secs = secs_until_boundary(&now, mins);
            let target_boundary = {
                use chrono::Timelike;
                let cm = u64::from(now.hour()) * 60 + u64::from(now.minute());
                next_clock_boundary(cm, mins)
            };
            tokio::time::sleep(std::time::Duration::from_secs(wait_secs)).await;

            // Wall-clock re-snap: if system sleep caused us to overshoot the
            // target boundary, recalculate from the actual wake-up time.
            let actual = chrono::Local::now();
            let actual_minute = {
                use chrono::Timelike;
                u64::from(actual.hour()) * 60 + u64::from(actual.minute())
            };
            // Allow ±2 minutes tolerance for normal jitter.
            // Normalize target_boundary for midnight wrap (1440 → 0).
            let target_normalized = target_boundary % 1440;
            let on_target = actual_minute >= target_normalized.saturating_sub(2)
                && actual_minute <= target_normalized.saturating_add(2);
            if !on_target {
                info!(
                    "heartbeat: system sleep detected (target :{:02}, actual :{:02}), re-aligning",
                    target_boundary % 60,
                    actual_minute % 60
                );
                // Don't fire now — loop back to recalculate from current time.
                continue;
            }

            // Re-check active hours after sleep (boundary might land outside).
            if !config.active_start.is_empty()
                && !config.active_end.is_empty()
                && !is_within_active_hours(&config.active_start, &config.active_end)
            {
                continue;
            }

            let cycle_start = chrono::Local::now();
            info!(
                "heartbeat: cycle started at {}",
                cycle_start.format("%H:%M")
            );

            let sender_id = &config.reply_target;
            let channel_name = &config.channel;

            // --- Global heartbeat ---
            if let Some(checklist) = read_heartbeat_file() {
                let enrichment = build_enrichment(&memory, None).await;
                let system = build_system_prompt(&prompts, None, None);

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
                            String::new(),
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
                                String::new(),
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
            } else {
                info!("heartbeat: no global checklist configured, skipping");
            }

            // --- Project heartbeats ---
            // Find active projects (users who have an active_project fact).
            let active_projects = memory
                .get_all_facts_by_key("active_project")
                .await
                .unwrap_or_default();

            // Deduplicate project names.
            let mut seen_projects = std::collections::HashSet::new();
            for (_sender_id, project_name) in &active_projects {
                if project_name.is_empty() || !seen_projects.insert(project_name.clone()) {
                    continue;
                }

                // Check if this project has its own HEARTBEAT.md.
                let project_checklist = match read_project_heartbeat_file(project_name) {
                    Some(cl) => cl,
                    None => continue,
                };

                info!("heartbeat: running project heartbeat for '{project_name}'");
                let enrichment = build_enrichment(&memory, Some(project_name)).await;
                let system = build_system_prompt(&prompts, Some(project_name), Some(&data_dir));

                // Project heartbeats always run as a single call (simpler, focused).
                let result = execute_heartbeat_group(
                    provider.clone(),
                    model_complex.clone(),
                    project_checklist,
                    prompts.heartbeat_checklist.clone(),
                    enrichment,
                    system,
                    skills.clone(),
                    memory.clone(),
                    sender_id.clone(),
                    channel_name.clone(),
                    interval.clone(),
                    project_name.clone(),
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

            let cycle_secs = chrono::Local::now()
                .signed_duration_since(cycle_start)
                .num_seconds();
            info!("heartbeat: cycle completed in {cycle_secs}s");
        }
    }
}

// --- Heartbeat helper functions ---

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
    project: String,
) -> Option<(String, i64)> {
    // Enrichment (facts, lessons, outcomes) goes BEFORE the checklist so learned
    // behavioral rules frame the AI's approach before it encounters detailed instructions.
    let mut prompt = enrichment;
    prompt.push('\n');
    prompt.push_str(&heartbeat_template.replace("{checklist}", &group_items));

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

    let text = process_heartbeat_markers(
        resp.text,
        &memory,
        &sender_id,
        &channel_name,
        &interval,
        &project,
    )
    .await;

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

#[cfg(test)]
mod tests {
    use super::*;

    // --- REQ-HB-005: Unit tests for next_clock_boundary ---

    #[test]
    fn test_boundary_at_exact_hour() {
        // At 09:00 (minute 540), next boundary with interval=60 → 10:00 (600)
        assert_eq!(next_clock_boundary(540, 60), 600);
    }

    #[test]
    fn test_boundary_mid_interval() {
        // At 09:01 (minute 541), next boundary with interval=60 → 10:00 (600)
        assert_eq!(next_clock_boundary(541, 60), 600);
    }

    #[test]
    fn test_boundary_near_midnight() {
        // At 23:50 (minute 1430), next boundary with interval=60 → 24:00 (1440)
        assert_eq!(next_clock_boundary(1430, 60), 1440);
    }

    #[test]
    fn test_boundary_30min_interval() {
        // At 00:15 (minute 15), next boundary with interval=30 → 00:30 (30)
        assert_eq!(next_clock_boundary(15, 30), 30);
    }

    #[test]
    fn test_boundary_non_divisor_interval() {
        // At 00:46 (minute 46), next boundary with interval=45 → 01:30 (90)
        // 46/45 = 1, (1+1)*45 = 90
        assert_eq!(next_clock_boundary(46, 45), 90);
    }

    #[test]
    fn test_boundary_1min_interval() {
        // At 00:00 (minute 0), next boundary with interval=1 → 00:01 (1)
        assert_eq!(next_clock_boundary(0, 1), 1);
    }

    #[test]
    fn test_boundary_at_half_hour() {
        // At 09:30 (minute 570), next boundary with interval=60 → 10:00 (600)
        assert_eq!(next_clock_boundary(570, 60), 600);
    }

    #[test]
    fn test_boundary_post_execution_realignment() {
        // REQ-HB-004: If heartbeat starts at 09:00 and takes 70 minutes
        // (finishes at 10:10, minute 610), next boundary = 11:00 (660)
        assert_eq!(next_clock_boundary(610, 60), 660);
    }

    #[test]
    fn test_boundary_post_execution_no_skip() {
        // REQ-HB-004: If heartbeat starts at 09:00 and takes 30 minutes
        // (finishes at 09:30, minute 570), next boundary = 10:00 (600)
        assert_eq!(next_clock_boundary(570, 60), 600);
    }

    #[test]
    fn test_boundary_midnight_wrap_normalized() {
        // At 23:50 (1430), next boundary = 1440 → normalized to 0 (midnight)
        let target = next_clock_boundary(1430, 60);
        assert_eq!(target, 1440);
        assert_eq!(target % 1440, 0); // normalizes to 0 for comparison
    }

    // --- secs_until_active_start ---

    #[test]
    fn test_secs_until_active_start_returns_positive() {
        // This is a smoke test — the result depends on current time but
        // must always be positive and ≤ 24 hours.
        let secs = secs_until_active_start("08:00");
        assert!(secs > 0);
        assert!(secs <= 24 * 3600);
    }
}
