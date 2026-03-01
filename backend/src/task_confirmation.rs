//! Task scheduling confirmation — anti-hallucination layer.
//!
//! When the AI emits SCHEDULE/SCHEDULE_ACTION markers, the gateway processes
//! them and collects results. This module formats those results into a
//! confirmation message sent AFTER the AI's response, ensuring users see
//! what was actually created (not what the AI claimed to create).

use crate::i18n;

/// Outcome of processing a single SCHEDULE or SCHEDULE_ACTION marker.
#[derive(Debug, Clone, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum MarkerResult {
    /// Task was successfully created in the database.
    TaskCreated {
        description: String,
        due_at: String,
        repeat: String,
        task_type: String,
    },
    /// Task creation failed (DB error, etc.).
    TaskFailed { description: String, reason: String },
    /// The marker line could not be parsed.
    TaskParseError { raw_line: String },
    /// Task was successfully cancelled.
    TaskCancelled { id_prefix: String },
    /// Task cancellation failed (no match or DB error).
    TaskCancelFailed { id_prefix: String, reason: String },
    /// Task was successfully updated.
    TaskUpdated { id_prefix: String },
    /// Task update failed (no match or DB error).
    TaskUpdateFailed { id_prefix: String, reason: String },
    /// Skill was successfully improved (lesson appended to SKILL.md).
    SkillImproved { skill_name: String, lesson: String },
    /// Skill improvement failed (skill not found, write error, etc.).
    SkillImproveFailed { skill_name: String, reason: String },
    /// Bug report was successfully logged to BUG.md.
    BugReported { description: String },
    /// Bug report logging failed.
    BugReportFailed { description: String, reason: String },
    /// Project was activated via marker (triggers persona greeting).
    ProjectActivated { name: String },
}

/// Check if two task descriptions are semantically similar using word overlap.
///
/// Extracts significant words (3+ chars, excluding stop words), then checks
/// if 50%+ of the smaller set overlaps with the larger set.
pub fn descriptions_are_similar(a: &str, b: &str) -> bool {
    let words_a = significant_words(a);
    let words_b = significant_words(b);

    if words_a.is_empty() || words_b.is_empty() {
        return false;
    }

    let (smaller, larger) = if words_a.len() <= words_b.len() {
        (&words_a, &words_b)
    } else {
        (&words_b, &words_a)
    };

    let overlap = smaller.iter().filter(|w| larger.contains(w)).count();
    let threshold = smaller.len().div_ceil(2);
    overlap >= threshold
}

/// Extract significant words from a description (lowercase, 3+ chars, no stop words).
fn significant_words(text: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "the", "and", "for", "that", "this", "with", "from", "are", "was", "were", "been", "have",
        "has", "had", "will", "would", "could", "should", "may", "might", "can", "about", "into",
        "over", "after", "before", "between", "under", "again", "then", "once", "daily", "weekly",
        "monthly",
    ];
    text.split(|c: char| !c.is_alphanumeric())
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() >= 3 && !STOP_WORDS.contains(&w.as_str()))
        .collect()
}

