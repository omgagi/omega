//! Built-in bot commands — instant responses, no provider call.

use crate::i18n;
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
    Purge,
    WhatsApp,
    Help,
}

impl Command {
    /// Parse a command from message text. Returns `None` for unknown `/` prefixes
    /// (which should pass through to the provider).
    pub fn parse(text: &str) -> Option<Self> {
        let first = text.split_whitespace().next()?;
        // Strip @botname suffix (e.g. "/help@omega_bot" → "/help").
        let cmd = first.split('@').next().unwrap_or(first);
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
            "/purge" => Some(Self::Purge),
            "/whatsapp" => Some(Self::WhatsApp),
            "/help" => Some(Self::Help),
            _ => None,
        }
    }
}

/// Resolve the user's preferred language, defaulting to English.
async fn resolve_lang(store: &Store, sender_id: &str) -> String {
    store
        .get_fact(sender_id, "preferred_language")
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| "English".to_string())
}

/// Handle a command and return the response text.
pub async fn handle(cmd: Command, ctx: &CommandContext<'_>) -> String {
    let lang = resolve_lang(ctx.store, ctx.sender_id).await;
    match cmd {
        Command::Status => {
            handle_status(
                ctx.store,
                ctx.uptime,
                ctx.provider_name,
                ctx.sandbox_mode,
                &lang,
            )
            .await
        }
        Command::Memory => handle_memory(ctx.store, ctx.sender_id, &lang).await,
        Command::History => handle_history(ctx.store, ctx.channel, ctx.sender_id, &lang).await,
        Command::Facts => handle_facts(ctx.store, ctx.sender_id, &lang).await,
        Command::Forget => handle_forget(ctx.store, ctx.channel, ctx.sender_id, &lang).await,
        Command::Tasks => handle_tasks(ctx.store, ctx.sender_id, &lang).await,
        Command::Cancel => handle_cancel(ctx.store, ctx.sender_id, ctx.text, &lang).await,
        Command::Language => handle_language(ctx.store, ctx.sender_id, ctx.text, &lang).await,
        Command::Personality => {
            handle_personality(ctx.store, ctx.sender_id, ctx.text, &lang).await
        }
        Command::Skills => handle_skills(ctx.skills, &lang),
        Command::Projects => {
            handle_projects(ctx.store, ctx.sender_id, ctx.projects, &lang).await
        }
        Command::Project => {
            handle_project(
                ctx.store,
                ctx.channel,
                ctx.sender_id,
                ctx.text,
                ctx.projects,
                &lang,
            )
            .await
        }
        Command::Purge => handle_purge(ctx.store, ctx.sender_id, &lang).await,
        Command::WhatsApp => handle_whatsapp(),
        Command::Help => handle_help(&lang),
    }
}

async fn handle_status(
    store: &Store,
    uptime: &Instant,
    provider_name: &str,
    sandbox_mode: &str,
    lang: &str,
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
        "{}\n\
         {} {hours}h {minutes}m {secs}s\n\
         {} {provider_name}\n\
         {} {sandbox_mode}\n\
         {} {db_size}",
        i18n::t("status_header", lang),
        i18n::t("uptime", lang),
        i18n::t("provider", lang),
        i18n::t("sandbox", lang),
        i18n::t("database", lang),
    )
}

