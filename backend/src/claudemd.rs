//! Workspace CLAUDE.md maintenance — ensures the Claude Code subprocess
//! has persistent project context in its working directory.
//!
//! Uses a **template-first** approach:
//! - A bundled template (`prompts/WORKSPACE_CLAUDE.md`) contains standard
//!   operational rules that survive across deployments and refreshes.
//! - Dynamic content (skills/projects tables) is appended below a marker
//!   line by `claude -p`.
//!
//! Two operations:
//! - **`ensure_claudemd`**: On startup, if `~/.omega/workspace/CLAUDE.md` doesn't
//!   exist, writes the bundled template then runs `claude -p` to append dynamic content.
//! - **`claudemd_loop`**: Background loop that re-deploys the template (preserving
//!   dynamic content) and asks Claude Code to update the dynamic sections.
//!
//! Both use direct subprocess calls (not the Provider trait) since this is a
//! meta-operation for workspace maintenance, not a user message.

use omega_core::config::bundled_workspace_claude;
use std::path::Path;
use tracing::{info, warn};

/// Timeout for CLAUDE.md maintenance calls (2 minutes).
const CLAUDEMD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Marker line that separates bundled template from dynamic content.
const DYNAMIC_MARKER: &str = "<!-- DYNAMIC CONTENT BELOW";

/// Ensure `CLAUDE.md` exists in the workspace directory.
///
/// If the file is missing, writes the bundled template first (guaranteeing
/// standard rules are present even if `claude -p` fails), then spawns
/// `claude -p` to append dynamic skills/projects content.
/// Non-fatal: logs a warning on failure, never blocks startup.
pub async fn ensure_claudemd(workspace: &Path, data_dir: &Path) {
    let claudemd_path = workspace.join("CLAUDE.md");
    if claudemd_path.exists() {
        info!("workspace CLAUDE.md already exists, skipping init");
        return;
    }

    info!("workspace CLAUDE.md missing, deploying bundled template");

    // Step 1: Write the bundled template — standard rules guaranteed.
    if let Err(e) = std::fs::write(&claudemd_path, bundled_workspace_claude()) {
        warn!("failed to write bundled CLAUDE.md template: {e}");
        return;
    }
    info!("deployed bundled CLAUDE.md template to workspace");

    // Step 2: Enrich with dynamic content via claude -p.
    let prompt = format!(
        "A CLAUDE.md exists in your working directory with standard operational rules. \
         Explore `{data_dir}/skills/` (SKILL.md files) and `{data_dir}/projects/` \
         (ROLE.md files). APPEND two markdown tables at the END of the file \
         (below the existing DYNAMIC CONTENT marker line):\n\
         1. '## Available Skills' — table with Skill and Purpose columns\n\
         2. '## Available Projects' — table with Project and Purpose columns\n\n\
         DO NOT modify or remove any existing sections above the DYNAMIC CONTENT marker. \
         Only append below it.",
        data_dir = data_dir.display(),
    );

    if let Err(e) = run_claude(&prompt, workspace, data_dir).await {
        warn!("failed to enrich CLAUDE.md with dynamic content: {e} (template still deployed)");
    }
}

/// Refresh the workspace CLAUDE.md — re-deploy bundled template, preserve
/// dynamic content, then ask Claude Code to update dynamic sections.
///
/// Non-fatal: logs a warning on failure.
pub async fn refresh_claudemd(workspace: &Path, data_dir: &Path) {
    let claudemd_path = workspace.join("CLAUDE.md");
    if !claudemd_path.exists() {
        // If somehow deleted between runs, re-create from scratch.
        ensure_claudemd(workspace, data_dir).await;
        return;
    }

    info!("refreshing workspace CLAUDE.md");

    // Step 1: Read current file and extract dynamic content below the marker.
    let current = match std::fs::read_to_string(&claudemd_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("failed to read CLAUDE.md for refresh: {e}");
            return;
        }
    };

    let dynamic_content = extract_dynamic_content(&current);

    // Step 2: Re-deploy template + preserved dynamic content.
    let template = bundled_workspace_claude();
    let refreshed = match dynamic_content {
        Some(dynamic) => format!("{template}\n{dynamic}"),
        None => template.to_string(),
    };

    if let Err(e) = std::fs::write(&claudemd_path, &refreshed) {
        warn!("failed to write refreshed CLAUDE.md: {e}");
        return;
    }
    info!("re-deployed bundled template (preserved dynamic content)");

    // Step 3: Ask claude -p to update dynamic sections.
    let prompt = format!(
        "Review and update the Available Skills and Available Projects sections \
         in the CLAUDE.md file in your working directory. \
         Check `{data_dir}/skills/` and `{data_dir}/projects/` for changes. \
         Update ONLY the content below the '<!-- DYNAMIC CONTENT BELOW' marker line. \
         DO NOT modify anything above it.",
        data_dir = data_dir.display(),
    );

    if let Err(e) = run_claude(&prompt, workspace, data_dir).await {
        warn!("failed to update dynamic content in CLAUDE.md: {e} (template preserved)");
    }
}

/// Extract everything below the dynamic content marker line.
///
/// Returns `Some(content)` if the marker is found and there is content below it,
/// `None` otherwise.
fn extract_dynamic_content(file_content: &str) -> Option<String> {
    let marker_pos = file_content.find(DYNAMIC_MARKER)?;
    // Find the end of the marker line.
    let after_marker = &file_content[marker_pos..];
    let line_end = after_marker.find('\n').map(|i| marker_pos + i + 1)?;
    let dynamic = file_content[line_end..].trim();
    if dynamic.is_empty() {
        None
    } else {
        Some(dynamic.to_string())
    }
}

