//! SCHEDULE and SCHEDULE_ACTION marker extraction, parsing, and stripping.

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
