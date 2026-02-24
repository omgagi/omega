mod channels;
mod defaults;
mod prompts;
mod providers;

#[cfg(test)]
mod tests;

pub use channels::*;
pub use prompts::*;
pub use providers::*;

use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{info, warn};

use crate::error::OmegaError;
use defaults::*;

/// Top-level Omega configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub omega: OmegaConfig,
    #[serde(default)]
    pub auth: AuthConfig,
    #[serde(default)]
    pub provider: ProviderConfig,
    #[serde(default)]
    pub channel: ChannelConfig,
    #[serde(default)]
    pub memory: MemoryConfig,
    #[serde(default)]
    pub heartbeat: HeartbeatConfig,
    #[serde(default)]
    pub scheduler: SchedulerConfig,
    #[serde(default)]
    pub api: ApiConfig,
}

/// Authentication configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthConfig {
    /// Whether auth is enforced (default: true).
    /// When true and no allowed_users are set on any channel, ALL messages are rejected.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Message sent to unauthorized users.
    #[serde(default = "default_deny_message")]
    pub deny_message: String,
}

/// General agent settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmegaConfig {
    #[serde(default = "default_name")]
    pub name: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
    #[serde(default = "default_log_level")]
    pub log_level: String,
}

impl Default for OmegaConfig {
    fn default() -> Self {
        Self {
            name: default_name(),
            data_dir: default_data_dir(),
            log_level: default_log_level(),
        }
    }
}

/// Memory config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    #[serde(default = "default_memory_backend")]
    pub backend: String,
    #[serde(default = "default_db_path")]
    pub db_path: String,
    #[serde(default = "default_max_context")]
    pub max_context_messages: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            backend: default_memory_backend(),
            db_path: default_db_path(),
            max_context_messages: default_max_context(),
        }
    }
}

/// Heartbeat configuration -- periodic AI check-ins.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_heartbeat_interval")]
    pub interval_minutes: u64,
    /// Active hours start (e.g. "08:00"). Empty = always active.
    #[serde(default)]
    pub active_start: String,
    /// Active hours end (e.g. "22:00"). Empty = always active.
    #[serde(default)]
    pub active_end: String,
    /// Channel to deliver heartbeat alerts (e.g. "telegram").
    #[serde(default)]
    pub channel: String,
    /// Platform-specific target for delivery (e.g. chat_id).
    #[serde(default)]
    pub reply_target: String,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            interval_minutes: default_heartbeat_interval(),
            active_start: String::new(),
            active_end: String::new(),
            channel: String::new(),
            reply_target: String::new(),
        }
    }
}

/// Scheduler configuration -- user-scheduled reminders and tasks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SchedulerConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            poll_interval_secs: default_poll_interval(),
        }
    }
}

/// HTTP API configuration -- lightweight server for SaaS dashboard integration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_api_host")]
    pub host: String,
    #[serde(default = "default_api_port")]
    pub port: u16,
    /// Bearer token for API authentication. Empty = no auth (for local-only use).
    #[serde(default)]
    pub api_key: String,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            host: default_api_host(),
            port: default_api_port(),
            api_key: String::new(),
        }
    }
}

/// System-managed fact keys that only bot commands may write.
///
/// Used to filter system facts from user profiles, protect them during `/purge`,
/// and reject them in `is_valid_fact()` validation.
pub const SYSTEM_FACT_KEYS: &[&str] = &[
    "welcomed",
    "preferred_language",
    "active_project",
    "personality",
    "onboarding_stage",
    "pending_build_request",
];

/// Expand `~` to home directory.
pub fn shellexpand(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return format!("{}/{rest}", home.to_string_lossy());
        }
    }
    path.to_string()
}

