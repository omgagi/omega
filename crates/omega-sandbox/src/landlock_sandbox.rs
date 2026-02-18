//! Linux Landlock LSM enforcement.
//!
//! Uses the `landlock` crate to restrict file writes in the child process
//! via a `pre_exec` hook. Requires Linux kernel 5.13+ with Landlock enabled.
//! Writes are allowed to the Omega data directory (`~/.omega/`), `/tmp`,
//! and `~/.claude`.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::warn;

use landlock::{
    path_beneath_rules, Access, AccessFs, Ruleset, RulesetAttr, RulesetCreatedAttr, ABI,
};

/// All read-related filesystem access flags.
fn read_access() -> AccessFs {
    AccessFs::ReadFile | AccessFs::ReadDir | AccessFs::Execute | AccessFs::Refer
}

/// All filesystem access flags (read + write).
fn full_access() -> AccessFs {
    AccessFs::from_all(ABI::V5)
}

/// Build a [`Command`] with Landlock write restrictions applied via `pre_exec`.
///
/// The child process will be allowed to:
/// - Read and execute from the entire filesystem (`/`)
/// - Read, write, and create files in `data_dir` (`~/.omega/`), `/tmp`, and `~/.claude`
///
/// If the kernel does not support Landlock, logs a warning and falls back
/// to a plain command.
pub(crate) fn sandboxed_command(program: &str, data_dir: &Path) -> Command {
    let data_dir = data_dir.to_path_buf();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    let claude_dir = PathBuf::from(&home).join(".claude");

    let mut cmd = Command::new(program);

    // SAFETY: pre_exec runs in the forked child before exec. We only call
    // the landlock crate (which uses syscalls), no async or allocator abuse.
    unsafe {
        cmd.pre_exec(move || {
            apply_landlock(&data_dir, &claude_dir).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string())
            })
        });
    }

    cmd
}

/// Apply Landlock restrictions to the current process.
fn apply_landlock(data_dir: &Path, claude_dir: &Path) -> Result<(), anyhow::Error> {
    let abi = ABI::V5;
    let status = Ruleset::default()
        .handle_access(full_access())?
        .create()?
        // Read + execute on entire filesystem.
        .add_rules(path_beneath_rules(&[PathBuf::from("/")], read_access()))?
        // Full access to Omega data directory (workspace, skills, projects, etc.).
        .add_rules(path_beneath_rules(&[data_dir], full_access()))?
        // Full access to /tmp.
        .add_rules(path_beneath_rules(&[PathBuf::from("/tmp")], full_access()))?
        // Full access to ~/.claude.
        .add_rules(path_beneath_rules(&[claude_dir], full_access()))?
        .restrict_self()?;

    if !status.ruleset.is_fully_enforced() {
        warn!(
            "landlock: not all restrictions enforced (kernel may lack full support); \
             best-effort sandbox active"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_access_flags() {
        let flags = read_access();
        assert!(flags.contains(AccessFs::ReadFile));
        assert!(flags.contains(AccessFs::ReadDir));
        assert!(flags.contains(AccessFs::Execute));
    }

    #[test]
    fn test_full_access_contains_writes() {
        let flags = full_access();
        assert!(flags.contains(AccessFs::WriteFile));
        assert!(flags.contains(AccessFs::ReadFile));
        assert!(flags.contains(AccessFs::MakeDir));
    }

    #[test]
    fn test_command_structure() {
        let ws = PathBuf::from("/tmp/ws");
        let cmd = sandboxed_command("claude", &ws);
        let program = cmd.as_std().get_program().to_string_lossy().to_string();
        assert_eq!(program, "claude");
    }
}
