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
    #[serde(default)]
    pub quant: Option<QuantConfig>,
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
    pub gemini: Option<GeminiConfig>,
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
    /// Subprocess timeout in seconds (default: 3600 = 60 minutes).
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    /// Max auto-resume attempts when Claude hits max_turns (default: 5).
    #[serde(default = "default_max_resume_attempts")]
    pub max_resume_attempts: u32,
    /// Fast model for classification and direct responses.
    #[serde(default = "default_model")]
    pub model: String,
    /// Complex model for multi-step autonomous execution.
    #[serde(default = "default_model_complex")]
    pub model_complex: String,
}

impl Default for ClaudeCodeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_turns: 100,
            allowed_tools: default_allowed_tools(),
            timeout_secs: default_timeout_secs(),
            max_resume_attempts: default_max_resume_attempts(),
            model: default_model(),
            model_complex: default_model_complex(),
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

/// Google Gemini API provider config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_gemini_model")]
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
    /// OpenAI API key for Whisper voice transcription. Presence = voice enabled.
    #[serde(default)]
    pub whisper_api_key: Option<String>,
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

/// Quantitative trading engine configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_quant_symbol")]
    pub default_symbol: String,
    #[serde(default = "default_quant_network")]
    pub network: String,
    #[serde(default = "default_quant_interval")]
    pub kline_interval: String,
    #[serde(default = "default_quant_risk_aversion")]
    pub risk_aversion: f64,
    #[serde(default = "default_quant_kelly_fraction")]
    pub kelly_fraction: f64,
    #[serde(default = "default_quant_max_position_pct")]
    pub max_position_pct: f64,
    #[serde(default = "default_quant_portfolio_value")]
    pub portfolio_value: f64,
    #[serde(default)]
    pub safety: QuantSafetyConfig,
}

impl Default for QuantConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            default_symbol: default_quant_symbol(),
            network: default_quant_network(),
            kline_interval: default_quant_interval(),
            risk_aversion: default_quant_risk_aversion(),
            kelly_fraction: default_quant_kelly_fraction(),
            max_position_pct: default_quant_max_position_pct(),
            portfolio_value: default_quant_portfolio_value(),
            safety: QuantSafetyConfig::default(),
        }
    }
}

/// Safety guardrails for the quantitative trading engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantSafetyConfig {
    #[serde(default = "default_quant_max_daily_trades")]
    pub max_daily_trades: u32,
    #[serde(default = "default_quant_max_daily_usd")]
    pub max_daily_usd: f64,
    /// Always true — human must confirm every trade.
    #[serde(default = "default_true")]
    pub require_confirmation: bool,
    #[serde(default = "default_quant_cooldown_minutes")]
    pub cooldown_minutes: u32,
}

