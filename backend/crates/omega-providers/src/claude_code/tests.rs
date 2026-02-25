//! Tests for the Claude Code CLI provider.

use super::mcp;
use super::*;
use omega_core::context::McpServer;
use omega_core::traits::Provider;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[test]
fn test_default_provider() {
    let provider = ClaudeCodeProvider::new();
    assert_eq!(provider.name(), "claude-code");
    assert!(!provider.requires_api_key());
    assert_eq!(provider.max_turns, 25);
    assert!(provider.allowed_tools.is_empty());
    assert_eq!(provider.timeout, Duration::from_secs(3600));
    assert!(provider.working_dir.is_none());
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
        5,
        String::new(),
    );
    assert_eq!(provider.working_dir, Some(dir));
}

#[test]
fn test_parse_response_max_turns_with_session() {
    let provider = ClaudeCodeProvider::new();
    let json = r#"{"type":"result","subtype":"error_max_turns","result":"partial work done","session_id":"sess-123","model":"claude-sonnet-4-20250514"}"#;
    let (text, model) = provider.parse_response(json, 100);
    assert_eq!(text, "partial work done");
    assert_eq!(model, Some("claude-sonnet-4-20250514".to_string()));
}

#[test]
fn test_parse_response_success() {
    let provider = ClaudeCodeProvider::new();
    let json = r#"{"type":"result","subtype":"success","result":"all done","model":"claude-sonnet-4-20250514"}"#;
    let (text, model) = provider.parse_response(json, 100);
    assert_eq!(text, "all done");
    assert_eq!(model, Some("claude-sonnet-4-20250514".to_string()));
}

// --- MCP tests ---

