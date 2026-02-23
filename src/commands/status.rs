//! Status and information command handlers: /status, /memory, /history, /facts, /help.

use crate::i18n;
use omega_memory::Store;
use std::time::Instant;

pub(super) async fn handle_status(
    store: &Store,
    uptime: &Instant,
    provider_name: &str,
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
         {} {db_size}",
        i18n::t("status_header", lang),
        i18n::t("uptime", lang),
        i18n::t("provider", lang),
        i18n::t("database", lang),
    )
}

pub(super) async fn handle_memory(store: &Store, sender_id: &str, lang: &str) -> String {
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

pub(super) async fn handle_history(
    store: &Store,
    channel: &str,
    sender_id: &str,
    lang: &str,
) -> String {
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

pub(super) async fn handle_facts(store: &Store, sender_id: &str, lang: &str) -> String {
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

pub(super) fn handle_help(lang: &str) -> String {
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
        i18n::t("help_learning", lang),
        i18n::t("help_heartbeat", lang),
        i18n::t("help_help", lang),
    )
}

/// Escape underscores for Telegram Markdown rendering.
pub(super) fn escape_md(s: &str) -> String {
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
