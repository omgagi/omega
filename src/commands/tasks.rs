//! Task and data management command handlers: /tasks, /cancel, /forget, /purge.

use super::status::escape_md;
use crate::i18n;
use omega_memory::Store;

pub(super) async fn handle_tasks(store: &Store, sender_id: &str, lang: &str) -> String {
    match store.get_tasks_for_sender(sender_id).await {
        Ok(tasks) if tasks.is_empty() => i18n::t("no_pending_tasks", lang).to_string(),
        Ok(tasks) => {
            let mut out = format!("{}\n", i18n::t("scheduled_tasks", lang));
            for (id, description, due_at, repeat, task_type) in &tasks {
                let short_id = &id[..8.min(id.len())];
                let repeat_label = repeat.as_deref().unwrap_or_else(|| i18n::t("once", lang));
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

pub(super) async fn handle_cancel(
    store: &Store,
    sender_id: &str,
    text: &str,
    lang: &str,
) -> String {
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

pub(super) async fn handle_forget(
    store: &Store,
    channel: &str,
    sender_id: &str,
    lang: &str,
) -> String {
    match store.close_current_conversation(channel, sender_id).await {
        Ok(true) => i18n::t("conversation_cleared", lang).to_string(),
        Ok(false) => i18n::t("no_active_conversation", lang).to_string(),
        Err(e) => format!("Error: {e}"),
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
pub(super) async fn handle_purge(store: &Store, sender_id: &str, lang: &str) -> String {
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
