//! System-wide uninstall command for OMEGA.
//!
//! Removes all Omega artifacts (data, service, binaries) with two-step
//! interactive confirmation. Supports "complete removal" and "keep config" modes.

use crate::init_style;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Uninstall mode selected by the user.
#[derive(Debug, PartialEq)]
enum UninstallMode {
    Complete,
    KeepConfig,
}

/// Result tracker for partial-failure reporting.
struct UninstallResult {
    warnings: Vec<String>,
}

impl UninstallResult {
    fn new() -> Self {
        Self {
            warnings: Vec::new(),
        }
    }

    fn warn(&mut self, msg: String) {
        self.warnings.push(msg);
    }
}

/// An artifact to be deleted (or preserved).
struct ArtifactEntry {
    /// Resolved filesystem path (used in tests for assertion).
    #[allow(dead_code)]
    path: PathBuf,
    label: String,
    preserved: bool,
}

// ---------------------------------------------------------------------------
// Known subdirectories removed in keep-config mode
// ---------------------------------------------------------------------------

const KEEP_CONFIG_SUBDIRS: &[&str] = &[
    "data",
    "logs",
    "workspace",
    "stores",
    "prompts",
    "skills",
    "projects",
    "topologies",
    "whatsapp_session",
];

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the interactive uninstall flow.
pub(crate) fn run() -> anyhow::Result<()> {
    let home = std::env::var("HOME")
        .map_err(|_| anyhow::anyhow!("cannot determine home directory (HOME not set)"))?;

    let _ = init_style::omega_step("Uninstall OMEGA");

    // Mode selection.
    let mode_str: &str = cliclack::select("Choose uninstall mode")
        .item(
            "complete",
            "Complete removal",
            "Delete everything including configuration",
        )
        .item(
            "keep",
            "Keep configuration",
            "Preserve ~/.omega/config.toml for reinstall",
        )
        .interact()?;
    let mode = UninstallMode::from(mode_str);

    // Scan artifacts.
    let artifacts = scan_artifacts(&home, &mode);

    if artifacts.is_empty() {
        let _ = init_style::omega_info("No Omega artifacts found on this system.");
        let _ = init_style::omega_outro("Nothing to remove");
        return Ok(());
    }

    // Display summary.
    display_summary(&artifacts);

    // Confirmation.
    let proceed = cliclack::confirm("Proceed with uninstall?")
        .initial_value(false)
        .interact()?;

    if !proceed {
        let _ = init_style::omega_outro_cancel("Uninstall cancelled");
        return Ok(());
    }

    // Execute deletion steps.
    let mut result = UninstallResult::new();

    step_stop_service(&mut result);
    step_remove_service_file(&mut result);
    step_daemon_reload(&mut result);
    step_remove_data_dir(&home, &mode, &mut result);
    step_remove_symlink("/usr/local/bin/omega", "omega binary", &mut result);
    step_remove_symlink("/usr/local/bin/omg-gog", "omg-gog binary", &mut result);

    // Outro.
    if result.warnings.is_empty() {
        if mode == UninstallMode::KeepConfig {
            let _ = init_style::omega_outro(
                "OMEGA has been removed. Config preserved at ~/.omega/config.toml",
            );
        } else {
            let _ = init_style::omega_outro("OMEGA has been completely removed");
        }
    } else {
        let _ = init_style::omega_warning(&format!(
            "Uninstall completed with {} warning(s)",
            result.warnings.len()
        ));
        if mode == UninstallMode::KeepConfig {
            let _ = init_style::omega_outro(
                "OMEGA has been removed. Config preserved at ~/.omega/config.toml",
            );
        } else {
            let _ = init_style::omega_outro("OMEGA has been completely removed");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Artifact scanning
// ---------------------------------------------------------------------------

/// Scan the filesystem and return existing artifact entries.
fn scan_artifacts(home: &str, mode: &UninstallMode) -> Vec<ArtifactEntry> {
    let mut artifacts = Vec::new();
    let omega_dir = PathBuf::from(home).join(".omega");

    match mode {
        UninstallMode::Complete => {
            if omega_dir.exists() {
                artifacts.push(ArtifactEntry {
                    path: omega_dir,
                    label: "~/.omega/ (entire data directory)".to_string(),
                    preserved: false,
                });
            }
        }
        UninstallMode::KeepConfig => {
            // Show config.toml as preserved if it exists.
            let config_path = omega_dir.join("config.toml");
            if config_path.exists() {
                artifacts.push(ArtifactEntry {
                    path: config_path,
                    label: "~/.omega/config.toml".to_string(),
                    preserved: true,
                });
            }

            // List each subdirectory individually.
            for subdir in KEEP_CONFIG_SUBDIRS {
                let path = omega_dir.join(subdir);
                if path.exists() {
                    artifacts.push(ArtifactEntry {
                        path,
                        label: format!("~/.omega/{subdir}/"),
                        preserved: false,
                    });
                }
            }
        }
    }

    // Service file.
    if let Ok(svc_path) = crate::service::service_file_path() {
        if svc_path.exists() {
            artifacts.push(ArtifactEntry {
                path: svc_path.clone(),
                label: format!("Service file ({})", svc_path.display()),
                preserved: false,
            });
        }
    }

    // Binary symlinks.
    for (path_str, label) in [
        ("/usr/local/bin/omega", "omega binary symlink"),
        ("/usr/local/bin/omg-gog", "omg-gog binary symlink"),
    ] {
        let path = PathBuf::from(path_str);
        // Use symlink_metadata to detect dangling symlinks (exists() follows the symlink).
        if std::fs::symlink_metadata(&path).is_ok() {
            artifacts.push(ArtifactEntry {
                path,
                label: format!("{label} ({path_str})"),
                preserved: false,
            });
        }
    }

    artifacts
}

// ---------------------------------------------------------------------------
// Display
// ---------------------------------------------------------------------------

/// Display the pre-deletion summary.
fn display_summary(artifacts: &[ArtifactEntry]) {
    let _ = init_style::omega_step("The following will be affected:");
    for entry in artifacts {
        if entry.preserved {
            let _ = init_style::omega_success(&format!("{} (preserved)", entry.label));
        } else {
            let _ = init_style::omega_info(&format!("{} (delete)", entry.label));
        }
    }
}

// ---------------------------------------------------------------------------
// Deletion steps
// ---------------------------------------------------------------------------

/// Stop the running service (if any).
fn step_stop_service(result: &mut UninstallResult) {
    if crate::service::is_running() {
        let _ = init_style::omega_step("Stopping service...");
        if let Ok(svc_path) = crate::service::service_file_path() {
            crate::service::stop_service(&svc_path);
            let _ = init_style::omega_success("Service stopped");
        } else {
            result.warn("Could not determine service file path to stop service".to_string());
            let _ =
                init_style::omega_warning("Could not determine service file path to stop service");
        }
    }
}

/// Remove the service file.
fn step_remove_service_file(result: &mut UninstallResult) {
    let svc_path = match crate::service::service_file_path() {
        Ok(p) => p,
        Err(_) => return,
    };

    if !svc_path.exists() {
        return;
    }

    match std::fs::remove_file(&svc_path) {
        Ok(()) => {
            let _ = init_style::omega_success(&format!("Removed {}", svc_path.display()));
        }
        Err(e) => {
            let msg = format!("Failed to remove {}: {e}", svc_path.display());
            let _ = init_style::omega_warning(&msg);
            result.warn(msg);
        }
    }
}

/// Run systemd daemon-reload on Linux. No-op on macOS.
fn step_daemon_reload(result: &mut UninstallResult) {
    if !cfg!(target_os = "linux") {
        return;
    }

    match std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .output()
    {
        Ok(output) if output.status.success() => {
            let _ = init_style::omega_success("systemd daemon reloaded");
        }
        Ok(_) => {
            let msg = "systemctl --user daemon-reload returned non-zero".to_string();
            let _ = init_style::omega_warning(&msg);
            result.warn(msg);
        }
        Err(e) => {
            let msg = format!("Failed to run systemctl daemon-reload: {e}");
            let _ = init_style::omega_warning(&msg);
            result.warn(msg);
        }
    }
}

/// Remove ~/.omega/ directory (complete mode) or subdirectories (keep-config mode).
fn step_remove_data_dir(home: &str, mode: &UninstallMode, result: &mut UninstallResult) {
    let omega_dir = PathBuf::from(home).join(".omega");

    if !omega_dir.exists() {
        return;
    }

    match mode {
        UninstallMode::Complete => match std::fs::remove_dir_all(&omega_dir) {
            Ok(()) => {
                let _ = init_style::omega_success("Removed ~/.omega/");
            }
            Err(e) => {
                let msg = format!("Failed to remove ~/.omega/: {e}");
                let _ = init_style::omega_warning(&msg);
                result.warn(msg);
            }
        },
        UninstallMode::KeepConfig => {
            for subdir in KEEP_CONFIG_SUBDIRS {
                let path = omega_dir.join(subdir);
                if !path.exists() {
                    continue;
                }
                match std::fs::remove_dir_all(&path) {
                    Ok(()) => {
                        let _ = init_style::omega_success(&format!("Removed ~/.omega/{subdir}/"));
                    }
                    Err(e) => {
                        let msg = format!("Failed to remove ~/.omega/{subdir}/: {e}");
                        let _ = init_style::omega_warning(&msg);
                        result.warn(msg);
                    }
                }
            }
        }
    }
}

/// Remove a binary symlink.
fn step_remove_symlink(path: &str, label: &str, result: &mut UninstallResult) {
    let p = PathBuf::from(path);
    // Use symlink_metadata to detect dangling symlinks (exists() follows the symlink).
    if std::fs::symlink_metadata(&p).is_err() {
        return;
    }

    match std::fs::remove_file(&p) {
        Ok(()) => {
            let _ = init_style::omega_success(&format!("Removed {label} ({path})"));
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let msg = format!("Permission denied removing {path}. Run: sudo rm {path}");
            let _ = init_style::omega_warning(&msg);
            result.warn(msg);
        }
        Err(e) => {
            let msg = format!("Failed to remove {path}: {e}");
            let _ = init_style::omega_warning(&msg);
            result.warn(msg);
        }
    }
}

// ---------------------------------------------------------------------------
// cliclack select adapter for UninstallMode
// ---------------------------------------------------------------------------

impl From<&str> for UninstallMode {
    fn from(s: &str) -> Self {
        match s {
            "keep" => UninstallMode::KeepConfig,
            _ => UninstallMode::Complete,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ===================================================================
    // scan_artifacts — complete mode
    // ===================================================================

    #[test]
    fn test_scan_artifacts_complete_mode_with_existing_dir() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let home = tmp.path().to_str().expect("temp path to str");

        // Create .omega directory with some content.
        let omega_dir = tmp.path().join(".omega");
        std::fs::create_dir_all(omega_dir.join("data")).expect("create data dir");
        std::fs::write(omega_dir.join("config.toml"), "test").expect("write config");

        let artifacts = scan_artifacts(home, &UninstallMode::Complete);

        // Should have exactly one entry for the top-level .omega/ dir.
        let omega_entries: Vec<_> = artifacts.iter().filter(|a| a.path == omega_dir).collect();
        assert_eq!(
            omega_entries.len(),
            1,
            "complete mode should list ~/.omega/ as single entry"
        );
        assert!(!omega_entries[0].preserved);
        assert!(omega_entries[0].label.contains(".omega/"));
    }

    // ===================================================================
    // scan_artifacts — keep-config mode
    // ===================================================================

    #[test]
    fn test_scan_artifacts_keep_config_mode() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let home = tmp.path().to_str().expect("temp path to str");

        let omega_dir = tmp.path().join(".omega");
        std::fs::create_dir_all(omega_dir.join("data")).expect("create data dir");
        std::fs::create_dir_all(omega_dir.join("logs")).expect("create logs dir");
        std::fs::write(omega_dir.join("config.toml"), "test").expect("write config");

        let artifacts = scan_artifacts(home, &UninstallMode::KeepConfig);

        // config.toml should be preserved.
        let config_entry = artifacts.iter().find(|a| a.label.contains("config.toml"));
        assert!(config_entry.is_some(), "config.toml should appear in list");
        assert!(config_entry.expect("checked").preserved);

        // data/ and logs/ should be listed for deletion.
        let data_entry = artifacts
            .iter()
            .find(|a| a.label.contains("data/") && !a.preserved);
        assert!(data_entry.is_some(), "data/ should be listed for deletion");

        let logs_entry = artifacts
            .iter()
            .find(|a| a.label.contains("logs/") && !a.preserved);
        assert!(logs_entry.is_some(), "logs/ should be listed for deletion");
    }

    // ===================================================================
    // scan_artifacts — non-existing directory
    // ===================================================================

    #[test]
    fn test_scan_artifacts_nonexisting_directory() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let home = tmp.path().to_str().expect("temp path to str");
        // Do NOT create .omega — it doesn't exist.

        let artifacts_complete = scan_artifacts(home, &UninstallMode::Complete);
        let omega_only: Vec<_> = artifacts_complete
            .iter()
            .filter(|a| a.path.starts_with(tmp.path().join(".omega")))
            .collect();
        assert!(
            omega_only.is_empty(),
            "no .omega entries when dir does not exist"
        );

        let artifacts_keep = scan_artifacts(home, &UninstallMode::KeepConfig);
        let omega_only_keep: Vec<_> = artifacts_keep
            .iter()
            .filter(|a| a.path.starts_with(tmp.path().join(".omega")))
            .collect();
        assert!(
            omega_only_keep.is_empty(),
            "no .omega entries when dir does not exist in keep-config mode"
        );
    }

    // ===================================================================
    // UninstallResult warning accumulation
    // ===================================================================

    #[test]
    fn test_uninstall_result_warning_accumulation() {
        let mut result = UninstallResult::new();
        assert!(result.warnings.is_empty());

        result.warn("first warning".to_string());
        result.warn("second warning".to_string());

        assert_eq!(result.warnings.len(), 2);
        assert_eq!(result.warnings[0], "first warning");
        assert_eq!(result.warnings[1], "second warning");
    }

    // ===================================================================
    // step_remove_data_dir — complete mode
    // ===================================================================

    #[test]
    fn test_step_remove_data_dir_complete_mode() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let home = tmp.path().to_str().expect("temp path to str");

        let omega_dir = tmp.path().join(".omega");
        std::fs::create_dir_all(omega_dir.join("data")).expect("create data dir");
        std::fs::write(omega_dir.join("config.toml"), "test").expect("write config");

        let mut result = UninstallResult::new();
        step_remove_data_dir(home, &UninstallMode::Complete, &mut result);

        assert!(!omega_dir.exists(), "~/.omega/ should be fully removed");
        assert!(result.warnings.is_empty(), "no warnings expected");
    }

    // ===================================================================
    // step_remove_data_dir — keep-config mode
    // ===================================================================

    #[test]
    fn test_step_remove_data_dir_keep_config_mode() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let home = tmp.path().to_str().expect("temp path to str");

        let omega_dir = tmp.path().join(".omega");
        std::fs::create_dir_all(omega_dir.join("data")).expect("create data dir");
        std::fs::create_dir_all(omega_dir.join("logs")).expect("create logs dir");
        std::fs::create_dir_all(omega_dir.join("workspace")).expect("create workspace dir");
        std::fs::write(omega_dir.join("config.toml"), "key = \"secret\"").expect("write config");

        let mut result = UninstallResult::new();
        step_remove_data_dir(home, &UninstallMode::KeepConfig, &mut result);

        assert!(
            omega_dir.join("config.toml").exists(),
            "config.toml must be preserved"
        );
        assert!(!omega_dir.join("data").exists(), "data/ should be removed");
        assert!(!omega_dir.join("logs").exists(), "logs/ should be removed");
        assert!(
            !omega_dir.join("workspace").exists(),
            "workspace/ should be removed"
        );
        assert!(result.warnings.is_empty(), "no warnings expected");
    }

    // ===================================================================
    // step_remove_symlink — existing file
    // ===================================================================

    #[test]
    fn test_step_remove_symlink_existing_file() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let file_path = tmp.path().join("test_binary");
        std::fs::write(&file_path, "binary").expect("write test binary");

        let path_str = file_path.to_str().expect("path to str");
        let mut result = UninstallResult::new();
        step_remove_symlink(path_str, "test binary", &mut result);

        assert!(!file_path.exists(), "symlink should be removed");
        assert!(result.warnings.is_empty(), "no warnings expected");
    }

    // ===================================================================
    // step_remove_symlink — non-existent path
    // ===================================================================

    #[test]
    fn test_step_remove_symlink_nonexistent_path() {
        let mut result = UninstallResult::new();
        step_remove_symlink("/tmp/nonexistent_omega_binary_12345", "ghost", &mut result);

        assert!(
            result.warnings.is_empty(),
            "no warnings for non-existent path"
        );
    }

    // ===================================================================
    // UninstallMode from &str
    // ===================================================================

    #[test]
    fn test_uninstall_mode_from_str() {
        assert_eq!(UninstallMode::from("complete"), UninstallMode::Complete);
        assert_eq!(UninstallMode::from("keep"), UninstallMode::KeepConfig);
        assert_eq!(
            UninstallMode::from("anything_else"),
            UninstallMode::Complete
        );
    }

    // ===================================================================
    // step_remove_symlink — dangling symlink (broken target)
    // ===================================================================

    #[cfg(unix)]
    #[test]
    fn test_step_remove_symlink_dangling_symlink() {
        let tmp = tempfile::tempdir().expect("create temp dir");
        let symlink_path = tmp.path().join("dangling_link");

        // Create a symlink pointing to a non-existent target.
        std::os::unix::fs::symlink("/tmp/nonexistent_target_99999", &symlink_path)
            .expect("create symlink");

        // Verify the dangling symlink exists as a symlink but not via exists().
        assert!(
            !symlink_path.exists(),
            "dangling symlink should return false for exists()"
        );
        assert!(
            std::fs::symlink_metadata(&symlink_path).is_ok(),
            "dangling symlink should be detectable via symlink_metadata"
        );

        let path_str = symlink_path.to_str().expect("path to str");
        let mut result = UninstallResult::new();
        step_remove_symlink(path_str, "dangling link", &mut result);

        assert!(
            std::fs::symlink_metadata(&symlink_path).is_err(),
            "dangling symlink should be removed"
        );
        assert!(result.warnings.is_empty(), "no warnings expected");
    }
}
