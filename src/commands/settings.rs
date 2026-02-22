//! Configuration command handlers: /language, /personality, /skills, /projects, /project,
//! /whatsapp, /heartbeat.

use crate::i18n;
use crate::markers::read_heartbeat_file;
use omega_memory::Store;

pub(super) async fn handle_language(
    store: &Store,
    sender_id: &str,
    text: &str,
    lang: &str,
) -> String {
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
pub(super) async fn handle_personality(
    store: &Store,
    sender_id: &str,
    text: &str,
    lang: &str,
) -> String {
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

pub(super) fn handle_skills(skills: &[omega_skills::Skill], lang: &str) -> String {
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
pub(super) async fn handle_projects(
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
pub(super) async fn handle_project(
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

/// Handle /whatsapp — returns a marker that the gateway intercepts.
pub(super) fn handle_whatsapp() -> String {
    "WHATSAPP_QR".to_string()
}

/// Handle /heartbeat — show heartbeat status, interval, and watchlist items.
pub(super) fn handle_heartbeat(enabled: bool, interval_mins: u64, lang: &str) -> String {
    let status_label = i18n::t("heartbeat_status", lang);
    let status_value = if enabled {
        i18n::t("heartbeat_enabled", lang)
    } else {
        i18n::t("heartbeat_disabled", lang)
    };

    let mut out = format!(
        "{}\n\n{} {}\n{} {} {}",
        i18n::t("heartbeat_header", lang),
        status_label,
        status_value,
        i18n::t("heartbeat_interval", lang),
        interval_mins,
        i18n::t("heartbeat_minutes", lang),
    );

    match read_heartbeat_file() {
        Some(content) => {
            out.push_str(&format!(
                "\n\n{}\n{}",
                i18n::t("heartbeat_watchlist", lang),
                content.trim()
            ));
        }
        None => {
            out.push_str(&format!("\n\n{}", i18n::t("heartbeat_no_watchlist", lang)));
        }
    }
    out
}
