//! Learning command handler: /learning — show outcomes and behavioral rules.

use crate::i18n;
use omega_memory::Store;

/// Handle `/learning` — display recent outcomes and distilled lessons.
pub(super) async fn handle_learning(store: &Store, sender_id: &str, lang: &str) -> String {
    let outcomes = store.get_recent_outcomes(sender_id, 20, None).await;
    let lessons = store.get_lessons(sender_id, None).await;

    let outcomes = outcomes.unwrap_or_default();
    let lessons = lessons.unwrap_or_default();

    if outcomes.is_empty() && lessons.is_empty() {
        return i18n::t("no_learning", lang).to_string();
    }

    let mut out = format!("{}\n", i18n::t("learning_header", lang));

    // --- Lessons (behavioral rules) ---
    if !lessons.is_empty() {
        out.push_str(&format!("\n*{}*\n", i18n::t("learning_rules", lang)));
        for (domain, rule, project) in &lessons {
            if project.is_empty() {
                out.push_str(&format!(
                    "- [{}] {}\n",
                    super::status::escape_md(domain),
                    super::status::escape_md(rule)
                ));
            } else {
                out.push_str(&format!(
                    "- [{}] ({}) {}\n",
                    super::status::escape_md(domain),
                    super::status::escape_md(project),
                    super::status::escape_md(rule)
                ));
            }
        }
    }

    // --- Recent outcomes ---
    if !outcomes.is_empty() {
        out.push_str(&format!("\n*{}*\n", i18n::t("learning_outcomes", lang)));
        for (score, domain, lesson, timestamp) in &outcomes {
            let icon = match *score {
                1 => "+",
                -1 => "-",
                _ => "~",
            };
            let time_ago = format_time_ago(timestamp);
            out.push_str(&format!(
                "- [{}] {}: {} ({})\n",
                icon,
                super::status::escape_md(domain),
                super::status::escape_md(lesson),
                time_ago
            ));
        }
    }

    out
}

/// Format a SQLite timestamp into a human-readable "time ago" string.
fn format_time_ago(timestamp: &str) -> String {
    let Ok(ts) = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S") else {
        return timestamp.to_string();
    };
    let now = chrono::Utc::now().naive_utc();
    let diff = now.signed_duration_since(ts);

    let mins = diff.num_minutes();
    let hours = diff.num_hours();
    let days = diff.num_days();

    if mins < 1 {
        "just now".to_string()
    } else if mins < 60 {
        format!("{mins}m ago")
    } else if hours < 24 {
        format!("{hours}h ago")
    } else {
        format!("{days}d ago")
    }
}
