//! Heartbeat markers: HEARTBEAT_ADD, HEARTBEAT_REMOVE, HEARTBEAT_INTERVAL,
//! HEARTBEAT_SUPPRESS_SECTION, HEARTBEAT_UNSUPPRESS_SECTION,
//! and heartbeat file operations (global and per-project).

use tracing::{info, warn};

/// Action extracted from a `HEARTBEAT_ADD:`, `HEARTBEAT_REMOVE:`, or `HEARTBEAT_INTERVAL:` marker.
#[derive(Debug, Clone, PartialEq)]
pub enum HeartbeatAction {
    Add(String),
    Remove(String),
    /// Dynamically change the heartbeat interval (in minutes, 1–1440).
    SetInterval(u64),
}

/// Action extracted from `HEARTBEAT_SUPPRESS_SECTION:` or `HEARTBEAT_UNSUPPRESS_SECTION:` markers.
#[derive(Debug, Clone, PartialEq)]
pub enum SuppressAction {
    Suppress(String),
    Unsuppress(String),
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

/// Read a project-specific heartbeat file at `~/.omega/projects/<name>/HEARTBEAT.md`.
pub fn read_project_heartbeat_file(project_name: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = format!("{home}/.omega/projects/{project_name}/HEARTBEAT.md");
    let content = std::fs::read_to_string(path).ok()?;
    if content.trim().is_empty() {
        None
    } else {
        Some(content)
    }
}

/// Apply heartbeat add/remove actions to the appropriate heartbeat file.
///
/// When `project` is None, writes to `~/.omega/prompts/HEARTBEAT.md` (global).
/// When `project` is Some, writes to `~/.omega/projects/<name>/HEARTBEAT.md`.
/// Creates the file if missing. Prevents duplicate adds. Uses case-insensitive
/// partial matching for removes. Skips comment lines (`#`) during removal.
pub fn apply_heartbeat_changes(actions: &[HeartbeatAction], project: Option<&str>) {
    let home = match std::env::var("HOME") {
        Ok(h) => h,
        Err(_) => return,
    };

    let (path, dir) = match project {
        Some(name) => (
            format!("{home}/.omega/projects/{name}/HEARTBEAT.md"),
            format!("{home}/.omega/projects/{name}"),
        ),
        None => (
            format!("{home}/.omega/prompts/HEARTBEAT.md"),
            format!("{home}/.omega/prompts"),
        ),
    };

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
// Section suppression — code-level gate for heartbeat sections (REQ-HB-010..014)
// ---------------------------------------------------------------------------

/// Extract section name from a `## HEADER` line.
///
/// Given `## TRADING — Autonomous Quant-Driven Execution Engine`, returns `TRADING`.
/// Given `## NON-TRADING ITEMS`, returns `NON-TRADING ITEMS`.
/// Text before the first ` — ` (em dash) is used; if no em dash, the full header text.
fn extract_section_name(header_line: &str) -> String {
    let text = header_line.trim().trim_start_matches('#').trim_start();
    // Split on ` — ` (space-emdash-space) to get just the section name.
    text.split(" — ").next().unwrap_or(text).trim().to_string()
}

/// Parse heartbeat content into (preamble, sections).
///
/// Sections are delimited by `## ` headers. Content before the first `##` is
/// preamble (never suppressed). Each section includes its header line.
pub fn parse_heartbeat_sections(content: &str) -> (String, Vec<(String, String)>) {
    let mut preamble = String::new();
    let mut sections: Vec<(String, String)> = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_body = String::new();

    for line in content.lines() {
        if line.starts_with("## ") {
            // Flush previous section.
            if let Some(name) = current_name.take() {
                sections.push((name, std::mem::take(&mut current_body)));
            }
            current_name = Some(extract_section_name(line));
            current_body = format!("{line}\n");
        } else if current_name.is_some() {
            current_body.push_str(line);
            current_body.push('\n');
        } else {
            preamble.push_str(line);
            preamble.push('\n');
        }
    }
    // Flush last section.
    if let Some(name) = current_name {
        sections.push((name, current_body));
    }

    (preamble, sections)
}

/// Build the suppress-file path for global or project heartbeat.
fn suppress_file_path(project: Option<&str>) -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    Some(match project {
        Some(name) if !name.is_empty() => {
            format!("{home}/.omega/projects/{name}/HEARTBEAT.suppress")
        }
        _ => format!("{home}/.omega/prompts/HEARTBEAT.suppress"),
    })
}

/// Read suppressed section names from the `.suppress` companion file.
///
/// Returns an empty vec if the file doesn't exist (all sections active).
pub fn read_suppress_file(project: Option<&str>) -> Vec<String> {
    let path = match suppress_file_path(project) {
        Some(p) => p,
        None => return Vec::new(),
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => content
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Add a section name to the suppress file (no duplicates, case-insensitive).
pub fn add_suppression(section: &str, project: Option<&str>) {
    let path = match suppress_file_path(project) {
        Some(p) => p,
        None => return,
    };
    let mut entries = read_suppress_file(project);
    let already = entries.iter().any(|e| e.eq_ignore_ascii_case(section));
    if already {
        return;
    }
    entries.push(section.to_string());

    // Ensure parent directory exists.
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Err(e) = std::fs::write(&path, entries.join("\n") + "\n") {
        warn!("heartbeat: failed to write suppress file: {e}");
        return;
    }
    info!("heartbeat: suppressed section '{section}'");
}

/// Remove a section name from the suppress file (case-insensitive).
pub fn remove_suppression(section: &str, project: Option<&str>) {
    let path = match suppress_file_path(project) {
        Some(p) => p,
        None => return,
    };
    let entries = read_suppress_file(project);
    let filtered: Vec<&String> = entries
        .iter()
        .filter(|e| !e.eq_ignore_ascii_case(section))
        .collect();
    if filtered.len() == entries.len() {
        return; // Nothing to remove.
    }
    let content: String = filtered.iter().map(|e| format!("{e}\n")).collect();
    if let Err(e) = std::fs::write(&path, content) {
        warn!("heartbeat: failed to write suppress file: {e}");
        return;
    }
    info!("heartbeat: unsuppressed section '{section}'");
}

/// Filter out suppressed sections from heartbeat content.
///
/// Reads the suppress file for the given project scope, parses the content
/// into sections, and returns only the preamble + non-suppressed sections.
/// Returns `None` if all sections are suppressed (empty checklist).
pub fn filter_suppressed_sections(content: &str, project: Option<&str>) -> Option<String> {
    let suppressed = read_suppress_file(project);
    if suppressed.is_empty() {
        return Some(content.to_string());
    }

    let (preamble, sections) = parse_heartbeat_sections(content);

    // Log unmatched suppress entries (stale config).
    for entry in &suppressed {
        let matched = sections
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(entry));
        if !matched {
            warn!("heartbeat: suppress entry '{entry}' does not match any section in HEARTBEAT.md");
        }
    }

    let preamble_empty = preamble.trim().is_empty();
    let mut result = preamble;
    let mut any_active = false;
    for (name, body) in &sections {
        let is_suppressed = suppressed.iter().any(|s| s.eq_ignore_ascii_case(name));
        if is_suppressed {
            info!("heartbeat: skipping suppressed section '{name}'");
        } else {
            result.push_str(body);
            any_active = true;
        }
    }

    if any_active {
        Some(result)
    } else if preamble_empty {
        None
    } else {
        // Only preamble remains — treat as empty checklist.
        None
    }
}

/// Extract `HEARTBEAT_SUPPRESS_SECTION:` and `HEARTBEAT_UNSUPPRESS_SECTION:` markers.
pub fn extract_suppress_section_markers(text: &str) -> Vec<SuppressAction> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if let Some(name) = trimmed.strip_prefix("HEARTBEAT_SUPPRESS_SECTION:") {
                let name = name.trim();
                if name.is_empty() {
                    None
                } else {
                    Some(SuppressAction::Suppress(name.to_string()))
                }
            } else if let Some(name) = trimmed.strip_prefix("HEARTBEAT_UNSUPPRESS_SECTION:") {
                let name = name.trim();
                if name.is_empty() {
                    None
                } else {
                    Some(SuppressAction::Unsuppress(name.to_string()))
                }
            } else {
                None
            }
        })
        .collect()
}

