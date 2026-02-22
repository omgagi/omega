//! Action markers: BUG_REPORT, SKILL_IMPROVE, ACTION_OUTCOME, REWARD, LESSON.

use super::{extract_inline_marker_value, strip_inline_marker};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// BUG_REPORT
// ---------------------------------------------------------------------------

/// Extract the first `BUG_REPORT:` value from response text.
/// Handles both standalone lines and inline markers.
pub fn extract_bug_report(text: &str) -> Option<String> {
    extract_inline_marker_value(text, "BUG_REPORT:")
}

/// Strip all `BUG_REPORT:` markers from response text (standalone or inline).
pub fn strip_bug_report(text: &str) -> String {
    strip_inline_marker(text, "BUG_REPORT:")
}

/// Append a bug report entry to `{data_dir}/BUG.md`, grouped by date.
///
/// Creates the file if missing. Adds a date header (`## YYYY-MM-DD`) when the
/// current date section does not yet exist, then appends the description as a
/// bulleted entry.
pub fn append_bug_report(data_dir: &str, description: &str) -> Result<(), String> {
    let path = PathBuf::from(data_dir).join("BUG.md");
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let date_header = format!("## {today}");

    let mut content = std::fs::read_to_string(&path).unwrap_or_default();

    // Ensure file header exists.
    if !content.contains("# OMEGA Bug Reports") {
        content = format!("# OMEGA Bug Reports\n\n{content}");
    }

    // Ensure today's date section exists.
    if !content.contains(&date_header) {
        // Trim trailing whitespace, then append date section.
        content = format!("{}\n\n{date_header}\n", content.trim_end());
    }

    // Append the entry after the date header.
    let entry = format!("- **{description}**\n");
    if let Some(pos) = content.find(&date_header) {
        let insert_pos = content[pos..]
            .find('\n')
            .map(|i| pos + i + 1)
            .unwrap_or(content.len());
        content.insert_str(insert_pos, &entry);
    }

    // Ensure parent directory exists.
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    std::fs::write(&path, &content).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// SKILL_IMPROVE
// ---------------------------------------------------------------------------

/// Extract the first `SKILL_IMPROVE:` line from response text.
pub fn extract_skill_improve(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("SKILL_IMPROVE:"))
        .map(|line| line.trim().to_string())
}

/// Parse a skill improve line: `SKILL_IMPROVE: skill_name | lesson`
pub fn parse_skill_improve_line(line: &str) -> Option<(String, String)> {
    let content = line.strip_prefix("SKILL_IMPROVE:")?.trim();
    let mut parts = content.splitn(2, '|');
    let skill_name = parts.next()?.trim();
    let lesson = parts.next()?.trim();
    if skill_name.is_empty() || lesson.is_empty() {
        return None;
    }
    Some((skill_name.to_string(), lesson.to_string()))
}

/// Apply a skill improvement by appending a lesson to the skill's SKILL.md file.
///
/// Creates a `## Lessons Learned` section if missing.
pub fn apply_skill_improve(data_dir: &str, skill_name: &str, lesson: &str) -> Result<(), String> {
    let skill_path = PathBuf::from(data_dir).join(format!("skills/{skill_name}/SKILL.md"));
    if !skill_path.exists() {
        return Err("skill not found".to_string());
    }
    let mut content =
        std::fs::read_to_string(&skill_path).map_err(|e| format!("read failed: {e}"))?;
    if let Some(pos) = content.find("## Lessons Learned") {
        let insert_pos = content[pos..]
            .find('\n')
            .map(|i| pos + i)
            .unwrap_or(content.len());
        content.insert_str(insert_pos, &format!("\n- {lesson}"));
    } else {
        content.push_str(&format!("\n\n## Lessons Learned\n- {lesson}\n"));
    }
    std::fs::write(&skill_path, &content).map_err(|e| format!("write failed: {e}"))
}

/// Strip all `SKILL_IMPROVE:` lines from response text.
pub fn strip_skill_improve(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("SKILL_IMPROVE:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// ACTION_OUTCOME
// ---------------------------------------------------------------------------

/// Result of an action task execution, parsed from the `ACTION_OUTCOME:` marker.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionOutcome {
    Success,
    Failed(String),
}

/// Extract the `ACTION_OUTCOME:` marker from response text.
///
/// Accepts `ACTION_OUTCOME: success` or `ACTION_OUTCOME: failed | <reason>`.
pub fn extract_action_outcome(text: &str) -> Option<ActionOutcome> {
    let value = extract_inline_marker_value(text, "ACTION_OUTCOME:")?;
    let lower = value.to_lowercase();
    if lower == "success" {
        Some(ActionOutcome::Success)
    } else if lower.starts_with("failed") {
        // Parse "failed | reason" or just "failed".
        let reason = value
            .split_once('|')
            .map(|(_, r)| r.trim().to_string())
            .unwrap_or_default();
        Some(ActionOutcome::Failed(reason))
    } else {
        None
    }
}

/// Strip the `ACTION_OUTCOME:` marker line from response text.
pub fn strip_action_outcome(text: &str) -> String {
    strip_inline_marker(text, "ACTION_OUTCOME:")
}

// ---------------------------------------------------------------------------
// REWARD
// ---------------------------------------------------------------------------

/// Extract all `REWARD:` lines from response text.
///
/// Format: `REWARD: +1|domain|lesson` or `REWARD: -1|domain|lesson`
pub fn extract_all_rewards(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.trim().starts_with("REWARD:"))
        .map(|line| line.trim().to_string())
        .collect()
}

/// Parse a reward line into `(score, domain, lesson)`.
///
/// Accepts `REWARD: +1|training|User completed calisthenics by 15:00`.
pub fn parse_reward_line(line: &str) -> Option<(i32, String, String)> {
    let content = line.strip_prefix("REWARD:")?.trim();
    let mut parts = content.splitn(3, '|');
    let score_str = parts.next()?.trim();
    let domain = parts.next()?.trim();
    let lesson = parts.next()?.trim();
    if domain.is_empty() || lesson.is_empty() {
        return None;
    }
    let score: i32 = score_str.parse().ok()?;
    if !(-1..=1).contains(&score) {
        return None;
    }
    Some((score, domain.to_string(), lesson.to_string()))
}

/// Strip all `REWARD:` lines from response text.
pub fn strip_reward_markers(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("REWARD:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// LESSON
// ---------------------------------------------------------------------------

/// Extract all `LESSON:` lines from response text.
///
/// Format: `LESSON: domain|rule`
pub fn extract_all_lessons(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.trim().starts_with("LESSON:"))
        .map(|line| line.trim().to_string())
        .collect()
}

/// Parse a lesson line into `(domain, rule)`.
///
/// Accepts `LESSON: training|User trains Saturday mornings, no need to nag after 12:00`.
pub fn parse_lesson_line(line: &str) -> Option<(String, String)> {
    let content = line.strip_prefix("LESSON:")?.trim();
    let mut parts = content.splitn(2, '|');
    let domain = parts.next()?.trim();
    let rule = parts.next()?.trim();
    if domain.is_empty() || rule.is_empty() {
        return None;
    }
    Some((domain.to_string(), rule.to_string()))
}

/// Strip all `LESSON:` lines from response text.
pub fn strip_lesson_markers(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("LESSON:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
