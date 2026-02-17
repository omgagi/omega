//! OS-aware service management — install, uninstall, and status for macOS LaunchAgent / Linux systemd.

use std::path::{Path, PathBuf};

/// macOS LaunchAgent label.
const LABEL: &str = "com.omega-cortex.omega";

// ---------------------------------------------------------------------------
// Pure functions (testable, no I/O)
// ---------------------------------------------------------------------------

/// Escape XML special characters for plist safety.
pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Generate a macOS LaunchAgent plist.
pub fn generate_plist(
    binary_path: &str,
    config_path: &str,
    working_dir: &str,
    data_dir: &str,
) -> String {
    let binary = xml_escape(binary_path);
    let config = xml_escape(config_path);
    let work = xml_escape(working_dir);
    let data = xml_escape(data_dir);

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{binary}</string>
        <string>-c</string>
        <string>{config}</string>
        <string>start</string>
    </array>
    <key>WorkingDirectory</key>
    <string>{work}</string>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{data}/omega.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>{data}/omega.stderr.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin</string>
    </dict>
</dict>
</plist>
"#
    )
}

/// Generate a Linux systemd user unit file.
pub fn generate_systemd_unit(
    binary_path: &str,
    config_path: &str,
    working_dir: &str,
    data_dir: &str,
) -> String {
    format!(
        r#"[Unit]
Description=OMEGA Ω AI Agent
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart={binary_path} -c {config_path} start
WorkingDirectory={working_dir}
Restart=on-failure
RestartSec=5
StandardOutput=append:{data_dir}/omega.stdout.log
StandardError=append:{data_dir}/omega.stderr.log

[Install]
WantedBy=default.target
"#
    )
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Return the path where the service file should live.
fn service_file_path() -> anyhow::Result<PathBuf> {
    let home_str = std::env::var("HOME")
        .map_err(|_| anyhow::anyhow!("cannot determine home directory (HOME not set)"))?;
    let home = PathBuf::from(home_str);

    if cfg!(target_os = "macos") {
        Ok(home
            .join("Library")
            .join("LaunchAgents")
            .join(format!("{LABEL}.plist")))
    } else if cfg!(target_os = "linux") {
        Ok(home
            .join(".config")
            .join("systemd")
            .join("user")
            .join("omega.service"))
    } else {
        anyhow::bail!("Unsupported OS — only macOS and Linux are supported for service management");
    }
}

// ---------------------------------------------------------------------------
// I/O functions (interactive, use cliclack)
// ---------------------------------------------------------------------------

/// Install Omega as a system service (LaunchAgent on macOS, systemd on Linux).
pub fn install(config_path: &str) -> anyhow::Result<()> {
    cliclack::intro(console::style("omega service install").bold().to_string())?;

    // 1. Resolve binary path.
    let binary = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(|e| anyhow::anyhow!("cannot resolve binary path: {e}"))?;
    let binary_str = binary.display().to_string();
    cliclack::log::info(format!("Binary: {binary_str}"))?;

    // 2. Resolve config path — bail if missing.
    let config_abs = Path::new(config_path).canonicalize().map_err(|_| {
        anyhow::anyhow!("config file '{config_path}' not found — run `omega init` first")
    })?;
    let config_str = config_abs.display().to_string();
    cliclack::log::info(format!("Config: {config_str}"))?;

    // 3. Working directory = parent of config file.
    let working_dir = config_abs
        .parent()
        .unwrap_or(Path::new("/"))
        .display()
        .to_string();

    // 4. Data directory.
    let data_dir = omega_core::shellexpand("~/.omega");

    // 5. Determine service file path.
    let svc_path = service_file_path()?;
    cliclack::log::info(format!("Service file: {}", svc_path.display()))?;

    // 6. If file exists, ask to overwrite and stop old service first.
    if svc_path.exists() {
        let overwrite: bool = cliclack::confirm("Service file already exists. Overwrite?")
            .initial_value(true)
            .interact()?;
        if !overwrite {
            cliclack::outro("Cancelled — existing service unchanged")?;
            return Ok(());
        }
        // Stop old service.
        stop_service(&svc_path);
    }

    // 7. Generate content.
    let content = if cfg!(target_os = "macos") {
        generate_plist(&binary_str, &config_str, &working_dir, &data_dir)
    } else {
        generate_systemd_unit(&binary_str, &config_str, &working_dir, &data_dir)
    };

    // 8. Write file (create parent dirs if needed).
    if let Some(parent) = svc_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&svc_path, &content)?;
    cliclack::log::success(format!("Wrote {}", svc_path.display()))?;

    // 9. Activate.
    let spinner = cliclack::spinner();
    spinner.start("Activating service...");
    let activated = activate_service(&svc_path);
    if activated {
        spinner.stop("Service activated");
    } else {
        spinner.error("Activation returned an error — check logs");
    }

    cliclack::outro("OMEGA Ω will now start automatically on login")?;
    Ok(())
}

