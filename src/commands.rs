//! Built-in bot commands — instant responses, no provider call.

use omega_memory::Store;
use std::time::Instant;

/// Grouped context for command execution.
pub struct CommandContext<'a> {
    pub store: &'a Store,
    pub channel: &'a str,
    pub sender_id: &'a str,
    pub text: &'a str,
    pub uptime: &'a Instant,
    pub provider_name: &'a str,
    pub skills: &'a [omega_skills::Skill],
    pub projects: &'a [omega_skills::Project],
    pub sandbox_mode: &'a str,
}

/// Known bot commands.
pub enum Command {
    Status,
    Memory,
    History,
    Facts,
    Forget,
    Tasks,
    Cancel,
    Language,
    Personality,
    Skills,
    Projects,
    Project,
    WhatsApp,
    Help,
}

impl Command {
    /// Parse a command from message text. Returns `None` for unknown `/` prefixes
    /// (which should pass through to the provider).
    pub fn parse(text: &str) -> Option<Self> {
        let cmd = text.split_whitespace().next()?;
        match cmd {
            "/status" => Some(Self::Status),
            "/memory" => Some(Self::Memory),
            "/history" => Some(Self::History),
            "/facts" => Some(Self::Facts),
            "/forget" => Some(Self::Forget),
            "/tasks" => Some(Self::Tasks),
            "/cancel" => Some(Self::Cancel),
            "/language" | "/lang" => Some(Self::Language),
            "/personality" => Some(Self::Personality),
            "/skills" => Some(Self::Skills),
            "/projects" => Some(Self::Projects),
            "/project" => Some(Self::Project),
            "/whatsapp" => Some(Self::WhatsApp),
            "/help" => Some(Self::Help),
            _ => None,
        }
    }
}

/// Handle a command and return the response text.
pub async fn handle(cmd: Command, ctx: &CommandContext<'_>) -> String {
    match cmd {
        Command::Status => {
            handle_status(ctx.store, ctx.uptime, ctx.provider_name, ctx.sandbox_mode).await
        }
        Command::Memory => handle_memory(ctx.store, ctx.sender_id).await,
        Command::History => handle_history(ctx.store, ctx.channel, ctx.sender_id).await,
        Command::Facts => handle_facts(ctx.store, ctx.sender_id).await,
        Command::Forget => handle_forget(ctx.store, ctx.channel, ctx.sender_id).await,
        Command::Tasks => handle_tasks(ctx.store, ctx.sender_id).await,
        Command::Cancel => handle_cancel(ctx.store, ctx.sender_id, ctx.text).await,
        Command::Language => handle_language(ctx.store, ctx.sender_id, ctx.text).await,
        Command::Personality => handle_personality(ctx.store, ctx.sender_id, ctx.text).await,
        Command::Skills => handle_skills(ctx.skills),
        Command::Projects => handle_projects(ctx.store, ctx.sender_id, ctx.projects).await,
        Command::Project => {
            handle_project(
                ctx.store,
                ctx.channel,
                ctx.sender_id,
                ctx.text,
                ctx.projects,
            )
            .await
        }
        Command::WhatsApp => handle_whatsapp(),
        Command::Help => handle_help(),
    }
}

async fn handle_status(
    store: &Store,
    uptime: &Instant,
    provider_name: &str,
    sandbox_mode: &str,
) -> String {
    let elapsed = uptime.elapsed();
    let hours = elapsed.as_secs() / 3600;
    let minutes = (elapsed.as_secs() % 3600) / 60;
    let secs = elapsed.as_secs() % 60;

    let db_size = store
        .db_size()
        .await
        .map(format_bytes)
        .unwrap_or_else(|_| "unknown".to_string());

    format!(
        "*OMEGA Ω* Status\n\
         Uptime: {hours}h {minutes}m {secs}s\n\
         Provider: {provider_name}\n\
         Sandbox: {sandbox_mode}\n\
         Database: {db_size}"
    )
}

