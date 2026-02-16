use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::error::OmegaError;

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
    pub sandbox: SandboxConfig,
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

/// Provider configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    #[serde(default = "default_provider")]
    pub default: String,
    #[serde(default, rename = "claude-code")]
    pub claude_code: Option<ClaudeCodeConfig>,
    pub anthropic: Option<AnthropicConfig>,
    pub openai: Option<OpenAiConfig>,
    pub ollama: Option<OllamaConfig>,
    pub openrouter: Option<OpenRouterConfig>,
}

/// Claude Code CLI provider config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeCodeConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_turns")]
    pub max_turns: u32,
    #[serde(default = "default_allowed_tools")]
    pub allowed_tools: Vec<String>,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_turns: 10,
            allowed_tools: default_allowed_tools(),
        }
    }
}

/// Anthropic API provider config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_anthropic_model")]
    pub model: String,
}

/// OpenAI-compatible provider config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_openai_model")]
    pub model: String,
    #[serde(default = "default_openai_base_url")]
    pub base_url: String,
}

/// Ollama local provider config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_ollama_base_url")]
    pub base_url: String,
    #[serde(default = "default_ollama_model")]
    pub model: String,
}

/// OpenRouter proxy config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub model: String,
}

/// Channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelConfig {
    pub telegram: Option<TelegramConfig>,
    pub whatsapp: Option<WhatsAppConfig>,
}

/// Telegram bot config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub allowed_users: Vec<i64>,
}

/// WhatsApp bridge config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bridge_url: String,
    #[serde(default)]
    pub phone_number: String,
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

/// Sandbox config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub allowed_commands: Vec<String>,
    #[serde(default)]
    pub blocked_paths: Vec<String>,
    #[serde(default = "default_execution_time")]
    pub max_execution_time_secs: u64,
    #[serde(default = "default_output_bytes")]
    pub max_output_bytes: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_commands: Vec::new(),
            blocked_paths: Vec::new(),
            max_execution_time_secs: default_execution_time(),
            max_output_bytes: default_output_bytes(),
        }
    }
}

// --- Default value functions ---

fn default_name() -> String {
    "Omega".to_string()
}
fn default_data_dir() -> String {
    "~/.omega".to_string()
}
fn default_log_level() -> String {
    "info".to_string()
}
fn default_provider() -> String {
    "claude-code".to_string()
}
fn default_true() -> bool {
    true
}
fn default_deny_message() -> String {
    "Access denied. You are not authorized to use this agent.".to_string()
}
fn default_max_turns() -> u32 {
    10
}
fn default_allowed_tools() -> Vec<String> {
    vec!["Bash".into(), "Read".into(), "Write".into(), "Edit".into()]
}
fn default_anthropic_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}
fn default_openai_model() -> String {
    "gpt-4o".to_string()
}
fn default_openai_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}
fn default_ollama_base_url() -> String {
    "http://localhost:11434".to_string()
}
fn default_ollama_model() -> String {
    "llama3".to_string()
}
fn default_memory_backend() -> String {
    "sqlite".to_string()
}
fn default_db_path() -> String {
    "~/.omega/memory.db".to_string()
}
fn default_max_context() -> usize {
    50
}
fn default_execution_time() -> u64 {
    30
}
fn default_output_bytes() -> usize {
    1_048_576
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
            sandbox: SandboxConfig::default(),
        });
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| OmegaError::Config(format!("failed to read {}: {}", path.display(), e)))?;

    let config: Config = toml::from_str(&content)
        .map_err(|e| OmegaError::Config(format!("failed to parse config: {}", e)))?;

    Ok(config)
}