/// Remove the Omega system service.
pub fn uninstall() -> anyhow::Result<()> {
    cliclack::intro(console::style("omega service uninstall").bold().to_string())?;

    let svc_path = service_file_path()?;

    if !svc_path.exists() {
        cliclack::outro("No service file found — nothing to do")?;
        return Ok(());
    }

    // Deactivate.
    let spinner = cliclack::spinner();
    spinner.start("Stopping service...");
    stop_service(&svc_path);
    spinner.stop("Service stopped");

    // Remove file.
    std::fs::remove_file(&svc_path)?;
    cliclack::log::success(format!("Removed {}", svc_path.display()))?;

    cliclack::outro("Service removed")?;
    Ok(())
}

/// Check service installation and running status.
pub fn status() -> anyhow::Result<()> {
    cliclack::intro(console::style("omega service status").bold().to_string())?;

    let svc_path = service_file_path()?;

    if !svc_path.exists() {
        cliclack::log::warning("Service is not installed")?;
        cliclack::log::info("Run `omega service install` to set it up")?;
        cliclack::outro("Done")?;
        return Ok(());
    }

    cliclack::log::success(format!("Installed: {}", svc_path.display()))?;

    // Check if running.
    let running = is_running();
    if running {
        cliclack::log::success("Status: running")?;
    } else {
        cliclack::log::warning("Status: not running")?;
    }

    cliclack::outro("Done")?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Platform helpers (subprocess calls)
// ---------------------------------------------------------------------------

/// Stop / unload the service.
fn stop_service(svc_path: &Path) {
    if cfg!(target_os = "macos") {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", &svc_path.display().to_string()])
            .output();
    } else {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "stop", "omega.service"])
            .output();
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "disable", "omega.service"])
            .output();
    }
}

/// Activate / load the service. Returns true on success.
fn activate_service(svc_path: &Path) -> bool {
    if cfg!(target_os = "macos") {
        std::process::Command::new("launchctl")
            .args(["load", &svc_path.display().to_string()])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        let _ = std::process::Command::new("systemctl")
            .args(["--user", "daemon-reload"])
            .output();
        let enable = std::process::Command::new("systemctl")
            .args(["--user", "enable", "--now", "omega.service"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        enable
    }
}

/// Check whether the service is currently running.
fn is_running() -> bool {
    if cfg!(target_os = "macos") {
        std::process::Command::new("launchctl")
            .args(["list"])
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout);
                stdout.contains(LABEL)
            })
            .unwrap_or(false)
    } else {
        std::process::Command::new("systemctl")
            .args(["--user", "is-active", "--quiet", "omega.service"])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_plist_content() {
        let plist = generate_plist(
            "/usr/local/bin/omega",
            "/home/user/config.toml",
            "/home/user",
            "/home/user/.omega",
        );
        assert!(plist.contains("<string>com.omega-cortex.omega</string>"));
        assert!(plist.contains("<string>/usr/local/bin/omega</string>"));
        assert!(plist.contains("<string>/home/user/config.toml</string>"));
        assert!(plist.contains("<string>/home/user</string>"));
        assert!(plist.contains("<true/>"), "RunAtLoad should be true");
        assert!(plist.contains("<key>KeepAlive</key>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("omega.stdout.log"));
        assert!(plist.contains("omega.stderr.log"));
    }

    #[test]
    fn test_generate_plist_xml_escaping() {
        let plist = generate_plist(
            "/path/with <angle> & amp",
            "/config/<special>.toml",
            "/work",
            "/data",
        );
        assert!(plist.contains("/path/with &lt;angle&gt; &amp; amp"));
        assert!(plist.contains("/config/&lt;special&gt;.toml"));
        assert!(
            !plist.contains("<angle>"),
            "raw angle brackets must be escaped"
        );
    }

    #[test]
    fn test_generate_systemd_unit_content() {
        let unit = generate_systemd_unit(
            "/usr/local/bin/omega",
            "/home/user/config.toml",
            "/home/user",
            "/home/user/.omega",
        );
        assert!(unit.contains("ExecStart=/usr/local/bin/omega -c /home/user/config.toml start"));
        assert!(unit.contains("WorkingDirectory=/home/user"));
        assert!(unit.contains("Restart=on-failure"));
        assert!(unit.contains("RestartSec=5"));
        assert!(unit.contains("omega.stdout.log"));
        assert!(unit.contains("omega.stderr.log"));
    }

    #[test]
    fn test_generate_systemd_unit_structure() {
        let unit = generate_systemd_unit("/bin/omega", "/etc/omega.toml", "/tmp", "/var/omega");
        assert!(unit.contains("[Unit]"));
        assert!(unit.contains("[Service]"));
        assert!(unit.contains("[Install]"));
        assert!(unit.contains("WantedBy=default.target"));
        assert!(unit.contains("Description=OMEGA Ω AI Agent"));
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("hello"), "hello");
        assert_eq!(xml_escape("a & b"), "a &amp; b");
        assert_eq!(xml_escape("<tag>"), "&lt;tag&gt;");
        assert_eq!(xml_escape("a < b > c & d"), "a &lt; b &gt; c &amp; d");
    }
}
