use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, warn};

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
    #[serde(default)]
    pub heartbeat: HeartbeatConfig,
    #[serde(default)]
    pub scheduler: SchedulerConfig,
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
    /// Subprocess timeout in seconds (default: 600 = 10 minutes).
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_turns: 10,
            allowed_tools: default_allowed_tools(),
            timeout_secs: default_timeout_secs(),
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

/// WhatsApp channel config.
///
/// Session data is stored at `{data_dir}/whatsapp_session/`.
/// Pairing is done by scanning a QR code (like WhatsApp Web).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Allowed phone numbers (e.g. `["5511999887766"]`). Empty = allow all.
    #[serde(default)]
    pub allowed_users: Vec<String>,
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

/// Sandbox mode — controls how far Claude Code can reach beyond the workspace.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode {
    /// Workspace only — no host access (default, safest).
    #[default]
    Sandbox,
    /// Read & execute on host, writes only inside workspace.
    Rx,
    /// Full host access (for power users).
    Rwx,
}

impl SandboxMode {
    /// Return the system prompt constraint for this mode, or `None` for unrestricted.
    pub fn prompt_constraint(&self, workspace_path: &str) -> Option<String> {
        match self {
            Self::Sandbox => Some(format!(
                "You are in SANDBOX mode. Your working directory is {workspace_path}.\n\
                 You MUST only create, modify, and read files within this directory.\n\
                 Do NOT access, read, or modify any files outside your working directory.\n\
                 You have full network access (curl, wget, API calls).\n\
                 Install dependencies locally (npm install, pip install --target, etc)."
            )),
            Self::Rx => Some(format!(
                "You are in READ-ONLY mode. Your working directory is {workspace_path}.\n\
                 You may READ files anywhere on the host filesystem to inspect and analyze.\n\
                 You may EXECUTE read-only commands (ls, cat, grep, ps, etc).\n\
                 You MUST only WRITE or CREATE files inside your working directory ({workspace_path}).\n\
                 Do NOT modify, delete, or create files outside your working directory."
            )),
            Self::Rwx => None,
        }
    }

    /// Human-readable name for display (e.g. in `/status`).
    pub fn display_name(&self) -> &str {
        match self {
            Self::Sandbox => "sandbox",
            Self::Rx => "rx",
            Self::Rwx => "rwx",
        }
    }
}

/// Sandbox config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SandboxConfig {
    #[serde(default)]
    pub mode: SandboxMode,
}

/// Heartbeat configuration — periodic AI check-ins.
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

/// Scheduler configuration — user-scheduled reminders and tasks.
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

// --- Default value functions ---

fn default_name() -> String {
    "OMEGA Ω".to_string()
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
fn default_heartbeat_interval() -> u64 {
    30
}
fn default_poll_interval() -> u64 {
    60
}
fn default_timeout_secs() -> u64 {
    600
}

/// Expand `~` to home directory.
pub fn shellexpand(path: &str) -> String {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return format!("{}/{rest}", home.to_string_lossy());
        }
    }
    path.to_string()
}

/// Externalized prompts and welcome messages, loaded from `~/.omega/` at startup.
///
/// If files are missing, hardcoded defaults are used (backward compatible).
#[derive(Debug, Clone)]
pub struct Prompts {
    /// Base system prompt (personality/rules).
    pub system: String,
    /// Conversation summarization instruction.
    pub summarize: String,
    /// Facts extraction instruction.
    pub facts: String,
    /// Heartbeat prompt (no checklist).
    pub heartbeat: String,
    /// Heartbeat prompt with `{checklist}` placeholder.
    pub heartbeat_checklist: String,
    /// Welcome messages keyed by language name.
    pub welcome: HashMap<String, String>,
}