#[test]
fn test_mcp_tool_patterns_empty() {
    assert!(mcp::mcp_tool_patterns(&[]).is_empty());
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
    let patterns = mcp::mcp_tool_patterns(&servers);
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

    let path = mcp::write_mcp_settings(&tmp, &servers).unwrap();
    assert!(path.exists());

    // Verify JSON structure.
    let content = std::fs::read_to_string(&path).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    let mcp_val = &parsed["mcpServers"]["playwright"];
    assert_eq!(mcp_val["command"], "npx");
    assert_eq!(mcp_val["args"][0], "@playwright/mcp");
    assert_eq!(mcp_val["args"][1], "--headless");

    // Cleanup.
    mcp::cleanup_mcp_settings(&path);
    assert!(!path.exists());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_cleanup_mcp_settings_nonexistent() {
    // Should not panic on missing file.
    mcp::cleanup_mcp_settings(Path::new("/tmp/__omega_nonexistent_mcp_settings__"));
}

// =========================================================================
// REQ-BAP-004 (Must): ClaudeCodeProvider --agent support
// =========================================================================
//
// These tests verify the `run_cli()` contract for the new `agent_name`
// parameter. The developer must add `agent_name: Option<&str>` to
// `run_cli()` and implement the argument construction logic.
//
// Since run_cli() executes a subprocess, these tests validate the
// `build_run_cli_args()` helper that the developer must extract/create
// to make argument construction testable without subprocess execution.
//
// Developer: Create `pub(super) fn build_run_cli_args(...)` in command.rs
// that returns `Vec<String>` of CLI arguments (excluding the binary name).
// Then `run_cli()` calls `build_run_cli_args()` and applies them to Command.

// Requirement: REQ-BAP-004 (Must)
// Acceptance: run_cli() does NOT emit --agent when agent_name is None
#[test]
fn test_build_run_cli_args_no_agent_name() {
    let args = ClaudeCodeProvider::build_run_cli_args(
        "hello world",       // prompt
        &[],                 // extra_allowed_tools
        100,                 // max_turns
        &[],                 // allowed_tools
        "claude-sonnet-4-6", // model
        false,               // context_disabled_tools
        None,                // session_id
        None,                // agent_name (None = current behavior)
    );
    assert!(
        !args.contains(&"--agent".to_string()),
        "Without agent_name, --agent flag must NOT be present"
    );
    // Verify -p is present with the prompt.
    let p_idx = args
        .iter()
        .position(|a| a == "-p")
        .expect("-p flag must be present");
    assert_eq!(args[p_idx + 1], "hello world", "prompt must follow -p");
    // Verify --output-format json is present.
    assert!(args.contains(&"--output-format".to_string()));
    assert!(args.contains(&"json".to_string()));
    // Verify --max-turns is present.
    assert!(args.contains(&"--max-turns".to_string()));
    assert!(args.contains(&"100".to_string()));
    // Verify --model is present.
    assert!(args.contains(&"--model".to_string()));
    assert!(args.contains(&"claude-sonnet-4-6".to_string()));
}

// Requirement: REQ-BAP-004 (Must)
// Acceptance: When --agent is used, --agent <name> is emitted before -p
#[test]
fn test_build_run_cli_args_with_agent_name() {
    let args = ClaudeCodeProvider::build_run_cli_args(
        "Build me a task tracker.", // prompt (just user message)
        &[],                        // extra_allowed_tools
        100,                        // max_turns
        &[],                        // allowed_tools
        "claude-opus-4-6",          // model
        false,                      // context_disabled_tools
        None,                       // session_id
        Some("build-analyst"),      // agent_name
    );
    // --agent must be present with the correct name.
    let agent_idx = args
        .iter()
        .position(|a| a == "--agent")
        .expect("--agent flag must be present when agent_name is Some");
    assert_eq!(
        args[agent_idx + 1],
        "build-analyst",
        "--agent must be followed by the agent name"
    );
    // -p must still be present with the user message.
    let p_idx = args
        .iter()
        .position(|a| a == "-p")
        .expect("-p flag must be present");
    assert_eq!(
        args[p_idx + 1],
        "Build me a task tracker.",
        "prompt must follow -p"
    );
}

// Requirement: REQ-BAP-004 (Must)
// Acceptance: --model still applied as override when --agent is used
#[test]
fn test_build_run_cli_args_agent_with_model_override() {
    let args = ClaudeCodeProvider::build_run_cli_args(
        "Begin.",
        &[],
        100,
        &[],
        "claude-opus-4-6",
        false,
        None,
        Some("build-architect"),
    );
    assert!(
        args.contains(&"--agent".to_string()),
        "--agent must be present"
    );
    assert!(
        args.contains(&"--model".to_string()),
        "--model must still be present with --agent"
    );
    assert!(
        args.contains(&"claude-opus-4-6".to_string()),
        "model value must be present"
    );
}

// Requirement: REQ-BAP-004 (Must)
// Acceptance: --dangerously-skip-permissions still applied with --agent
#[test]
fn test_build_run_cli_args_agent_with_skip_permissions() {
    let args = ClaudeCodeProvider::build_run_cli_args(
        "Begin.",
        &[],
        100,
        &[], // empty allowed_tools => bypass permissions
        "",  // empty model
        false,
        None,
        Some("build-developer"),
    );
    assert!(
        args.contains(&"--agent".to_string()),
        "--agent must be present"
    );
    assert!(
        args.contains(&"--dangerously-skip-permissions".to_string()),
        "--dangerously-skip-permissions must be present when allowed_tools is empty"
    );
}

// Requirement: REQ-BAP-004 (Must)
// Acceptance: --max-turns still applied with --agent
#[test]
fn test_build_run_cli_args_agent_with_max_turns() {
    let args = ClaudeCodeProvider::build_run_cli_args(
        "Begin.",
        &[],
        25,
        &[],
        "",
        false,
        None,
        Some("build-analyst"),
    );
    let mt_idx = args
        .iter()
        .position(|a| a == "--max-turns")
        .expect("--max-turns must be present");
    assert_eq!(
        args[mt_idx + 1],
        "25",
        "--max-turns must have correct value"
    );
}

// Requirement: REQ-BAP-004 (Must)
// Failure mode: agent_name with empty string should NOT emit --agent
#[test]
fn test_build_run_cli_args_agent_name_empty_string() {
    let args = ClaudeCodeProvider::build_run_cli_args(
        "Begin.",
        &[],
        100,
        &[],
        "",
        false,
        None,
        Some(""), // empty agent name
    );
    // Empty agent name should NOT produce --agent flag.
    assert!(
        !args.contains(&"--agent".to_string()),
        "Empty agent_name should not emit --agent flag"
    );
}

// Requirement: REQ-BAP-004 (Must)
// Edge case: agent_name with session_id — agent_name should take priority
// (builds never use session_id, but we test the edge case)
#[test]
fn test_build_run_cli_args_agent_name_with_session_id() {
    let args = ClaudeCodeProvider::build_run_cli_args(
        "Begin.",
        &[],
        100,
        &[],
        "",
        false,
        Some("sess-123"), // session_id
        Some("build-qa"), // agent_name
    );
    // --agent should be present (agent_name takes priority for builds).
    assert!(
        args.contains(&"--agent".to_string()),
        "--agent must be present when agent_name is set"
    );
    // --resume should NOT be present (agent mode does not use sessions).
    assert!(
        !args.contains(&"--resume".to_string()),
        "--resume must NOT be present when agent_name is set"
    );
}

// Requirement: REQ-BAP-004 (Must)
// Security: agent_name with path traversal characters
#[test]
fn test_build_run_cli_args_agent_name_path_traversal() {
    // The CLI receives the agent name as-is; it's the agent file writer's
    // job to sanitize. But run_cli must not crash.
    let args = ClaudeCodeProvider::build_run_cli_args(
        "Begin.",
        &[],
        100,
        &[],
        "",
        false,
        None,
        Some("../../../etc/passwd"),
    );
    let agent_idx = args.iter().position(|a| a == "--agent").unwrap();
    assert_eq!(
        args[agent_idx + 1],
        "../../../etc/passwd",
        "agent_name is passed through as-is (validation is elsewhere)"
    );
}

// Requirement: REQ-BAP-004 (Must)
// Regression: current run_cli behavior with explicit allowed_tools (no --agent)
#[test]
fn test_build_run_cli_args_explicit_allowed_tools_no_agent() {
    let args = ClaudeCodeProvider::build_run_cli_args(
        "hello",
        &[],
        50,
        &["Bash".to_string(), "Read".to_string()],
        "claude-sonnet-4-6",
        false,
        None,
        None,
    );
    // Should have --allowedTools for each tool.
    let at_count = args.iter().filter(|a| *a == "--allowedTools").count();
    assert_eq!(
        at_count, 2,
        "Should have 2 --allowedTools entries for explicit tools"
    );
    // Should NOT have --dangerously-skip-permissions.
    assert!(
        !args.contains(&"--dangerously-skip-permissions".to_string()),
        "Explicit tools should not bypass permissions"
    );
}

// Requirement: REQ-BAP-004 (Must)
// Regression: disabled tools with no agent
#[test]
fn test_build_run_cli_args_disabled_tools() {
    let args = ClaudeCodeProvider::build_run_cli_args(
        "classify this",
        &[],
        5,
        &[],
        "",
        true, // context_disabled_tools
        None,
        None,
    );
    // Should have --allowedTools with empty string.
    let at_idx = args.iter().position(|a| a == "--allowedTools").unwrap();
    assert_eq!(
        args[at_idx + 1],
        "",
        "Disabled tools should pass empty --allowedTools"
    );
}
