//! Claude Code CLI provider.
//!
//! Uses the locally installed `claude` CLI as a subprocess.
//! Zero API keys needed — relies on the user's existing `claude` authentication.

use async_trait::async_trait;
use omega_core::{
    config::SandboxMode,
    context::{Context, McpServer},
    error::OmegaError,
    message::{MessageMetadata, OutgoingMessage},
    traits::Provider,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// Default timeout for Claude Code CLI subprocess (60 minutes).
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3600);

/// Claude Code CLI provider configuration.
pub struct ClaudeCodeProvider {
    /// Optional session ID for conversation continuity.
    session_id: Option<String>,
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
            session_id: None,
            max_turns: 100,
            allowed_tools: vec![
                "Bash".to_string(),
                "Read".to_string(),
                "Write".to_string(),
                "Edit".to_string(),
            ],
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
            session_id: None,
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

        // Write MCP settings if any servers are declared.
        let mcp_settings_path = if !context.mcp_servers.is_empty() {
            if let Some(ref dir) = self.working_dir {
                match write_mcp_settings(dir, &context.mcp_servers) {
                    Ok(path) => Some(path),
                    Err(e) => {
                        warn!("failed to write MCP settings: {e}");
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        let extra_tools = mcp_tool_patterns(&context.mcp_servers);

        // Resolve effective max_turns, allowed_tools, and model from context overrides.
        let effective_max_turns = context.max_turns.unwrap_or(self.max_turns);
        let tools_disabled = matches!(&context.allowed_tools, Some(t) if t.is_empty());
        let effective_tools: Vec<String> = context
            .allowed_tools
            .clone()
            .unwrap_or_else(|| self.allowed_tools.clone());
        let effective_model = context.model.as_deref().unwrap_or(&self.model);

        // First call with original prompt.
        let result = self
            .run_cli(
                &prompt,
                &extra_tools,
                effective_max_turns,
                &effective_tools,
                effective_model,
                tools_disabled,
            )
            .await;

        // Always cleanup MCP settings, regardless of success or failure.
        if let Some(ref path) = mcp_settings_path {
            cleanup_mcp_settings(path);
        }

        let output = result?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let (mut text, mut model) = self.parse_response(&stdout);
        // CLI doesn't always echo the model back — fall back to what we requested.
        if model.is_none() && !effective_model.is_empty() {
            model = Some(effective_model.to_string());
        }

        // Auto-resume: if Claude hit max_turns and returned a session_id, retry.
        // Skip auto-resume when max_turns was explicitly set by the caller (e.g., planning calls).
        let parsed: Option<ClaudeCliResponse> = serde_json::from_str(&stdout).ok();
        if context.max_turns.is_some() {
            // Explicit max_turns override — caller controls the limit, no auto-resume.
        } else if let Some(ref resp) = parsed {
            if resp.subtype.as_deref() == Some("error_max_turns") {
                if let Some(ref session_id) = resp.session_id {
                    let mut accumulated = text.clone();
                    let mut resume_session = session_id.clone();

                    for attempt in 1..=self.max_resume_attempts {
                        info!(
                            "auto-resume: attempt {}/{} with session {}",
                            attempt, self.max_resume_attempts, resume_session
                        );

                        let resume_result = self
                            .run_cli_with_session(
                                "Continue where you left off. Complete the remaining work.",
                                &extra_tools,
                                &resume_session,
                                effective_max_turns,
                                &effective_tools,
                                effective_model,
                            )
                            .await;

                        match resume_result {
                            Ok(resume_output) => {
                                let resume_stdout = String::from_utf8_lossy(&resume_output.stdout);
                                let (resume_text, resume_model) =
                                    self.parse_response(&resume_stdout);
                                accumulated = format!("{accumulated}\n\n---\n\n{resume_text}");

                                if resume_model.is_some() {
                                    model = resume_model;
                                }

                                // Check if this resume also hit max_turns.
                                let resume_parsed: Option<ClaudeCliResponse> =
                                    serde_json::from_str(&resume_stdout).ok();
                                match resume_parsed {
                                    Some(ref rr)
                                        if rr.subtype.as_deref() == Some("error_max_turns") =>
                                    {
                                        if let Some(ref new_sid) = rr.session_id {
                                            resume_session = new_sid.clone();
                                            continue;
                                        }
                                        break;
                                    }
                                    _ => break,
                                }
                            }
                            Err(e) => {
                                warn!("auto-resume attempt {} failed: {e}", attempt);
                                break;
                            }
                        }
                    }

                    text = accumulated;
                }
            }
        }

        let elapsed_ms = start.elapsed().as_millis() as u64;

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

impl ClaudeCodeProvider {
    /// Run the claude CLI subprocess with a timeout.
    async fn run_cli(
        &self,
        prompt: &str,
        extra_allowed_tools: &[String],
        max_turns: u32,
        allowed_tools: &[String],
        model: &str,
        context_disabled_tools: bool,
    ) -> Result<std::process::Output, OmegaError> {
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

        // Session continuity.
        if let Some(ref session) = self.session_id {
            cmd.arg("--session-id").arg(session);
        }

        // Allowed tools — pass explicit entries, or disable all tools with an
        // empty string when the caller explicitly set an empty list (e.g.,
        // classification calls that need no tool access).
        if allowed_tools.is_empty() && extra_allowed_tools.is_empty() {
            if context_disabled_tools {
                cmd.arg("--allowedTools").arg("");
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

        let output = tokio::time::timeout(self.timeout, cmd.output())
            .await
            .map_err(|_| {
                OmegaError::Provider(format!(
                    "claude CLI timed out after {}s",
                    self.timeout.as_secs()
                ))
            })?
            .map_err(|e| OmegaError::Provider(format!("failed to run claude CLI: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OmegaError::Provider(format!(
                "claude CLI exited with {}: {stderr}",
                output.status
            )));
        }

        Ok(output)
    }

    /// Run the claude CLI subprocess with a specific session ID (for auto-resume).
    async fn run_cli_with_session(
        &self,
        prompt: &str,
        extra_allowed_tools: &[String],
        session_id: &str,
        max_turns: u32,
        allowed_tools: &[String],
        model: &str,
    ) -> Result<std::process::Output, OmegaError> {
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
        cmd.env_remove("CLAUDECODE");

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

        for tool in allowed_tools {
            cmd.arg("--allowedTools").arg(tool);
        }
        for tool in extra_allowed_tools {
            cmd.arg("--allowedTools").arg(tool);
        }

        debug!("executing: claude -p <resume> --session-id {session_id}");

        let output = tokio::time::timeout(self.timeout, cmd.output())
            .await
            .map_err(|_| {
                OmegaError::Provider(format!(
                    "claude CLI resume timed out after {}s",
                    self.timeout.as_secs()
                ))
            })?
            .map_err(|e| OmegaError::Provider(format!("failed to run claude CLI resume: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(OmegaError::Provider(format!(
                "claude CLI resume exited with {}: {stderr}",
                output.status
            )));
        }

        Ok(output)
    }

    /// Parse the JSON response from Claude Code CLI, with diagnostic logging.
    fn parse_response(&self, stdout: &str) -> (String, Option<String>) {
        match serde_json::from_str::<ClaudeCliResponse>(stdout) {
            Ok(resp) => {
                // Handle error_max_turns — still extract whatever result exists.
                if resp.subtype.as_deref() == Some("error_max_turns") {
                    warn!("claude hit max_turns limit ({} turns)", self.max_turns);
                }

                let text = match resp.result {
                    Some(ref r) if !r.is_empty() => r.clone(),
                    _ => {
                        // Log the full response for debugging empty results.
                        error!(
                            "claude returned empty result | is_error={} subtype={:?} \
                             type={:?} num_turns={:?} | raw_len={}",
                            resp.is_error,
                            resp.subtype,
                            resp.response_type,
                            resp.num_turns,
                            stdout.len(),
                        );
                        debug!("full claude response for empty result: {stdout}");

                        if resp.is_error {
                            format!(
                                "Error from Claude: {}",
                                resp.subtype.as_deref().unwrap_or("unknown")
                            )
                        } else {
                            // Try to provide something useful instead of a generic fallback.
                            "I received your message but wasn't able to generate a response. \
                             Please try again."
                                .to_string()
                        }
                    }
                };
                (text, resp.model)
            }
            Err(e) => {
                // JSON parsing failed — try to extract text from raw output.
                warn!("failed to parse claude JSON response: {e}");
                debug!(
                    "raw stdout (first 500 chars): {}",
                    &stdout[..stdout.len().min(500)]
                );

                let trimmed = stdout.trim();
                if trimmed.is_empty() {
                    (
                        "I received your message but the response was empty. Please try again."
                            .to_string(),
                        None,
                    )
                } else {
                    (trimmed.to_string(), None)
                }
            }
        }
    }
}

/// Write `.claude/settings.local.json` with MCP server configuration.
///
/// Claude Code reads this file from `current_dir` on startup.
fn write_mcp_settings(workspace: &Path, servers: &[McpServer]) -> Result<PathBuf, OmegaError> {
    let claude_dir = workspace.join(".claude");
    std::fs::create_dir_all(&claude_dir)
        .map_err(|e| OmegaError::Provider(format!("failed to create .claude dir: {e}")))?;

    let path = claude_dir.join("settings.local.json");

    let mut mcp_servers = serde_json::Map::new();
    for srv in servers {
        let mut entry = serde_json::Map::new();
        entry.insert(
            "command".to_string(),
            serde_json::Value::String(srv.command.clone()),
        );
        entry.insert(
            "args".to_string(),
            serde_json::Value::Array(
                srv.args
                    .iter()
                    .map(|a| serde_json::Value::String(a.clone()))
                    .collect(),
            ),
        );
        mcp_servers.insert(srv.name.clone(), serde_json::Value::Object(entry));
    }

    let mut root = serde_json::Map::new();
    root.insert(
        "mcpServers".to_string(),
        serde_json::Value::Object(mcp_servers),
    );

    let json = serde_json::to_string_pretty(&root)
        .map_err(|e| OmegaError::Provider(format!("failed to serialize MCP settings: {e}")))?;

    std::fs::write(&path, json)
        .map_err(|e| OmegaError::Provider(format!("failed to write MCP settings: {e}")))?;

    info!("mcp: wrote settings to {}", path.display());
    Ok(path)
}

/// Remove the temporary MCP settings file.
fn cleanup_mcp_settings(path: &Path) {
    if path.exists() {
        if let Err(e) = std::fs::remove_file(path) {
            warn!("mcp: failed to cleanup {}: {e}", path.display());
        } else {
            debug!("mcp: cleaned up {}", path.display());
        }
    }
}

/// Generate `--allowedTools` patterns for MCP servers.
///
/// Each server gets a `mcp__<name>__*` wildcard pattern.
pub fn mcp_tool_patterns(servers: &[McpServer]) -> Vec<String> {
    servers
        .iter()
        .map(|s| format!("mcp__{}__*", s.name))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_provider() {
        let provider = ClaudeCodeProvider::new();
        assert_eq!(provider.name(), "claude-code");
        assert!(!provider.requires_api_key());
        assert_eq!(provider.max_turns, 100);
        assert_eq!(provider.allowed_tools.len(), 4);
        assert_eq!(provider.timeout, Duration::from_secs(3600));
        assert!(provider.working_dir.is_none());
        assert_eq!(provider.sandbox_mode, SandboxMode::Sandbox);
        assert_eq!(provider.max_resume_attempts, 5);
        assert!(provider.model.is_empty());
    }

    #[test]
    fn test_from_config_with_timeout() {
        let provider = ClaudeCodeProvider::from_config(
            5,
            vec!["Bash".into()],
            300,
            None,
            SandboxMode::default(),
            3,
            "claude-sonnet-4-6".into(),
        );
        assert_eq!(provider.max_turns, 5);
        assert_eq!(provider.timeout, Duration::from_secs(300));
        assert!(provider.working_dir.is_none());
        assert_eq!(provider.max_resume_attempts, 3);
        assert_eq!(provider.model, "claude-sonnet-4-6");
    }

    #[test]
    fn test_from_config_with_working_dir() {
        let dir = PathBuf::from("/home/user/.omega/workspace");
        let provider = ClaudeCodeProvider::from_config(
            10,
            vec!["Bash".into()],
            600,
            Some(dir.clone()),
            SandboxMode::Sandbox,
            5,
            String::new(),
        );
        assert_eq!(provider.working_dir, Some(dir));
    }

    #[test]
    fn test_from_config_with_sandbox_mode() {
        let dir = PathBuf::from("/home/user/.omega/workspace");
        let provider = ClaudeCodeProvider::from_config(
            10,
            vec!["Bash".into()],
            600,
            Some(dir),
            SandboxMode::Rx,
            5,
            String::new(),
        );
        assert_eq!(provider.sandbox_mode, SandboxMode::Rx);
    }

    #[test]
    fn test_parse_response_max_turns_with_session() {
        let provider = ClaudeCodeProvider::new();
        let json = r#"{"type":"result","subtype":"error_max_turns","result":"partial work done","session_id":"sess-123","model":"claude-sonnet-4-20250514"}"#;
        let (text, model) = provider.parse_response(json);
        assert_eq!(text, "partial work done");
        assert_eq!(model, Some("claude-sonnet-4-20250514".to_string()));
    }

    #[test]
    fn test_parse_response_success() {
        let provider = ClaudeCodeProvider::new();
        let json = r#"{"type":"result","subtype":"success","result":"all done","model":"claude-sonnet-4-20250514"}"#;
        let (text, model) = provider.parse_response(json);
        assert_eq!(text, "all done");
        assert_eq!(model, Some("claude-sonnet-4-20250514".to_string()));
    }

    // --- MCP tests ---

    #[test]
    fn test_mcp_tool_patterns_empty() {
        assert!(mcp_tool_patterns(&[]).is_empty());
    }

    #[test]
    fn test_mcp_tool_patterns() {
        let servers = vec![
            McpServer {
                name: "playwright".into(),
                command: "npx".into(),
                args: vec!["@playwright/mcp".into()],
            },
            McpServer {
                name: "postgres".into(),
                command: "npx".into(),
                args: vec!["@pg/mcp".into()],
            },
        ];
        let patterns = mcp_tool_patterns(&servers);
        assert_eq!(patterns, vec!["mcp__playwright__*", "mcp__postgres__*"]);
    }

    #[test]
    fn test_write_and_cleanup_mcp_settings() {
        let tmp = std::env::temp_dir().join("__omega_test_mcp_settings__");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let servers = vec![McpServer {
            name: "playwright".into(),
            command: "npx".into(),
            args: vec!["@playwright/mcp".into(), "--headless".into()],
        }];

        let path = write_mcp_settings(&tmp, &servers).unwrap();
        assert!(path.exists());

        // Verify JSON structure.
        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        let mcp = &parsed["mcpServers"]["playwright"];
        assert_eq!(mcp["command"], "npx");
        assert_eq!(mcp["args"][0], "@playwright/mcp");
        assert_eq!(mcp["args"][1], "--headless");

        // Cleanup.
        cleanup_mcp_settings(&path);
        assert!(!path.exists());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_cleanup_mcp_settings_nonexistent() {
        // Should not panic on missing file.
        cleanup_mcp_settings(Path::new("/tmp/__omega_nonexistent_mcp_settings__"));
    }
}
