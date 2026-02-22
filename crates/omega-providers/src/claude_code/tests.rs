//! Tests for the Claude Code CLI provider.

use super::mcp;
use super::*;
use omega_core::{config::SandboxMode, context::McpServer, traits::Provider};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[test]
fn test_default_provider() {
    let provider = ClaudeCodeProvider::new();
    assert_eq!(provider.name(), "claude-code");
    assert!(!provider.requires_api_key());
    assert_eq!(provider.max_turns, 100);
    assert!(provider.allowed_tools.is_empty());
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
