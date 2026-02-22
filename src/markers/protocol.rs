//! Simple protocol markers: LANG_SWITCH, PERSONALITY, FORGET_CONVERSATION,
//! CANCEL_TASK, UPDATE_TASK, PURGE_FACTS, PROJECT, WHATSAPP_QR.

use super::{extract_inline_marker_value, strip_inline_marker};

// ---------------------------------------------------------------------------
// LANG_SWITCH
// ---------------------------------------------------------------------------

/// Extract the language from a `LANG_SWITCH:` marker in response text.
/// Handles both standalone lines and inline markers.
pub fn extract_lang_switch(text: &str) -> Option<String> {
    extract_inline_marker_value(text, "LANG_SWITCH:")
}

/// Strip all `LANG_SWITCH:` markers from response text (standalone or inline).
pub fn strip_lang_switch(text: &str) -> String {
    strip_inline_marker(text, "LANG_SWITCH:")
}

// ---------------------------------------------------------------------------
// PERSONALITY
// ---------------------------------------------------------------------------

/// Extract the personality value from a `PERSONALITY:` marker in response text.
/// Handles both standalone lines and inline markers.
pub fn extract_personality(text: &str) -> Option<String> {
    extract_inline_marker_value(text, "PERSONALITY:")
}

/// Strip all `PERSONALITY:` markers from response text (standalone or inline).
pub fn strip_personality(text: &str) -> String {
    strip_inline_marker(text, "PERSONALITY:")
}

// ---------------------------------------------------------------------------
// FORGET_CONVERSATION
// ---------------------------------------------------------------------------

/// Check if response text contains a `FORGET_CONVERSATION` marker line.
pub fn has_forget_marker(text: &str) -> bool {
    text.lines()
        .any(|line| line.trim() == "FORGET_CONVERSATION")
}

/// Strip all `FORGET_CONVERSATION` lines from response text.
pub fn strip_forget_marker(text: &str) -> String {
    text.lines()
        .filter(|line| line.trim() != "FORGET_CONVERSATION")
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// CANCEL_TASK
// ---------------------------------------------------------------------------

/// Extract ALL `CANCEL_TASK:` ID prefixes from response text.
pub fn extract_all_cancel_tasks(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.trim().starts_with("CANCEL_TASK:"))
        .filter_map(|line| {
            let val = line.trim().strip_prefix("CANCEL_TASK:")?.trim().to_string();
            if val.is_empty() {
                None
            } else {
                Some(val)
            }
        })
        .collect()
}

/// Strip all `CANCEL_TASK:` lines from response text.
pub fn strip_cancel_task(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("CANCEL_TASK:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// UPDATE_TASK
// ---------------------------------------------------------------------------

/// Extract ALL `UPDATE_TASK:` lines from response text.
pub fn extract_all_update_tasks(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.trim().starts_with("UPDATE_TASK:"))
        .map(|line| line.trim().to_string())
        .collect()
}

/// Parse an update task line: `UPDATE_TASK: id | desc | due_at | repeat`.
///
/// Empty fields (between pipes) are returned as `None`, meaning "keep existing".
#[allow(clippy::type_complexity)]
pub fn parse_update_task_line(
    line: &str,
) -> Option<(String, Option<String>, Option<String>, Option<String>)> {
    let content = line.strip_prefix("UPDATE_TASK:")?.trim();
    let parts: Vec<&str> = content.splitn(4, '|').collect();
    if parts.len() != 4 {
        return None;
    }
    let id = parts[0].trim().to_string();
    if id.is_empty() {
        return None;
    }
    let desc = non_empty_field(parts[1]);
    let due_at = non_empty_field(parts[2]);
    let repeat = non_empty_field(parts[3]);
    Some((id, desc, due_at, repeat))
}

/// Return `Some(trimmed)` if the field is non-empty after trimming, else `None`.
fn non_empty_field(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Strip all `UPDATE_TASK:` lines from response text.
pub fn strip_update_task(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("UPDATE_TASK:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// PURGE_FACTS
// ---------------------------------------------------------------------------

/// Check if response text contains a `PURGE_FACTS` marker line.
pub fn has_purge_marker(text: &str) -> bool {
    text.lines().any(|line| line.trim() == "PURGE_FACTS")
}

/// Strip all `PURGE_FACTS` lines from response text.
pub fn strip_purge_marker(text: &str) -> String {
    text.lines()
        .filter(|line| line.trim() != "PURGE_FACTS")
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// PROJECT_ACTIVATE / PROJECT_DEACTIVATE
// ---------------------------------------------------------------------------

/// Extract project name from a `PROJECT_ACTIVATE: <name>` marker line.
pub fn extract_project_activate(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("PROJECT_ACTIVATE:"))
        .and_then(|line| {
            let name = line
                .trim()
                .strip_prefix("PROJECT_ACTIVATE:")?
                .trim()
                .to_string();
            if name.is_empty() {
                None
            } else {
                Some(name)
            }
        })
}

/// Check if response text contains a `PROJECT_DEACTIVATE` marker line.
pub fn has_project_deactivate(text: &str) -> bool {
    text.lines().any(|line| line.trim() == "PROJECT_DEACTIVATE")
}

/// Strip all `PROJECT_ACTIVATE:` and `PROJECT_DEACTIVATE` lines from response text.
pub fn strip_project_markers(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let t = line.trim();
            !t.starts_with("PROJECT_ACTIVATE:") && t != "PROJECT_DEACTIVATE"
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// WHATSAPP_QR
// ---------------------------------------------------------------------------

/// Check if response text contains a `WHATSAPP_QR` marker line.
pub fn has_whatsapp_qr_marker(text: &str) -> bool {
    text.lines().any(|line| line.trim() == "WHATSAPP_QR")
}

/// Strip all `WHATSAPP_QR` lines from response text.
pub fn strip_whatsapp_qr_marker(text: &str) -> String {
    text.lines()
        .filter(|line| line.trim() != "WHATSAPP_QR")
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}
