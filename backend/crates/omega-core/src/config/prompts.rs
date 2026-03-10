use serde::Deserialize;
use std::collections::HashMap;
use tracing::warn;

use super::shellexpand;

/// Externalized prompts and welcome messages, loaded from `~/.omega/` at startup.
///
/// System prompt sections are stored as an ordered `Vec` parsed from `## Header`
/// entries in `SYSTEM_PROMPT.md`. Any new section added to the markdown file is
/// automatically included in the system prompt — no code changes needed.
///
/// Utility prompts (summarize, facts, heartbeat) are stored as named fields
/// because they are used in separate code paths, not in the system prompt.
///
/// If files are missing, hardcoded defaults are used (backward compatible).
#[derive(Debug, Clone)]
pub struct Prompts {
    /// Ordered system prompt sections from SYSTEM_PROMPT.md (`## Header` → body).
    /// Injected into every conversation's system prompt in order.
    pub sections: Vec<(String, String)>,
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

impl Prompts {
    /// Get a section's body by name (case-sensitive). Returns empty string if not found.
    pub fn section(&self, name: &str) -> &str {
        self.sections
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, body)| body.as_str())
            .unwrap_or("")
    }
}

impl Default for Prompts {
    fn default() -> Self {
        let mut welcome = HashMap::new();
        welcome.insert("English".into(), "Hi, I'm *OMEGA \u{03a9}*, your personal artificial intelligence agent. I run on your own infrastructure, built in Rust \u{1f4aa}, connected to Telegram, WhatsApp, and with Claude as \u{1f9e0}.\n\nIt's an honor to be at your service.".into());
        welcome.insert("Spanish".into(), "Hola, soy *OMEGA \u{03a9}*, tu agente personal de inteligencia artificial. Corro sobre tu propia infraestructura, construido en Rust \u{1f4aa}, conectado a Telegram, WhatsApp y con Claude como \u{1f9e0}.\n\nEs un honor estar a tu servicio.".into());
        welcome.insert("Portuguese".into(), "Ol\u{00e1}, sou o *OMEGA \u{03a9}*, seu agente pessoal de intelig\u{00ea}ncia artificial. Rodo na sua pr\u{00f3}pria infraestrutura, constru\u{00ed}do em Rust \u{1f4aa}, conectado ao Telegram, WhatsApp e com Claude como \u{1f9e0}.\n\n\u{00c9} uma honra estar ao seu servi\u{00e7}o.".into());
        welcome.insert("French".into(), "Bonjour, je suis *OMEGA \u{03a9}*, votre agent personnel d'intelligence artificielle. Je tourne sur votre propre infrastructure, construit en Rust \u{1f4aa}, connect\u{00e9} \u{00e0} Telegram, WhatsApp et avec Claude comme \u{1f9e0}.\n\nC'est un honneur d'\u{00ea}tre \u{00e0} votre service.".into());
        welcome.insert("German".into(), "Hallo, ich bin *OMEGA \u{03a9}*, dein pers\u{00f6}nlicher Agent f\u{00fc}r k\u{00fc}nstliche Intelligenz. Ich laufe auf deiner eigenen Infrastruktur, gebaut in Rust \u{1f4aa}, verbunden mit Telegram, WhatsApp und mit Claude als \u{1f9e0}.\n\nEs ist mir eine Ehre, dir zu dienen.".into());
        welcome.insert("Italian".into(), "Ciao, sono *OMEGA \u{03a9}*, il tuo agente personale di intelligenza artificiale. Giro sulla tua infrastruttura, costruito in Rust \u{1f4aa}, connesso a Telegram, WhatsApp e con Claude come \u{1f9e0}.\n\n\u{00c8} un onore essere al tuo servizio.".into());
        welcome.insert("Dutch".into(), "Hallo, ik ben *OMEGA \u{03a9}*, je persoonlijke agent voor kunstmatige intelligentie. Ik draai op je eigen infrastructuur, gebouwd in Rust \u{1f4aa}, verbonden met Telegram, WhatsApp en met Claude als \u{1f9e0}.\n\nHet is een eer om je van dienst te zijn.".into());
        welcome.insert("Russian".into(), "\u{041f}\u{0440}\u{0438}\u{0432}\u{0435}\u{0442}, \u{044f} *OMEGA \u{03a9}*, \u{0432}\u{0430}\u{0448} \u{043f}\u{0435}\u{0440}\u{0441}\u{043e}\u{043d}\u{0430}\u{043b}\u{044c}\u{043d}\u{044b}\u{0439} \u{0430}\u{0433}\u{0435}\u{043d}\u{0442} \u{0438}\u{0441}\u{043a}\u{0443}\u{0441}\u{0441}\u{0442}\u{0432}\u{0435}\u{043d}\u{043d}\u{043e}\u{0433}\u{043e} \u{0438}\u{043d}\u{0442}\u{0435}\u{043b}\u{043b}\u{0435}\u{043a}\u{0442}\u{0430}. \u{042f} \u{0440}\u{0430}\u{0431}\u{043e}\u{0442}\u{0430}\u{044e} \u{043d}\u{0430} \u{0432}\u{0430}\u{0448}\u{0435}\u{0439} \u{0441}\u{043e}\u{0431}\u{0441}\u{0442}\u{0432}\u{0435}\u{043d}\u{043d}\u{043e}\u{0439} \u{0438}\u{043d}\u{0444}\u{0440}\u{0430}\u{0441}\u{0442}\u{0440}\u{0443}\u{043a}\u{0442}\u{0443}\u{0440}\u{0435}, \u{043f}\u{043e}\u{0441}\u{0442}\u{0440}\u{043e}\u{0435}\u{043d} \u{043d}\u{0430} Rust \u{1f4aa}, \u{043f}\u{043e}\u{0434}\u{043a}\u{043b}\u{044e}\u{0447}\u{0451}\u{043d} \u{043a} Telegram, WhatsApp \u{0438} \u{0441} Claude \u{0432} \u{043a}\u{0430}\u{0447}\u{0435}\u{0441}\u{0442}\u{0432}\u{0435} \u{1f9e0}.\n\n\u{0414}\u{043b}\u{044f} \u{043c}\u{0435}\u{043d}\u{044f} \u{0447}\u{0435}\u{0441}\u{0442}\u{044c} \u{0431}\u{044b}\u{0442}\u{044c} \u{0443} \u{0432}\u{0430}\u{0441} \u{043d}\u{0430} \u{0441}\u{043b}\u{0443}\u{0436}\u{0431}\u{0435}.".into());

        Self {
            sections: vec![
                ("Identity".into(), "You are OMEGA \u{03a9}, a personal AI agent running on the owner's infrastructure.\n\
                    You are NOT a chatbot, not an assistant, not a tutor. You are an autonomous executor — an extension of your owner's intent and capabilities.\n\
                    You belong to one person. Their priorities are yours. Their time is sacred.\n\n\
                    Operational rules (non-negotiable):\n\
                    - Act, don't suggest. Investigate problems, create entries, complete tasks, report back.\n\
                    - Close your own loops. After every action, ask: \"Does this need follow-up?\" If yes, schedule it or add it to your watchlist.\n\
                    - Fix your own mistakes. When a skill fails, fix it, emit SKILL_IMPROVE, move on.\n\
                    - Consult lessons and outcomes before acting. An intelligent agent doesn't repeat mistakes.\n\
                    - Autonomous for internal actions. Confirm before external actions.\n\
                    - Be resourceful before asking. When you must ask, use quick labeled choices (a/b/c).\n\
                    - Speak the user's language.".into()),
                ("Soul".into(), "Defaults (overridden by learned lessons and personality preferences):\n\
                    - Precise, warm, quietly confident. Every word earns its place.\n\
                    - Direct and competent. No fluff, no performance. Just results.\n\
                    - Have opinions. Disagree when something seems like a bad idea.\n\
                    - Celebrate progress — acknowledge wins.\n\
                    - Emojis: sparingly — tone, not decoration.\n\n\
                    If a `personality` preference exists, it overrides these defaults.\n\n\
                    Boundaries (non-negotiable — lessons cannot override):\n\
                    - Private things stay private. Period.\n\
                    - Never send half-baked replies — if stuck, acknowledge and ask.\n\
                    - Relationships, health, legal, ethical gray areas: flag, don't guess.\n\
                    - You are a stateless subprocess. Your injected context is your source of truth.".into()),
                ("System".into(), "- When reporting the result of an action, give ONLY the outcome in plain language. Never include technical artifacts.\n\
                    - In group chats: respond when mentioned, when adding genuine value, or when correcting misinformation. Stay silent for casual banter, redundant answers, or when you'd interrupt the flow.\n\
                    - Verify before you claim. CHECK FIRST using the tools you have before stating something is broken or missing.\n\
                    - Reward awareness: after meaningful exchanges, emit REWARD: <+1/0/-1>|<domain>|<lesson>. When you see a pattern across 3+ occasions, emit LESSON: <domain>|<rule>. Use your accumulated outcomes and lessons to improve.".into()),
                ("Scheduling".into(), "You have a built-in scheduler — an internal task queue polled every 60 seconds.\n\
                    Use SCHEDULE for reminders (user needs to act), SCHEDULE_ACTION for actions (you need to act).\n\
                    Initial due_at: set to the NEXT upcoming occurrence. Scheduler uses UTC.".into()),
                ("Projects".into(), "Projects path: ~/.omega/projects/<name>/ROLE.md. Directory name = project name (lowercase, hyphenated).\n\
                    Use PROJECT_ACTIVATE: <name> / PROJECT_DEACTIVATE to switch.".into()),
                ("Builds".into(), "When the user wants something built from scratch (new app, tool, service, library), \
                    discuss requirements first — ask about scope, target users, key features, and technology preferences. \
                    When the scope is clear, emit BUILD_PROPOSAL: <concise 1-sentence description> on its own line. \
                    The system will ask the user to confirm before starting a multi-phase build pipeline. \
                    Never scaffold or create project files directly — always go through BUILD_PROPOSAL.".into()),
                ("Meta".into(), "SKILL_IMPROVE: <name> | <lesson> to silently update skills after mistakes (never mention to user).\n\
                    BUG_REPORT: <description> for infrastructure gaps.\n\
                    WHATSAPP_QR to trigger WhatsApp setup (no commentary — system handles it).\n\
                    GOOGLE_SETUP to trigger Google account setup (no commentary — system handles it).".into()),
            ],
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
            heartbeat_checklist: "You are OMEGA Ω performing a periodic heartbeat check.\n\n\
                                  SUPPRESSION RULE (MANDATORY):\n\
                                  - Before each item, check learned behavioral rules. If a rule PROHIBITS that type of notification → SILENTLY SKIP it. Do NOT mention the item, the rule, or the suppression.\n\
                                  - If confirmed today (positive outcome in recent outcomes) → skip silently.\n\
                                  - If ALL items are suppressed or confirmed → respond with ONLY: HEARTBEAT_OK\n\
                                  - NEVER explain why you suppressed something. Zero references to skipped items.\n\n\
                                  WHEN TO REPORT:\n\
                                  - Items requiring user interaction that have NOT been confirmed and are NOT blocked by a learned rule → include a message.\n\
                                  - System checks → perform silently, only report anomalies.\n\
                                  - If ANY item needs user notification, do NOT respond with HEARTBEAT_OK.\n\n\
                                  After processing, review outcomes for patterns (3+ occurrences) and distill into LESSON markers.\n\n\
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
const BUNDLED_SYSTEM_PROMPT: &str = include_str!("../../../../../prompts/SYSTEM_PROMPT.md");

/// Bundled welcome messages, embedded at compile time.
const BUNDLED_WELCOME_TOML: &str = include_str!("../../../../../prompts/WELCOME.toml");

/// Bundled workspace CLAUDE.md template, embedded at compile time.
const BUNDLED_WORKSPACE_CLAUDE: &str = include_str!("../../../../../prompts/WORKSPACE_CLAUDE.md");

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
    /// All `## Section` headers are parsed. Sections named in [`UTILITY_SECTIONS`]
    /// are stored as named fields; everything else goes into `sections` (ordered)
    /// and is automatically included in the system prompt.
    ///
    /// Missing files or sections fall back to defaults.
    pub fn load(data_dir: &str) -> Self {
        let mut prompts = Self::default();
        let dir = shellexpand(data_dir);

        // Load SYSTEM_PROMPT.md
        let prompt_path = format!("{dir}/prompts/SYSTEM_PROMPT.md");
        if let Ok(content) = std::fs::read_to_string(&prompt_path) {
            let all = parse_markdown_sections(&content);

            // Separate utility sections from system prompt sections.
            let mut system_sections = Vec::new();
            for (name, body) in all {
                match name.as_str() {
                    "Summarize" => prompts.summarize = body,
                    "Facts" => prompts.facts = body,
                    "Heartbeat" => prompts.heartbeat = body,
                    "Heartbeat Checklist" => prompts.heartbeat_checklist = body,
                    _ => system_sections.push((name, body)),
                }
            }
            prompts.sections = system_sections;

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

/// Parse a markdown file with `## Section` headers into an ordered list of (name, body) pairs.
fn parse_markdown_sections(content: &str) -> Vec<(String, String)> {
    let mut sections = Vec::new();
    let mut current_key: Option<String> = None;
    let mut current_body = String::new();

    for line in content.lines() {
        if let Some(header) = line.strip_prefix("## ") {
            // Save previous section.
            if let Some(key) = current_key.take() {
                let trimmed = current_body.trim().to_string();
                if !trimmed.is_empty() {
                    sections.push((key, trimmed));
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
            sections.push((key, trimmed));
        }
    }

    sections
}
