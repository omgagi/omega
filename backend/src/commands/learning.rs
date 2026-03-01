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
            let time_ago = format_time_ago(timestamp, lang);
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
fn format_time_ago(timestamp: &str, lang: &str) -> String {
    let Ok(ts) = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S") else {
        return timestamp.to_string();
    };
    let now = chrono::Utc::now().naive_utc();
    let diff = now.signed_duration_since(ts);

    let mins = diff.num_minutes();
    let hours = diff.num_hours();
    let days = diff.num_days();

    match lang {
        "Spanish" => {
            if mins < 1 {
                "ahora".into()
            } else if mins < 60 {
                format!("hace {mins}m")
            } else if hours < 24 {
                format!("hace {hours}h")
            } else {
                format!("hace {days}d")
            }
        }
        "Portuguese" => {
            if mins < 1 {
                "agora".into()
            } else if mins < 60 {
                format!("h\u{00e1} {mins}m")
            } else if hours < 24 {
                format!("h\u{00e1} {hours}h")
            } else {
                format!("h\u{00e1} {days}d")
            }
        }
        "French" => {
            if mins < 1 {
                "maintenant".into()
            } else if mins < 60 {
                format!("il y a {mins}m")
            } else if hours < 24 {
                format!("il y a {hours}h")
            } else {
                format!("il y a {days}j")
            }
        }
        "German" => {
            if mins < 1 {
                "jetzt".into()
            } else if mins < 60 {
                format!("vor {mins}m")
            } else if hours < 24 {
                format!("vor {hours}h")
            } else {
                format!("vor {days}T")
            }
        }
        "Italian" => {
            if mins < 1 {
                "adesso".into()
            } else if mins < 60 {
                format!("{mins}m fa")
            } else if hours < 24 {
                format!("{hours}h fa")
            } else {
                format!("{days}g fa")
            }
        }
        "Dutch" => {
            if mins < 1 {
                "zojuist".into()
            } else if mins < 60 {
                format!("{mins}m geleden")
            } else if hours < 24 {
                format!("{hours}u geleden")
            } else {
                format!("{days}d geleden")
            }
        }
        "Russian" => {
            if mins < 1 {
                "\u{0442}\u{043e}\u{043b}\u{044c}\u{043a}\u{043e} \u{0447}\u{0442}\u{043e}".into()
            } else if mins < 60 {
                format!("{mins}\u{043c} \u{043d}\u{0430}\u{0437}\u{0430}\u{0434}")
            } else if hours < 24 {
                format!("{hours}\u{0447} \u{043d}\u{0430}\u{0437}\u{0430}\u{0434}")
            } else {
                format!("{days}\u{0434} \u{043d}\u{0430}\u{0437}\u{0430}\u{0434}")
            }
        }
        _ => {
            if mins < 1 {
                "just now".into()
            } else if mins < 60 {
                format!("{mins}m ago")
            } else if hours < 24 {
                format!("{hours}h ago")
            } else {
                format!("{days}d ago")
            }
        }
    }
}