/// Migrate the flat `~/.omega/` layout to the structured subdirectory layout.
///
/// Creates `data/`, `logs/`, `prompts/` subdirectories and moves files from
/// old locations to new ones. Only moves if source exists AND destination does
/// NOT (idempotent, no overwrites). Also patches `config.toml` if it contains
/// the old default `db_path`.
pub fn migrate_layout(data_dir: &str, config_path: &str) {
    let dir = shellexpand(data_dir);
    let base = Path::new(&dir);

    // Create subdirectories.
    for sub in &["data", "logs", "prompts"] {
        let _ = std::fs::create_dir_all(base.join(sub));
    }

    // Migration pairs: (old_relative, new_relative).
    let pairs: &[(&str, &str)] = &[
        ("memory.db", "data/memory.db"),
        ("memory.db-wal", "data/memory.db-wal"),
        ("memory.db-shm", "data/memory.db-shm"),
        ("omega.log", "logs/omega.log"),
        ("omega.stdout.log", "logs/omega.stdout.log"),
        ("omega.stderr.log", "logs/omega.stderr.log"),
        ("SYSTEM_PROMPT.md", "prompts/SYSTEM_PROMPT.md"),
        ("WELCOME.toml", "prompts/WELCOME.toml"),
        ("HEARTBEAT.md", "prompts/HEARTBEAT.md"),
    ];

    for (old_rel, new_rel) in pairs {
        let src = base.join(old_rel);
        let dst = base.join(new_rel);
        if src.exists() && !dst.exists() {
            if let Err(e) = std::fs::rename(&src, &dst) {
                warn!(
                    "migrate: failed to move {} → {}: {e}",
                    src.display(),
                    dst.display()
                );
            } else {
                info!("migrate: {} → {}", old_rel, new_rel);
            }
        }
    }

    // Patch config.toml if it contains the old default db_path.
    let config_file = Path::new(config_path);
    if config_file.exists() {
        if let Ok(content) = std::fs::read_to_string(config_file) {
            if content.contains("~/.omega/memory.db") {
                let patched = content.replace("~/.omega/memory.db", "~/.omega/data/memory.db");
                if let Err(e) = std::fs::write(config_file, patched) {
                    warn!("migrate: failed to patch config db_path: {e}");
                } else {
                    info!("migrate: patched config.toml db_path");
                }
            }
        }
    }
}

/// Patch `interval_minutes` in config.toml's `[heartbeat]` section.
///
/// Text-based patching preserves comments and formatting (same approach as `migrate_layout`).
/// Non-fatal: logs on error but never panics.
pub fn patch_heartbeat_interval(config_path: &str, minutes: u64) {
    let path = Path::new(config_path);
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("patch_heartbeat_interval: failed to read {config_path}: {e}");
            return;
        }
    };

    let new_line = format!("interval_minutes = {minutes}");
    let patched = if let Some(pos) = content.find("[heartbeat]") {
        // Find the section body (after the header line).
        let section_start = content[pos..]
            .find('\n')
            .map(|i| pos + i + 1)
            .unwrap_or(content.len());
        // Look for existing `interval_minutes` within this section (before next section header).
        let section_end = content[section_start..]
            .find("\n[")
            .map(|i| section_start + i)
            .unwrap_or(content.len());
        let section_body = &content[section_start..section_end];

        if let Some(key_offset) = section_body.find("interval_minutes") {
            // Replace the existing line.
            let abs_start = section_start + key_offset;
            let line_end = content[abs_start..]
                .find('\n')
                .map(|i| abs_start + i)
                .unwrap_or(content.len());
            format!(
                "{}{}{}",
                &content[..abs_start],
                new_line,
                &content[line_end..]
            )
        } else {
            // Section exists but no interval_minutes — insert after header.
            format!(
                "{}{}\n{}",
                &content[..section_start],
                new_line,
                &content[section_start..]
            )
        }
    } else {
        // No [heartbeat] section — append it.
        format!("{}\n[heartbeat]\n{}\n", content.trim_end(), new_line)
    };

    if let Err(e) = std::fs::write(path, patched) {
        warn!("patch_heartbeat_interval: failed to write {config_path}: {e}");
    } else {
        info!("heartbeat interval_minutes persisted to config: {minutes}");
    }
}

/// Load configuration from a TOML file.
///
/// Falls back to defaults if the file does not exist.
pub fn load(path: &str) -> Result<Config, OmegaError> {
    let path = Path::new(path);
    if !path.exists() {
        tracing::info!(
            "Config file not found at {}, using defaults",
            path.display()
        );
        return Ok(Config {
            omega: OmegaConfig::default(),
            auth: AuthConfig::default(),
            provider: ProviderConfig {
                default: default_provider(),
                claude_code: Some(ClaudeCodeConfig::default()),
                ..Default::default()
            },
            channel: ChannelConfig::default(),
            memory: MemoryConfig::default(),
            heartbeat: HeartbeatConfig::default(),
            scheduler: SchedulerConfig::default(),
            api: ApiConfig::default(),
        });
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| OmegaError::Config(format!("failed to read {}: {}", path.display(), e)))?;

    let config: Config = toml::from_str(&content)
        .map_err(|e| OmegaError::Config(format!("failed to parse config: {}", e)))?;

    Ok(config)
}