impl Default for Prompts {
    fn default() -> Self {
        let mut welcome = HashMap::new();
        welcome.insert("English".into(), "I'm *OMEGA Ω*, your personal artificial intelligence agent. I run on your own infrastructure, built in Rust \u{01f4aa}, connected to Telegram, WhatsApp and with Claude as my \u{01f9e0}.\n\nIt's an honor to be at your service. What do you need from me?".into());
        welcome.insert("Spanish".into(), "Soy *OMEGA Ω*, tu agente personal de inteligencia artificial. Corro sobre tu propia infraestructura, construido en Rust \u{01f4aa}, conectado a Telegram, WhatsApp y con Claude como \u{01f9e0}.\n\nEs un honor estar a tu servicio. \u{00bf}Qu\u{00e9} necesitas de m\u{00ed}?".into());
        welcome.insert("Portuguese".into(), "Sou o *OMEGA Ω*, seu agente pessoal de intelig\u{00ea}ncia artificial. Rodo na sua pr\u{00f3}pria infraestrutura, constru\u{00ed}do em Rust \u{01f4aa}, conectado ao Telegram, WhatsApp e com Claude como \u{01f9e0}.\n\n\u{00c9} uma honra estar ao seu servi\u{00e7}o. Do que voc\u{00ea} precisa?".into());
        welcome.insert("French".into(), "Je suis *OMEGA Ω*, votre agent personnel d'intelligence artificielle. Je tourne sur votre propre infrastructure, construit en Rust \u{01f4aa}, connect\u{00e9} \u{00e0} Telegram, WhatsApp et avec Claude comme \u{01f9e0}.\n\nC'est un honneur d'\u{00ea}tre \u{00e0} votre service. De quoi avez-vous besoin\u{00a0}?".into());
        welcome.insert("German".into(), "Ich bin *OMEGA Ω*, dein pers\u{00f6}nlicher KI-Agent. Ich laufe auf deiner eigenen Infrastruktur, gebaut in Rust \u{01f4aa}, verbunden mit Telegram, WhatsApp und mit Claude als \u{01f9e0}.\n\nEs ist mir eine Ehre, dir zu dienen. Was brauchst du von mir?".into());
        welcome.insert("Italian".into(), "Sono *OMEGA Ω*, il tuo agente personale di intelligenza artificiale. Giro sulla tua infrastruttura, costruito in Rust \u{01f4aa}, connesso a Telegram, WhatsApp e con Claude come \u{01f9e0}.\n\n\u{00c8} un onore essere al tuo servizio. Di cosa hai bisogno?".into());
        welcome.insert("Dutch".into(), "Ik ben *OMEGA Ω*, je persoonlijke AI-agent. Ik draai op je eigen infrastructuur, gebouwd in Rust \u{01f4aa}, verbonden met Telegram, WhatsApp en met Claude als \u{01f9e0}.\n\nHet is een eer om je van dienst te zijn. Wat heb je nodig?".into());
        welcome.insert("Russian".into(), "\u{042f} *OMEGA Ω*, \u{0432}\u{0430}\u{0448} \u{043f}\u{0435}\u{0440}\u{0441}\u{043e}\u{043d}\u{0430}\u{043b}\u{044c}\u{043d}\u{044b}\u{0439} \u{0430}\u{0433}\u{0435}\u{043d}\u{0442} \u{0438}\u{0441}\u{043a}\u{0443}\u{0441}\u{0441}\u{0442}\u{0432}\u{0435}\u{043d}\u{043d}\u{043e}\u{0433}\u{043e} \u{0438}\u{043d}\u{0442}\u{0435}\u{043b}\u{043b}\u{0435}\u{043a}\u{0442}\u{0430}. \u{042f} \u{0440}\u{0430}\u{0431}\u{043e}\u{0442}\u{0430}\u{044e} \u{043d}\u{0430} \u{0432}\u{0430}\u{0448}\u{0435}\u{0439} \u{0441}\u{043e}\u{0431}\u{0441}\u{0442}\u{0432}\u{0435}\u{043d}\u{043d}\u{043e}\u{0439} \u{0438}\u{043d}\u{0444}\u{0440}\u{0430}\u{0441}\u{0442}\u{0440}\u{0443}\u{043a}\u{0442}\u{0443}\u{0440}\u{0435}, \u{043d}\u{0430}\u{043f}\u{0438}\u{0441}\u{0430}\u{043d} \u{043d}\u{0430} Rust \u{01f4aa}, \u{043f}\u{043e}\u{0434}\u{043a}\u{043b}\u{044e}\u{0447}\u{0451}\u{043d} \u{043a} Telegram, WhatsApp \u{0438} \u{0438}\u{0441}\u{043f}\u{043e}\u{043b}\u{044c}\u{0437}\u{0443}\u{044e} Claude \u{043a}\u{0430}\u{043a} \u{01f9e0}.\n\n\u{0414}\u{043b}\u{044f} \u{043c}\u{0435}\u{043d}\u{044f} \u{0447}\u{0435}\u{0441}\u{0442}\u{044c} \u{0441}\u{043b}\u{0443}\u{0436}\u{0438}\u{0442}\u{044c} \u{0432}\u{0430}\u{043c}. \u{0427}\u{0442}\u{043e} \u{0432}\u{0430}\u{043c} \u{043d}\u{0443}\u{0436}\u{043d}\u{043e}?".into());

        Self {
            system: "You are OMEGA Ω, a personal AI agent running on the owner's infrastructure.\n\
                     You are NOT a chatbot. You are an agent that DOES things.\n\n\
                     Rules:\n\
                     - When asked to DO something, DO IT. Don't explain how.\n\
                     - Answer concisely. No preamble.\n\
                     - Speak the same language the user uses.\n\
                     - Reference past conversations naturally when relevant.\n\
                     - Never apologize unnecessarily.\n\
                     - NEVER introduce yourself or describe what you can do. The user already received a welcome message. Just answer what they ask.\n\
                     - When the user asks to connect, set up, or configure WhatsApp, respond with exactly WHATSAPP_QR on its own line. Do not explain the process — the system will handle QR generation automatically.".into(),
            summarize: "Summarize this conversation in 1-2 sentences. Be factual and concise. \
                        Do not add commentary.".into(),
            facts: "Extract key facts about the user from this conversation. \
                    Return each fact as 'key: value' on its own line. \
                    Only include concrete, personal facts (name, preferences, location, etc.). \
                    If no facts are apparent, respond with 'none'.".into(),
            heartbeat: "You are OMEGA Ω performing a periodic heartbeat check. \
                        If everything is fine, respond with exactly HEARTBEAT_OK. \
                        Otherwise, respond with a brief alert.".into(),
            heartbeat_checklist: "You are OMEGA Ω performing a periodic heartbeat check.\n\
                                  Review this checklist and report anything that needs attention.\n\
                                  If everything is fine, respond with exactly HEARTBEAT_OK.\n\n\
                                  {checklist}".into(),
            welcome,
        }
    }
}

