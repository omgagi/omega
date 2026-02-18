//! # omega-sandbox
//!
//! OS-level filesystem enforcement for the Omega agent.
//!
//! Provides [`sandboxed_command`] which wraps a program in platform-native
//! write restrictions. Writes are allowed to the Omega data directory
//! (`~/.omega/`), `/tmp`, and `~/.claude`.
//!
//! - **macOS**: Apple Seatbelt via `sandbox-exec -p <profile>`
//! - **Linux**: Landlock LSM via `pre_exec` hook (kernel 5.13+)
//! - **Other**: Falls back to a plain command with a warning

use omega_core::config::SandboxMode;
use std::path::Path;
use tokio::process::Command;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use tracing::warn;

#[cfg(target_os = "macos")]
mod seatbelt;

#[cfg(target_os = "linux")]
mod landlock_sandbox;

/// Build a [`Command`] with OS-level sandbox enforcement based on the mode.
///
/// - `Rwx` → plain `Command::new(program)` (no restrictions)
/// - `Sandbox` / `Rx` → platform-specific write restrictions (writes only to
///   `data_dir` (`~/.omega/`), `/tmp`, `~/.claude`)
///
/// `data_dir` is the Omega data directory — writes are allowed to the entire
/// tree (workspace, skills, projects, etc.).
///
/// On unsupported platforms, logs a warning and returns a plain command.
pub fn sandboxed_command(program: &str, mode: SandboxMode, data_dir: &Path) -> Command {
    match mode {
        SandboxMode::Rwx => Command::new(program),
        SandboxMode::Sandbox | SandboxMode::Rx => platform_command(program, data_dir),
    }
}

/// Dispatch to the platform-specific sandbox implementation.
#[cfg(target_os = "macos")]
fn platform_command(program: &str, data_dir: &Path) -> Command {
    seatbelt::sandboxed_command(program, data_dir)
}

/// Dispatch to the platform-specific sandbox implementation.
#[cfg(target_os = "linux")]
fn platform_command(program: &str, data_dir: &Path) -> Command {
    landlock_sandbox::sandboxed_command(program, data_dir)
}

/// Fallback for unsupported platforms.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_command(program: &str, _data_dir: &Path) -> Command {
    warn!("OS-level sandbox not available on this platform; using prompt-only enforcement");
    Command::new(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_rwx_returns_plain_command() {
        let ws = PathBuf::from("/tmp/ws");
        let cmd = sandboxed_command("claude", SandboxMode::Rwx, &ws);
        let program = cmd.as_std().get_program().to_string_lossy().to_string();
        assert_eq!(program, "claude");
    }

    #[test]
    fn test_sandbox_mode_returns_command() {
        let ws = PathBuf::from("/tmp/ws");
        let cmd = sandboxed_command("claude", SandboxMode::Sandbox, &ws);
        // Should not panic; platform dispatch works.
        let program = cmd.as_std().get_program().to_string_lossy().to_string();
        assert!(!program.is_empty());
    }

    #[test]
    fn test_rx_mode_returns_command() {
        let ws = PathBuf::from("/tmp/ws");
        let cmd = sandboxed_command("claude", SandboxMode::Rx, &ws);
        let program = cmd.as_std().get_program().to_string_lossy().to_string();
        assert!(!program.is_empty());
    }
}
