//! Claude Code CLI provider.
//!
//! Uses the locally installed `claude` CLI as a subprocess.
//! Zero API keys needed — relies on the user's existing `claude` authentication.

mod command;
mod mcp;
mod provider;
mod response;

#[cfg(test)]
mod tests;

use omega_core::config::SandboxMode;
use serde::Deserialize;
use std::path::PathBuf;
use std::time::Duration;
use tokio::process::Command;

pub use mcp::mcp_tool_patterns;

/// Default timeout for Claude Code CLI subprocess (60 minutes).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3600);

/// Claude Code CLI provider configuration.
pub struct ClaudeCodeProvider {
    /// Maximum agentic turns per invocation.
    max_turns: u32,
    /// Tools the CLI is allowed to use.
    allowed_tools: Vec<String>,
    /// Subprocess timeout.
    timeout: Duration,
    /// Working directory for the CLI subprocess (sandbox workspace).
    working_dir: Option<PathBuf>,
    /// Sandbox mode for OS-level filesystem enforcement.
    sandbox_mode: SandboxMode,
    /// Max auto-resume attempts when Claude hits max_turns.
    max_resume_attempts: u32,
    /// Default model to pass via `--model` (empty = let CLI decide).
    model: String,
}

/// JSON response from `claude -p --output-format json`.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ClaudeCliResponse {
    /// "result"
    #[serde(default, rename = "type")]
    response_type: Option<String>,
    /// "success", "error_max_turns", etc.
    #[serde(default)]
    subtype: Option<String>,
    /// The actual text response.
    #[serde(default)]
    result: Option<String>,
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    cost_usd: Option<f64>,
    #[serde(default)]
    total_cost_usd: Option<f64>,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    num_turns: Option<u32>,
}

impl ClaudeCodeProvider {
    /// Create a new Claude Code provider with default settings.
    pub fn new() -> Self {
        Self {
            max_turns: 100,
            allowed_tools: vec![],
            timeout: DEFAULT_TIMEOUT,
            working_dir: None,
            sandbox_mode: SandboxMode::default(),
            max_resume_attempts: 5,
            model: String::new(),
        }
    }

    /// Create a provider from config values.
    pub fn from_config(
        max_turns: u32,
        allowed_tools: Vec<String>,
        timeout_secs: u64,
        working_dir: Option<PathBuf>,
        sandbox_mode: SandboxMode,
        max_resume_attempts: u32,
        model: String,
    ) -> Self {
        Self {
            max_turns,
            allowed_tools,
            timeout: Duration::from_secs(timeout_secs),
            working_dir,
            sandbox_mode,
            max_resume_attempts,
            model,
        }
    }

    /// Check if the `claude` CLI is installed and accessible.
    pub async fn check_cli() -> bool {
        Command::new("claude")
            .arg("--version")
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

impl Default for ClaudeCodeProvider {
    fn default() -> Self {
        Self::new()
    }
}