/// TOML structure for `WELCOME.toml`.
#[derive(Deserialize)]
struct WelcomeFile {
    messages: HashMap<String, String>,
}

/// Bundled system prompt, embedded at compile time.
const BUNDLED_SYSTEM_PROMPT: &str = include_str!("../../../prompts/SYSTEM_PROMPT.md");

/// Bundled welcome messages, embedded at compile time.
const BUNDLED_WELCOME_TOML: &str = include_str!("../../../prompts/WELCOME.toml");

/// Deploy bundled prompt files to `data_dir`, creating the directory if needed.
///
/// Never overwrites existing files so user edits are preserved.
pub fn install_bundled_prompts(data_dir: &str) {
    let expanded = shellexpand(data_dir);
    let dir = Path::new(&expanded);
    if let Err(e) = std::fs::create_dir_all(dir) {
        warn!("prompts: failed to create {}: {e}", dir.display());
        return;
    }

    for (filename, content) in [
        ("SYSTEM_PROMPT.md", BUNDLED_SYSTEM_PROMPT),
        ("WELCOME.toml", BUNDLED_WELCOME_TOML),
    ] {
        let dest = dir.join(filename);
        if !dest.exists() {
            if let Err(e) = std::fs::write(&dest, content) {
                warn!("prompts: failed to write {}: {e}", dest.display());
            } else {
                info!("prompts: deployed bundled {filename}");
            }
        }
    }
}

impl Prompts {
    /// Load prompts from `SYSTEM_PROMPT.md` and `WELCOME.toml` in `data_dir`.
    ///
    /// Missing files or sections fall back to defaults.
    pub fn load(data_dir: &str) -> Self {
        let mut prompts = Self::default();
        let dir = shellexpand(data_dir);

        // Load SYSTEM_PROMPT.md
        let prompt_path = format!("{dir}/SYSTEM_PROMPT.md");
        if let Ok(content) = std::fs::read_to_string(&prompt_path) {
            let sections = parse_markdown_sections(&content);
            if let Some(v) = sections.get("System") {
                prompts.system = v.clone();
            }
            if let Some(v) = sections.get("Summarize") {
                prompts.summarize = v.clone();
            }
            if let Some(v) = sections.get("Facts") {
                prompts.facts = v.clone();
            }
            if let Some(v) = sections.get("Heartbeat") {
                prompts.heartbeat = v.clone();
            }
            if let Some(v) = sections.get("Heartbeat Checklist") {
                prompts.heartbeat_checklist = v.clone();
            }
            tracing::info!("loaded prompts from {prompt_path}");
        }

        // Load WELCOME.toml
        let welcome_path = format!("{dir}/WELCOME.toml");
        if let Ok(content) = std::fs::read_to_string(&welcome_path) {
            match toml::from_str::<WelcomeFile>(&content) {
                Ok(w) => {
                    prompts.welcome = w.messages;
                    tracing::info!("loaded welcome messages from {welcome_path}");
                }
                Err(e) => {
                    tracing::warn!("failed to parse {welcome_path}: {e}");
                }
            }
        }

        prompts
    }
}

