//! Action markers: BUG_REPORT, SKILL_IMPROVE, ACTION_OUTCOME.

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
