//! Marker extraction, parsing, and stripping for the gateway protocol.
//!
//! All system markers (SCHEDULE:, LANG_SWITCH:, SKILL_IMPROVE:, etc.) are emitted
//! by the AI in response text and processed here. This module centralizes the
//! extract/strip/parse functions that were previously scattered in gateway.rs.

use std::path::PathBuf;

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
    ];
    let mut result = text.to_string();
    for marker in MARKERS {
        if result.contains(marker) {
            result = strip_inline_marker(&result, marker);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// SCHEDULE
// ---------------------------------------------------------------------------

/// Extract the first `SCHEDULE:` line from response text.
#[allow(dead_code)]
pub fn extract_schedule_marker(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("SCHEDULE:"))
        .map(|line| line.trim().to_string())
}

/// Extract ALL `SCHEDULE:` lines from response text.
pub fn extract_all_schedule_markers(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.trim().starts_with("SCHEDULE:"))
        .map(|line| line.trim().to_string())
        .collect()
}

/// Parse a schedule line: `SCHEDULE: desc | ISO datetime | repeat`
pub fn parse_schedule_line(line: &str) -> Option<(String, String, String)> {
    let content = line.strip_prefix("SCHEDULE:")?.trim();
    let parts: Vec<&str> = content.splitn(3, '|').collect();
    if parts.len() != 3 {
        return None;
    }
    let desc = parts[0].trim().to_string();
    let due_at = parts[1].trim().to_string();
    let repeat = parts[2].trim().to_lowercase();
    if desc.is_empty() || due_at.is_empty() {
        return None;
    }
    Some((desc, due_at, repeat))
}

/// Strip all `SCHEDULE:` lines from response text.
pub fn strip_schedule_marker(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("SCHEDULE:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// SCHEDULE_ACTION
// ---------------------------------------------------------------------------

/// Extract the first `SCHEDULE_ACTION:` line from response text.
#[allow(dead_code)]
pub fn extract_schedule_action_marker(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("SCHEDULE_ACTION:"))
        .map(|line| line.trim().to_string())
}

/// Extract ALL `SCHEDULE_ACTION:` lines from response text.
pub fn extract_all_schedule_action_markers(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.trim().starts_with("SCHEDULE_ACTION:"))
        .map(|line| line.trim().to_string())
        .collect()
}

/// Parse a schedule action line: `SCHEDULE_ACTION: desc | ISO datetime | repeat`
pub fn parse_schedule_action_line(line: &str) -> Option<(String, String, String)> {
    let content = line.strip_prefix("SCHEDULE_ACTION:")?.trim();
    let parts: Vec<&str> = content.splitn(3, '|').collect();
    if parts.len() != 3 {
        return None;
    }
    let desc = parts[0].trim().to_string();
    let due_at = parts[1].trim().to_string();
    let repeat = parts[2].trim().to_lowercase();
    if desc.is_empty() || due_at.is_empty() {
        return None;
    }
    Some((desc, due_at, repeat))
}

/// Strip all `SCHEDULE_ACTION:` lines from response text.
pub fn strip_schedule_action_markers(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("SCHEDULE_ACTION:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

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

// ---------------------------------------------------------------------------
// HEARTBEAT_ADD / HEARTBEAT_REMOVE / HEARTBEAT_INTERVAL
// ---------------------------------------------------------------------------

/// Action extracted from a `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, or `HEARTBEAT_INTERVAL:` marker.
#[derive(Debug, Clone, PartialEq)]
pub enum HeartbeatAction {
    Add(String),
    Remove(String),
    /// Dynamically change the heartbeat interval (in minutes, 1–1440).
    SetInterval(u64),
}

/// Extract all `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, and `HEARTBEAT_INTERVAL:` markers from response text.
pub fn extract_heartbeat_markers(text: &str) -> Vec<HeartbeatAction> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if let Some(item) = trimmed.strip_prefix("HEARTBEAT_ADD:") {
                let item = item.trim();
                if item.is_empty() {
                    None
                } else {
                    Some(HeartbeatAction::Add(item.to_string()))
                }
            } else if let Some(item) = trimmed.strip_prefix("HEARTBEAT_REMOVE:") {
                let item = item.trim();
                if item.is_empty() {
                    None
                } else {
                    Some(HeartbeatAction::Remove(item.to_string()))
                }
            } else if let Some(val) = trimmed.strip_prefix("HEARTBEAT_INTERVAL:") {
                val.trim()
                    .parse::<u64>()
                    .ok()
                    .filter(|m| (1..=1440).contains(m))
                    .map(HeartbeatAction::SetInterval)
            } else {
                None
            }
        })
        .collect()
}

/// Strip all `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, and `HEARTBEAT_INTERVAL:` lines from response text.
pub fn strip_heartbeat_markers(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("HEARTBEAT_ADD:")
                && !trimmed.starts_with("HEARTBEAT_REMOVE:")
                && !trimmed.starts_with("HEARTBEAT_INTERVAL:")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Read `~/.omega/HEARTBEAT.md` if it exists.
pub fn read_heartbeat_file() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = format!("{home}/.omega/HEARTBEAT.md");
    let content = std::fs::read_to_string(path).ok()?;
    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}

/// Apply heartbeat add/remove actions to `~/.omega/HEARTBEAT.md`.
///
/// Creates the file if missing. Prevents duplicate adds. Uses case-insensitive
/// partial matching for removes. Skips comment lines (`#`) during removal.
pub fn apply_heartbeat_changes(actions: &[HeartbeatAction]) {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return,
    };
    let path = format!("{home}/.omega/HEARTBEAT.md");

    // Read existing lines (or start empty).
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap_or_default()
        .lines()
        .map(|l| l.to_string())
        .collect();

    for action in actions {
        match action {
            HeartbeatAction::Add(item) => {
                // Prevent duplicates (case-insensitive).
                let already_exists = lines.iter().any(|l| {
                    let trimmed = l.trim();
                    !trimmed.starts_with('#')
                        && trimmed.trim_start_matches("- ").eq_ignore_ascii_case(item)
                });
                if !already_exists {
                    lines.push(format!("- {item}"));
                }
            }
            HeartbeatAction::Remove(item) => {
                let needle = item.to_lowercase();
                lines.retain(|l| {
                    let trimmed = l.trim();
                    // Never remove comment lines.
                    if trimmed.starts_with('#') {
                        return true;
                    }
                    let content = trimmed.trim_start_matches("- ").to_lowercase();
                    // Remove if content contains the needle (partial match).
                    !content.contains(&needle)
                });
            }
            // SetInterval is handled by process_markers / scheduler_loop, not here.
            HeartbeatAction::SetInterval(_) => {}
        }
    }

    // Write back.
    let content = lines.join("\n");
    // Ensure parent directory exists.
    let dir = format!("{home}/.omega");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        &path,
        if content.is_empty() {
            content
        } else {
            content + "\n"
        },
    );
}

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
// Misc helpers
// ---------------------------------------------------------------------------

