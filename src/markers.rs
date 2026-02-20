//! Marker extraction, parsing, and stripping for the gateway protocol.
//!
//! All system markers (SCHEDULE:, LANG_SWITCH:, SELF_HEAL:, etc.) are emitted
//! by the AI in response text and processed here. This module centralizes the
//! 40+ extract/strip/parse functions that were previously scattered in gateway.rs.

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
        "PROJECT_ACTIVATE:",
        "PROJECT_DEACTIVATE",
        "SCHEDULE_ACTION:",
        "SCHEDULE:",
        "HEARTBEAT_ADD:",
        "HEARTBEAT_REMOVE:",
        "HEARTBEAT_INTERVAL:",
        "LIMITATION:",
        "SELF_HEAL_RESOLVED",
        "SELF_HEAL:",
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
pub fn extract_schedule_marker(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("SCHEDULE:"))
        .map(|line| line.trim().to_string())
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
pub fn extract_schedule_action_marker(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("SCHEDULE_ACTION:"))
        .map(|line| line.trim().to_string())
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

/// Extract the task ID prefix from a `CANCEL_TASK:` line in response text.
pub fn extract_cancel_task(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("CANCEL_TASK:"))
        .and_then(|line| {
            let val = line.trim().strip_prefix("CANCEL_TASK:")?.trim().to_string();
            if val.is_empty() {
                None
            } else {
                Some(val)
            }
        })
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
// LIMITATION
// ---------------------------------------------------------------------------

/// Extract the first `LIMITATION:` line from response text.
pub fn extract_limitation_marker(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("LIMITATION:"))
        .map(|line| line.trim().to_string())
}

/// Parse a limitation line: `LIMITATION: title | description | proposed plan`
pub fn parse_limitation_line(line: &str) -> Option<(String, String, String)> {
    let content = line.strip_prefix("LIMITATION:")?.trim();
    let parts: Vec<&str> = content.splitn(3, '|').collect();
    if parts.len() != 3 {
        return None;
    }
    let title = parts[0].trim().to_string();
    let description = parts[1].trim().to_string();
    let plan = parts[2].trim().to_string();
    if title.is_empty() || description.is_empty() {
        return None;
    }
    Some((title, description, plan))
}

