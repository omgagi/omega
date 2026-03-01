//! Marker extraction and processing from provider responses.

use super::keywords::SYSTEM_FACT_KEYS;
use super::Gateway;
use crate::markers::*;
use crate::task_confirmation::{self, MarkerResult};
use omega_core::{config, config::shellexpand, message::IncomingMessage};
use std::sync::atomic::Ordering;
use tracing::{error, info, warn};

impl Gateway {
    /// Extract and process all markers from a provider response text.
    ///
    /// Handles: SCHEDULE, SCHEDULE_ACTION, PROJECT_ACTIVATE/DEACTIVATE,
    /// BUILD_PROPOSAL, WHATSAPP_QR, LANG_SWITCH, HEARTBEAT_ADD/REMOVE, SKILL_IMPROVE, BUG_REPORT.
    /// Strips processed markers from the text.
    pub(super) async fn process_markers(
        &self,
        incoming: &IncomingMessage,
        text: &mut String,
        active_project: Option<&str>,
    ) -> Vec<MarkerResult> {
        let project = active_project.unwrap_or("");
        let mut marker_results = Vec::new();

        // SCHEDULE — process ALL markers
        for schedule_line in extract_all_schedule_markers(text) {
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
                        project,
                    )
                    .await
                {
                    Ok(id) => {
                        info!("scheduled task {id}: {desc} at {due_at}");
                        marker_results.push(MarkerResult::TaskCreated {
                            description: desc,
                            due_at,
                            repeat,
                            task_type: "reminder".to_string(),
                        });
                    }
                    Err(e) => {
                        error!("failed to create scheduled task: {e}");
                        marker_results.push(MarkerResult::TaskFailed {
                            description: desc,
                            reason: e.to_string(),
                        });
                    }
                }
            } else {
                marker_results.push(MarkerResult::TaskParseError {
                    raw_line: schedule_line,
                });
            }
        }
        *text = strip_schedule_marker(text);

        // SCHEDULE_ACTION — process ALL markers
        for sched_line in extract_all_schedule_action_markers(text) {
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
                        project,
                    )
                    .await
                {
                    Ok(id) => {
                        info!("scheduled action task {id}: {desc} at {due_at}");
                        marker_results.push(MarkerResult::TaskCreated {
                            description: desc,
                            due_at,
                            repeat,
                            task_type: "action".to_string(),
                        });
                    }
                    Err(e) => {
                        error!("failed to create action task: {e}");
                        marker_results.push(MarkerResult::TaskFailed {
                            description: desc,
                            reason: e.to_string(),
                        });
                    }
                }
            } else {
                marker_results.push(MarkerResult::TaskParseError {
                    raw_line: sched_line,
                });
            }
        }
        *text = strip_schedule_action_markers(text);

        // PROJECT_DEACTIVATE must run BEFORE PROJECT_ACTIVATE so that combined
        // markers (deactivate old + activate new) read the old project name first.
        let fresh_projects = omega_skills::load_projects(&self.data_dir);
        if has_project_deactivate(text) {
            // Create .disabled marker for current project to stop its heartbeat.
            if let Ok(Some(current)) = self
                .memory
                .get_fact(&incoming.sender_id, "active_project")
                .await
            {
                if let Some(proj) = fresh_projects.iter().find(|p| p.name == current) {
                    let _ = std::fs::write(proj.path.join(".disabled"), "");
                }
            }
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
        if let Some(project_name) = extract_project_activate(text) {
            if omega_skills::get_project_instructions(&fresh_projects, &project_name).is_some() {
                // Remove .disabled marker so heartbeat runs for this project.
                if let Some(proj) = fresh_projects.iter().find(|p| p.name == project_name) {
                    let _ = std::fs::remove_file(proj.path.join(".disabled"));
                }
                if let Err(e) = self
                    .memory
                    .store_fact(&incoming.sender_id, "active_project", &project_name)
                    .await
                {
                    error!("failed to activate project {project_name}: {e}");
                } else {
                    info!("project activated: {project_name}");
                    marker_results.push(MarkerResult::ProjectActivated {
                        name: project_name.clone(),
                    });
                }
            } else {
                warn!("project activate marker for unknown project: {project_name}");
            }
            *text = strip_project_markers(text);
        }

        // BUILD_PROPOSAL — OMEGA suggests a build; store as pending so user can confirm.
        if let Some(description) = extract_build_proposal(text) {
            let stamped = format!("{}|{}", chrono::Utc::now().timestamp(), description);
            if let Err(e) = self
                .memory
                .store_fact(&incoming.sender_id, "pending_build_request", &stamped)
                .await
            {
                error!("failed to store build proposal: {e}");
            } else {
                info!(
                    "build proposal stored for {}: {}",
                    incoming.sender_id, description
                );
            }
            *text = strip_build_proposal(text);
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
                .close_current_conversation(&incoming.channel, &incoming.sender_id, project)
                .await
            {
                Ok(true) => info!("conversation cleared via marker for {}", incoming.sender_id),
                Ok(false) => {
                    info!("no active conversation to clear for {}", incoming.sender_id)
                }
                Err(e) => error!("failed to clear conversation via marker: {e}"),
            }
            // Clear CLI session — next message starts fresh.
            let _ = self
                .memory
                .clear_session(&incoming.channel, &incoming.sender_id, project)
                .await;
            *text = strip_forget_marker(text);
        }

        // PURGE_FACTS
        if has_purge_marker(text) {
            self.process_purge_facts(&incoming.sender_id).await;
            *text = strip_purge_marker(text);
        }

        // HEARTBEAT_ADD / HEARTBEAT_REMOVE / HEARTBEAT_INTERVAL
        let heartbeat_actions = extract_heartbeat_markers(text);
        if !heartbeat_actions.is_empty() {
            let hb_project = if project.is_empty() {
                None
            } else {
                Some(project)
            };
            apply_heartbeat_changes(&heartbeat_actions, hb_project);
            for action in &heartbeat_actions {
                match action {
                    HeartbeatAction::Add(item) => info!("heartbeat: added '{item}' to checklist"),
                    HeartbeatAction::Remove(item) => {
                        info!("heartbeat: removed '{item}' from checklist")
                    }
                    HeartbeatAction::SetInterval(mins) => {
                        self.heartbeat_interval.store(*mins, Ordering::Relaxed);
                        config::patch_heartbeat_interval(&self.config_path, *mins);
                        self.heartbeat_notify.notify_one();
                        info!("heartbeat: interval changed to {mins} minutes");
                    }
                }
            }
            *text = strip_heartbeat_markers(text);
        }

        // HEARTBEAT_SUPPRESS_SECTION / HEARTBEAT_UNSUPPRESS_SECTION
        let suppress_actions = extract_suppress_section_markers(text);
        if !suppress_actions.is_empty() {
            let hb_project = if project.is_empty() {
                None
            } else {
                Some(project)
            };
            apply_suppress_actions(&suppress_actions, hb_project);
            *text = strip_suppress_section_markers(text);
        }

        // SKILL_IMPROVE + BUG_REPORT
        self.process_improvement_markers(text, &mut marker_results);

        // CANCEL_TASK + UPDATE_TASK + REWARD + LESSON (shared across pipelines).
        let shared_results = super::shared_markers::process_task_and_learning_markers(
            text,
            &self.memory,
            &incoming.sender_id,
            project,
            "conversation",
        )
        .await;
        marker_results.extend(shared_results);

        // Safety net: strip any markers still remaining (catches inline markers
        // from small models that don't put them on their own line).
        *text = strip_all_remaining_markers(text);

        marker_results
    }

    /// Send task scheduling confirmation after processing markers.
    ///
    /// Checks for similar existing tasks and formats a confirmation message
    /// with the actual results from the database (anti-hallucination).
    pub(super) async fn send_task_confirmation(
        &self,
        incoming: &IncomingMessage,
        marker_results: &[MarkerResult],
    ) {
        // Resolve language for i18n.
        let lang = self
            .memory
            .get_fact(&incoming.sender_id, "preferred_language")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "English".to_string());

        // Check for similar existing tasks (only against tasks that existed
        // BEFORE this batch — exclude descriptions we just created).
        let just_created: std::collections::HashSet<&str> = marker_results
            .iter()
            .filter_map(|r| match r {
                MarkerResult::TaskCreated { description, .. } => Some(description.as_str()),
                _ => None,
            })
            .collect();

        let mut similar_warnings = Vec::new();
        let mut seen_warnings = std::collections::HashSet::new();
        if let Ok(existing_tasks) = self.memory.get_tasks_for_sender(&incoming.sender_id).await {
            for (_, existing_desc, existing_due, _, _, _) in &existing_tasks {
                // Skip tasks we just created in this batch.
                if just_created.contains(existing_desc.as_str()) {
                    continue;
                }
                // Check if any newly created task is similar to this existing one.
                let is_similar = just_created.iter().any(|new_desc| {
                    task_confirmation::descriptions_are_similar(new_desc, existing_desc)
                });
                if is_similar && seen_warnings.insert(existing_desc.clone()) {
                    similar_warnings.push((existing_desc.clone(), existing_due.clone()));
                }
            }
        }

        if let Some(confirmation) =
            task_confirmation::format_task_confirmation(marker_results, &similar_warnings, &lang)
        {
            self.send_text(incoming, &confirmation).await;
        }
    }

    /// Process SKILL_IMPROVE and BUG_REPORT markers.
    fn process_improvement_markers(
        &self,
        text: &mut String,
        marker_results: &mut Vec<MarkerResult>,
    ) {
        if let Some(improve_line) = extract_skill_improve(text) {
            if let Some((skill_name, lesson)) = parse_skill_improve_line(&improve_line) {
                let data_dir = shellexpand(&self.data_dir);
                match apply_skill_improve(&data_dir, &skill_name, &lesson) {
                    Ok(()) => {
                        info!("skill improved: {skill_name} — {lesson}");
                        marker_results.push(MarkerResult::SkillImproved { skill_name, lesson });
                    }
                    Err(reason) => {
                        error!("skill improve failed: {skill_name}: {reason}");
                        marker_results
                            .push(MarkerResult::SkillImproveFailed { skill_name, reason });
                    }
                }
            }
            *text = strip_skill_improve(text);
        }
        if let Some(description) = extract_bug_report(text) {
            let data_dir = shellexpand(&self.data_dir);
            match append_bug_report(&data_dir, &description) {
                Ok(()) => {
                    info!("bug reported: {description}");
                    marker_results.push(MarkerResult::BugReported { description });
                }
                Err(e) => {
                    error!("bug report: failed to write BUG.md: {e}");
                    marker_results.push(MarkerResult::BugReportFailed {
                        description,
                        reason: e,
                    });
                }
            }
            *text = strip_bug_report(text);
        }
    }

    /// Purge all non-system facts for a sender, preserving system-managed keys.
    async fn process_purge_facts(&self, sender_id: &str) {
        let preserved: Vec<(String, String)> = match self.memory.get_facts(sender_id).await {
            Ok(facts) => facts
                .into_iter()
                .filter(|(k, _)| SYSTEM_FACT_KEYS.contains(&k.as_str()))
                .collect(),
            Err(e) => {
                error!("purge marker: failed to read facts: {e}");
                return;
            }
        };
        match self.memory.delete_facts(sender_id, None).await {
            Ok(n) => {
                for (key, value) in &preserved {
                    let _ = self.memory.store_fact(sender_id, key, value).await;
                }
                let purged = n as usize - preserved.len();
                info!("purged {purged} facts via marker for {sender_id}");
            }
            Err(e) => error!("purge marker: failed to delete facts: {e}"),
        }
    }
}