/// Return localized status messages for the delayed provider nudge.
/// Returns `(first_nudge, still_working)`.
pub fn status_messages(lang: &str) -> (&'static str, &'static str) {
    match lang {
        "Spanish" => ("Déjame pensar en esto... 🧠", "Sigo en ello ⏳"),
        "Portuguese" => ("Deixa eu pensar nisso... 🧠", "Ainda estou nessa ⏳"),
        "French" => ("Laisse-moi réfléchir... 🧠", "J'y suis encore ⏳"),
        "German" => ("Lass mich kurz nachdenken... 🧠", "Bin noch dran ⏳"),
        "Italian" => ("Fammi pensare... 🧠", "Ci sto ancora lavorando ⏳"),
        "Dutch" => ("Even nadenken... 🧠", "Nog mee bezig ⏳"),
        "Russian" => ("Дай подумать... 🧠", "Ещё работаю ⏳"),
        _ => ("Let me think about this... 🧠", "Still on it ⏳"),
    }
}

/// Map raw provider errors to user-friendly messages.
pub fn friendly_provider_error(raw: &str) -> String {
    if raw.contains("timed out") {
        "I took too long to respond. Please try again — sometimes complex requests need a second attempt.".to_string()
    } else {
        "Something went wrong. Please try again.".to_string()
    }
}

/// Image file extensions recognized for workspace diff.
pub const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

/// Snapshot top-level image files in the workspace directory.
///
/// Returns a map of path → modification time. Returns an empty map on any
/// error (non-existent dir, permission issues). Tracks mtime so we can detect
/// both new files and overwritten files (same name, newer mtime).
pub fn snapshot_workspace_images(
    workspace: &std::path::Path,
) -> std::collections::HashMap<PathBuf, std::time::SystemTime> {
    let entries = match std::fs::read_dir(workspace) {
        Ok(e) => e,
        Err(_) => return std::collections::HashMap::new(),
    };
    entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().map(|ft| ft.is_file()).unwrap_or(false)
                && entry
                    .path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
                    .unwrap_or(false)
        })
        .filter_map(|entry| {
            let mtime = entry.metadata().ok()?.modified().ok()?;
            Some((entry.path(), mtime))
        })
        .collect()
}

/// Check if the current local time is within the active hours window.
pub fn is_within_active_hours(start: &str, end: &str) -> bool {
    let now = chrono::Local::now().format("%H:%M").to_string();
    if start <= end {
        // Normal range: e.g. 08:00 to 22:00
        now.as_str() >= start && now.as_str() < end
    } else {
        // Midnight wrap: e.g. 22:00 to 06:00
        now.as_str() >= start || now.as_str() < end
    }
}

// ---------------------------------------------------------------------------
// Classification helpers
// ---------------------------------------------------------------------------

/// Build a short context string for the complexity classifier.
///
/// Includes active project, last 3 messages (truncated to 80 chars each),
/// and available skill names. Returns empty string if all fields are empty.
pub fn build_classification_context(
    active_project: Option<&str>,
    history: &[omega_core::context::ContextEntry],
    skill_names: &[&str],
) -> String {
    let mut ctx = String::new();

    if let Some(proj) = active_project {
        ctx.push_str(&format!("Active project: {proj}\n"));
    }

    // Last 3 messages, each truncated to ~80 chars.
    let recent: Vec<_> = history.iter().rev().take(3).collect::<Vec<_>>();
    if !recent.is_empty() {
        ctx.push_str("Recent conversation:\n");
        for entry in recent.iter().rev() {
            let role = if entry.role == "user" {
                "User"
            } else {
                "Assistant"
            };
            let content = if entry.content.len() > 80 {
                format!("{}...", &entry.content[..80])
            } else {
                entry.content.clone()
            };
            ctx.push_str(&format!("{role}: {content}\n"));
        }
    }

    if !skill_names.is_empty() {
        ctx.push_str(&format!("Available skills: {}\n", skill_names.join(", ")));
    }

    ctx.trim().to_string()
}