/// Strip all `LIMITATION:` lines from response text.
pub fn strip_limitation_markers(text: &str) -> String {
    text.lines()
        .filter(|line| !line.trim().starts_with("LIMITATION:"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------------------
// SELF_HEAL / SELF_HEAL_RESOLVED
// ---------------------------------------------------------------------------

/// State tracked in `~/.omega/self-healing.json` during active self-healing.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SelfHealingState {
    /// Description of the anomaly being healed.
    pub anomaly: String,
    /// Concrete verification test to confirm the fix works.
    pub verification: String,
    /// Current iteration (1-based).
    pub iteration: u32,
    /// Maximum iterations before escalation.
    pub max_iterations: u32,
    /// ISO 8601 timestamp when self-healing started.
    pub started_at: String,
    /// History of what was tried in each iteration.
    pub attempts: Vec<String>,
}

/// Extract the first `SELF_HEAL:` line from response text.
pub fn extract_self_heal_marker(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim().starts_with("SELF_HEAL:"))
        .map(|line| line.trim().to_string())
}

/// Parse the description and verification test from a `SELF_HEAL: description | verification` line.
pub fn parse_self_heal_line(line: &str) -> Option<(String, String)> {
    let content = line.strip_prefix("SELF_HEAL:")?.trim();
    let mut parts = content.splitn(2, '|');
    let description = parts.next()?.trim();
    let verification = parts.next()?.trim();
    if description.is_empty() || verification.is_empty() {
        return None;
    }
    Some((description.to_string(), verification.to_string()))
}

/// Check if response text contains a `SELF_HEAL_RESOLVED` marker line.
pub fn has_self_heal_resolved_marker(text: &str) -> bool {
    text.lines().any(|line| line.trim() == "SELF_HEAL_RESOLVED")
}

/// Strip all `SELF_HEAL:` and `SELF_HEAL_RESOLVED` lines from response text.
pub fn strip_self_heal_markers(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("SELF_HEAL:") && trimmed != "SELF_HEAL_RESOLVED"
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Return the path to `~/.omega/self-healing.json`.
pub fn self_healing_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(PathBuf::from(format!("{home}/.omega/self-healing.json")))
}

/// Read the current self-healing state from disk.
pub fn read_self_healing_state() -> Option<SelfHealingState> {
    let path = self_healing_path()?;
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Write the self-healing state to disk.
pub fn write_self_healing_state(state: &SelfHealingState) -> anyhow::Result<()> {
    let path = self_healing_path().ok_or_else(|| anyhow::anyhow!("HOME not set"))?;
    let json = serde_json::to_string_pretty(state)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Delete the self-healing state file.
pub fn delete_self_healing_state() -> anyhow::Result<()> {
    let path = self_healing_path().ok_or_else(|| anyhow::anyhow!("HOME not set"))?;
    if path.exists() {
        std::fs::remove_file(path)?;
    }
    Ok(())
}

/// Auto-detect the repo path from the running binary location.
///
/// The release binary lives at `{repo}/target/release/omega`, so we go up 3
/// levels and verify `Cargo.toml` exists. Returns `None` if the binary was
/// moved or installed elsewhere.
pub fn detect_repo_path() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let repo = exe.parent()?.parent()?.parent()?;
    if repo.join("Cargo.toml").exists() {
        Some(repo.to_string_lossy().to_string())
    } else {
        None
    }
}

/// Build the self-healing follow-up task description with repo context.
pub fn self_heal_follow_up(anomaly: &str, verification: &str) -> String {
    let repo_hint = detect_repo_path()
        .map(|p| {
            format!(
                " The source code is at {p}. \
                 Build with: nix --extra-experimental-features \
                 \"nix-command flakes\" develop --command bash -c \
                 \"cargo build --release && cargo clippy -- -D warnings\"."
            )
        })
        .unwrap_or_default();
    format!(
        "Self-healing verification — read ~/.omega/self-healing.json for context. \
         Run this verification: {verification}. \
         If the test passes, emit SELF_HEAL_RESOLVED. \
         If it fails, diagnose the root cause, fix it, build+clippy until clean, \
         restart service, update the attempts array in self-healing.json, \
         and emit SELF_HEAL: {anomaly} | {verification} to continue.{repo_hint}"
    )
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
pub fn save_attachments_to_inbox(
    inbox: &std::path::Path,
    attachments: &[omega_core::message::Attachment],
) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for attachment in attachments {
        if !matches!(
            attachment.file_type,
            omega_core::message::AttachmentType::Image
        ) {
            continue;
        }
        if let Some(ref data) = attachment.data {
            let filename = attachment
                .filename
                .as_deref()
                .unwrap_or("image.jpg")
                .to_string();
            let path = inbox.join(&filename);
            if std::fs::write(&path, data).is_ok() {
                paths.push(path);
            }
        }
    }
    paths
}

/// Delete inbox images after they have been processed.
pub fn cleanup_inbox_images(paths: &[PathBuf]) {
    for path in paths {
        let _ = std::fs::remove_file(path);
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

    // --- LIMITATION ---

    #[test]
    fn test_extract_limitation_marker() {
        let text =
            "I noticed an issue.\nLIMITATION: No email | Cannot send emails | Add SMTP provider";
        let result = extract_limitation_marker(text);
        assert_eq!(
            result,
            Some("LIMITATION: No email | Cannot send emails | Add SMTP provider".to_string())
        );
    }

    #[test]
    fn test_extract_limitation_marker_none() {
        let text = "Everything is working fine.";
        assert!(extract_limitation_marker(text).is_none());
    }

    #[test]
    fn test_parse_limitation_line() {
        let line = "LIMITATION: No email | Cannot send emails | Add SMTP provider";
        let result = parse_limitation_line(line).unwrap();
        assert_eq!(result.0, "No email");
        assert_eq!(result.1, "Cannot send emails");
        assert_eq!(result.2, "Add SMTP provider");
    }

    #[test]
    fn test_parse_limitation_line_invalid() {
        assert!(parse_limitation_line("LIMITATION: only one part").is_none());
        assert!(parse_limitation_line("not a limitation line").is_none());
        assert!(parse_limitation_line("LIMITATION:  | desc | plan").is_none());
    }

    #[test]
    fn test_strip_limitation_markers() {
        let text =
            "I found a gap.\nLIMITATION: No email | Cannot send | Add SMTP\nHope this helps.";
        let result = strip_limitation_markers(text);
        assert_eq!(result, "I found a gap.\nHope this helps.");
    }

    #[test]
    fn test_strip_limitation_markers_multiple() {
        let text = "Response.\nLIMITATION: A | B | C\nMore text.\nLIMITATION: D | E | F\nEnd.";
        let result = strip_limitation_markers(text);
        assert_eq!(result, "Response.\nMore text.\nEnd.");
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

    // --- SELF_HEAL ---

    #[test]
    fn test_extract_self_heal_marker() {
        let text = "Something is wrong.\nSELF_HEAL: Build pipeline broken | run cargo build and confirm exit code 0\nLet me fix it.";
        let result = extract_self_heal_marker(text);
        assert_eq!(
            result,
            Some(
                "SELF_HEAL: Build pipeline broken | run cargo build and confirm exit code 0"
                    .to_string()
            )
        );
    }

    #[test]
    fn test_extract_self_heal_marker_none() {
        let text = "Everything is working fine.";
        assert!(extract_self_heal_marker(text).is_none());
    }

    #[test]
    fn test_parse_self_heal_line() {
        let line = "SELF_HEAL: Build pipeline broken | run cargo build and confirm exit code 0";
        let (desc, verif) = parse_self_heal_line(line).unwrap();
        assert_eq!(desc, "Build pipeline broken");
        assert_eq!(verif, "run cargo build and confirm exit code 0");
    }

    #[test]
    fn test_parse_self_heal_line_empty() {
        assert!(parse_self_heal_line("SELF_HEAL:").is_none());
        assert!(parse_self_heal_line("SELF_HEAL:   ").is_none());
        assert!(parse_self_heal_line("not a self-heal line").is_none());
        assert!(parse_self_heal_line("SELF_HEAL: desc only").is_none());
        assert!(parse_self_heal_line("SELF_HEAL: desc |").is_none());
        assert!(parse_self_heal_line("SELF_HEAL: | verification").is_none());
    }

    #[test]
    fn test_has_self_heal_resolved_marker() {
        let text = "Fixed the issue.\nSELF_HEAL_RESOLVED\nAll good now.";
        assert!(has_self_heal_resolved_marker(text));
    }

    #[test]
    fn test_has_self_heal_resolved_marker_none() {
        let text = "No resolved marker here.";
        assert!(!has_self_heal_resolved_marker(text));
    }

    #[test]
    fn test_strip_self_heal_markers() {
        let text = "Detected issue.\nSELF_HEAL: Build broken | run cargo build\nFixing now.";
        let result = strip_self_heal_markers(text);
        assert_eq!(result, "Detected issue.\nFixing now.");
    }

    #[test]
    fn test_strip_self_heal_markers_resolved() {
        let text = "Fixed it.\nSELF_HEAL_RESOLVED\nAll done.";
        let result = strip_self_heal_markers(text);
        assert_eq!(result, "Fixed it.\nAll done.");
    }

    #[test]
    fn test_strip_self_heal_markers_both() {
        let text =
            "Start.\nSELF_HEAL: Bug found | run cargo test\nMiddle.\nSELF_HEAL_RESOLVED\nEnd.";
        let result = strip_self_heal_markers(text);
        assert_eq!(result, "Start.\nMiddle.\nEnd.");
    }

    #[test]
    fn test_self_healing_state_serde_roundtrip() {
        let state = SelfHealingState {
            anomaly: "Build broken".to_string(),
            verification: "run cargo build and confirm exit code 0".to_string(),
            iteration: 3,
            max_iterations: 10,
            started_at: "2026-02-18T12:00:00Z".to_string(),
            attempts: vec![
                "1: Tried restarting service".to_string(),
                "2: Fixed import path".to_string(),
                "3: Rebuilt binary".to_string(),
            ],
        };
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: SelfHealingState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.anomaly, "Build broken");
        assert_eq!(
            deserialized.verification,
            "run cargo build and confirm exit code 0"
        );
        assert_eq!(deserialized.iteration, 3);
        assert_eq!(deserialized.max_iterations, 10);
        assert_eq!(deserialized.attempts.len(), 3);
    }

    #[test]
    fn test_self_heal_full_flow_simulation() {
        let ai_response = "I found a bug in the audit system.\n\
                           SELF_HEAL: audit_log missing model field | query audit_log for last entry and confirm model is not null\n\
                           Investigating now.";

        let heal_line = extract_self_heal_marker(ai_response).unwrap();
        assert!(heal_line.contains("audit_log missing model field"));

        let (description, verification) = parse_self_heal_line(&heal_line).unwrap();
        assert_eq!(description, "audit_log missing model field");
        assert_eq!(
            verification,
            "query audit_log for last entry and confirm model is not null"
        );

        let state = SelfHealingState {
            anomaly: description.clone(),
            verification: verification.clone(),
            iteration: 1,
            max_iterations: 10,
            started_at: "2026-02-18T20:00:00Z".to_string(),
            attempts: Vec::new(),
        };

        let json = serde_json::to_string_pretty(&state).unwrap();
        assert!(json.contains("\"verification\""));
        let restored: SelfHealingState = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.verification, verification);

        let follow_up = self_heal_follow_up(&restored.anomaly, &restored.verification);
        assert!(follow_up.contains("Run this verification: query audit_log"));

        let cleaned = strip_self_heal_markers(ai_response);
        assert!(!cleaned.contains("SELF_HEAL:"));
        assert!(cleaned.contains("I found a bug"));
    }

    #[test]
    fn test_self_healing_state_old_format_graceful_fallback() {
        let old_json = r#"{
            "anomaly": "Build broken",
            "iteration": 3,
            "max_iterations": 10,
            "started_at": "2026-02-18T12:00:00Z",
            "attempts": ["1: tried X", "2: tried Y"]
        }"#;
        let result: Result<SelfHealingState, _> = serde_json::from_str(old_json);
        assert!(
            result.is_err(),
            "Old state without verification must fail to deserialize"
        );
    }

    #[test]
    fn test_self_heal_old_marker_format_rejected() {
        let old_response = "Found a bug.\nSELF_HEAL: Build pipeline broken\nFixing.";

        let heal_line = extract_self_heal_marker(old_response);
        assert!(heal_line.is_some());

        let parsed = parse_self_heal_line(&heal_line.unwrap());
        assert!(
            parsed.is_none(),
            "Old format without | must be rejected by parse_self_heal_line"
        );

        let cleaned = strip_self_heal_markers(old_response);
        assert!(!cleaned.contains("SELF_HEAL:"));
        assert_eq!(cleaned, "Found a bug.\nFixing.");
    }

    #[test]
    fn test_self_heal_verification_with_internal_pipes() {
        let line =
            "SELF_HEAL: DB error | run sqlite3 ~/.omega/memory.db 'SELECT count(*) | grep -v 0'";
        let (desc, verif) = parse_self_heal_line(line).unwrap();
        assert_eq!(desc, "DB error");
        assert_eq!(
            verif,
            "run sqlite3 ~/.omega/memory.db 'SELECT count(*) | grep -v 0'"
        );
    }

    #[test]
    fn test_detect_repo_path() {
        let result = detect_repo_path();
        if let Some(ref path) = result {
            assert!(
                PathBuf::from(path).join("Cargo.toml").exists(),
                "detected repo path should contain Cargo.toml"
            );
        }
    }

    #[test]
    fn test_self_heal_follow_up_content() {
        let desc = self_heal_follow_up("broken audit", "run cargo test");
        assert!(desc.contains("Run this verification: run cargo test"));
        assert!(desc.contains("SELF_HEAL: broken audit | run cargo test"));
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
    fn test_extract_cancel_task() {
        let text = "I'll cancel that.\nCANCEL_TASK: a1b2c3d4";
        assert_eq!(extract_cancel_task(text), Some("a1b2c3d4".to_string()));
    }

    #[test]
    fn test_extract_cancel_task_none() {
        assert!(extract_cancel_task("Just a normal response.").is_none());
    }

    #[test]
    fn test_extract_cancel_task_empty() {
        assert!(extract_cancel_task("CANCEL_TASK: ").is_none());
    }

    #[test]
    fn test_strip_cancel_task() {
        let text = "Cancelled.\nCANCEL_TASK: abc123\nDone.";
        assert_eq!(strip_cancel_task(text), "Cancelled.\nDone.");
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