async fn handle_memory(store: &Store, sender_id: &str) -> String {
    match store.get_memory_stats(sender_id).await {
        Ok((convos, msgs, facts)) => {
            format!(
                "Your Memory\n\
                 Conversations: {convos}\n\
                 Messages: {msgs}\n\
                 Facts: {facts}"
            )
        }
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_history(store: &Store, channel: &str, sender_id: &str) -> String {
    match store.get_history(channel, sender_id, 5).await {
        Ok(entries) if entries.is_empty() => "No conversation history yet.".to_string(),
        Ok(entries) => {
            let mut out = String::from("Recent Conversations\n");
            for (summary, timestamp) in &entries {
                out.push_str(&format!("\n[{timestamp}]\n{summary}\n"));
            }
            out
        }
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_facts(store: &Store, sender_id: &str) -> String {
    match store.get_facts(sender_id).await {
        Ok(facts) if facts.is_empty() => "No facts stored yet.".to_string(),
        Ok(facts) => {
            let mut out = String::from("Known Facts\n");
            for (key, value) in &facts {
                out.push_str(&format!("\n- {key}: {value}"));
            }
            out
        }
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_forget(store: &Store, channel: &str, sender_id: &str) -> String {
    match store.close_current_conversation(channel, sender_id).await {
        Ok(true) => "Conversation cleared. Starting fresh.".to_string(),
        Ok(false) => "No active conversation to clear.".to_string(),
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_tasks(store: &Store, sender_id: &str) -> String {
    match store.get_tasks_for_sender(sender_id).await {
        Ok(tasks) if tasks.is_empty() => "No pending tasks.".to_string(),
        Ok(tasks) => {
            let mut out = String::from("Scheduled Tasks\n");
            for (id, description, due_at, repeat) in &tasks {
                let short_id = &id[..8.min(id.len())];
                let repeat_label = repeat.as_deref().unwrap_or("once");
                out.push_str(&format!(
                    "\n[{short_id}] {description}\n  Due: {due_at} ({repeat_label})"
                ));
            }
            out
        }
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_cancel(store: &Store, sender_id: &str, text: &str) -> String {
    let id_prefix = text.split_whitespace().nth(1).unwrap_or("").trim();
    if id_prefix.is_empty() {
        return "Usage: /cancel <task-id>".to_string();
    }
    match store.cancel_task(id_prefix, sender_id).await {
        Ok(true) => "Task cancelled.".to_string(),
        Ok(false) => "No matching task found.".to_string(),
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_language(store: &Store, sender_id: &str, text: &str) -> String {
    let arg = text
        .split_whitespace()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ");
    if arg.is_empty() {
        // Show current preference.
        match store.get_facts(sender_id).await {
            Ok(facts) => {
                let lang = facts
                    .iter()
                    .find(|(k, _)| k == "preferred_language")
                    .map(|(_, v)| v.as_str())
                    .unwrap_or("not set");
                format!("Language: {lang}\nUsage: /language <language>")
            }
            Err(e) => format!("Error: {e}"),
        }
    } else {
        match store
            .store_fact(sender_id, "preferred_language", &arg)
            .await
        {
            Ok(()) => format!("Language set to: {arg}"),
            Err(e) => format!("Error: {e}"),
        }
    }
}

/// Handle /personality — show, set, or reset personality preferences.
async fn handle_personality(store: &Store, sender_id: &str, text: &str) -> String {
    let arg = text
        .split_whitespace()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ");

    if arg.is_empty() {
        // Show current personality preference.
        return match store.get_fact(sender_id, "personality").await {
            Ok(Some(p)) => format!(
                "Your personality preference:\n_{p}_\n\n\
                 Use /personality reset to go back to defaults.\n\
                 Or just tell me how you'd like me to be."
            ),
            Ok(None) => "Using default personality. Just tell me how you'd like me to be — \
                         more formal, more casual, funnier, straight to the point — anything."
                .to_string(),
            Err(e) => format!("Error: {e}"),
        };
    }

    if arg == "reset" {
        return match store.delete_fact(sender_id, "personality").await {
            Ok(true) => "Personality reset to defaults.".to_string(),
            Ok(false) => "Already using default personality.".to_string(),
            Err(e) => format!("Error: {e}"),
        };
    }

    // Store the personality preference as a fact.
    match store.store_fact(sender_id, "personality", &arg).await {
        Ok(()) => format!("Personality updated: _{arg}_"),
        Err(e) => format!("Error: {e}"),
    }
}

fn handle_skills(skills: &[omega_skills::Skill]) -> String {
    if skills.is_empty() {
        return "No skills installed. Create a directory in ~/.omega/skills/ with a SKILL.md file."
            .to_string();
    }
    let mut out = String::from("Installed Skills\n");
    for s in skills {
        let status = if s.available {
            "available"
        } else {
            "missing deps"
        };
        out.push_str(&format!("\n- {} [{}]: {}", s.name, status, s.description));
    }
    out
}

/// Handle /projects — list available projects, marking the active one.
async fn handle_projects(
    store: &Store,
    sender_id: &str,
    projects: &[omega_skills::Project],
) -> String {
    if projects.is_empty() {
        return "No projects found. Create folders in ~/.omega/projects/ with ROLE.md".to_string();
    }
    let active = store
        .get_fact(sender_id, "active_project")
        .await
        .ok()
        .flatten();
    let mut out = String::from("Projects\n");
    for p in projects {
        let marker = if active.as_deref() == Some(&p.name) {
            " (active)"
        } else {
            ""
        };
        out.push_str(&format!("\n- {}{marker}", p.name));
    }
    out.push_str("\n\nUse /project <name> to activate, /project off to deactivate.");
    out
}

/// Handle /project — activate, deactivate, or show current project.
async fn handle_project(
    store: &Store,
    channel: &str,
    sender_id: &str,
    text: &str,
    projects: &[omega_skills::Project],
) -> String {
    let arg = text
        .split_whitespace()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ");

    if arg.is_empty() {
        // Show current project.
        return match store.get_fact(sender_id, "active_project").await {
            Ok(Some(name)) => format!("Active project: {name}\nUse /project off to deactivate."),
            Ok(None) => "No active project. Use /project <name> to activate.".to_string(),
            Err(e) => format!("Error: {e}"),
        };
    }

    if arg == "off" {
        // Deactivate.
        match store.delete_fact(sender_id, "active_project").await {
            Ok(true) => {
                // Close conversation for a clean context.
                let _ = store.close_current_conversation(channel, sender_id).await;
                "Project deactivated. Conversation cleared.".to_string()
            }
            Ok(false) => "No active project.".to_string(),
            Err(e) => format!("Error: {e}"),
        }
    } else {
        // Activate a project.
        if omega_skills::get_project_instructions(projects, &arg).is_none() {
            return format!("Project '{arg}' not found. Use /projects to see available projects.");
        }
        match store.store_fact(sender_id, "active_project", &arg).await {
            Ok(()) => {
                // Close conversation for a clean context.
                let _ = store.close_current_conversation(channel, sender_id).await;
                format!("Project '{arg}' activated. Conversation cleared.")
            }
            Err(e) => format!("Error: {e}"),
        }
    }
}

/// Handle /whatsapp — returns a marker that the gateway intercepts.
fn handle_whatsapp() -> String {
    "WHATSAPP_QR".to_string()
}

fn handle_help() -> String {
    "\
*OMEGA Ω* Commands\n\n\
/status   — Uptime, provider, database info\n\
/memory   — Your conversation and facts stats\n\
/history  — Last 5 conversation summaries\n\
/facts    — List known facts about you\n\
/forget   — Clear current conversation\n\
/tasks    — List your scheduled tasks\n\
/cancel   — Cancel a task by ID\n\
/language — Show or set your language\n\
/personality — Show or set how I behave\n\
/skills   — List available skills\n\
/projects — List available projects\n\
/project  — Show, activate, or deactivate a project\n\
/whatsapp — Connect WhatsApp via QR code\n\
/help     — This message"
        .to_string()
}

/// Format bytes into a human-readable string.
fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_core::config::MemoryConfig;

    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Create a temporary on-disk store for testing (unique per call).
    async fn test_store() -> Store {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("__omega_cmd_test_{}_{}__", std::process::id(), id));
        let _ = std::fs::create_dir_all(&dir);
        let db_path = dir.join("test.db").to_string_lossy().to_string();
        let _ = std::fs::remove_file(&db_path);
        let config = MemoryConfig {
            backend: "sqlite".to_string(),
            db_path,
            max_context_messages: 10,
        };
        Store::new(&config).await.unwrap()
    }

    #[test]
    fn test_parse_personality_command() {
        assert!(matches!(
            Command::parse("/personality"),
            Some(Command::Personality)
        ));
        assert!(matches!(
            Command::parse("/personality be more casual"),
            Some(Command::Personality)
        ));
        assert!(matches!(
            Command::parse("/personality reset"),
            Some(Command::Personality)
        ));
    }

    #[test]
    fn test_parse_all_commands() {
        assert!(matches!(Command::parse("/status"), Some(Command::Status)));
        assert!(matches!(Command::parse("/memory"), Some(Command::Memory)));
        assert!(matches!(Command::parse("/history"), Some(Command::History)));
        assert!(matches!(Command::parse("/facts"), Some(Command::Facts)));
        assert!(matches!(Command::parse("/forget"), Some(Command::Forget)));
        assert!(matches!(Command::parse("/tasks"), Some(Command::Tasks)));
        assert!(matches!(Command::parse("/cancel x"), Some(Command::Cancel)));
        assert!(matches!(
            Command::parse("/language"),
            Some(Command::Language)
        ));
        assert!(matches!(Command::parse("/lang"), Some(Command::Language)));
        assert!(matches!(
            Command::parse("/personality"),
            Some(Command::Personality)
        ));
        assert!(matches!(Command::parse("/skills"), Some(Command::Skills)));
        assert!(matches!(
            Command::parse("/projects"),
            Some(Command::Projects)
        ));
        assert!(matches!(Command::parse("/project"), Some(Command::Project)));
        assert!(matches!(
            Command::parse("/whatsapp"),
            Some(Command::WhatsApp)
        ));
        assert!(matches!(Command::parse("/help"), Some(Command::Help)));
    }

    #[test]
    fn test_parse_unknown_returns_none() {
        assert!(Command::parse("/unknown").is_none());
        assert!(Command::parse("hello").is_none());
        assert!(Command::parse("").is_none());
    }

    #[tokio::test]
    async fn test_personality_show_default() {
        let store = test_store().await;
        let result = handle_personality(&store, "user1", "/personality").await;
        assert!(
            result.contains("default personality"),
            "should show default when no preference set"
        );
    }

    #[tokio::test]
    async fn test_personality_set_and_show() {
        let store = test_store().await;
        let result =
            handle_personality(&store, "user1", "/personality be more casual and funny").await;
        assert!(
            result.contains("be more casual and funny"),
            "should confirm the personality was set"
        );

        let result = handle_personality(&store, "user1", "/personality").await;
        assert!(
            result.contains("be more casual and funny"),
            "should show the stored personality"
        );
    }

    #[tokio::test]
    async fn test_personality_reset() {
        let store = test_store().await;
        let _ = handle_personality(&store, "user1", "/personality be formal").await;
        let result = handle_personality(&store, "user1", "/personality reset").await;
        assert!(result.contains("reset to defaults"), "should confirm reset");

        let result = handle_personality(&store, "user1", "/personality").await;
        assert!(
            result.contains("default personality"),
            "should show default after reset"
        );
    }

    #[tokio::test]
    async fn test_personality_reset_when_already_default() {
        let store = test_store().await;
        let result = handle_personality(&store, "user1", "/personality reset").await;
        assert!(
            result.contains("Already using default"),
            "should indicate already default"
        );
    }
}
