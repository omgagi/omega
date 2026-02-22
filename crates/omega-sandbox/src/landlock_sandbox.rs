//! Linux Landlock LSM enforcement — broad allowlist approach.
//!
//! Landlock cannot deny subdirectories of an allowed parent, so we use a broad
//! allowlist: read-only on `/` (covers system dirs), full access to `$HOME`,
//! `/tmp`, `/var/tmp`, `/opt`, `/srv`, `/run`, `/media`, `/mnt`.
//!
//! Note: memory.db protection on Linux relies on `is_write_blocked()` (code-level),
//! not Landlock (can't deny subdirs of allowed parent).

use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use tokio::process::Command;
use tracing::warn;

use landlock::{
    path_beneath_rules, Access, AccessFs, BitFlags, Ruleset, RulesetAttr, RulesetCreatedAttr,
    RulesetStatus, ABI,
};

/// All read-related filesystem access flags.
fn read_access() -> BitFlags<AccessFs> {
    AccessFs::ReadFile | AccessFs::ReadDir | AccessFs::Execute | AccessFs::Refer
}

/// All filesystem access flags (read + write).
fn full_access() -> BitFlags<AccessFs> {
    AccessFs::from_all(ABI::V5)
}

/// Build a [`Command`] with Landlock write restrictions applied via `pre_exec`.
///
/// The child process will have:
/// - Read and execute access to the entire filesystem (`/`)
/// - Full access to `$HOME`, `/tmp`, `/var/tmp`, `/opt`, `/srv`, `/run`, `/media`, `/mnt`
///
/// System directories (`/bin`, `/sbin`, `/usr`, `/etc`, `/lib`, etc.) are implicitly
/// read-only because only `/` gets read access and writable paths are explicitly listed.
///
/// If the kernel does not support Landlock, logs a warning and falls back
/// to a plain command.
pub(crate) fn protected_command(program: &str, _data_dir: &std::path::Path) -> Command {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());

    let mut cmd = Command::new(program);

    // SAFETY: pre_exec runs in the forked child before exec. We only call
    // the landlock crate (which uses syscalls), no async or allocator abuse.
    unsafe {
        cmd.pre_exec(move || {
            apply_landlock(&home).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::PermissionDenied, e.to_string())
            })
        });
    }

    cmd
}

/// Apply Landlock restrictions to the current process.
fn apply_landlock(home: &str) -> Result<(), anyhow::Error> {
    let home_dir = PathBuf::from(home);

    let mut ruleset = Ruleset::default()
        .handle_access(full_access())?
        .create()?
        // Read + execute on entire filesystem (system dirs become read-only).
        .add_rules(path_beneath_rules(&[PathBuf::from("/")], read_access()))?
        // Full access to home directory.
        .add_rules(path_beneath_rules(&[home_dir], full_access()))?
        // Full access to /tmp.
        .add_rules(path_beneath_rules(&[PathBuf::from("/tmp")], full_access()))?;

    // Optional writable paths — skip if they don't exist (common in containers).
    let optional_paths = ["/var/tmp", "/opt", "/srv", "/run", "/media", "/mnt"];
    for path in &optional_paths {
        let p = PathBuf::from(path);
        if p.exists() {
            ruleset = ruleset.add_rules(path_beneath_rules(&[p], full_access()))?;
        }
    }

    let status = ruleset.restrict_self()?;

    if status.ruleset != RulesetStatus::FullyEnforced {
        warn!(
            "landlock: not all restrictions enforced (kernel may lack full support); \
             best-effort protection active"
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
        let data_dir = PathBuf::from("/tmp/ws");
        let cmd = protected_command("claude", &data_dir);
        let program = cmd.as_std().get_program().to_string_lossy().to_string();
        assert_eq!(program, "claude");
    }
}
