use serde::Deserialize;
use std::collections::HashMap;
use tracing::warn;

use super::shellexpand;

/// Externalized prompts and welcome messages, loaded from `~/.omega/` at startup.
///
/// If files are missing, hardcoded defaults are used (backward compatible).
#[derive(Debug, Clone)]
pub struct Prompts {
    /// Identity prompt — who Omega is.
    pub identity: String,
    /// Soul prompt — values and personality.
    pub soul: String,
    /// System prompt — core behavioral rules (always injected).
    pub system: String,
    /// Scheduling rules — conditionally injected when message mentions scheduling.
    pub scheduling: String,
    /// Project management rules — conditionally injected when projects are relevant.
    pub projects_rules: String,
    /// Meta rules (skill improvement, bug reporting, WhatsApp, heartbeat) — conditionally injected.
    pub meta: String,
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
                     - Verify before you claim. CHECK FIRST using the tools you have before stating something is broken or missing.".into(),
            scheduling: "You have a built-in scheduler — an internal task queue polled every 60 seconds.\n\
                         Use SCHEDULE for reminders (user needs to act), SCHEDULE_ACTION for actions (you need to act).\n\
                         Initial due_at: set to the NEXT upcoming occurrence. Scheduler uses UTC.".into(),
            projects_rules: "Projects path: ~/.omega/projects/<name>/ROLE.md. Directory name = project name (lowercase, hyphenated).\n\
                             Use PROJECT_ACTIVATE: <name> / PROJECT_DEACTIVATE to switch.".into(),
            meta: "SKILL_IMPROVE: <name> | <lesson> to update skills after mistakes.\n\
                   BUG_REPORT: <description> for infrastructure gaps.\n\
                   WHATSAPP_QR to trigger WhatsApp setup.".into(),
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
                                  Execute each item in this checklist actively:\n\
                                  - Items requiring user interaction (reminders, accountability, motivation) → send the message to the user.\n\
                                  - Items requiring system checks (commands, APIs, monitoring) → perform the check and report results.\n\
                                  - Default: include results in your response. Only omit an item if it explicitly says to stay silent when OK.\n\
                                  - Respond with exactly HEARTBEAT_OK only if ALL items have been checked AND none require user notification.\n\n\
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
const BUNDLED_SYSTEM_PROMPT: &str = include_str!("../../../../prompts/SYSTEM_PROMPT.md");

/// Bundled welcome messages, embedded at compile time.
const BUNDLED_WELCOME_TOML: &str = include_str!("../../../../prompts/WELCOME.toml");

/// Bundled workspace CLAUDE.md template, embedded at compile time.
const BUNDLED_WORKSPACE_CLAUDE: &str = include_str!("../../../../prompts/WORKSPACE_CLAUDE.md");

/// Return the bundled workspace CLAUDE.md template.
///
/// Contains standard operational rules (directory structure, infrastructure,
/// diagnostic protocol, known false diagnoses, key conventions) that survive
/// across deployments and 24h refreshes. Dynamic content (skills/projects
/// tables) is appended below the `<!-- DYNAMIC CONTENT BELOW -->` marker.
pub fn bundled_workspace_claude() -> &'static str {
    BUNDLED_WORKSPACE_CLAUDE
}

/// Deploy bundled prompt files to `{data_dir}/prompts/`, creating the directory if needed.
///
/// Never overwrites existing files so user edits are preserved.
pub fn install_bundled_prompts(data_dir: &str) {
    let expanded = shellexpand(data_dir);
    let dir = std::path::Path::new(&expanded).join("prompts");
    if let Err(e) = std::fs::create_dir_all(&dir) {
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
                tracing::info!("prompts: deployed bundled {filename}");
            }
        }
    }
}

impl Prompts {
    /// Load prompts from `{data_dir}/prompts/SYSTEM_PROMPT.md` and `{data_dir}/prompts/WELCOME.toml`.
    ///
    /// Missing files or sections fall back to defaults.
    pub fn load(data_dir: &str) -> Self {
        let mut prompts = Self::default();
        let dir = shellexpand(data_dir);

        // Load SYSTEM_PROMPT.md
        let prompt_path = format!("{dir}/prompts/SYSTEM_PROMPT.md");
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
            if let Some(v) = sections.get("Scheduling") {
                prompts.scheduling = v.clone();
            }
            if let Some(v) = sections.get("Projects") {
                prompts.projects_rules = v.clone();
            }
            if let Some(v) = sections.get("Meta") {
                prompts.meta = v.clone();
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
        let welcome_path = format!("{dir}/prompts/WELCOME.toml");
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

/// Parse a markdown file with `## Section` headers into a map of section name -> body.
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
