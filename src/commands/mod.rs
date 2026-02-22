//! Built-in bot commands — instant responses, no provider call.

mod settings;
mod status;
mod tasks;

#[cfg(test)]
mod tests;

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
    pub heartbeat_enabled: bool,
    pub heartbeat_interval_mins: u64,
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
    Heartbeat,
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
            "/heartbeat" => Some(Self::Heartbeat),
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
            status::handle_status(
                ctx.store,
                ctx.uptime,
                ctx.provider_name,
                ctx.sandbox_mode,
                &lang,
            )
            .await
        }
        Command::Memory => status::handle_memory(ctx.store, ctx.sender_id, &lang).await,
        Command::History => {
            status::handle_history(ctx.store, ctx.channel, ctx.sender_id, &lang).await
        }
        Command::Facts => status::handle_facts(ctx.store, ctx.sender_id, &lang).await,
        Command::Forget => tasks::handle_forget(ctx.store, ctx.channel, ctx.sender_id, &lang).await,
        Command::Tasks => tasks::handle_tasks(ctx.store, ctx.sender_id, &lang).await,
        Command::Cancel => tasks::handle_cancel(ctx.store, ctx.sender_id, ctx.text, &lang).await,
        Command::Language => {
            settings::handle_language(ctx.store, ctx.sender_id, ctx.text, &lang).await
        }
        Command::Personality => {
            settings::handle_personality(ctx.store, ctx.sender_id, ctx.text, &lang).await
        }
        Command::Skills => settings::handle_skills(ctx.skills, &lang),
        Command::Projects => {
            settings::handle_projects(ctx.store, ctx.sender_id, ctx.projects, &lang).await
        }
        Command::Project => {
            settings::handle_project(
                ctx.store,
                ctx.channel,
                ctx.sender_id,
                ctx.text,
                ctx.projects,
                &lang,
            )
            .await
        }
        Command::Purge => tasks::handle_purge(ctx.store, ctx.sender_id, &lang).await,
        Command::WhatsApp => settings::handle_whatsapp(),
        Command::Heartbeat => {
            settings::handle_heartbeat(ctx.heartbeat_enabled, ctx.heartbeat_interval_mins, &lang)
        }
        Command::Help => status::handle_help(&lang),
    }
}
