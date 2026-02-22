//! Context building, system prompt composition, user profile formatting,
//! progressive onboarding, and language detection.

use super::Store;
use omega_core::{
    context::{Context, ContextEntry, ContextNeeds},
    error::OmegaError,
    message::IncomingMessage,
};

/// System fact keys filtered out of the user profile.
const SYSTEM_FACT_KEYS: &[&str] = &[
    "welcomed",
    "preferred_language",
    "active_project",
    "personality",
    "onboarding_stage",
];

/// Identity fact keys — shown first in the user profile.
const IDENTITY_KEYS: &[&str] = &["name", "preferred_name", "pronouns"];

/// Context fact keys — shown second in the user profile.
const CONTEXT_KEYS: &[&str] = &["timezone", "location", "occupation"];

impl Store {
    /// Build a conversation context from memory for the provider.
    pub async fn build_context(
        &self,
        incoming: &IncomingMessage,
        base_system_prompt: &str,
        needs: &ContextNeeds,
    ) -> Result<Context, OmegaError> {
        let conv_id = self
            .get_or_create_conversation(&incoming.channel, &incoming.sender_id)
            .await?;

        // Load recent messages from this conversation.
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT role, content FROM messages WHERE conversation_id = ? ORDER BY timestamp DESC LIMIT ?",
        )
        .bind(&conv_id)
        .bind(self.max_context_messages as i64)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("query failed: {e}")))?;

        // Rows come newest-first, reverse for chronological order.
        let history: Vec<ContextEntry> = rows
            .into_iter()
            .rev()
            .map(|(role, content)| ContextEntry { role, content })
            .collect();

        // Facts and summaries are always loaded (small, essential).
        let facts = self
            .get_facts(&incoming.sender_id)
            .await
            .unwrap_or_default();
        let summaries = self
            .get_recent_summaries(&incoming.channel, &incoming.sender_id, 3)
            .await
            .unwrap_or_default();

        // Semantic recall and pending tasks are conditionally loaded.
        let recall = if needs.recall {
            self.search_messages(&incoming.text, &conv_id, &incoming.sender_id, 5)
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };
        let pending_tasks = if needs.pending_tasks {
            self.get_tasks_for_sender(&incoming.sender_id)
                .await
                .unwrap_or_default()
        } else {
            vec![]
        };

        // Outcomes and lessons are always loaded (small, essential for reward awareness).
        let outcomes = self
            .get_recent_outcomes(&incoming.sender_id, 15)
            .await
            .unwrap_or_default();
        let lessons = self
            .get_lessons(&incoming.sender_id)
            .await
            .unwrap_or_default();

        // Resolve language: stored preference > auto-detect > English.
        let language =
            if let Some((_, lang)) = facts.iter().find(|(k, _)| k == "preferred_language") {
                lang.clone()
            } else {
                let detected = detect_language(&incoming.text).to_string();
                let _ = self
                    .store_fact(&incoming.sender_id, "preferred_language", &detected)
                    .await;
                detected
            };

        // Progressive onboarding: compute stage and inject hint on transitions.
        let real_fact_count = facts
            .iter()
            .filter(|(k, _)| !SYSTEM_FACT_KEYS.contains(&k.as_str()))
            .count();
        let has_tasks = !pending_tasks.is_empty();

        let current_stage: u8 = facts
            .iter()
            .find(|(k, _)| k == "onboarding_stage")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0);

        let new_stage = compute_onboarding_stage(current_stage, real_fact_count, has_tasks);

        let onboarding_hint = if new_stage != current_stage {
            // Stage advanced — store it and show the hint for the NEW stage.
            let _ = self
                .store_fact(
                    &incoming.sender_id,
                    "onboarding_stage",
                    &new_stage.to_string(),
                )
                .await;
            Some(new_stage)
        } else if current_stage == 0 && real_fact_count == 0 {
            // First contact — no stored stage yet, show intro.
            Some(0u8)
        } else {
            // Pre-existing user with no stage fact: silently store current stage, no hint.
            if facts.iter().all(|(k, _)| k != "onboarding_stage") && current_stage == 0 {
                let bootstrapped = compute_onboarding_stage(0, real_fact_count, has_tasks);
                // Walk through all stages up to current state.
                let final_stage = (0..=4).fold(0u8, |s, _| {
                    compute_onboarding_stage(s, real_fact_count, has_tasks)
                });
                if final_stage > 0 {
                    let _ = self
                        .store_fact(
                            &incoming.sender_id,
                            "onboarding_stage",
                            &final_stage.to_string(),
                        )
                        .await;
                }
                let _ = bootstrapped; // suppress unused warning
                None
            } else {
                None
            }
        };

        let system_prompt = build_system_prompt(
            base_system_prompt,
            &facts,
            &summaries,
            &recall,
            &pending_tasks,
            &outcomes,
            &lessons,
            &language,
            onboarding_hint,
        );

        Ok(Context {
            system_prompt,
            history,
            current_message: incoming.text.clone(),
            mcp_servers: Vec::new(),
            max_turns: None,
            allowed_tools: None,
            model: None,
            session_id: None,
        })
    }
}

