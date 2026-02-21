//! Workspace CLAUDE.md maintenance — ensures the Claude Code subprocess
//! has persistent project context in its working directory.
//!
//! Two operations:
//! - **`ensure_claudemd`**: On startup, if `~/.omega/workspace/CLAUDE.md` doesn't exist,
//!   runs `claude -p` with an init prompt to create one.
//! - **`claudemd_loop`**: Background loop that periodically asks Claude Code to review
//!   and update the existing CLAUDE.md.
//!
//! Both use direct subprocess calls (not the Provider trait) since this is a
//! meta-operation for workspace maintenance, not a user message.

use omega_core::config::SandboxMode;
use std::path::Path;
use tracing::{info, warn};

/// Timeout for CLAUDE.md maintenance calls (2 minutes).
const CLAUDEMD_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// Ensure `CLAUDE.md` exists in the workspace directory.
///
/// If the file is missing, spawns `claude -p` with an init prompt that asks
/// Claude Code to explore the workspace and create a useful CLAUDE.md.
/// Non-fatal: logs a warning on failure, never blocks startup.
pub async fn ensure_claudemd(workspace: &Path, data_dir: &Path, sandbox_mode: SandboxMode) {
    let claudemd_path = workspace.join("CLAUDE.md");
    if claudemd_path.exists() {
        info!("workspace CLAUDE.md already exists, skipping init");
        return;
    }

    info!("workspace CLAUDE.md missing, creating via claude CLI");
    let prompt = format!(
        "You are initializing a workspace for an AI agent called OMEGA. \
         Your working directory is {workspace}. \
         Explore the workspace and its sibling directories ({data_dir}) — \
         look at skills/ (SKILL.md files), projects/ (ROLE.md files), \
         and any other files present. Then create a CLAUDE.md file in the \
         current directory that describes:\n\
         1. What this workspace is (OMEGA's sandboxed working directory)\n\
         2. Available skills and their purposes (from ../skills/*/SKILL.md)\n\
         3. Available projects (from ../projects/*/ROLE.md)\n\
         4. Key conventions (sandbox mode, file locations)\n\n\
         Keep it concise — under 100 lines. This file will be read by future \
         Claude Code invocations to have workspace context.",
        workspace = workspace.display(),
        data_dir = data_dir.display(),
    );

    if let Err(e) = run_claude(&prompt, workspace, data_dir, sandbox_mode).await {
        warn!("failed to create workspace CLAUDE.md: {e}");
    }
}

/// Ask Claude Code to review and update the existing workspace CLAUDE.md.
///
/// Non-fatal: logs a warning on failure.
pub async fn refresh_claudemd(workspace: &Path, data_dir: &Path, sandbox_mode: SandboxMode) {
    let claudemd_path = workspace.join("CLAUDE.md");
    if !claudemd_path.exists() {
        // If somehow deleted between runs, re-create instead of update.
        ensure_claudemd(workspace, data_dir, sandbox_mode).await;
        return;
    }

    info!("refreshing workspace CLAUDE.md");
    let prompt = format!(
        "Review and update the CLAUDE.md file in your current directory. \
         Check if the workspace has changed — new skills in {data_dir}/skills/, \
         new projects in {data_dir}/projects/, new files in the workspace. \
         Update the CLAUDE.md to reflect the current state. \
         If nothing has changed, leave it as-is. Keep it concise — under 100 lines.",
        data_dir = data_dir.display(),
    );

    if let Err(e) = run_claude(&prompt, workspace, data_dir, sandbox_mode).await {
        warn!("failed to refresh workspace CLAUDE.md: {e}");
    }
}

/// Background loop that periodically refreshes the workspace CLAUDE.md.
///
/// Sleeps for `interval_hours` between refreshes. Runs indefinitely until
/// the task is aborted on shutdown.
pub async fn claudemd_loop(
    workspace: std::path::PathBuf,
    data_dir: std::path::PathBuf,
    sandbox_mode: SandboxMode,
    interval_hours: u64,
) {
    let interval = std::time::Duration::from_secs(interval_hours * 3600);
    loop {
        tokio::time::sleep(interval).await;
        refresh_claudemd(&workspace, &data_dir, sandbox_mode).await;
    }
}

/// Run `claude -p` as a direct subprocess for CLAUDE.md maintenance.
async fn run_claude(
    prompt: &str,
    workspace: &Path,
    data_dir: &Path,
    sandbox_mode: SandboxMode,
) -> Result<(), String> {
    let mut cmd = omega_sandbox::sandboxed_command("claude", sandbox_mode, data_dir);
    cmd.current_dir(workspace)
        .env_remove("CLAUDECODE")
        .arg("-p")
        .arg(prompt)
        .arg("--output-format")
        .arg("json")
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
        ensure_claudemd(&tmp, &tmp, SandboxMode::Rwx).await;

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
}