/// Strip `HEARTBEAT_SUPPRESS_SECTION:` and `HEARTBEAT_UNSUPPRESS_SECTION:` lines from text.
pub fn strip_suppress_section_markers(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim();
            !trimmed.starts_with("HEARTBEAT_SUPPRESS_SECTION:")
                && !trimmed.starts_with("HEARTBEAT_UNSUPPRESS_SECTION:")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Apply suppress/unsuppress actions from extracted markers.
pub fn apply_suppress_actions(actions: &[SuppressAction], project: Option<&str>) {
    for action in actions {
        match action {
            SuppressAction::Suppress(name) => add_suppression(name, project),
            SuppressAction::Unsuppress(name) => remove_suppression(name, project),
        }
    }
}

/// Strip sections from global heartbeat content that belong to active projects
/// with their own `HEARTBEAT.md`.
///
/// Prevents duplicate execution: if a project has its own heartbeat file, its
/// section in the global checklist is redundant and should be removed before
/// the global phase runs.
///
/// Matching is case-insensitive: section name is normalized (lowercased,
/// hyphens/underscores → spaces) and checked for containment of the project
/// name (also normalized).
///
/// Returns `None` if all sections are stripped (global phase should be skipped).
pub fn strip_project_sections(content: &str, project_names: &[String]) -> Option<String> {
    if project_names.is_empty() {
        return Some(content.to_string());
    }

    let normalized_projects: Vec<String> = project_names
        .iter()
        .map(|p| p.to_lowercase().replace(['-', '_'], " "))
        .collect();

    let (preamble, sections) = parse_heartbeat_sections(content);
    let mut result = preamble;
    let mut any_active = false;

    for (name, body) in &sections {
        let norm_section = name.to_lowercase().replace(['-', '_'], " ");
        let is_project_section = normalized_projects
            .iter()
            .any(|proj| norm_section.contains(proj.as_str()));
        if is_project_section {
            info!(
                "heartbeat: stripping global section '{}' (covered by project heartbeat)",
                name
            );
        } else {
            result.push_str(body);
            any_active = true;
        }
    }

    if any_active {
        Some(result)
    } else {
        None
    }
}