/// Format user facts into a structured profile, filtering system keys
/// and grouping identity facts first, then context, then the rest.
///
/// Returns an empty string when only system facts exist.
pub fn format_user_profile(facts: &[(String, String)]) -> String {
    let user_facts: Vec<&(String, String)> = facts
        .iter()
        .filter(|(k, _)| !SYSTEM_FACT_KEYS.contains(&k.as_str()))
        .collect();

    if user_facts.is_empty() {
        return String::new();
    }

    let mut lines = vec!["User profile:".to_string()];

    // Identity group first.
    for key in IDENTITY_KEYS {
        if let Some((_, v)) = user_facts.iter().find(|(k, _)| k == key) {
            lines.push(format!("- {key}: {v}"));
        }
    }

    // Context group second.
    for key in CONTEXT_KEYS {
        if let Some((_, v)) = user_facts.iter().find(|(k, _)| k == key) {
            lines.push(format!("- {key}: {v}"));
        }
    }

    // Everything else, preserving original order.
    let known_keys: Vec<&str> = IDENTITY_KEYS
        .iter()
        .chain(CONTEXT_KEYS.iter())
        .copied()
        .collect();
    for (k, v) in &user_facts {
        if !known_keys.contains(&k.as_str()) {
            lines.push(format!("- {k}: {v}"));
        }
    }

    lines.join("\n")
}

/// Compute the next onboarding stage based on current state.
///
/// Stages are sequential — can't skip. Each fires exactly once then advances.
/// - Stage 0: First contact (intro)
/// - Stage 1: 1+ real facts → teach /help
/// - Stage 2: 3+ real facts → teach personality
/// - Stage 3: First task created → teach task management
/// - Stage 4: 5+ real facts → teach projects
/// - Stage 5: Done (no more hints)
pub(super) fn compute_onboarding_stage(
    current_stage: u8,
    real_fact_count: usize,
    has_tasks: bool,
) -> u8 {
    match current_stage {
        0 if real_fact_count >= 1 => 1,
        1 if real_fact_count >= 3 => 2,
        2 if has_tasks => 3,
        3 if real_fact_count >= 5 => 4,
        4 => 5,
        _ => current_stage,
    }
}

/// Return the onboarding hint text for a given stage, or `None` if no hint.
pub(super) fn onboarding_hint_text(stage: u8, language: &str) -> Option<String> {
    match stage {
        0 => Some(format!(
            "\n\nThis is your first conversation with this person. Respond ONLY with this \
             introduction in {language} (adapt naturally, do NOT translate literally):\n\n\
             Start with '\u{1f44b}' followed by an appropriate greeting in {language} on the same line.\n\n\
             Glad to have them here. You are *OMEGA \u{03a9}* (always bold), their personal agent — \
             but before jumping into action, you'd like to get to know them a bit.\n\n\
             Ask their name and what they do, so you can be more useful from the start.\n\n\
             Do NOT mention infrastructure, Rust, Claude, or any technical details. \
             Do NOT answer their message yet. Just this introduction, nothing else.",
        )),
        1 => Some(format!(
            "\n\nOnboarding hint: This person is new. At the end of your response, \
             casually mention that they can ask you anything or type /help to see what you can do. \
             Keep it brief and natural — one sentence max. Respond in {language}."
        )),
        2 => Some(format!(
            "\n\nOnboarding hint: This person hasn't customized your personality yet. \
             At the end of your response, casually mention they can tell you how to behave \
             (e.g. 'be more casual') or use /personality. One sentence max, only if it fits naturally. \
             Respond in {language}."
        )),
        3 => Some(format!(
            "\n\nOnboarding hint: This person just created their first task! \
             At the end of your response, briefly mention they can say 'show my tasks' \
             or type /tasks to see scheduled items. One sentence max. Respond in {language}."
        )),
        4 => Some(format!(
            "\n\nOnboarding hint: This person is getting comfortable. \
             At the end of your response, briefly mention they can organize work into projects — \
             just say 'create a project' or type /projects to see how. One sentence max. \
             Respond in {language}."
        )),
        _ => None,
    }
}