/// Parse a plan/classification response into numbered steps.
///
/// Returns `None` if the response is "DIRECT" (case-insensitive) or has fewer
/// than 2 steps. Steps are extracted from lines starting with `N.` where N is
/// a digit.
pub fn parse_plan_response(text: &str) -> Option<Vec<String>> {
    let trimmed = text.trim();
    if trimmed.eq_ignore_ascii_case("DIRECT") {
        return None;
    }

    let steps: Vec<String> = trimmed
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            // Match lines starting with "N. " where N is a digit.
            if line.len() >= 3 && line.as_bytes()[0].is_ascii_digit() && line.as_bytes()[1] == b'.'
            {
                Some(line[2..].trim().to_string())
            } else {
                None
            }
        })
        .collect();

    if steps.len() >= 2 {
        Some(steps)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Inbox helpers
// ---------------------------------------------------------------------------

/// Ensure the workspace inbox directory exists and return its path.
pub fn ensure_inbox_dir(data_dir: &str) -> PathBuf {
    let dir = PathBuf::from(omega_core::config::shellexpand(data_dir))
        .join("workspace")
        .join("inbox");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Save image attachments to the inbox directory and return the paths.
///
/// Rejects zero-byte attachments and uses `sync_all` to guarantee
/// the data hits disk before the path is returned.
pub fn save_attachments_to_inbox(
    inbox: &std::path::Path,
    attachments: &[omega_core::message::Attachment],
) -> Vec<PathBuf> {
    use std::io::Write;

    let mut paths = Vec::new();
    for attachment in attachments {
        if !matches!(
            attachment.file_type,
            omega_core::message::AttachmentType::Image
        ) {
            continue;
        }
        if let Some(ref data) = attachment.data {
            if data.is_empty() {
                tracing::warn!("skipping zero-byte image attachment");
                continue;
            }
            let filename = attachment
                .filename
                .as_deref()
                .unwrap_or("image.jpg")
                .to_string();
            let path = inbox.join(&filename);
            match std::fs::File::create(&path) {
                Ok(mut file) => {
                    if file.write_all(data).is_ok() && file.sync_all().is_ok() {
                        tracing::debug!("inbox: wrote {} ({} bytes)", path.display(), data.len());
                        paths.push(path);
                    } else {
                        tracing::warn!("inbox: failed to write {}", path.display());
                    }
                }
                Err(e) => {
                    tracing::warn!("inbox: failed to create {}: {e}", path.display());
                }
            }
        }
    }
    paths
}

/// RAII guard that cleans up inbox image files when dropped.
///
/// Guarantees cleanup regardless of early returns in `handle_message()`.
pub struct InboxGuard {
    paths: Vec<PathBuf>,
}

impl InboxGuard {
    /// Create a new guard that will clean up the given paths on drop.
    pub fn new(paths: Vec<PathBuf>) -> Self {
        Self { paths }
    }
}

impl Drop for InboxGuard {
    fn drop(&mut self) {
        cleanup_inbox_images(&self.paths);
    }
}

/// Delete inbox images after they have been processed.
pub fn cleanup_inbox_images(paths: &[PathBuf]) {
    for path in paths {
        let _ = std::fs::remove_file(path);
    }
}

/// Purge all files in the inbox directory (startup cleanup).
pub fn purge_inbox(data_dir: &str) {
    let inbox = ensure_inbox_dir(data_dir);
    if let Ok(entries) = std::fs::read_dir(&inbox) {
        let mut count = 0u32;
        for entry in entries.flatten() {
            if entry.path().is_file() {
                let _ = std::fs::remove_file(entry.path());
                count += 1;
            }
        }
        if count > 0 {
            tracing::info!("startup: purged {count} orphaned inbox file(s)");
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- SCHEDULE ---

    #[test]
    fn test_extract_schedule_marker() {
        let text = "Sure, I'll remind you.\nSCHEDULE: Call John | 2026-02-17T15:00:00 | once";
        let result = extract_schedule_marker(text);
        assert_eq!(
            result,
            Some("SCHEDULE: Call John | 2026-02-17T15:00:00 | once".to_string())
        );
    }

    #[test]
    fn test_extract_schedule_marker_none() {
        let text = "No schedule here, just a normal response.";
        assert!(extract_schedule_marker(text).is_none());
    }

    #[test]
    fn test_parse_schedule_line() {
        let line = "SCHEDULE: Call John | 2026-02-17T15:00:00 | once";
        let result = parse_schedule_line(line).unwrap();
        assert_eq!(result.0, "Call John");
        assert_eq!(result.1, "2026-02-17T15:00:00");
        assert_eq!(result.2, "once");
    }

    #[test]
    fn test_parse_schedule_line_daily() {
        let line = "SCHEDULE: Stand-up meeting | 2026-02-18T09:00:00 | daily";
        let result = parse_schedule_line(line).unwrap();
        assert_eq!(result.0, "Stand-up meeting");
        assert_eq!(result.2, "daily");
    }

    #[test]
    fn test_parse_schedule_line_invalid() {
        assert!(parse_schedule_line("SCHEDULE: missing parts").is_none());
        assert!(parse_schedule_line("not a schedule line").is_none());
    }

    #[test]
    fn test_strip_schedule_marker() {
        let text = "Sure, I'll remind you.\nSCHEDULE: Call John | 2026-02-17T15:00:00 | once";
        let result = strip_schedule_marker(text);
        assert_eq!(result, "Sure, I'll remind you.");
    }

    #[test]
    fn test_strip_schedule_marker_preserves_other_lines() {
        let text = "Line 1\nLine 2\nSCHEDULE: test | 2026-01-01T00:00:00 | once\nLine 3";
        let result = strip_schedule_marker(text);
        assert_eq!(result, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_extract_all_schedule_markers_multiple() {
        let text = "I'll set up your reminders.\n\
                    SCHEDULE: Cancel Hostinger | 2026-03-01T09:00:00 | once\n\
                    SCHEDULE: Cancel Hostinger 2 | 2026-03-05T09:00:00 | once\n\
                    SCHEDULE: Cancel Hostinger 3 | 2026-03-10T09:00:00 | once\n\
                    Done!";
        let result = extract_all_schedule_markers(text);
        assert_eq!(result.len(), 3);
        assert!(result[0].contains("Cancel Hostinger |"));
        assert!(result[1].contains("Cancel Hostinger 2"));
        assert!(result[2].contains("Cancel Hostinger 3"));
    }

    #[test]
    fn test_extract_all_schedule_markers_single() {
        let text = "Sure.\nSCHEDULE: Call John | 2026-02-17T15:00:00 | once";
        let result = extract_all_schedule_markers(text);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_extract_all_schedule_markers_none() {
        let text = "No schedule markers here.";
        let result = extract_all_schedule_markers(text);
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_all_schedule_markers_ignores_schedule_action() {
        let text = "SCHEDULE: Reminder | 2026-02-17T09:00:00 | once\n\
                    SCHEDULE_ACTION: Check price | 2026-02-17T14:00:00 | daily";
        let result = extract_all_schedule_markers(text);
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("Reminder"));
    }

    // --- LANG_SWITCH ---

    #[test]
    fn test_extract_lang_switch() {
        let text = "Sure, I'll speak French now.\nLANG_SWITCH: French";
        assert_eq!(extract_lang_switch(text), Some("French".to_string()));
    }

    #[test]
    fn test_extract_lang_switch_inline() {
        let text = "Estoy usando el modelo Claude. LANG_SWITCH: espanol";
        assert_eq!(extract_lang_switch(text), Some("espanol".to_string()));
    }

    #[test]
    fn test_extract_lang_switch_none() {
        assert!(extract_lang_switch("Just a normal response.").is_none());
    }

    #[test]
    fn test_strip_lang_switch() {
        let text = "Sure, I'll speak French now.\nLANG_SWITCH: French";
        assert_eq!(strip_lang_switch(text), "Sure, I'll speak French now.");
    }

    #[test]
    fn test_strip_lang_switch_inline() {
        let text = "Estoy usando el modelo Claude. LANG_SWITCH: espanol";
        assert_eq!(strip_lang_switch(text), "Estoy usando el modelo Claude.");
    }

    #[test]
    fn test_strip_all_remaining_markers() {
        let text = "Hello world. LANG_SWITCH: english\nMore text. PERSONALITY: friendly";
        let result = strip_all_remaining_markers(text);
        assert!(!result.contains("LANG_SWITCH:"));
        assert!(!result.contains("PERSONALITY:"));
        assert!(result.contains("Hello world."));
        assert!(result.contains("More text."));
    }

    // --- PROJECT ---

    #[test]
    fn test_extract_project_activate() {
        let text = "I've created a project for you.\nPROJECT_ACTIVATE: real-estate";
        assert_eq!(
            extract_project_activate(text),
            Some("real-estate".to_string())
        );
    }

    #[test]
    fn test_extract_project_activate_none() {
        assert!(extract_project_activate("Just a normal response.").is_none());
    }

    #[test]
    fn test_extract_project_activate_empty_name() {
        assert!(extract_project_activate("PROJECT_ACTIVATE: ").is_none());
    }

    #[test]
    fn test_has_project_deactivate() {
        let text = "Project deactivated.\nPROJECT_DEACTIVATE";
        assert!(has_project_deactivate(text));
    }

    #[test]
    fn test_has_project_deactivate_false() {
        assert!(!has_project_deactivate("No marker here."));
    }

    #[test]
    fn test_strip_project_markers() {
        let text =
            "I've set up the project.\nPROJECT_ACTIVATE: stocks\nLet me know if you need more.";
        let result = strip_project_markers(text);
        assert_eq!(
            result,
            "I've set up the project.\nLet me know if you need more."
        );
    }

    #[test]
    fn test_strip_project_markers_deactivate() {
        let text = "Done, project deactivated.\nPROJECT_DEACTIVATE";
        let result = strip_project_markers(text);
        assert_eq!(result, "Done, project deactivated.");
    }

    #[test]
    fn test_strip_project_markers_both() {
        let text = "Switching.\nPROJECT_DEACTIVATE\nPROJECT_ACTIVATE: new-proj\nEnjoy!";
        let result = strip_project_markers(text);
        assert_eq!(result, "Switching.\nEnjoy!");
    }

    // --- Active hours ---

    #[test]
    fn test_is_within_active_hours_normal_range() {
        assert!(is_within_active_hours("00:00", "23:59"));
    }

    #[test]
    fn test_is_within_active_hours_narrow_miss() {
        assert!(!is_within_active_hours("00:00", "00:00"));
    }

    // --- Status messages ---

    #[test]
    fn test_friendly_provider_error_timeout() {
        let msg = friendly_provider_error("claude CLI timed out after 600s");
        assert!(msg.contains("too long"));
        assert!(!msg.contains("timed out"));
    }

    #[test]
    fn test_friendly_provider_error_generic() {
        let msg = friendly_provider_error("failed to run claude CLI: No such file");
        assert_eq!(msg, "Something went wrong. Please try again.");
    }

    #[test]
    fn test_status_messages_all_languages() {
        let languages = [
            "English",
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ];
        for lang in &languages {
            let (nudge, still) = status_messages(lang);
            assert!(!nudge.is_empty(), "nudge for {lang} should not be empty");
            assert!(!still.is_empty(), "still for {lang} should not be empty");
        }
    }

    #[test]
    fn test_status_messages_unknown_falls_back_to_english() {
        let (nudge, still) = status_messages("Klingon");
        assert!(nudge.contains("think about this"));
        assert!(still.contains("Still on it"));
    }

    #[test]
    fn test_status_messages_spanish() {
        let (nudge, still) = status_messages("Spanish");
        assert!(nudge.contains("pensar"));
        assert!(still.contains("ello"));
    }

    // --- Heartbeat ---

    #[test]
    fn test_read_heartbeat_file_returns_none_when_missing() {
        let result = read_heartbeat_file();
        let _ = result;
    }

    #[test]
    fn test_extract_heartbeat_add() {
        let text = "Sure, I'll monitor that.\nHEARTBEAT_ADD: Check exercise habits";
        let actions = extract_heartbeat_markers(text);
        assert_eq!(
            actions,
            vec![HeartbeatAction::Add("Check exercise habits".to_string())]
        );
    }

    #[test]
    fn test_extract_heartbeat_remove() {
        let text = "I'll stop monitoring that.\nHEARTBEAT_REMOVE: exercise";
        let actions = extract_heartbeat_markers(text);
        assert_eq!(
            actions,
            vec![HeartbeatAction::Remove("exercise".to_string())]
        );
    }

    #[test]
    fn test_extract_heartbeat_multiple() {
        let text =
            "Updating your checklist.\nHEARTBEAT_ADD: Water plants\nHEARTBEAT_REMOVE: old task";
        let actions = extract_heartbeat_markers(text);
        assert_eq!(
            actions,
            vec![
                HeartbeatAction::Add("Water plants".to_string()),
                HeartbeatAction::Remove("old task".to_string()),
            ]
        );
    }

    #[test]
    fn test_extract_heartbeat_empty_ignored() {
        let text = "HEARTBEAT_ADD: \nHEARTBEAT_REMOVE:   \nSome response.";
        let actions = extract_heartbeat_markers(text);
        assert!(actions.is_empty());
    }

    #[test]
    fn test_strip_heartbeat_markers() {
        let text = "Sure, I'll monitor that.\nHEARTBEAT_ADD: Check exercise habits\nDone!";
        let result = strip_heartbeat_markers(text);
        assert_eq!(result, "Sure, I'll monitor that.\nDone!");
    }

    #[test]
    fn test_strip_heartbeat_both_types() {
        let text = "Response.\nHEARTBEAT_ADD: new item\nHEARTBEAT_REMOVE: old item\nEnd.";
        let result = strip_heartbeat_markers(text);
        assert_eq!(result, "Response.\nEnd.");
    }

    #[test]
    fn test_extract_heartbeat_interval() {
        let text = "Updating interval.\nHEARTBEAT_INTERVAL: 15\nDone.";
        let actions = extract_heartbeat_markers(text);
        assert_eq!(actions, vec![HeartbeatAction::SetInterval(15)]);
    }

    #[test]
    fn test_extract_heartbeat_interval_invalid() {
        let text = "HEARTBEAT_INTERVAL: 0";
        assert!(extract_heartbeat_markers(text).is_empty());
        let text = "HEARTBEAT_INTERVAL: -5";
        assert!(extract_heartbeat_markers(text).is_empty());
        let text = "HEARTBEAT_INTERVAL: abc";
        assert!(extract_heartbeat_markers(text).is_empty());
        let text = "HEARTBEAT_INTERVAL: 1441";
        assert!(extract_heartbeat_markers(text).is_empty());
        let text = "HEARTBEAT_INTERVAL: 1440";
        assert_eq!(
            extract_heartbeat_markers(text),
            vec![HeartbeatAction::SetInterval(1440)]
        );
        let text = "HEARTBEAT_INTERVAL: 1";
        assert_eq!(
            extract_heartbeat_markers(text),
            vec![HeartbeatAction::SetInterval(1)]
        );
    }

    #[test]
    fn test_strip_heartbeat_interval() {
        let text = "Updated.\nHEARTBEAT_INTERVAL: 10\nDone.";
        let result = strip_heartbeat_markers(text);
        assert_eq!(result, "Updated.\nDone.");
    }

    #[test]
    fn test_extract_heartbeat_mixed() {
        let text =
            "Ok.\nHEARTBEAT_INTERVAL: 20\nHEARTBEAT_ADD: new check\nHEARTBEAT_REMOVE: old\nEnd.";
        let actions = extract_heartbeat_markers(text);
        assert_eq!(
            actions,
            vec![
                HeartbeatAction::SetInterval(20),
                HeartbeatAction::Add("new check".to_string()),
                HeartbeatAction::Remove("old".to_string()),
            ]
        );
    }

    #[test]
    fn test_apply_heartbeat_add() {
        let fake_home = std::env::temp_dir().join("omega_test_hb_add_home");
        let _ = std::fs::create_dir_all(fake_home.join(".omega"));
        std::fs::write(
            fake_home.join(".omega/HEARTBEAT.md"),
            "# My checklist\n- Existing item\n",
        )
        .unwrap();
        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", &fake_home);

        apply_heartbeat_changes(&[HeartbeatAction::Add("New item".to_string())]);

        let content = std::fs::read_to_string(fake_home.join(".omega/HEARTBEAT.md")).unwrap();
        assert!(content.contains("- Existing item"), "should keep existing");
        assert!(content.contains("- New item"), "should add new item");

        apply_heartbeat_changes(&[HeartbeatAction::Add("New item".to_string())]);
        let content = std::fs::read_to_string(fake_home.join(".omega/HEARTBEAT.md")).unwrap();
        assert_eq!(
            content.matches("New item").count(),
            1,
            "should not duplicate"
        );

        std::env::set_var("HOME", &original_home);
        let _ = std::fs::remove_dir_all(&fake_home);
    }

    #[test]
    fn test_apply_heartbeat_remove() {
        let fake_home = std::env::temp_dir().join("omega_test_hb_remove_home");
        let _ = std::fs::create_dir_all(fake_home.join(".omega"));
        std::fs::write(
            fake_home.join(".omega/HEARTBEAT.md"),
            "# My checklist\n- Check exercise habits\n- Water the plants\n",
        )
        .unwrap();

        let original_home = std::env::var("HOME").unwrap();
        std::env::set_var("HOME", &fake_home);

        apply_heartbeat_changes(&[HeartbeatAction::Remove("exercise".to_string())]);

        let content = std::fs::read_to_string(fake_home.join(".omega/HEARTBEAT.md")).unwrap();
        assert!(!content.contains("exercise"), "should remove exercise line");
        assert!(
            content.contains("Water the plants"),
            "should keep other items"
        );
        assert!(content.contains("# My checklist"), "should keep comments");

        std::env::set_var("HOME", &original_home);
        let _ = std::fs::remove_dir_all(&fake_home);
    }

    // --- Workspace images ---

    #[test]
    fn test_snapshot_workspace_images_finds_images() {
        let dir = std::env::temp_dir().join("omega_test_snap_images");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("screenshot.png"), b"fake png").unwrap();
        std::fs::write(dir.join("photo.jpg"), b"fake jpg").unwrap();
        std::fs::write(dir.join("readme.txt"), b"not an image").unwrap();

        let result = snapshot_workspace_images(&dir);
        assert_eq!(result.len(), 2);
        assert!(result.contains_key(&dir.join("screenshot.png")));
        assert!(result.contains_key(&dir.join("photo.jpg")));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_snapshot_workspace_images_empty_dir() {
        let dir = std::env::temp_dir().join("omega_test_snap_empty");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let result = snapshot_workspace_images(&dir);
        assert!(result.is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_snapshot_workspace_images_nonexistent_dir() {
        let dir = std::env::temp_dir().join("omega_test_snap_nonexistent");
        let _ = std::fs::remove_dir_all(&dir);

        let result = snapshot_workspace_images(&dir);
        assert!(result.is_empty());
    }

    #[test]
    fn test_snapshot_workspace_images_all_extensions() {
        let dir = std::env::temp_dir().join("omega_test_snap_all_ext");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for ext in IMAGE_EXTENSIONS {
            std::fs::write(dir.join(format!("test.{ext}")), b"fake").unwrap();
        }

        let result = snapshot_workspace_images(&dir);
        assert_eq!(result.len(), IMAGE_EXTENSIONS.len());

        let _ = std::fs::remove_dir_all(&dir);
    }

    // --- Inbox ---

    #[test]
    fn test_ensure_inbox_dir() {
        let tmp = std::env::temp_dir().join("omega_test_inbox_dir");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let inbox = ensure_inbox_dir(tmp.to_str().unwrap());
        assert!(inbox.exists());
        assert!(inbox.is_dir());
        assert!(inbox.ends_with("workspace/inbox"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_save_and_cleanup_inbox_images() {
        use omega_core::message::{Attachment, AttachmentType};

        let tmp = std::env::temp_dir().join("omega_test_save_inbox");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let attachments = vec![Attachment {
            file_type: AttachmentType::Image,
            url: None,
            data: Some(b"fake image data".to_vec()),
            filename: Some("test_photo.jpg".to_string()),
        }];

        let paths = save_attachments_to_inbox(&tmp, &attachments);
        assert_eq!(paths.len(), 1);
        assert!(paths[0].exists());
        assert_eq!(std::fs::read(&paths[0]).unwrap(), b"fake image data");

        cleanup_inbox_images(&paths);
        assert!(!paths[0].exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_save_attachments_skips_non_images() {
        use omega_core::message::{Attachment, AttachmentType};

        let tmp = std::env::temp_dir().join("omega_test_skip_non_img");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let attachments = vec![
            Attachment {
                file_type: AttachmentType::Document,
                url: None,
                data: Some(b"some doc".to_vec()),
                filename: Some("doc.pdf".to_string()),
            },
            Attachment {
                file_type: AttachmentType::Audio,
                url: None,
                data: Some(b"some audio".to_vec()),
                filename: Some("audio.mp3".to_string()),
            },
        ];

        let paths = save_attachments_to_inbox(&tmp, &attachments);
        assert!(paths.is_empty());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_save_attachments_rejects_empty_data() {
        use omega_core::message::{Attachment, AttachmentType};

        let tmp = std::env::temp_dir().join("omega_test_reject_empty");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let attachments = vec![Attachment {
            file_type: AttachmentType::Image,
            url: None,
            data: Some(Vec::new()),
            filename: Some("empty.jpg".to_string()),
        }];

        let paths = save_attachments_to_inbox(&tmp, &attachments);
        assert!(paths.is_empty(), "zero-byte attachment must be rejected");

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_inbox_guard_cleans_up_on_drop() {
        let tmp = std::env::temp_dir().join("omega_test_guard_cleanup");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let file = tmp.join("guard_test.jpg");
        std::fs::write(&file, b"image data").unwrap();
        assert!(file.exists());

        {
            let _guard = InboxGuard::new(vec![file.clone()]);
            // Guard is alive — file should still exist.
            assert!(file.exists());
        }
        // Guard dropped — file should be cleaned up.
        assert!(!file.exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_inbox_guard_empty_is_noop() {
        // An empty guard should not panic or error on drop.
        let _guard = InboxGuard::new(Vec::new());
    }

    // --- Classification ---

    #[test]
    fn test_parse_plan_response_direct() {
        assert!(parse_plan_response("DIRECT").is_none());
        assert!(parse_plan_response("  DIRECT  ").is_none());
        assert!(parse_plan_response("direct").is_none());
    }

    #[test]
    fn test_parse_plan_response_numbered_list() {
        let text = "1. Set up the database schema\n\
                    2. Create the API endpoint\n\
                    3. Write integration tests";
        let steps = parse_plan_response(text).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], "Set up the database schema");
        assert_eq!(steps[1], "Create the API endpoint");
        assert_eq!(steps[2], "Write integration tests");
    }

    #[test]
    fn test_parse_plan_response_single_step() {
        let text = "1. Just do the thing";
        assert!(parse_plan_response(text).is_none());
    }

    #[test]
    fn test_parse_plan_response_with_preamble() {
        let text = "Here are the steps:\n\
                    1. First step\n\
                    2. Second step\n\
                    3. Third step";
        let steps = parse_plan_response(text).unwrap();
        assert_eq!(steps.len(), 3);
        assert_eq!(steps[0], "First step");
    }

    // --- SCHEDULE_ACTION ---

    #[test]
    fn test_extract_schedule_action_marker() {
        let text =
            "I'll handle that.\nSCHEDULE_ACTION: Check BTC price | 2026-02-18T14:00:00 | daily";
        let result = extract_schedule_action_marker(text);
        assert_eq!(
            result,
            Some("SCHEDULE_ACTION: Check BTC price | 2026-02-18T14:00:00 | daily".to_string())
        );
    }

    #[test]
    fn test_extract_schedule_action_marker_none() {
        let text = "No action scheduled here.";
        assert!(extract_schedule_action_marker(text).is_none());
    }

    #[test]
    fn test_parse_schedule_action_line() {
        let line = "SCHEDULE_ACTION: Check BTC price | 2026-02-18T14:00:00 | daily";
        let result = parse_schedule_action_line(line).unwrap();
        assert_eq!(result.0, "Check BTC price");
        assert_eq!(result.1, "2026-02-18T14:00:00");
        assert_eq!(result.2, "daily");
    }

    #[test]
    fn test_parse_schedule_action_line_once() {
        let line = "SCHEDULE_ACTION: Run scraper | 2026-02-18T22:00:00 | once";
        let result = parse_schedule_action_line(line).unwrap();
        assert_eq!(result.0, "Run scraper");
        assert_eq!(result.2, "once");
    }

    #[test]
    fn test_parse_schedule_action_line_invalid() {
        assert!(parse_schedule_action_line("SCHEDULE_ACTION: missing parts").is_none());
        assert!(parse_schedule_action_line("not an action line").is_none());
        assert!(parse_schedule_action_line("SCHEDULE_ACTION:  | time | once").is_none());
    }

    #[test]
    fn test_strip_schedule_action_markers() {
        let text = "I'll do that.\nSCHEDULE_ACTION: Check BTC | 2026-02-18T14:00:00 | daily\nDone.";
        let result = strip_schedule_action_markers(text);
        assert_eq!(result, "I'll do that.\nDone.");
    }

    #[test]
    fn test_strip_schedule_action_preserves_schedule() {
        let text = "Response.\nSCHEDULE: Remind me | 2026-02-18T09:00:00 | once\nSCHEDULE_ACTION: Check prices | 2026-02-18T14:00:00 | daily\nEnd.";
        let result = strip_schedule_action_markers(text);
        assert!(
            result.contains("SCHEDULE: Remind me"),
            "should keep SCHEDULE lines"
        );
        assert!(
            !result.contains("SCHEDULE_ACTION:"),
            "should strip SCHEDULE_ACTION lines"
        );
    }

    #[test]
    fn test_extract_all_schedule_action_markers_multiple() {
        let text = "Setting up monitoring.\n\
                    SCHEDULE_ACTION: Check BTC | 2026-02-18T14:00:00 | daily\n\
                    SCHEDULE_ACTION: Check ETH | 2026-02-18T14:05:00 | daily\n\
                    SCHEDULE_ACTION: Check SOL | 2026-02-18T14:10:00 | daily\n\
                    All set!";
        let result = extract_all_schedule_action_markers(text);
        assert_eq!(result.len(), 3);
        assert!(result[0].contains("Check BTC"));
        assert!(result[1].contains("Check ETH"));
        assert!(result[2].contains("Check SOL"));
    }

    #[test]
    fn test_extract_all_schedule_action_markers_none() {
        let text = "No action markers here.";
        let result = extract_all_schedule_action_markers(text);
        assert!(result.is_empty());
    }

    #[test]
    fn test_extract_all_schedule_action_markers_ignores_schedule() {
        let text = "SCHEDULE: Reminder | 2026-02-17T09:00:00 | once\n\
                    SCHEDULE_ACTION: Check price | 2026-02-17T14:00:00 | daily";
        let result = extract_all_schedule_action_markers(text);
        assert_eq!(result.len(), 1);
        assert!(result[0].contains("Check price"));
    }

    // --- PERSONALITY ---

    #[test]
    fn test_extract_personality() {
        let text = "Sure, I'll be more casual.\nPERSONALITY: casual and friendly";
        assert_eq!(
            extract_personality(text),
            Some("casual and friendly".to_string())
        );
    }

    #[test]
    fn test_extract_personality_none() {
        assert!(extract_personality("Just a normal response.").is_none());
    }

    #[test]
    fn test_extract_personality_empty() {
        assert!(extract_personality("PERSONALITY: ").is_none());
    }

    #[test]
    fn test_extract_personality_reset() {
        let text = "Back to defaults.\nPERSONALITY: reset";
        assert_eq!(extract_personality(text), Some("reset".to_string()));
    }

    #[test]
    fn test_strip_personality() {
        let text = "Sure, I'll adjust.\nPERSONALITY: formal and precise\nLet me know.";
        assert_eq!(strip_personality(text), "Sure, I'll adjust.\nLet me know.");
    }

    // --- FORGET ---

    #[test]
    fn test_has_forget_marker() {
        let text = "Starting fresh.\nFORGET_CONVERSATION\nDone!";
        assert!(has_forget_marker(text));
    }

    #[test]
    fn test_has_forget_marker_false() {
        assert!(!has_forget_marker("No marker here."));
    }

    #[test]
    fn test_has_forget_marker_partial_no_match() {
        assert!(!has_forget_marker("FORGET_CONVERSATION_EXTRA"));
    }

    #[test]
    fn test_strip_forget_marker() {
        let text = "Clearing now.\nFORGET_CONVERSATION\nAll fresh!";
        assert_eq!(strip_forget_marker(text), "Clearing now.\nAll fresh!");
    }

    // --- CANCEL_TASK ---

    #[test]
    fn test_extract_all_cancel_tasks_single() {
        let text = "I'll cancel that.\nCANCEL_TASK: a1b2c3d4";
        let ids = extract_all_cancel_tasks(text);
        assert_eq!(ids, vec!["a1b2c3d4"]);
    }

    #[test]
    fn test_extract_all_cancel_tasks_none_found() {
        assert!(extract_all_cancel_tasks("Just a normal response.").is_empty());
    }

    #[test]
    fn test_extract_all_cancel_tasks_empty_value() {
        assert!(extract_all_cancel_tasks("CANCEL_TASK: ").is_empty());
    }

    #[test]
    fn test_extract_all_cancel_tasks_multiple() {
        let text =
            "Cancelling all.\nCANCEL_TASK: aaa111\nCANCEL_TASK: bbb222\nCANCEL_TASK: ccc333\nDone.";
        let ids = extract_all_cancel_tasks(text);
        assert_eq!(ids, vec!["aaa111", "bbb222", "ccc333"]);
    }

    #[test]
    fn test_extract_all_cancel_tasks_skips_empty() {
        let text = "CANCEL_TASK: abc\nCANCEL_TASK: \nCANCEL_TASK: def";
        let ids = extract_all_cancel_tasks(text);
        assert_eq!(ids, vec!["abc", "def"]);
    }

    #[test]
    fn test_strip_cancel_task() {
        let text = "Cancelled.\nCANCEL_TASK: abc123\nDone.";
        assert_eq!(strip_cancel_task(text), "Cancelled.\nDone.");
    }

    // --- UPDATE_TASK ---

    #[test]
    fn test_extract_all_update_tasks_single_line() {
        let text = "I've updated that.\nUPDATE_TASK: a1b2c3d4 | New description | 2026-03-01T09:00:00 | daily";
        let lines = extract_all_update_tasks(text);
        assert_eq!(lines.len(), 1);
        assert!(lines[0].contains("a1b2c3d4"));
    }

    #[test]
    fn test_extract_all_update_tasks_none_found() {
        assert!(extract_all_update_tasks("Just a normal response.").is_empty());
    }

    #[test]
    fn test_parse_update_task_line_all_fields() {
        let line = "UPDATE_TASK: abc123 | New desc | 2026-03-01T09:00:00 | daily";
        let (id, desc, due_at, repeat) = parse_update_task_line(line).unwrap();
        assert_eq!(id, "abc123");
        assert_eq!(desc, Some("New desc".to_string()));
        assert_eq!(due_at, Some("2026-03-01T09:00:00".to_string()));
        assert_eq!(repeat, Some("daily".to_string()));
    }

    #[test]
    fn test_parse_update_task_line_empty_fields() {
        let line = "UPDATE_TASK: abc123 | | | daily";
        let (id, desc, due_at, repeat) = parse_update_task_line(line).unwrap();
        assert_eq!(id, "abc123");
        assert!(desc.is_none());
        assert!(due_at.is_none());
        assert_eq!(repeat, Some("daily".to_string()));
    }

    #[test]
    fn test_parse_update_task_line_only_description() {
        let line = "UPDATE_TASK: abc123 | Updated reminder text | | ";
        let (id, desc, due_at, repeat) = parse_update_task_line(line).unwrap();
        assert_eq!(id, "abc123");
        assert_eq!(desc, Some("Updated reminder text".to_string()));
        assert!(due_at.is_none());
        assert!(repeat.is_none());
    }

    #[test]
    fn test_parse_update_task_line_invalid() {
        assert!(parse_update_task_line("UPDATE_TASK: missing pipes").is_none());
        assert!(parse_update_task_line("not an update line").is_none());
        assert!(parse_update_task_line("UPDATE_TASK:  | desc | time | once").is_none());
    }

    #[test]
    fn test_extract_all_update_tasks_multiple() {
        let text = "Updating.\nUPDATE_TASK: aaa | New A | | daily\nUPDATE_TASK: bbb | New B | | weekly\nDone.";
        let lines = extract_all_update_tasks(text);
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("aaa"));
        assert!(lines[1].contains("bbb"));
    }

    #[test]
    fn test_strip_update_task() {
        let text = "Updated.\nUPDATE_TASK: abc123 | | | daily\nDone.";
        assert_eq!(strip_update_task(text), "Updated.\nDone.");
    }

    // --- PURGE_FACTS ---

    #[test]
    fn test_has_purge_marker() {
        let text = "Deleting everything.\nPURGE_FACTS\nClean slate.";
        assert!(has_purge_marker(text));
    }

    #[test]
    fn test_has_purge_marker_false() {
        assert!(!has_purge_marker("No purge here."));
    }

    #[test]
    fn test_has_purge_marker_partial_no_match() {
        assert!(!has_purge_marker("PURGE_FACTS_EXTRA"));
    }

    #[test]
    fn test_strip_purge_marker() {
        let text = "All gone.\nPURGE_FACTS\nStarting over.";
        assert_eq!(strip_purge_marker(text), "All gone.\nStarting over.");
    }

    // --- WHATSAPP_QR ---

    #[test]
    fn test_has_whatsapp_qr_marker() {
        let text = "Let me set that up.\nWHATSAPP_QR\nDone.";
        assert!(has_whatsapp_qr_marker(text));
    }

    #[test]
    fn test_strip_whatsapp_qr_marker() {
        let text = "Setting up.\nWHATSAPP_QR\nAll done.";
        assert_eq!(strip_whatsapp_qr_marker(text), "Setting up.\nAll done.");
    }

    // --- SKILL_IMPROVE ---

    #[test]
    fn test_extract_skill_improve() {
        let text =
            "I've updated the skill.\nSKILL_IMPROVE: google-workspace | Always search by both name and email when looking up contacts";
        let result = extract_skill_improve(text);
        assert_eq!(
            result,
            Some("SKILL_IMPROVE: google-workspace | Always search by both name and email when looking up contacts".to_string())
        );
    }

    #[test]
    fn test_extract_skill_improve_none() {
        assert!(extract_skill_improve("No skill improve here.").is_none());
    }

    #[test]
    fn test_parse_skill_improve_line() {
        let line = "SKILL_IMPROVE: google-workspace | Always search by both name and email";
        let (skill, lesson) = parse_skill_improve_line(line).unwrap();
        assert_eq!(skill, "google-workspace");
        assert_eq!(lesson, "Always search by both name and email");
    }

    #[test]
    fn test_parse_skill_improve_line_with_internal_pipes() {
        let line = "SKILL_IMPROVE: playwright-mcp | Use page.waitForSelector('div | span') before clicking";
        let (skill, lesson) = parse_skill_improve_line(line).unwrap();
        assert_eq!(skill, "playwright-mcp");
        assert_eq!(
            lesson,
            "Use page.waitForSelector('div | span') before clicking"
        );
    }

    #[test]
    fn test_parse_skill_improve_line_invalid() {
        assert!(parse_skill_improve_line("SKILL_IMPROVE:").is_none());
        assert!(parse_skill_improve_line("SKILL_IMPROVE: skill_only").is_none());
        assert!(parse_skill_improve_line("SKILL_IMPROVE:  | lesson").is_none());
        assert!(parse_skill_improve_line("SKILL_IMPROVE: skill |").is_none());
        assert!(parse_skill_improve_line("not a skill improve line").is_none());
    }

    #[test]
    fn test_strip_skill_improve() {
        let text =
            "Fixed the issue.\nSKILL_IMPROVE: google-workspace | Search by name and email\nDone.";
        let result = strip_skill_improve(text);
        assert_eq!(result, "Fixed the issue.\nDone.");
    }

    // --- BUG_REPORT ---

    #[test]
    fn test_extract_bug_report() {
        let text =
            "I can't read my heartbeat config.\nBUG_REPORT: Cannot read own heartbeat interval";
        assert_eq!(
            extract_bug_report(text),
            Some("Cannot read own heartbeat interval".to_string())
        );
    }

    #[test]
    fn test_extract_bug_report_inline() {
        let text = "I noticed a gap. BUG_REPORT: No introspection for MCP connections";
        assert_eq!(
            extract_bug_report(text),
            Some("No introspection for MCP connections".to_string())
        );
    }

    #[test]
    fn test_extract_bug_report_none() {
        assert!(extract_bug_report("Just a normal response.").is_none());
    }

    #[test]
    fn test_extract_bug_report_empty() {
        assert!(extract_bug_report("BUG_REPORT: ").is_none());
    }

    #[test]
    fn test_strip_bug_report() {
        let text = "I noticed an issue.\nBUG_REPORT: Cannot list active MCP connections\nDone.";
        assert_eq!(strip_bug_report(text), "I noticed an issue.\nDone.");
    }

    #[test]
    fn test_strip_bug_report_inline() {
        let text = "There's a gap here. BUG_REPORT: No runtime config access";
        assert_eq!(strip_bug_report(text), "There's a gap here.");
    }

    #[test]
    fn test_append_bug_report_creates_file() {
        let tmp = std::env::temp_dir().join("omega_test_bug_report_create");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        append_bug_report(tmp.to_str().unwrap(), "Cannot read heartbeat interval").unwrap();

        let content = std::fs::read_to_string(tmp.join("BUG.md")).unwrap();
        assert!(content.contains("# OMEGA Bug Reports"));
        assert!(content.contains("- **Cannot read heartbeat interval**"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_append_bug_report_groups_by_date() {
        let tmp = std::env::temp_dir().join("omega_test_bug_report_group");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        append_bug_report(tmp.to_str().unwrap(), "First bug").unwrap();
        append_bug_report(tmp.to_str().unwrap(), "Second bug").unwrap();

        let content = std::fs::read_to_string(tmp.join("BUG.md")).unwrap();
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        // Only one date header for today.
        assert_eq!(
            content.matches(&format!("## {today}")).count(),
            1,
            "should have exactly one date header"
        );
        assert!(content.contains("- **First bug**"));
        assert!(content.contains("- **Second bug**"));

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_strip_all_remaining_markers_includes_bug_report() {
        let text = "Hello. BUG_REPORT: some limitation\nMore text.";
        let result = strip_all_remaining_markers(text);
        assert!(!result.contains("BUG_REPORT:"));
        assert!(result.contains("Hello."));
        assert!(result.contains("More text."));
    }

    // --- Classification context ---

    #[test]
    fn test_classification_context_full() {
        let history = vec![
            omega_core::context::ContextEntry {
                role: "user".into(),
                content: "Check BTC price".into(),
            },
            omega_core::context::ContextEntry {
                role: "assistant".into(),
                content: "BTC is at $45,000".into(),
            },
            omega_core::context::ContextEntry {
                role: "user".into(),
                content: "Set up a trailing stop".into(),
            },
        ];
        let result = build_classification_context(
            Some("trader"),
            &history,
            &["claude-code", "playwright-mcp"],
        );
        assert!(result.contains("Active project: trader"));
        assert!(result.contains("Recent conversation:"));
        assert!(result.contains("User: Check BTC price"));
        assert!(result.contains("Assistant: BTC is at $45,000"));
        assert!(result.contains("User: Set up a trailing stop"));
        assert!(result.contains("Available skills: claude-code, playwright-mcp"));
    }

    #[test]
    fn test_classification_context_empty() {
        let result = build_classification_context(None, &[], &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_classification_context_truncation() {
        let long_msg = "a".repeat(120);
        let history = vec![omega_core::context::ContextEntry {
            role: "user".into(),
            content: long_msg,
        }];
        let result = build_classification_context(None, &history, &[]);
        assert!(result.contains("..."));
        assert!(!result.contains(&"a".repeat(120)));
        assert!(result.contains(&"a".repeat(80)));
    }

    #[test]
    fn test_classification_context_partial() {
        let result = build_classification_context(Some("trader"), &[], &[]);
        assert!(result.contains("Active project: trader"));
        assert!(!result.contains("Recent conversation:"));
        assert!(!result.contains("Available skills:"));
    }
}
