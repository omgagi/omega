# src/service.rs — Service Management Documentation

## Overview

The service module provides **OS-aware service management** for Omega. It detects whether you're on macOS or Linux and installs the appropriate service file so Omega starts automatically on login and restarts on crash.

| OS | Service Type | File Location |
|----|-------------|---------------|
| macOS | LaunchAgent | `~/Library/LaunchAgents/com.omega-cortex.omega.plist` |
| Linux | systemd user unit | `~/.config/systemd/user/omega.service` |

No new dependencies are required — the module uses `std::process::Command`, `std::fs`, and the existing `cliclack`/`console` crates.

---

## Commands

### `omega service install`

Installs Omega as a system service that starts automatically on login.

```bash
omega service install
```

With a custom config path:
```bash
omega --config /path/to/config.toml service install
```

**What happens:**
1. Resolves the Omega binary path automatically via `current_exe()`
2. Resolves the config file to an absolute path (must exist)
3. Generates the appropriate service file for your OS
4. Writes the service file to the correct location
5. Activates the service (`launchctl load` on macOS, `systemctl --user enable --now` on Linux)

**Example output:**
```
┌  omega service install
│
◇  Binary: /usr/local/bin/omega
◇  Config: /Users/you/omega/config.toml
◇  Service file: /Users/you/Library/LaunchAgents/com.omega-cortex.omega.plist
◇  Wrote /Users/you/Library/LaunchAgents/com.omega-cortex.omega.plist
◇  Service activated
│
└  Omega will now start automatically on login
```

**If the service file already exists:**
```
◆  Service file already exists. Overwrite?
│  Yes / No
```
If you confirm, the old service is stopped before overwriting.

---

### `omega service uninstall`

Removes the Omega system service.

```bash
omega service uninstall
```

**What happens:**
1. Checks if a service file exists
2. Stops the running service
3. Removes the service file

**Example output:**
```
┌  omega service uninstall
│
◇  Service stopped
◇  Removed /Users/you/Library/LaunchAgents/com.omega-cortex.omega.plist
│
└  Service removed
```

If no service is installed:
```
┌  omega service uninstall
│
└  No service file found — nothing to do
```

---

### `omega service status`

Checks whether the service is installed and running.

```bash
omega service status
```

**Example output (installed and running):**
```
┌  omega service status
│
◇  Installed: /Users/you/Library/LaunchAgents/com.omega-cortex.omega.plist
◇  Status: running
│
└  Done
```

**Example output (not installed):**
```
┌  omega service status
│
▲  Service is not installed
◇  Run `omega service install` to set it up
│
└  Done
```

---

## Init Wizard Integration

The `omega init` wizard offers service installation as its final step (Phase 8), before the "Next steps" summary:

```
◆  Install Omega as a system service?
│  Yes / No
```

If the user accepts, the service is installed immediately. If it fails, the wizard continues with a warning and suggests running `omega service install` later.

---

## Service File Contents

### macOS LaunchAgent (plist)

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.omega-cortex.omega</string>
    <key>ProgramArguments</key>
    <array>
        <string>/path/to/omega</string>
        <string>-c</string>
        <string>/path/to/config.toml</string>
        <string>start</string>
    </array>
    <key>WorkingDirectory</key>
    <string>/path/to/project</string>
    <key>RunAtLoad</key><true/>
    <key>KeepAlive</key><true/>
    <key>StandardOutPath</key>
    <string>/Users/you/.omega/logs/omega.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/Users/you/.omega/logs/omega.stderr.log</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>PATH</key>
        <string>/usr/local/bin:/usr/bin:/bin:/opt/homebrew/bin</string>
    </dict>
</dict>
</plist>
```

**Key features:**
- `RunAtLoad` — Starts when the user logs in
- `KeepAlive` — Restarts automatically if Omega crashes
- `PATH` includes `/opt/homebrew/bin` for Apple Silicon Macs

### Linux systemd User Unit

```ini
[Unit]
Description=Omega AI Agent
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
ExecStart=/path/to/omega -c /path/to/config.toml start
WorkingDirectory=/path/to/project
Restart=on-failure
RestartSec=5
StandardOutput=append:/home/you/.omega/logs/omega.stdout.log
StandardError=append:/home/you/.omega/logs/omega.stderr.log

[Install]
WantedBy=default.target
```

**Key features:**
- `Restart=on-failure` with 5-second delay
- User-level service (no root required)
- Logs to `~/.omega/` alongside the main Omega log

---

## How Paths Are Resolved

| Path | Resolution |
|------|-----------|
| Binary | `std::env::current_exe()` + `canonicalize()` — always absolute |
| Config | `Path::canonicalize()` on the `-c` flag value — must exist |
| Working directory | Parent of the config file |
| Data directory | `~/.omega` via `shellexpand` |
| Service file | OS-specific, derived from `$HOME` |

---

## Troubleshooting

### "config file not found — run `omega init` first"
The config file specified via `-c` (default: `config.toml`) doesn't exist. Run `omega init` to create it, or specify the correct path.

### "cannot determine home directory"
The `HOME` environment variable is not set. This is unusual — check your shell environment.

### "Unsupported OS"
Service management only supports macOS and Linux. On other platforms, run Omega manually with `omega start`.

### Service installed but not running
Check the logs:
```bash
# macOS
cat ~/.omega/logs/omega.stdout.log
cat ~/.omega/logs/omega.stderr.log

# Linux
journalctl --user -u omega.service
cat ~/.omega/logs/omega.stderr.log
```

### Reinstalling after moving the binary
If you move or rebuild the Omega binary, re-run `omega service install` to update the service file with the new binary path.

---

## Related Commands

| Command | Purpose |
|---------|---------|
| `omega init` | Setup wizard (offers service install at the end) |
| `omega start` | Start Omega manually (without service manager) |
| `omega status` | Check provider and channel health |
| `omega service install` | Install as system service |
| `omega service uninstall` | Remove system service |
| `omega service status` | Check service installation and running state |
