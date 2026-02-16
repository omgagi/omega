//! Built-in bot commands — instant responses, no provider call.

use omega_memory::Store;
use std::time::Instant;

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
            "/whatsapp" => Some(Self::WhatsApp),
            "/help" => Some(Self::Help),
            _ => None,
        }
    }
}

/// Handle a command and return the response text.
pub async fn handle(
    cmd: Command,
    store: &Store,
    channel: &str,
    sender_id: &str,
    text: &str,
    uptime: &Instant,
    provider_name: &str,
) -> String {
    match cmd {
        Command::Status => handle_status(store, uptime, provider_name).await,
        Command::Memory => handle_memory(store, sender_id).await,
        Command::History => handle_history(store, channel, sender_id).await,
        Command::Facts => handle_facts(store, sender_id).await,
        Command::Forget => handle_forget(store, channel, sender_id).await,
        Command::Tasks => handle_tasks(store, sender_id).await,
        Command::Cancel => handle_cancel(store, sender_id, text).await,
        Command::Language => handle_language(store, sender_id, text).await,
        Command::WhatsApp => handle_whatsapp(),
        Command::Help => handle_help(),
    }
}

async fn handle_status(store: &Store, uptime: &Instant, provider_name: &str) -> String {
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
        "Omega Status\n\
         Uptime: {hours}h {minutes}m {secs}s\n\
         Provider: {provider_name}\n\
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

/// Handle /whatsapp — returns a marker that the gateway intercepts.
fn handle_whatsapp() -> String {
    "WHATSAPP_QR".to_string()
}

fn handle_help() -> String {
    "\
Omega Commands\n\n\
/status   — Uptime, provider, database info\n\
/memory   — Your conversation and facts stats\n\
/history  — Last 5 conversation summaries\n\
/facts    — List known facts about you\n\
/forget   — Clear current conversation\n\
/tasks    — List your scheduled tasks\n\
/cancel   — Cancel a task by ID\n\
/language — Show or set your language\n\
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