async fn handle_memory(store: &Store, sender_id: &str, lang: &str) -> String {
    match store.get_memory_stats(sender_id).await {
        Ok((convos, msgs, facts)) => {
            format!(
                "{}\n\
                 {} {convos}\n\
                 {} {msgs}\n\
                 {} {facts}",
                i18n::t("your_memory", lang),
                i18n::t("conversations", lang),
                i18n::t("messages", lang),
                i18n::t("facts_label", lang),
            )
        }
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_history(store: &Store, channel: &str, sender_id: &str, lang: &str) -> String {
    match store.get_history(channel, sender_id, 5).await {
        Ok(entries) if entries.is_empty() => i18n::t("no_history", lang).to_string(),
        Ok(entries) => {
            let mut out = format!("{}\n", i18n::t("recent_conversations", lang));
            for (summary, timestamp) in &entries {
                out.push_str(&format!("\n[{timestamp}]\n{summary}\n"));
            }
            out
        }
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_facts(store: &Store, sender_id: &str, lang: &str) -> String {
    match store.get_facts(sender_id).await {
        Ok(facts) if facts.is_empty() => i18n::t("no_facts", lang).to_string(),
        Ok(facts) => {
            let mut out = format!("{}\n", i18n::t("known_facts", lang));
            for (key, value) in &facts {
                out.push_str(&format!("\n- {}: {}", escape_md(key), escape_md(value)));
            }
            out
        }
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_forget(store: &Store, channel: &str, sender_id: &str, lang: &str) -> String {
    match store.close_current_conversation(channel, sender_id).await {
        Ok(true) => i18n::t("conversation_cleared", lang).to_string(),
        Ok(false) => i18n::t("no_active_conversation", lang).to_string(),
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_tasks(store: &Store, sender_id: &str, lang: &str) -> String {
    match store.get_tasks_for_sender(sender_id).await {
        Ok(tasks) if tasks.is_empty() => i18n::t("no_pending_tasks", lang).to_string(),
        Ok(tasks) => {
            let mut out = format!("{}\n", i18n::t("scheduled_tasks", lang));
            for (id, description, due_at, repeat, task_type) in &tasks {
                let short_id = &id[..8.min(id.len())];
                let repeat_label = repeat
                    .as_deref()
                    .unwrap_or_else(|| i18n::t("once", lang));
                let type_badge = if task_type == "action" {
                    " [action]"
                } else {
                    ""
                };
                out.push_str(&format!(
                    "\n[{short_id}] {description}{type_badge}\n  {} {due_at} ({repeat_label})",
                    i18n::t("due", lang),
                ));
            }
            out
        }
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_cancel(store: &Store, sender_id: &str, text: &str, lang: &str) -> String {
    let id_prefix = text.split_whitespace().nth(1).unwrap_or("").trim();
    if id_prefix.is_empty() {
        return i18n::t("cancel_usage", lang).to_string();
    }
    match store.cancel_task(id_prefix, sender_id).await {
        Ok(true) => i18n::t("task_cancelled", lang).to_string(),
        Ok(false) => i18n::t("no_matching_task", lang).to_string(),
        Err(e) => format!("Error: {e}"),
    }
}

async fn handle_language(store: &Store, sender_id: &str, text: &str, lang: &str) -> String {
    let arg = text
        .split_whitespace()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ");
    if arg.is_empty() {
        // Show current preference.
        match store.get_facts(sender_id).await {
            Ok(facts) => {
                let current = facts
                    .iter()
                    .find(|(k, _)| k == "preferred_language")
                    .map(|(_, v)| v.as_str())
                    .unwrap_or_else(|| i18n::t("not_set", lang));
                i18n::language_show(lang, current)
            }
            Err(e) => format!("Error: {e}"),
        }
    } else {
        match store
            .store_fact(sender_id, "preferred_language", &arg)
            .await
        {
            Ok(()) => i18n::language_set(lang, &arg),
            Err(e) => format!("Error: {e}"),
        }
    }
}

/// Handle /personality — show, set, or reset personality preferences.
async fn handle_personality(store: &Store, sender_id: &str, text: &str, lang: &str) -> String {
    let arg = text
        .split_whitespace()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ");

    if arg.is_empty() {
        // Show current personality preference.
        return match store.get_fact(sender_id, "personality").await {
            Ok(Some(p)) => i18n::personality_show(lang, &p),
            Ok(None) => i18n::t("personality_default_prompt", lang).to_string(),
            Err(e) => format!("Error: {e}"),
        };
    }

    if arg == "reset" {
        return match store.delete_fact(sender_id, "personality").await {
            Ok(true) => i18n::t("personality_reset", lang).to_string(),
            Ok(false) => i18n::t("personality_already_default", lang).to_string(),
            Err(e) => format!("Error: {e}"),
        };
    }

    // Store the personality preference as a fact.
    match store.store_fact(sender_id, "personality", &arg).await {
        Ok(()) => i18n::personality_updated(lang, &arg),
        Err(e) => format!("Error: {e}"),
    }
}

fn handle_skills(skills: &[omega_skills::Skill], lang: &str) -> String {
    if skills.is_empty() {
        return i18n::t("no_skills", lang).to_string();
    }
    let mut out = format!("{}\n", i18n::t("installed_skills", lang));
    for s in skills {
        let status = if s.available {
            i18n::t("available", lang)
        } else {
            i18n::t("missing_deps", lang)
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
    lang: &str,
) -> String {
    if projects.is_empty() {
        return i18n::t("no_projects", lang).to_string();
    }
    let active = store
        .get_fact(sender_id, "active_project")
        .await
        .ok()
        .flatten();
    let mut out = format!("{}\n", i18n::t("projects_header", lang));
    for p in projects {
        let marker = if active.as_deref() == Some(&p.name) {
            " (active)"
        } else {
            ""
        };
        out.push_str(&format!("\n- {}{marker}", p.name));
    }
    out.push_str(&format!("\n\n{}", i18n::t("projects_footer", lang)));
    out
}

/// Handle /project — activate, deactivate, or show current project.
async fn handle_project(
    store: &Store,
    channel: &str,
    sender_id: &str,
    text: &str,
    projects: &[omega_skills::Project],
    lang: &str,
) -> String {
    let arg = text
        .split_whitespace()
        .skip(1)
        .collect::<Vec<_>>()
        .join(" ");

    if arg.is_empty() {
        // Show current project.
        return match store.get_fact(sender_id, "active_project").await {
            Ok(Some(name)) => i18n::active_project(lang, &name),
            Ok(None) => i18n::t("no_active_project_hint", lang).to_string(),
            Err(e) => format!("Error: {e}"),
        };
    }

    if arg == "off" {
        // Deactivate.
        match store.delete_fact(sender_id, "active_project").await {
            Ok(true) => {
                // Close conversation for a clean context.
                let _ = store.close_current_conversation(channel, sender_id).await;
                i18n::t("project_deactivated", lang).to_string()
            }
            Ok(false) => i18n::t("no_active_project", lang).to_string(),
            Err(e) => format!("Error: {e}"),
        }
    } else {
        // Activate a project.
        if omega_skills::get_project_instructions(projects, &arg).is_none() {
            return i18n::project_not_found(lang, &arg);
        }
        match store.store_fact(sender_id, "active_project", &arg).await {
            Ok(()) => {
                // Close conversation for a clean context.
                let _ = store.close_current_conversation(channel, sender_id).await;
                i18n::project_activated(lang, &arg)
            }
            Err(e) => format!("Error: {e}"),
        }
    }
}

/// System fact keys preserved by /purge.
const SYSTEM_FACT_KEYS: &[&str] = &[
    "welcomed",
    "preferred_language",
    "active_project",
    "personality",
    "onboarding_stage",
];

/// Handle /purge — delete all non-system facts, giving the user a clean slate.
async fn handle_purge(store: &Store, sender_id: &str, lang: &str) -> String {
    // Save system facts first.
    let preserved: Vec<(String, String)> = match store.get_facts(sender_id).await {
        Ok(facts) => facts
            .into_iter()
            .filter(|(k, _)| SYSTEM_FACT_KEYS.contains(&k.as_str()))
            .collect(),
        Err(e) => return format!("Error: {e}"),
    };

    // Delete all facts.
    let deleted = match store.delete_facts(sender_id, None).await {
        Ok(n) => n,
        Err(e) => return format!("Error: {e}"),
    };

    // Restore system facts.
    for (key, value) in &preserved {
        let _ = store.store_fact(sender_id, key, value).await;
    }

    let purged = deleted as usize - preserved.len();
    let keys_display: Vec<String> = SYSTEM_FACT_KEYS.iter().map(|k| escape_md(k)).collect();
    i18n::purge_result(lang, purged, &keys_display.join(", "))
}

/// Handle /whatsapp — returns a marker that the gateway intercepts.
fn handle_whatsapp() -> String {
    "WHATSAPP_QR".to_string()
}

fn handle_help(lang: &str) -> String {
    format!(
        "{}\n\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}\n\
         {}",
        i18n::t("commands_header", lang),
        i18n::t("help_status", lang),
        i18n::t("help_memory", lang),
        i18n::t("help_history", lang),
        i18n::t("help_facts", lang),
        i18n::t("help_forget", lang),
        i18n::t("help_tasks", lang),
        i18n::t("help_cancel", lang),
        i18n::t("help_language", lang),
        i18n::t("help_personality", lang),
        i18n::t("help_purge", lang),
        i18n::t("help_skills", lang),
        i18n::t("help_projects", lang),
        i18n::t("help_project", lang),
        i18n::t("help_whatsapp", lang),
        i18n::t("help_help", lang),
    )
}

/// Escape underscores for Telegram Markdown rendering.
fn escape_md(s: &str) -> String {
    s.replace('_', "\\_")
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
        assert!(matches!(Command::parse("/purge"), Some(Command::Purge)));
        assert!(matches!(
            Command::parse("/whatsapp"),
            Some(Command::WhatsApp)
        ));
        assert!(matches!(Command::parse("/help"), Some(Command::Help)));
    }

    #[test]
    fn test_parse_commands_with_botname_suffix() {
        assert!(matches!(
            Command::parse("/help@omega_bot"),
            Some(Command::Help)
        ));
        assert!(matches!(
            Command::parse("/status@omega_bot"),
            Some(Command::Status)
        ));
        assert!(matches!(
            Command::parse("/cancel@omega_bot task123"),
            Some(Command::Cancel)
        ));
        assert!(matches!(
            Command::parse("/lang@omega_bot"),
            Some(Command::Language)
        ));
        // Unknown command with @botname should still return None.
        assert!(Command::parse("/unknown@omega_bot").is_none());
    }

    #[test]
    fn test_parse_purge_command() {
        assert!(matches!(Command::parse("/purge"), Some(Command::Purge)));
        assert!(matches!(
            Command::parse("/purge@omega_bot"),
            Some(Command::Purge)
        ));
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
        let result = handle_personality(&store, "user1", "/personality", "English").await;
        assert!(
            result.contains("default personality"),
            "should show default when no preference set"
        );
    }

    #[tokio::test]
    async fn test_personality_set_and_show() {
        let store = test_store().await;
        let result = handle_personality(
            &store,
            "user1",
            "/personality be more casual and funny",
            "English",
        )
        .await;
        assert!(
            result.contains("be more casual and funny"),
            "should confirm the personality was set"
        );

        let result = handle_personality(&store, "user1", "/personality", "English").await;
        assert!(
            result.contains("be more casual and funny"),
            "should show the stored personality"
        );
    }

    #[tokio::test]
    async fn test_personality_reset() {
        let store = test_store().await;
        let _ = handle_personality(&store, "user1", "/personality be formal", "English").await;
        let result = handle_personality(&store, "user1", "/personality reset", "English").await;
        assert!(result.contains("reset to defaults"), "should confirm reset");

        let result = handle_personality(&store, "user1", "/personality", "English").await;
        assert!(
            result.contains("default personality"),
            "should show default after reset"
        );
    }

    #[tokio::test]
    async fn test_personality_reset_when_already_default() {
        let store = test_store().await;
        let result = handle_personality(&store, "user1", "/personality reset", "English").await;
        assert!(
            result.contains("Already using default"),
            "should indicate already default"
        );
    }

    #[tokio::test]
    async fn test_purge_preserves_system_facts() {
        let store = test_store().await;
        // Store system facts.
        store.store_fact("user1", "welcomed", "true").await.unwrap();
        store
            .store_fact("user1", "preferred_language", "Spanish")
            .await
            .unwrap();
        store
            .store_fact("user1", "personality", "casual")
            .await
            .unwrap();
        // Store junk facts.
        store
            .store_fact("user1", "btc_price", "45000")
            .await
            .unwrap();
        store
            .store_fact("user1", "target", "0.5 BTC")
            .await
            .unwrap();
        store.store_fact("user1", "name", "Juan").await.unwrap();

        let result = handle_purge(&store, "user1", "English").await;
        assert!(
            result.contains("Purged 3 facts"),
            "should report 3 purged: {result}"
        );

        // System facts preserved.
        let facts = store.get_facts("user1").await.unwrap();
        let keys: Vec<&str> = facts.iter().map(|(k, _)| k.as_str()).collect();
        assert!(keys.contains(&"welcomed"));
        assert!(keys.contains(&"preferred_language"));
        assert!(keys.contains(&"personality"));
        // Junk facts removed.
        assert!(!keys.contains(&"btc_price"));
        assert!(!keys.contains(&"target"));
        // Non-system personal facts also removed.
        assert!(!keys.contains(&"name"));
    }

    #[tokio::test]
    async fn test_help_spanish() {
        let result = handle_help("Spanish");
        assert!(
            result.contains("Comandos de *OMEGA Ω*"),
            "should have Spanish header: {result}"
        );
        assert!(
            result.contains("/status"),
            "should still contain command names"
        );
    }

    #[tokio::test]
    async fn test_forget_localized() {
        let store = test_store().await;
        let result = handle_forget(&store, "telegram", "user1", "Spanish").await;
        assert!(
            result.contains("No hay conversación activa"),
            "should show Spanish empty state: {result}"
        );
    }
}
