//! macOS Seatbelt (sandbox-exec) enforcement.
//!
//! Wraps a command with `sandbox-exec -p <profile>` to restrict file writes
//! to the Omega data directory (`~/.omega/`), `/tmp`, `~/.claude`, and `~/.cargo`.

use std::path::Path;
use tokio::process::Command;
use tracing::warn;

/// Path to the sandbox-exec binary (built into macOS).
const SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";

/// Generate a Seatbelt profile that allows all operations except file writes
/// outside the permitted directories.
fn build_profile(data_dir: &Path) -> String {
    let data_dir_str = data_dir.display();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());

    format!(
        r#"(version 1)
(allow default)
(deny file-write*)
(allow file-write*
  (subpath "{data_dir_str}")
  (subpath "/private/tmp")
  (subpath "/private/var/folders")
  (subpath "{home}/.claude")
  (subpath "{home}/.cargo")
  (literal "/dev/null")
  (literal "/dev/zero")
  (subpath "/dev/fd")
)"#
    )
}

/// Build a [`Command`] wrapped with `sandbox-exec` write restrictions.
///
/// `data_dir` is the Omega data directory (e.g. `~/.omega/`) — writes are
/// allowed to the entire tree (workspace, skills, projects, etc.).
/// Also allows writes to `~/.cargo` for cargo registry cache.
///
/// If `/usr/bin/sandbox-exec` does not exist, logs a warning and returns
/// a plain command without OS-level enforcement.
pub(crate) fn sandboxed_command(program: &str, data_dir: &Path) -> Command {
    if !Path::new(SANDBOX_EXEC).exists() {
        warn!("sandbox-exec not found at {SANDBOX_EXEC}; falling back to prompt-only sandbox");
        return Command::new(program);
    }

    let profile = build_profile(data_dir);
    let mut cmd = Command::new(SANDBOX_EXEC);
    cmd.arg("-p").arg(profile).arg("--").arg(program);
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_profile_contains_data_dir() {
        let data_dir = PathBuf::from("/home/user/.omega");
        let profile = build_profile(&data_dir);
        assert!(profile.contains("/home/user/.omega"));
    }

    #[test]
    fn test_profile_denies_writes_then_allows() {
        let ws = PathBuf::from("/tmp/test-ws");
        let profile = build_profile(&ws);
        assert!(profile.contains("(deny file-write*)"));
        assert!(profile.contains("(allow file-write*"));
        assert!(profile.contains("(subpath \"/private/tmp\")"));
        assert!(profile.contains("(subpath \"/private/var/folders\")"));
    }

    #[test]
    fn test_profile_allows_claude_dir() {
        let ws = PathBuf::from("/tmp/ws");
        let profile = build_profile(&ws);
        assert!(profile.contains(".claude"));
    }

    #[test]
    fn test_profile_allows_cargo_dir() {
        let ws = PathBuf::from("/tmp/ws");
        let profile = build_profile(&ws);
        assert!(profile.contains(".cargo"));
    }

    #[test]
    fn test_profile_allows_dev_null() {
        let ws = PathBuf::from("/tmp/ws");
        let profile = build_profile(&ws);
        assert!(profile.contains(r#"(literal "/dev/null")"#));
        assert!(profile.contains(r#"(literal "/dev/zero")"#));
        assert!(profile.contains(r#"(subpath "/dev/fd")"#));
    }

    #[test]
    fn test_command_structure() {
        // This test verifies the command is built correctly.
        // On macOS CI with sandbox-exec present, it wraps the program.
        // On other platforms, it falls back to a plain command.
        let ws = PathBuf::from("/tmp/ws");
        let cmd = sandboxed_command("claude", &ws);
        // The command should be created without panicking.
        let program = cmd.as_std().get_program().to_string_lossy().to_string();
        // Either sandbox-exec (macOS) or claude (fallback).
        assert!(
            program.contains("sandbox-exec") || program.contains("claude"),
            "unexpected program: {program}"
        );
    }
}
