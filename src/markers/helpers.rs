//! Miscellaneous helpers: status messages, provider errors, workspace images,
//! active hours, classification, and inbox operations.

use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Status messages
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

// ---------------------------------------------------------------------------
// Workspace images
// ---------------------------------------------------------------------------

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