/// Background loop that periodically refreshes the workspace CLAUDE.md.
///
/// Sleeps for `interval_hours` between refreshes. Runs indefinitely until
/// the task is aborted on shutdown.
pub async fn claudemd_loop(
    workspace: std::path::PathBuf,
    data_dir: std::path::PathBuf,
    interval_hours: u64,
) {
    let interval = std::time::Duration::from_secs(interval_hours * 3600);
    loop {
        tokio::time::sleep(interval).await;
        refresh_claudemd(&workspace, &data_dir).await;
    }
}

/// Run `claude -p` as a direct subprocess for CLAUDE.md maintenance.
async fn run_claude(prompt: &str, workspace: &Path, data_dir: &Path) -> Result<(), String> {
    let mut cmd = omega_sandbox::protected_command("claude", data_dir);
    cmd.current_dir(workspace)
        .env_remove("CLAUDECODE")
        .arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("json")
        // Required: CLI runs non-interactively (no user to approve permission prompts).
        // Scope is limited: sandboxed via protected_command, only updates CLAUDE.md in workspace.
        .arg("--dangerously-skip-permissions");

    let output = tokio::time::timeout(CLAUDEMD_TIMEOUT, cmd.output())
        .await
        .map_err(|_| "claude CLI timed out after 120s".to_string())?
        .map_err(|e| format!("failed to spawn claude CLI: {e}"))?;

    if output.status.success() {
        info!("workspace CLAUDE.md maintenance completed successfully");
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "claude CLI exited with {}: {}",
            output.status,
            stderr.chars().take(200).collect::<String>()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[tokio::test]
    async fn test_ensure_claudemd_skips_when_exists() {
        let tmp = std::env::temp_dir().join("omega_test_claudemd");
        let _ = std::fs::create_dir_all(&tmp);
        // Create a fake CLAUDE.md so ensure_claudemd skips.
        std::fs::write(tmp.join("CLAUDE.md"), "# Test").unwrap();

        // Should return immediately without error (no claude CLI needed).
        ensure_claudemd(&tmp, &tmp).await;

        // Cleanup.
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_claudemd_path_construction() {
        let workspace = PathBuf::from("/home/user/.omega/workspace");
        let expected = workspace.join("CLAUDE.md");
        assert_eq!(
            expected.to_string_lossy(),
            "/home/user/.omega/workspace/CLAUDE.md"
        );
    }

    #[test]
    fn test_template_contains_standard_sections() {
        let template = bundled_workspace_claude();
        assert!(
            template.contains("# OMEGA Workspace"),
            "should have main heading"
        );
        assert!(
            template.contains("## Directory Structure"),
            "should have directory structure"
        );
        assert!(
            template.contains("## Your Infrastructure"),
            "should have infrastructure section"
        );
        assert!(
            template.contains("## Diagnostic Protocol"),
            "should have diagnostic protocol"
        );
        assert!(
            template.contains("## Known False Diagnoses"),
            "should have known false diagnoses"
        );
        assert!(
            template.contains("## Key Conventions"),
            "should have key conventions"
        );
        assert!(
            template.contains(DYNAMIC_MARKER),
            "should have dynamic content marker"
        );
    }

    #[test]
    fn test_extract_dynamic_content_with_content() {
        let file = "# Standard Rules\n\n\
                     <!-- DYNAMIC CONTENT BELOW — auto-generated, do not edit above this line -->\n\n\
                     ## Available Skills\n\n\
                     | Skill | Purpose |\n\
                     |-------|---------|";
        let dynamic = extract_dynamic_content(file);
        assert!(dynamic.is_some());
        let content = dynamic.unwrap();
        assert!(content.contains("## Available Skills"));
        assert!(content.contains("| Skill | Purpose |"));
    }

    #[test]
    fn test_extract_dynamic_content_empty() {
        let file = "# Standard Rules\n\n\
                     <!-- DYNAMIC CONTENT BELOW — auto-generated, do not edit above this line -->\n";
        let dynamic = extract_dynamic_content(file);
        assert!(
            dynamic.is_none(),
            "empty dynamic section should return None"
        );
    }

    #[test]
    fn test_extract_dynamic_content_no_marker() {
        let file = "# Standard Rules\n\nSome content without a marker.";
        let dynamic = extract_dynamic_content(file);
        assert!(dynamic.is_none(), "no marker should return None");
    }

    #[test]
    fn test_refresh_preserves_template_sections() {
        // Simulate a file with template + dynamic content.
        let template = bundled_workspace_claude();
        let dynamic = "\n## Available Skills\n\n| Skill | Purpose |\n|-------|---------|";
        let full_file = format!("{template}\n{dynamic}");

        // Extract dynamic content.
        let extracted = extract_dynamic_content(&full_file);
        assert!(extracted.is_some());

        // Reconstruct: template + extracted dynamic.
        let refreshed = format!("{template}\n{}", extracted.unwrap());

        // Verify template sections survived.
        assert!(refreshed.contains("## Diagnostic Protocol"));
        assert!(refreshed.contains("## Known False Diagnoses"));
        assert!(refreshed.contains("## Key Conventions"));
        // Verify dynamic content survived.
        assert!(refreshed.contains("## Available Skills"));
    }
}