impl Default for QuantSafetyConfig {
    fn default() -> Self {
        Self {
            max_daily_trades: default_quant_max_daily_trades(),
            max_daily_usd: default_quant_max_daily_usd(),
            require_confirmation: true,
            cooldown_minutes: default_quant_cooldown_minutes(),
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
    100
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
fn default_gemini_model() -> String {
    "gemini-2.0-flash".to_string()
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
    3600
}
fn default_max_resume_attempts() -> u32 {
    5
}
fn default_quant_symbol() -> String {
    "BTCUSDT".to_string()
}
fn default_quant_network() -> String {
    "testnet".to_string()
}
fn default_quant_interval() -> String {
    "1m".to_string()
}
fn default_quant_risk_aversion() -> f64 {
    2.0
}
fn default_quant_kelly_fraction() -> f64 {
    0.25
}
fn default_quant_max_position_pct() -> f64 {
    0.10
}
fn default_quant_portfolio_value() -> f64 {
    10_000.0
}
fn default_quant_max_daily_trades() -> u32 {
    10
}
fn default_quant_max_daily_usd() -> f64 {
    5_000.0
}
fn default_quant_cooldown_minutes() -> u32 {
    5
}
fn default_model() -> String {
    "claude-sonnet-4-6".to_string()
}
fn default_model_complex() -> String {
    "claude-opus-4-6".to_string()
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
    /// Identity prompt — who Omega is.
    pub identity: String,
    /// Soul prompt — values and personality.
    pub soul: String,
    /// System prompt — behavioral rules.
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
        welcome.insert("English".into(), "Hi, I'm *OMEGA* \u{03a9}, your personal artificial intelligence agent. I run on your own infrastructure, built in Rust \u{1f4aa}, connected to Telegram, WhatsApp, and with Claude as \u{1f9e0}.\n\nIt's an honor to be at your service.".into());
        welcome.insert("Spanish".into(), "Hola, soy *OMEGA* \u{03a9}, tu agente personal de inteligencia artificial. Corro sobre tu propia infraestructura, construido en Rust \u{1f4aa}, conectado a Telegram, WhatsApp y con Claude como \u{1f9e0}.\n\nEs un honor estar a tu servicio.".into());
        welcome.insert("Portuguese".into(), "Ol\u{00e1}, sou o *OMEGA* \u{03a9}, seu agente pessoal de intelig\u{00ea}ncia artificial. Rodo na sua pr\u{00f3}pria infraestrutura, constru\u{00ed}do em Rust \u{1f4aa}, conectado ao Telegram, WhatsApp e com Claude como \u{1f9e0}.\n\n\u{00c9} uma honra estar ao seu servi\u{00e7}o.".into());
        welcome.insert("French".into(), "Bonjour, je suis *OMEGA* \u{03a9}, votre agent personnel d'intelligence artificielle. Je tourne sur votre propre infrastructure, construit en Rust \u{1f4aa}, connect\u{00e9} \u{00e0} Telegram, WhatsApp et avec Claude comme \u{1f9e0}.\n\nC'est un honneur d'\u{00ea}tre \u{00e0} votre service.".into());
        welcome.insert("German".into(), "Hallo, ich bin *OMEGA* \u{03a9}, dein pers\u{00f6}nlicher Agent f\u{00fc}r k\u{00fc}nstliche Intelligenz. Ich laufe auf deiner eigenen Infrastruktur, gebaut in Rust \u{1f4aa}, verbunden mit Telegram, WhatsApp und mit Claude als \u{1f9e0}.\n\nEs ist mir eine Ehre, dir zu dienen.".into());
        welcome.insert("Italian".into(), "Ciao, sono *OMEGA* \u{03a9}, il tuo agente personale di intelligenza artificiale. Giro sulla tua infrastruttura, costruito in Rust \u{1f4aa}, connesso a Telegram, WhatsApp e con Claude come \u{1f9e0}.\n\n\u{00c8} un onore essere al tuo servizio.".into());
        welcome.insert("Dutch".into(), "Hallo, ik ben *OMEGA* \u{03a9}, je persoonlijke agent voor kunstmatige intelligentie. Ik draai op je eigen infrastructuur, gebouwd in Rust \u{1f4aa}, verbonden met Telegram, WhatsApp en met Claude als \u{1f9e0}.\n\nHet is een eer om je van dienst te zijn.".into());
        welcome.insert("Russian".into(), "\u{041f}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}, \u{044f} *OMEGA* \u{03a9}, \u{0432}\u{0430}\u{0448} \u{043f}\u{0435}\u{0440}\u{0441}\u{043e}\u{043d}\u{0430}\u{043b}\u{044c}\u{043d}\u{044b}\u{0439} \u{0430}\u{0433}\u{0435}\u{043d}\u{0442} \u{0438}\u{0441}\u{043a}\u{0443}\u{0441}\u{0441}\u{0442}\u{0432}\u{0435}\u{043d}\u{043d}\u{043e}\u{0433}\u{043e} \u{0438}\u{043d}\u{0442}\u{0435}\u{043b}\u{043b}\u{0435}\u{043a}\u{0442}\u{0430}. \u{042f} \u{0440}\u{0430}\u{0431}\u{043e}\u{0442}\u{0430}\u{044e} \u{043d}\u{0430} \u{0432}\u{0430}\u{0448}\u{0435}\u{0439} \u{0441}\u{043e}\u{0431}\u{0441}\u{0442}\u{0432}\u{0435}\u{043d}\u{043d}\u{043e}\u{0439} \u{0438}\u{043d}\u{0444}\u{0440}\u{0430}\u{0441}\u{0442}\u{0440}\u{0443}\u{043a}\u{0442}\u{0443}\u{0440}\u{0435}, \u{043f}\u{043e}\u{0441}\u{0442}\u{0440}\u{043e}\u{0435}\u{043d} \u{043d}\u{0430} Rust \u{1f4aa}, \u{043f}\u{043e}\u{0434}\u{043a}\u{043b}\u{044e}\u{0447}\u{0451}\u{043d} \u{043a} Telegram, WhatsApp \u{0438} \u{0441} Claude \u{0432} \u{043a}\u{0430}\u{0447}\u{0435}\u{0441}\u{0442}\u{0432}\u{0435} \u{1f9e0}.\n\n\u{0414}\u{043b}\u{044f} \u{043c}\u{0435}\u{043d}\u{044f} \u{0447}\u{0435}\u{0441}\u{0442}\u{044c} \u{0431}\u{044b}\u{0442}\u{044c} \u{0443} \u{0432}\u{0430}\u{0441} \u{043d}\u{0430} \u{0441}\u{043b}\u{0443}\u{0436}\u{0431}\u{0435}.".into());

        Self {
            identity: "You are OMEGA, a personal AI agent running on the owner's infrastructure.\n\
                       You are NOT a chatbot, not an assistant, not a tutor. You are an autonomous executor — an extension of your owner's intent and capabilities.\n\
                       You belong to one person. Their priorities are yours. Their time is sacred.".into(),
            soul: "- You are precise, warm, and quietly confident. Every word you say should earn its place.\n\
                   - Be the agent you'd actually want in your life — competent, trustworthy, not a corporate drone.\n\
                   - Have opinions. You can disagree, express preferences, or flag when something seems like a bad idea.\n\
                   - Be resourceful before asking. Use context, memory, and available information first. Only ask when truly stuck.\n\
                   - Act autonomously for internal actions (reading, thinking, organizing, scheduling). Confirm before external actions (sending messages to others, public posts, outward-facing changes).\n\
                   - Celebrate progress — acknowledge wins, no matter how small.\n\
                   - Speak the same language the user uses. Reference past conversations naturally when relevant.\n\
                   - Never apologize unnecessarily.\n\
                   - Don't introduce yourself on every message. Only on the very first interaction — after that, just answer what they ask.\n\
                   - If the user profile includes a `personality` preference, honor it — it overrides your default tone.\n\
                   - You have access to someone's personal life. That's trust. Private things stay private. Period.".into(),
            system: "- When reporting the result of an action, give ONLY the outcome in plain language. Never include technical artifacts.\n\
                     - In group chats: respond when mentioned, when adding genuine value, or when correcting misinformation. Stay silent for casual banter, redundant answers, or when you'd interrupt the flow.\n\
                     - When the user asks to connect, set up, or configure WhatsApp, respond with exactly WHATSAPP_QR on its own line. Do not explain the process — the system will handle QR generation automatically.".into(),
            summarize: "Summarize this conversation in 1-2 sentences. Be factual and concise. \
                        Do not add commentary.".into(),
            facts: "Extract ONLY personal facts about the user — things that describe WHO they are, not what was discussed.\n\
                    Allowed keys: name, preferred_name, pronouns, timezone, location, occupation, interests, personality, communication_style, technical_level, autonomy_preference.\n\
                    Rules: A fact must be about the PERSON, not about a topic, market, project, algorithm, or conversation. \
                    Do NOT extract trading data, prices, market analysis, technical instructions, code snippets, recommendations, numbered steps, timestamps, or anything the AI said. \
                    Do NOT extract facts that only make sense in the context of a single conversation.\n\
                    IMPORTANT: Always use English keys regardless of conversation language. Values may be in any language.\n\
                    Format: one 'key: value' per line. Keys: 1-3 words, lowercase. Values: under 100 chars.\n\
                    If no personal facts are apparent, respond with 'none'.".into(),
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
            if let Some(v) = sections.get("Identity") {
                prompts.identity = v.clone();
            }
            if let Some(v) = sections.get("Soul") {
                prompts.soul = v.clone();
            }
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
            quant: None,
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
        assert_eq!(cc.timeout_secs, 3600);
        assert_eq!(cc.max_resume_attempts, 5);
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
        assert_eq!(cc.timeout_secs, 3600);
        assert_eq!(cc.max_resume_attempts, 5);
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

    #[test]
    fn test_parse_identity_soul_system_sections() {
        let content = "## Identity\nI am OMEGA.\n\n## Soul\nBe helpful.\n\n## System\nRules here.";
        let sections = parse_markdown_sections(content);
        assert_eq!(sections.get("Identity").unwrap(), "I am OMEGA.");
        assert_eq!(sections.get("Soul").unwrap(), "Be helpful.");
        assert_eq!(sections.get("System").unwrap(), "Rules here.");
    }

    #[test]
    fn test_backward_compat_system_only() {
        // When a user's SYSTEM_PROMPT.md only has ## System, identity and soul
        // should keep their compiled defaults.
        let tmp = std::env::temp_dir().join("__omega_test_compat__");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        std::fs::write(
            tmp.join("SYSTEM_PROMPT.md"),
            "## System\nCustom rules only.",
        )
        .unwrap();

        let prompts = Prompts::load(tmp.to_str().unwrap());
        assert_eq!(prompts.system, "Custom rules only.");
        // Identity and soul should be defaults (not empty).
        assert!(
            prompts.identity.contains("OMEGA"),
            "identity should keep default"
        );
        assert!(
            prompts.soul.contains("quietly confident"),
            "soul should keep default"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_prompts_default_has_identity_soul() {
        let prompts = Prompts::default();
        assert!(
            prompts.identity.contains("OMEGA"),
            "default identity should mention OMEGA"
        );
        assert!(
            prompts.soul.contains("quietly confident"),
            "default soul should contain personality"
        );
        assert!(
            prompts.system.contains("outcome in plain language"),
            "default system should contain rules"
        );
    }

    #[test]
    fn test_telegram_config_with_whisper() {
        let toml_str = r#"
            enabled = true
            bot_token = "tok:EN"
            allowed_users = [42]
            whisper_api_key = "sk-test123"
        "#;
        let cfg: TelegramConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.whisper_api_key.as_deref(), Some("sk-test123"));
    }

    #[test]
    fn test_telegram_config_without_whisper() {
        let toml_str = r#"
            enabled = true
            bot_token = "tok:EN"
            allowed_users = [42]
        "#;
        let cfg: TelegramConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.whisper_api_key.is_none());
    }

    #[test]
    fn test_quant_config_defaults() {
        let cfg = QuantConfig::default();
        assert!(!cfg.enabled);
        assert_eq!(cfg.default_symbol, "BTCUSDT");
        assert_eq!(cfg.network, "testnet");
        assert_eq!(cfg.kline_interval, "1m");
        assert!((cfg.risk_aversion - 2.0).abs() < f64::EPSILON);
        assert!((cfg.kelly_fraction - 0.25).abs() < f64::EPSILON);
        assert!((cfg.max_position_pct - 0.10).abs() < f64::EPSILON);
    }

    #[test]
    fn test_quant_safety_defaults() {
        let safety = QuantSafetyConfig::default();
        assert_eq!(safety.max_daily_trades, 10);
        assert!((safety.max_daily_usd - 5000.0).abs() < f64::EPSILON);
        assert!(safety.require_confirmation);
        assert_eq!(safety.cooldown_minutes, 5);
    }

    #[test]
    fn test_quant_config_from_toml() {
        let toml_str = r#"
            enabled = true
            default_symbol = "ETHUSDT"
            network = "mainnet"
            portfolio_value = 50000.0
        "#;
        let cfg: QuantConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.default_symbol, "ETHUSDT");
        assert_eq!(cfg.network, "mainnet");
        assert!((cfg.portfolio_value - 50000.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_quant_testnet_default() {
        // Safety invariant #1: testnet by default
        assert_eq!(default_quant_network(), "testnet");
    }

    #[test]
    fn test_gemini_config_defaults() {
        let toml_str = r#"
            enabled = true
            api_key = "AIza-test"
        "#;
        let cfg: GeminiConfig = toml::from_str(toml_str).unwrap();
        assert!(cfg.enabled);
        assert_eq!(cfg.api_key, "AIza-test");
        assert_eq!(cfg.model, "gemini-2.0-flash");
    }
}
