//! CLI command building and subprocess execution.

use super::ClaudeCodeProvider;
use omega_core::error::OmegaError;
use tokio::process::Command;
use tracing::debug;

impl ClaudeCodeProvider {
    /// Run the claude CLI subprocess with a timeout.
    #[allow(clippy::too_many_arguments)]
    pub(super) async fn run_cli(
        &self,
        prompt: &str,
        extra_allowed_tools: &[String],
        max_turns: u32,
        allowed_tools: &[String],
        model: &str,
        context_disabled_tools: bool,
        session_id: Option<&str>,
    ) -> Result<std::process::Output, OmegaError> {
        let mut cmd = self.base_command();

        cmd.arg("-p")
            .arg(prompt)
            .arg("--output-format")
            .arg("json")
            .arg("--max-turns")
            .arg(max_turns.to_string());

        // Model override.
        if !model.is_empty() {
            cmd.arg("--model").arg(model);
        }

        // Session continuity: --resume resumes an existing conversation by session ID.
        if let Some(sid) = session_id {
            cmd.arg("--resume").arg(sid);
        }

        // Tool permissions: In `-p` (non-interactive) mode, Claude Code
        // cannot prompt for approval — tools must be pre-approved or
        // permissions bypassed entirely.
        //
        // - `context_disabled_tools` = caller wants NO tools (classification).
        // - `allowed_tools` empty = full access intended → bypass
        //   permissions so every tool works autonomously (the OS-level
        //   sandbox provides the real security boundary). MCP patterns
        //   are still appended so those servers are discoverable.
        // - `allowed_tools` non-empty = explicit whitelist → pre-approve
        //   only those tools (plus any MCP patterns).
        if context_disabled_tools {
            cmd.arg("--allowedTools").arg("");
        } else if allowed_tools.is_empty() {
            cmd.arg("--dangerously-skip-permissions");
            // MCP tool patterns still needed so Claude knows about them.
            for tool in extra_allowed_tools {
                cmd.arg("--allowedTools").arg(tool);
            }
        } else {
            for tool in allowed_tools {
                cmd.arg("--allowedTools").arg(tool);
            }
            for tool in extra_allowed_tools {
                cmd.arg("--allowedTools").arg(tool);
            }
        }

        debug!("executing: claude -p <prompt> --output-format json");
        self.execute_with_timeout(cmd, "claude CLI").await
    }

    /// Run the claude CLI subprocess with a specific session ID (for auto-resume).
    pub(super) async fn run_cli_with_session(
        &self,
        prompt: &str,
        extra_allowed_tools: &[String],
        session_id: &str,
        max_turns: u32,
        allowed_tools: &[String],
        model: &str,
    ) -> Result<std::process::Output, OmegaError> {
        let mut cmd = self.base_command();

        cmd.arg("-p")
            .arg(prompt)
            .arg("--output-format")
            .arg("json")
            .arg("--max-turns")
            .arg(max_turns.to_string())
            .arg("--session-id")
            .arg(session_id);

        // Model override.
        if !model.is_empty() {
            cmd.arg("--model").arg(model);
        }

        // Same permission logic as run_cli: bypass when full access,
        // otherwise pre-approve only the listed tools.
        if allowed_tools.is_empty() {
            cmd.arg("--dangerously-skip-permissions");
            for tool in extra_allowed_tools {
                cmd.arg("--allowedTools").arg(tool);
            }
        } else {
            for tool in allowed_tools {
                cmd.arg("--allowedTools").arg(tool);
            }
            for tool in extra_allowed_tools {
                cmd.arg("--allowedTools").arg(tool);
            }
        }

        debug!("executing: claude -p <resume> --session-id {session_id}");
        self.execute_with_timeout(cmd, "claude CLI resume").await
    }

    /// Build the base `Command` with working directory and sandbox settings.
    fn base_command(&self) -> Command {
        let mut cmd = match self.working_dir {
            Some(ref dir) => {
                // Sandbox protects the data dir (parent of workspace) so
                // skills, projects, etc. are writable — not just workspace.
                let data_dir = dir.parent().unwrap_or(dir);
                let mut c = omega_sandbox::sandboxed_command("claude", self.sandbox_mode, data_dir);
                c.current_dir(dir);
                c
            }
            None => Command::new("claude"),
        };
        // Remove CLAUDECODE env var so the CLI doesn't think it's nested.
        cmd.env_remove("CLAUDECODE");
        cmd
    }

    /// Execute a command with the configured timeout and standard error handling.
    async fn execute_with_timeout(
        &self,
        mut cmd: Command,
        label: &str,
    ) -> Result<std::process::Output, OmegaError> {
        let output = tokio::time::timeout(self.timeout, cmd.output())
            .await
            .map_err(|_| {
                OmegaError::Provider(format!(
                    "{label} timed out after {}s",
                    self.timeout.as_secs()
                ))
            })?
            .map_err(|e| OmegaError::Provider(format!("failed to run {label}: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OmegaError::Provider(format!(
                "{label} exited with {}: {stderr}",
                output.status
            )));
        }

        Ok(output)
    }
}
