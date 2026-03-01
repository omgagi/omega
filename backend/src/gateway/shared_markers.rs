//! Shared marker processing for CANCEL_TASK, UPDATE_TASK, REWARD, and LESSON.
//!
//! Deduplicated from process_markers.rs, scheduler_action.rs, and heartbeat_helpers.rs.

use crate::markers::*;
use crate::task_confirmation::MarkerResult;
use omega_memory::Store;
use tracing::{error, info, warn};

/// Process CANCEL_TASK, UPDATE_TASK, REWARD, and LESSON markers from response text.
///
/// Shared across the main pipeline, action tasks, and heartbeat processing.
/// `source` labels the origin for logging and outcome tracking (e.g. "conversation",
/// "action", "heartbeat").
pub(super) async fn process_task_and_learning_markers(
    text: &mut String,
    store: &Store,
    sender_id: &str,
    project: &str,
    source: &str,
) -> Vec<MarkerResult> {
    let mut results = Vec::new();

    // CANCEL_TASK — process ALL markers.
    for id_prefix in extract_all_cancel_tasks(text) {
        match store.cancel_task(&id_prefix, sender_id).await {
            Ok(true) => {
                info!("{source}: cancelled task {id_prefix}");
                results.push(MarkerResult::TaskCancelled {
                    id_prefix: id_prefix.clone(),
                });
            }
            Ok(false) => {
                warn!("{source}: no matching task to cancel: {id_prefix}");
                results.push(MarkerResult::TaskCancelFailed {
                    id_prefix: id_prefix.clone(),
                    reason: "no matching task".to_string(),
                });
            }
            Err(e) => {
                error!("{source}: failed to cancel task: {e}");
                results.push(MarkerResult::TaskCancelFailed {
                    id_prefix: id_prefix.clone(),
                    reason: e.to_string(),
                });
            }
        }
    }
    *text = strip_cancel_task(text);

    // UPDATE_TASK — process ALL markers.
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
                Ok(true) => {
                    info!("{source}: updated task {id_prefix}");
                    results.push(MarkerResult::TaskUpdated {
                        id_prefix: id_prefix.clone(),
                    });
                }
                Ok(false) => {
                    warn!("{source}: no matching task to update: {id_prefix}");
                    results.push(MarkerResult::TaskUpdateFailed {
                        id_prefix: id_prefix.clone(),
                        reason: "no matching task".to_string(),
                    });
                }
                Err(e) => {
                    error!("{source}: failed to update task: {e}");
                    results.push(MarkerResult::TaskUpdateFailed {
                        id_prefix: id_prefix.clone(),
                        reason: e.to_string(),
                    });
                }
            }
        }
    }
    *text = strip_update_task(text);

    // REWARD — process ALL markers (project-tagged).
    for reward_line in extract_all_rewards(text) {
        if let Some((score, domain, lesson)) = parse_reward_line(&reward_line) {
            match store
                .store_outcome(sender_id, &domain, score, &lesson, source, project)
                .await
            {
                Ok(()) => info!("{source} outcome: {score:+} | {domain} | {lesson}"),
                Err(e) => error!("{source}: failed to store outcome: {e}"),
            }
        }
    }
    *text = strip_reward_markers(text);

    // LESSON — process ALL markers (project-tagged).
    for lesson_line in extract_all_lessons(text) {
        if let Some((domain, rule)) = parse_lesson_line(&lesson_line) {
            match store.store_lesson(sender_id, &domain, &rule, project).await {
                Ok(()) => info!("{source} lesson: {domain} | {rule}"),
                Err(e) => error!("{source}: failed to store lesson: {e}"),
            }
        }
    }
    *text = strip_lesson_markers(text);

    results
}