/// Format a human-readable confirmation of task scheduling results.
///
/// Returns `None` if there are no results to report.
pub fn format_task_confirmation(
    results: &[MarkerResult],
    similar_warnings: &[(String, String)],
    lang: &str,
) -> Option<String> {
    if results.is_empty() {
        return None;
    }

    let created: Vec<_> = results
        .iter()
        .filter_map(|r| {
            if let MarkerResult::TaskCreated {
                description,
                due_at,
                repeat,
                ..
            } = r
            {
                let repeat_label = if repeat == "once" {
                    i18n::t("once", lang).to_string()
                } else {
                    repeat.clone()
                };
                Some(format!("{description} — {due_at} ({repeat_label})"))
            } else {
                None
            }
        })
        .collect();

    let failed_count = results
        .iter()
        .filter(|r| {
            matches!(
                r,
                MarkerResult::TaskFailed { .. } | MarkerResult::TaskParseError { .. }
            )
        })
        .count();

    // Cancelled tasks
    let cancelled: Vec<_> = results
        .iter()
        .filter_map(|r| {
            if let MarkerResult::TaskCancelled { id_prefix } = r {
                Some(id_prefix.as_str())
            } else {
                None
            }
        })
        .collect();

    let cancel_failed: Vec<_> = results
        .iter()
        .filter_map(|r| {
            if let MarkerResult::TaskCancelFailed { id_prefix, reason } = r {
                Some((id_prefix.as_str(), reason.as_str()))
            } else {
                None
            }
        })
        .collect();

    // Updated tasks
    let updated: Vec<_> = results
        .iter()
        .filter_map(|r| {
            if let MarkerResult::TaskUpdated { id_prefix } = r {
                Some(id_prefix.as_str())
            } else {
                None
            }
        })
        .collect();

    let update_failed: Vec<_> = results
        .iter()
        .filter_map(|r| {
            if let MarkerResult::TaskUpdateFailed { id_prefix, reason } = r {
                Some((id_prefix.as_str(), reason.as_str()))
            } else {
                None
            }
        })
        .collect();

    let mut parts = Vec::new();

    // Created section
    match created.len() {
        0 => {}
        1 => parts.push(format!(
            "{} {}",
            i18n::t("task_confirmed", lang),
            created[0]
        )),
        n => {
            let mut lines = vec![i18n::tasks_confirmed(lang, n)];
            for item in &created {
                lines.push(format!("  • {item}"));
            }
            parts.push(lines.join("\n"));
        }
    }

    // Cancelled/Updated sections — only show when standalone (no creates in same batch).
    // When creates are present, cancellations are implicit replacements, not user-requested.
    let has_creates = !created.is_empty();

    if !has_creates {
        // Cancelled section
        match cancelled.len() {
            0 => {}
            1 => parts.push(format!(
                "{} [{}]",
                i18n::t("task_cancelled_confirmed", lang),
                cancelled[0]
            )),
            n => {
                let mut lines = vec![i18n::tasks_cancelled_confirmed(lang, n)];
                for id in &cancelled {
                    lines.push(format!("  • [{id}]"));
                }
                parts.push(lines.join("\n"));
            }
        }

        // Updated section
        match updated.len() {
            0 => {}
            1 => parts.push(format!(
                "{} [{}]",
                i18n::t("task_updated_confirmed", lang),
                updated[0]
            )),
            n => {
                let mut lines = vec![i18n::tasks_updated_confirmed(lang, n)];
                for id in &updated {
                    lines.push(format!("  • [{id}]"));
                }
                parts.push(lines.join("\n"));
            }
        }
    }

    // Similar task warnings
    for (desc, due) in similar_warnings {
        parts.push(format!(
            "{} \"{desc}\" — {due}",
            i18n::t("task_similar_warning", lang),
        ));
    }

    // Failure section
    if failed_count > 0 {
        parts.push(i18n::task_save_failed(lang, failed_count));
    }

    // Cancel/Update failure sections — only show when standalone (no creates).
    if !has_creates {
        for (id, reason) in &cancel_failed {
            parts.push(format!(
                "{} [{id}]: {reason}",
                i18n::t("task_cancel_failed", lang),
            ));
        }

        for (id, reason) in &update_failed {
            parts.push(format!(
                "{} [{id}]: {reason}",
                i18n::t("task_update_failed", lang),
            ));
        }
    }

    // Skill improvement results
    for r in results {
        match r {
            MarkerResult::SkillImproved { skill_name, lesson } => {
                parts.push(format!(
                    "{} {skill_name} — {lesson}",
                    i18n::t("skill_improved", lang),
                ));
            }
            MarkerResult::SkillImproveFailed { skill_name, reason } => {
                parts.push(format!(
                    "{} {skill_name}: {reason}",
                    i18n::t("skill_improve_failed", lang),
                ));
            }
            MarkerResult::BugReported { description } => {
                parts.push(format!("{} {description}", i18n::t("bug_reported", lang),));
            }
            MarkerResult::BugReportFailed {
                description,
                reason,
            } => {
                parts.push(format!(
                    "{} {description}: {reason}",
                    i18n::t("bug_report_failed", lang),
                ));
            }
            _ => {}
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptions_are_similar_exact_match() {
        assert!(descriptions_are_similar(
            "Cancel Hostinger VPS",
            "Cancel Hostinger VPS"
        ));
    }

    #[test]
    fn test_descriptions_are_similar_reworded() {
        assert!(descriptions_are_similar(
            "Cancel Hostinger VPS subscription",
            "Cancel the Hostinger VPS"
        ));
    }

    #[test]
    fn test_descriptions_are_similar_different() {
        assert!(!descriptions_are_similar(
            "Cancel Hostinger VPS",
            "Call the dentist"
        ));
    }

    #[test]
    fn test_descriptions_are_similar_empty() {
        assert!(!descriptions_are_similar("", "something"));
        assert!(!descriptions_are_similar("something", ""));
        assert!(!descriptions_are_similar("", ""));
    }

    #[test]
    fn test_descriptions_are_similar_short_words_ignored() {
        // "go to gym" has only "gym" as significant (3+ chars, not stop word)
        assert!(!descriptions_are_similar("go to gym", "go to store"));
    }

    #[test]
    fn test_descriptions_are_similar_case_insensitive() {
        assert!(descriptions_are_similar(
            "Cancel HOSTINGER VPS",
            "cancel hostinger vps"
        ));
    }

    #[test]
    fn test_format_task_confirmation_single_created() {
        let results = vec![MarkerResult::TaskCreated {
            description: "Call dentist".to_string(),
            due_at: "2026-02-25T10:00:00".to_string(),
            repeat: "once".to_string(),
            task_type: "reminder".to_string(),
        }];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("Call dentist"));
        assert!(msg.contains("2026-02-25T10:00:00"));
    }

    #[test]
    fn test_format_task_confirmation_multiple_created() {
        let results = vec![
            MarkerResult::TaskCreated {
                description: "Task A".to_string(),
                due_at: "2026-02-22T09:00:00".to_string(),
                repeat: "daily".to_string(),
                task_type: "reminder".to_string(),
            },
            MarkerResult::TaskCreated {
                description: "Task B".to_string(),
                due_at: "2026-02-25T10:00:00".to_string(),
                repeat: "once".to_string(),
                task_type: "reminder".to_string(),
            },
        ];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("Task A"));
        assert!(msg.contains("Task B"));
        assert!(msg.contains("2 tasks"));
    }

    #[test]
    fn test_format_task_confirmation_with_failure() {
        let results = vec![MarkerResult::TaskFailed {
            description: "Bad task".to_string(),
            reason: "DB error".to_string(),
        }];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("Failed"));
    }

    #[test]
    fn test_format_task_confirmation_with_similar_warning() {
        let results = vec![MarkerResult::TaskCreated {
            description: "Cancel VPS".to_string(),
            due_at: "2026-03-15T09:00:00".to_string(),
            repeat: "once".to_string(),
            task_type: "reminder".to_string(),
        }];
        let warnings = vec![("Cancel Hostinger VPS".to_string(), "Mar 15".to_string())];
        let msg = format_task_confirmation(&results, &warnings, "English").unwrap();
        assert!(msg.contains("Similar"));
        assert!(msg.contains("Cancel Hostinger VPS"));
    }

    #[test]
    fn test_format_task_confirmation_empty() {
        assert!(format_task_confirmation(&[], &[], "English").is_none());
    }

    #[test]
    fn test_format_task_confirmation_single_cancelled() {
        let results = vec![MarkerResult::TaskCancelled {
            id_prefix: "abc123".to_string(),
        }];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("cancelled"));
        assert!(msg.contains("abc123"));
    }

    #[test]
    fn test_format_task_confirmation_multiple_cancelled() {
        let results = vec![
            MarkerResult::TaskCancelled {
                id_prefix: "aaa".to_string(),
            },
            MarkerResult::TaskCancelled {
                id_prefix: "bbb".to_string(),
            },
            MarkerResult::TaskCancelled {
                id_prefix: "ccc".to_string(),
            },
        ];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("Cancelled 3 tasks"));
        assert!(msg.contains("aaa"));
        assert!(msg.contains("bbb"));
        assert!(msg.contains("ccc"));
    }

    #[test]
    fn test_format_task_confirmation_cancel_failed() {
        let results = vec![MarkerResult::TaskCancelFailed {
            id_prefix: "xyz".to_string(),
            reason: "no matching task".to_string(),
        }];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("Failed to cancel"));
        assert!(msg.contains("xyz"));
    }

    #[test]
    fn test_format_task_confirmation_single_updated() {
        let results = vec![MarkerResult::TaskUpdated {
            id_prefix: "abc123".to_string(),
        }];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("updated"));
        assert!(msg.contains("abc123"));
    }

    #[test]
    fn test_format_task_confirmation_mixed_suppresses_cancels() {
        // When creates and cancels happen together, cancels are implicit replacements.
        let results = vec![
            MarkerResult::TaskCreated {
                description: "New task".to_string(),
                due_at: "2026-03-01T09:00:00".to_string(),
                repeat: "once".to_string(),
                task_type: "reminder".to_string(),
            },
            MarkerResult::TaskCancelled {
                id_prefix: "old1".to_string(),
            },
            MarkerResult::TaskCancelled {
                id_prefix: "old2".to_string(),
            },
        ];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("Scheduled"));
        assert!(
            !msg.contains("Cancelled"),
            "cancels should be suppressed when creates are present"
        );
    }

    #[test]
    fn test_format_task_confirmation_skill_improved() {
        let results = vec![MarkerResult::SkillImproved {
            skill_name: "google-workspace".to_string(),
            lesson: "Always search by name and email".to_string(),
        }];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("google-workspace"));
        assert!(msg.contains("Always search by name and email"));
    }

    #[test]
    fn test_format_task_confirmation_skill_improve_failed() {
        let results = vec![MarkerResult::SkillImproveFailed {
            skill_name: "nonexistent".to_string(),
            reason: "skill not found".to_string(),
        }];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("nonexistent"));
        assert!(msg.contains("skill not found"));
    }

    #[test]
    fn test_format_task_confirmation_bug_reported() {
        let results = vec![MarkerResult::BugReported {
            description: "Cannot read own heartbeat interval".to_string(),
        }];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("Cannot read own heartbeat interval"));
        assert!(msg.contains("Bug logged"));
    }

    #[test]
    fn test_format_task_confirmation_bug_report_failed() {
        let results = vec![MarkerResult::BugReportFailed {
            description: "Some limitation".to_string(),
            reason: "write error".to_string(),
        }];
        let msg = format_task_confirmation(&results, &[], "English").unwrap();
        assert!(msg.contains("Some limitation"));
        assert!(msg.contains("write error"));
    }

    #[test]
    fn test_significant_words() {
        let words = significant_words("Cancel the Hostinger VPS subscription");
        assert!(words.contains(&"cancel".to_string()));
        assert!(words.contains(&"hostinger".to_string()));
        assert!(words.contains(&"vps".to_string()));
        assert!(words.contains(&"subscription".to_string()));
        assert!(!words.contains(&"the".to_string()));
    }
}
