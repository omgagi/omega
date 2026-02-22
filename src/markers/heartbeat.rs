//! Heartbeat markers: HEARTBEAT_ADD, HEARTBEAT_REMOVE, HEARTBEAT_INTERVAL,
//! and heartbeat file operations.

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

/// Read `~/.omega/prompts/HEARTBEAT.md` if it exists.
pub fn read_heartbeat_file() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = format!("{home}/.omega/prompts/HEARTBEAT.md");
    let content = std::fs::read_to_string(path).ok()?;
    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}

/// Apply heartbeat add/remove actions to `~/.omega/prompts/HEARTBEAT.md`.
///
/// Creates the file if missing. Prevents duplicate adds. Uses case-insensitive
/// partial matching for removes. Skips comment lines (`#`) during removal.
pub fn apply_heartbeat_changes(actions: &[HeartbeatAction]) {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return,
    };
    let path = format!("{home}/.omega/prompts/HEARTBEAT.md");

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
    let dir = format!("{home}/.omega/prompts");
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
