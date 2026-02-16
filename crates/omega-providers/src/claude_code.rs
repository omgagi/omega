//! Claude Code CLI provider.
//!
//! Uses the locally installed `claude` CLI as a subprocess.
//! Zero API keys needed — relies on the user's existing `claude` authentication.

use async_trait::async_trait;
use omega_core::{
    context::Context,
    error::OmegaError,
    message::{MessageMetadata, OutgoingMessage},
    traits::Provider,
};
use serde::Deserialize;
use std::time::Instant;
use tokio::process::Command;
use tracing::{debug, warn};

/// Claude Code CLI provider configuration.
pub struct ClaudeCodeProvider {
    /// Optional session ID for conversation continuity.
    session_id: Option<String>,
    /// Maximum agentic turns per invocation.
    max_turns: u32,
    /// Tools the CLI is allowed to use.
    allowed_tools: Vec<String>,
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
            session_id: None,
            max_turns: 10,
            allowed_tools: vec![
                "Bash".to_string(),
                "Read".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
            ],
        }
    }

    /// Create a provider from config values.
    pub fn from_config(max_turns: u32, allowed_tools: Vec<String>) -> Self {
        Self {
            session_id: None,
            max_turns,
            allowed_tools,
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

#[async_trait]
impl Provider for ClaudeCodeProvider {
    fn name(&self) -> &str {
        "claude-code"
    }

    fn requires_api_key(&self) -> bool {
        false
    }

    async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError> {
        let prompt = context.to_prompt_string();
        let start = Instant::now();

        let mut cmd = Command::new("claude");
        // Remove CLAUDECODE env var so the CLI doesn't think it's nested.
        cmd.env_remove("CLAUDECODE");
        cmd.arg("-p")
            .arg(&prompt)
            .arg("--output-format")
            .arg("json")
            .arg("--max-turns")
            .arg(self.max_turns.to_string());

        // Session continuity.
        if let Some(ref session) = self.session_id {
            cmd.arg("--session-id").arg(session);
        }

        // Allowed tools.
        for tool in &self.allowed_tools {
            cmd.arg("--allowedTools").arg(tool);
        }

        debug!("executing: claude -p <prompt> --output-format json");

        let output = cmd
            .output()
            .await
            .map_err(|e| OmegaError::Provider(format!("failed to run claude CLI: {e}")))?;

        let elapsed_ms = start.elapsed().as_millis() as u64;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OmegaError::Provider(format!(
                "claude CLI exited with {}: {stderr}",
                output.status
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Try to parse structured JSON response.
        let (text, model) = match serde_json::from_str::<ClaudeCliResponse>(&stdout) {
            Ok(resp) => {
                // Handle error_max_turns — still extract whatever result exists.
                if resp.subtype.as_deref() == Some("error_max_turns") {
                    warn!("claude hit max_turns limit ({} turns)", self.max_turns);
                }

                let text = match resp.result {
                    Some(r) if !r.is_empty() => r,
                    _ => {
                        // No result text — provide a meaningful fallback.
                        if resp.is_error {
                            format!(
                                "Error from Claude: {}",
                                resp.subtype.as_deref().unwrap_or("unknown")
                            )
                        } else {
                            "(No response text returned)".to_string()
                        }
                    }
                };
                (text, resp.model)
            }
            Err(e) => {
                // Fall back to raw text if JSON parsing fails.
                warn!("failed to parse claude JSON response: {e}");
                (stdout.trim().to_string(), None)
            }
        };

        Ok(OutgoingMessage {
            text,
            metadata: MessageMetadata {
                provider_used: "claude-code".to_string(),
                tokens_used: None,
                processing_time_ms: elapsed_ms,
                model,
            },
            reply_target: None,
        })
    }

    async fn is_available(&self) -> bool {
        Self::check_cli().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_provider() {
        let provider = ClaudeCodeProvider::new();
        assert_eq!(provider.name(), "claude-code");
        assert!(!provider.requires_api_key());
        assert_eq!(provider.max_turns, 10);
        assert_eq!(provider.allowed_tools.len(), 4);
    }
}
