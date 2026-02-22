//! Marker extraction, parsing, and stripping for the gateway protocol.
//!
//! Split into focused submodules:
//! - `schedule` — SCHEDULE and SCHEDULE_ACTION markers
//! - `protocol` — Simple markers (LANG_SWITCH, PERSONALITY, FORGET, CANCEL_TASK, etc.)
//! - `heartbeat` — Heartbeat markers and file operations
//! - `actions` — BUG_REPORT, SKILL_IMPROVE, ACTION_OUTCOME
//! - `helpers` — Status messages, workspace images, inbox, classification

mod actions;
mod heartbeat;
mod helpers;
mod protocol;
mod schedule;

pub use actions::*;
pub use heartbeat::*;
pub use helpers::*;
pub use protocol::*;
pub use schedule::*;

// ---------------------------------------------------------------------------
// Generic inline marker helpers
// ---------------------------------------------------------------------------

/// Extract the value after a marker prefix, searching both line-start and inline.
/// Returns the text from the marker to end of line, trimmed.
pub fn extract_inline_marker_value(text: &str, prefix: &str) -> Option<String> {
    // First try line-start match (most common with capable models).
    if let Some(val) = text
        .lines()
        .find(|line| line.trim().starts_with(prefix))
        .and_then(|line| {
            let v = line.trim().strip_prefix(prefix)?.trim().to_string();
            if v.is_empty() {
                None
            } else {
                Some(v)
            }
        })
    {
        return Some(val);
    }
    // Fallback: inline match (small models put markers mid-sentence).
    text.find(prefix).and_then(|pos| {
        let after = &text[pos + prefix.len()..];
        let end = after.find('\n').unwrap_or(after.len());
        let val = after[..end].trim().to_string();
        if val.is_empty() {
            None
        } else {
            Some(val)
        }
    })
}

/// Strip a marker (and everything after it to end of line) from text.
/// If marker is at line start, removes the whole line.
/// If marker is inline, removes from marker to end of line, keeping text before it.
pub fn strip_inline_marker(text: &str, prefix: &str) -> String {
    let mut result = text.to_string();
    while let Some(pos) = result.find(prefix) {
        let line_end = result[pos..]
            .find('\n')
            .map(|i| pos + i)
            .unwrap_or(result.len());
        let line_start = result[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0);
        let before = result[line_start..pos].trim();
        if before.is_empty() {
            // Marker at line start — remove entire line (including newline).
            let remove_end = if line_end < result.len() {
                line_end + 1
            } else {
                line_end
            };
            result = format!("{}{}", &result[..line_start], &result[remove_end..]);
        } else {
            // Marker inline — remove from marker to end of line, keep text before.
            result = format!("{}{}", result[..pos].trim_end(), &result[line_end..]);
        }
    }
    result.trim().to_string()
}

/// Safety net: strip any known marker that still appears in the text.
/// Called at the end of `process_markers()` to catch markers that individual
/// strip functions missed (e.g. inline markers from small models).
pub fn strip_all_remaining_markers(text: &str) -> String {
    const MARKERS: &[&str] = &[
        "LANG_SWITCH:",
        "PERSONALITY:",
        "CANCEL_TASK:",
        "UPDATE_TASK:",
        "PROJECT_ACTIVATE:",
        "PROJECT_DEACTIVATE",
        "SCHEDULE_ACTION:",
        "SCHEDULE:",
        "HEARTBEAT_ADD:",
        "HEARTBEAT_REMOVE:",
        "HEARTBEAT_INTERVAL:",
        "SKILL_IMPROVE:",
        "BUG_REPORT:",
        "FORGET_CONVERSATION",
        "PURGE_FACTS",
        "WHATSAPP_QR",
        "ACTION_OUTCOME:",
    ];
    let mut result = text.to_string();
    for marker in MARKERS {
        if result.contains(marker) {
            result = strip_inline_marker(&result, marker);
        }
    }
    result
}

#[cfg(test)]
mod tests;
