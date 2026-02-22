//! # omega-sandbox
//!
//! OS-level system protection for the Omega agent.
//!
//! Uses a **blocklist** approach: everything is allowed by default, then
//! dangerous system directories and OMEGA's core database are blocked.
//!
//! - **macOS**: Apple Seatbelt via `sandbox-exec -p <profile>` — denies writes
//!   to `/System`, `/bin`, `/sbin`, `/usr/{bin,sbin,lib,libexec}`, `/private/etc`,
//!   `/Library`, and `{data_dir}/data/` (memory.db).
//! - **Linux**: Landlock LSM via `pre_exec` hook (kernel 5.13+) — broad
//!   read-only on `/` with full access to `$HOME`, `/tmp`, `/var/tmp`, `/opt`,
//!   `/srv`, `/run`, `/media`, `/mnt`.
//! - **Other**: Falls back to a plain command with a warning.
//!
//! Also provides [`is_write_blocked`] for code-level enforcement in HTTP
//! provider tool executors (protects memory.db on all platforms).

use std::path::Path;
use tokio::process::Command;

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
use tracing::warn;

#[cfg(target_os = "macos")]
mod seatbelt;

#[cfg(target_os = "linux")]
mod landlock_sandbox;

/// Build a [`Command`] with OS-level system protection.
///
/// Always active — blocks writes to dangerous system directories and
/// OMEGA's core data directory (memory.db). No configuration needed.
///
/// `data_dir` is the Omega data directory (`~/.omega/`). Writes to
/// `{data_dir}/data/` are blocked (protects memory.db). All other
/// paths under `data_dir` (workspace, skills, projects) remain writable.
///
/// On unsupported platforms, logs a warning and returns a plain command.
pub fn protected_command(program: &str, data_dir: &Path) -> Command {
    platform_command(program, data_dir)
}

/// Check if a write to the given path should be blocked.
///
/// Returns `true` if the path targets a protected location:
/// - Dangerous OS directories (`/System`, `/bin`, `/sbin`, `/usr/bin`, etc.)
/// - OMEGA's core data directory (`{data_dir}/data/`) — protects memory.db
///
/// Used by the HTTP provider `ToolExecutor` for code-level enforcement.
pub fn is_write_blocked(path: &Path, data_dir: &Path) -> bool {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        // Relative paths can't be resolved without a cwd, treat as not blocked.
        return false;
    };

    let path_str = abs.to_string_lossy();

    // Block writes to OMEGA's core data directory (memory.db, etc.).
    let data_data = data_dir.join("data");
    if abs.starts_with(&data_data) {
        return true;
    }

    // Block writes to dangerous OS directories.
    let blocked_prefixes: &[&str] = &[
        "/System",
        "/bin",
        "/sbin",
        "/usr/bin",
        "/usr/sbin",
        "/usr/lib",
        "/usr/libexec",
        "/private/etc",
        "/Library",
        "/etc",
        "/boot",
        "/proc",
        "/sys",
        "/dev",
    ];

    for prefix in blocked_prefixes {
        if path_str.starts_with(prefix) {
            return true;
        }
    }

    false
}

/// Dispatch to the platform-specific protection implementation.
#[cfg(target_os = "macos")]
fn platform_command(program: &str, data_dir: &Path) -> Command {
    seatbelt::protected_command(program, data_dir)
}

/// Dispatch to the platform-specific protection implementation.
#[cfg(target_os = "linux")]
fn platform_command(program: &str, data_dir: &Path) -> Command {
    landlock_sandbox::protected_command(program, data_dir)
}

/// Fallback for unsupported platforms.
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_command(program: &str, _data_dir: &Path) -> Command {
    warn!("OS-level protection not available on this platform; using code-level enforcement only");
    Command::new(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_protected_command_returns_command() {
        let data_dir = PathBuf::from("/tmp/ws");
        let cmd = protected_command("claude", &data_dir);
        let program = cmd.as_std().get_program().to_string_lossy().to_string();
        assert!(!program.is_empty());
    }

    #[test]
    fn test_is_write_blocked_data_dir() {
        let data_dir = PathBuf::from("/home/user/.omega");
        assert!(is_write_blocked(
            Path::new("/home/user/.omega/data/memory.db"),
            &data_dir
        ));
        assert!(is_write_blocked(
            Path::new("/home/user/.omega/data/"),
            &data_dir
        ));
    }

    #[test]
    fn test_is_write_blocked_allows_workspace() {
        let data_dir = PathBuf::from("/home/user/.omega");
        assert!(!is_write_blocked(
            Path::new("/home/user/.omega/workspace/test.txt"),
            &data_dir
        ));
        assert!(!is_write_blocked(
            Path::new("/home/user/.omega/skills/test/SKILL.md"),
            &data_dir
        ));
    }

    #[test]
    fn test_is_write_blocked_system_dirs() {
        let data_dir = PathBuf::from("/home/user/.omega");
        assert!(is_write_blocked(Path::new("/System/Library/test"), &data_dir));
        assert!(is_write_blocked(Path::new("/bin/sh"), &data_dir));
        assert!(is_write_blocked(Path::new("/usr/bin/env"), &data_dir));
        assert!(is_write_blocked(Path::new("/private/etc/hosts"), &data_dir));
        assert!(is_write_blocked(Path::new("/Library/Preferences/test"), &data_dir));
    }

    #[test]
    fn test_is_write_blocked_allows_normal_paths() {
        let data_dir = PathBuf::from("/home/user/.omega");
        assert!(!is_write_blocked(Path::new("/tmp/test"), &data_dir));
        assert!(!is_write_blocked(Path::new("/home/user/documents/test"), &data_dir));
        assert!(!is_write_blocked(
            Path::new("/usr/local/bin/something"),
            &data_dir
        ));
    }

    #[test]
    fn test_is_write_blocked_relative_path() {
        let data_dir = PathBuf::from("/home/user/.omega");
        assert!(!is_write_blocked(Path::new("relative/path"), &data_dir));
    }
}