/// Parse a markdown file with `## Section` headers into a map of section name → body.
fn parse_markdown_sections(content: &str) -> HashMap<String, String> {
    let mut sections = HashMap::new();
    let mut current_key: Option<String> = None;
    let mut current_body = String::new();

    for line in content.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            // Save previous section.
            if let Some(key) = current_key.take() {
                let trimmed = current_body.trim().to_string();
                if !trimmed.is_empty() {
                    sections.insert(key, trimmed);
                }
            }
            current_key = Some(header.trim().to_string());
            current_body.clear();
        } else if current_key.is_some() {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    // Save last section.
    if let Some(key) = current_key {
        let trimmed = current_body.trim().to_string();
        if !trimmed.is_empty() {
            sections.insert(key, trimmed);
        }
    }

    sections
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
            heartbeat: HeartbeatConfig::default(),
            scheduler: SchedulerConfig::default(),
        });
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| OmegaError::Config(format!("failed to read {}: {}", path.display(), e)))?;

    let config: Config = toml::from_str(&content)
        .map_err(|e| OmegaError::Config(format!("failed to parse config: {}", e)))?;

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_config_default() {
        let cc = ClaudeCodeConfig::default();
        assert_eq!(cc.timeout_secs, 600);
    }

    #[test]
    fn test_timeout_config_from_toml() {
        let toml_str = r#"
            enabled = true
            max_turns = 10
            timeout_secs = 300
        "#;
        let cc: ClaudeCodeConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cc.timeout_secs, 300);
    }

    #[test]
    fn test_timeout_config_default_when_missing() {
        let toml_str = r#"
            enabled = true
            max_turns = 10
        "#;
        let cc: ClaudeCodeConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cc.timeout_secs, 600);
    }

    #[test]
    fn test_sandbox_mode_default_is_sandbox() {
        let mode = SandboxMode::default();
        assert_eq!(mode, SandboxMode::Sandbox);
        assert_eq!(mode.display_name(), "sandbox");
    }

    #[test]
    fn test_sandbox_mode_from_toml() {
        let toml_str = r#"mode = "sandbox""#;
        let cfg: SandboxConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.mode, SandboxMode::Sandbox);

        let toml_str = r#"mode = "rx""#;
        let cfg: SandboxConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.mode, SandboxMode::Rx);

        let toml_str = r#"mode = "rwx""#;
        let cfg: SandboxConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.mode, SandboxMode::Rwx);
    }

    #[test]
    fn test_sandbox_mode_default_when_missing() {
        let toml_str = "";
        let cfg: SandboxConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.mode, SandboxMode::Sandbox);
    }

    #[test]
    fn test_sandbox_mode_prompt_constraint() {
        let ws = "/home/user/.omega/workspace";

        let constraint = SandboxMode::Sandbox.prompt_constraint(ws);
        assert!(constraint.is_some());
        assert!(constraint.as_ref().unwrap().contains("SANDBOX mode"));
        assert!(constraint.as_ref().unwrap().contains(ws));

        let constraint = SandboxMode::Rx.prompt_constraint(ws);
        assert!(constraint.is_some());
        assert!(constraint.as_ref().unwrap().contains("READ-ONLY mode"));
        assert!(constraint.as_ref().unwrap().contains(ws));

        let constraint = SandboxMode::Rwx.prompt_constraint(ws);
        assert!(constraint.is_none());
    }

    #[test]
    fn test_sandbox_mode_display_names() {
        assert_eq!(SandboxMode::Sandbox.display_name(), "sandbox");
        assert_eq!(SandboxMode::Rx.display_name(), "rx");
        assert_eq!(SandboxMode::Rwx.display_name(), "rwx");
    }

    #[test]
    fn test_install_bundled_prompts_creates_files() {
        let tmp = std::env::temp_dir().join("__omega_test_bundled_prompts__");
        let _ = std::fs::remove_dir_all(&tmp);

        install_bundled_prompts(tmp.to_str().unwrap());

        let prompt_path = tmp.join("SYSTEM_PROMPT.md");
        let welcome_path = tmp.join("WELCOME.toml");
        assert!(prompt_path.exists(), "SYSTEM_PROMPT.md should be deployed");
        assert!(welcome_path.exists(), "WELCOME.toml should be deployed");

        let prompt_content = std::fs::read_to_string(&prompt_path).unwrap();
        assert!(
            prompt_content.contains("## System"),
            "should contain System section"
        );
        assert!(
            prompt_content.contains("## Summarize"),
            "should contain Summarize section"
        );

        let welcome_content = std::fs::read_to_string(&welcome_path).unwrap();
        assert!(
            welcome_content.contains("[messages]"),
            "should contain messages table"
        );
        assert!(
            welcome_content.contains("English"),
            "should contain English key"
        );

        // Run again with custom content — should not overwrite.
        std::fs::write(&prompt_path, "custom prompt").unwrap();
        std::fs::write(&welcome_path, "custom welcome").unwrap();
        install_bundled_prompts(tmp.to_str().unwrap());
        assert_eq!(
            std::fs::read_to_string(&prompt_path).unwrap(),
            "custom prompt",
            "should not overwrite user edits to SYSTEM_PROMPT.md"
        );
        assert_eq!(
            std::fs::read_to_string(&welcome_path).unwrap(),
            "custom welcome",
            "should not overwrite user edits to WELCOME.toml"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