/// Build a dynamic system prompt enriched with facts, conversation history, and recalled messages.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_system_prompt(
    base_rules: &str,
    facts: &[(String, String)],
    summaries: &[(String, String)],
    recall: &[(String, String, String)],
    pending_tasks: &[(String, String, String, Option<String>, String)],
    outcomes: &[(i32, String, String, String)],
    lessons: &[(String, String)],
    language: &str,
    onboarding_hint: Option<u8>,
) -> String {
    let mut prompt = String::from(base_rules);

    let profile = format_user_profile(facts);
    if !profile.is_empty() {
        prompt.push_str("\n\n");
        prompt.push_str(&profile);
    }

    if !summaries.is_empty() {
        prompt.push_str("\n\nRecent conversation history:");
        for (summary, timestamp) in summaries {
            prompt.push_str(&format!("\n- [{timestamp}] {summary}"));
        }
    }

    if !recall.is_empty() {
        prompt.push_str("\n\nRelated past context:");
        for (_role, content, timestamp) in recall {
            let truncated = if content.len() > 200 {
                format!("{}...", &content[..200])
            } else {
                content.clone()
            };
            prompt.push_str(&format!("\n- [{timestamp}] User: {truncated}"));
        }
    }

    if !pending_tasks.is_empty() {
        prompt.push_str("\n\nUser's scheduled tasks:");
        for (id, desc, due_at, repeat, task_type) in pending_tasks {
            let r = repeat.as_deref().unwrap_or("once");
            let type_badge = if task_type == "action" {
                " [action]"
            } else {
                ""
            };
            prompt.push_str(&format!(
                "\n- [{id_short}] {desc}{type_badge} (due: {due_at}, {r})",
                id_short = &id[..8.min(id.len())]
            ));
        }
    }

    if !lessons.is_empty() {
        prompt.push_str("\n\nLearned behavioral rules:");
        for (domain, rule) in lessons {
            prompt.push_str(&format!("\n- [{domain}] {rule}"));
        }
    }

    if !outcomes.is_empty() {
        prompt.push_str("\n\nRecent outcomes:");
        let now = chrono::Utc::now();
        for (score, domain, lesson, timestamp) in outcomes {
            let ago = format_relative_time(timestamp, &now);
            let sign = if *score > 0 {
                "+"
            } else if *score < 0 {
                "-"
            } else {
                "~"
            };
            prompt.push_str(&format!("\n- [{sign}] {domain}: {lesson} ({ago})"));
        }
    }

    prompt.push_str(&format!("\n\nIMPORTANT: Always respond in {language}."));

    // Progressive onboarding: inject hint only when a stage transition fires.
    if let Some(stage) = onboarding_hint {
        if let Some(hint) = onboarding_hint_text(stage, language) {
            prompt.push_str(&hint);
        }
    }

    prompt.push_str(
        "\n\nIf the user explicitly asks you to change language (e.g. 'speak in French'), \
         respond in the requested language. Include LANG_SWITCH: <language> on its own line \
         at the END of your response.",
    );

    prompt
}

/// Detect the most likely language of a text using stop-word heuristics.
/// Returns a language name like "English", "Spanish", etc.
pub fn detect_language(text: &str) -> &'static str {
    let lower = text.to_lowercase();

    let languages: &[(&str, &[&str])] = &[
        (
            "Spanish",
            &[
                " que ", " por ", " para ", " como ", " con ", " una ", " los ", " las ", " del ",
                " pero ", "hola", "gracias", "necesito", "quiero", "puedes",
            ],
        ),
        (
            "Portuguese",
            &[
                " que ", " com ", " para ", " uma ", " dos ", " das ", " não ", " mais ", " tem ",
                " isso ", "olá", "obrigado", "preciso", "você",
            ],
        ),
        (
            "French",
            &[
                " que ", " les ", " des ", " une ", " est ", " pas ", " pour ", " dans ", " avec ",
                " sur ", "bonjour", "merci", " je ", " nous ",
            ],
        ),
        (
            "German",
            &[
                " und ", " der ", " die ", " das ", " ist ", " nicht ", " ein ", " eine ", " ich ",
                " auf ", " mit ", " für ", " den ", "hallo",
            ],
        ),
        (
            "Italian",
            &[
                " che ", " per ", " con ", " una ", " gli ", " non ", " sono ", " della ", " nel ",
                " questo ", "ciao", "grazie", " io ", " anche ",
            ],
        ),
        (
            "Dutch",
            &[
                " de ", " het ", " een ", " van ", " en ", " niet ", " dat ", " met ", " voor ",
                " zijn ", " ook ", " maar ", "hallo", " ik ",
            ],
        ),
        (
            "Russian",
            &[
                " и ",
                " в ",
                " не ",
                " на ",
                " что ",
                " это ",
                " как ",
                " но ",
                " от ",
                " по ",
                "привет",
                "спасибо",
                " мне ",
                " для ",
            ],
        ),
    ];

    let mut best = "English";
    let mut best_score = 0usize;

    for (lang, words) in languages {
        let score = words.iter().filter(|w| lower.contains(**w)).count();
        if score > best_score {
            best_score = score;
            best = lang;
        }
    }

    // Short messages (≤3 words): 1 match suffices (e.g. "hola", "bonjour").
    // Longer messages: require 2+ to avoid false positives.
    let word_count = lower.split_whitespace().count();
    let threshold = if word_count <= 3 { 1 } else { 2 };
    if best_score >= threshold {
        best
    } else {
        "English"
    }
}

/// Format a UTC timestamp as a relative time string (e.g., "3h ago", "1d ago").
fn format_relative_time(timestamp: &str, now: &chrono::DateTime<chrono::Utc>) -> String {
    let parsed = chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|dt| dt.and_utc());
    match parsed {
        Some(ts) => {
            let diff = *now - ts;
            let minutes = diff.num_minutes();
            if minutes < 60 {
                format!("{minutes}m ago")
            } else if minutes < 1440 {
                format!("{}h ago", minutes / 60)
            } else {
                format!("{}d ago", minutes / 1440)
            }
        }
        None => timestamp.to_string(),
    }
}
