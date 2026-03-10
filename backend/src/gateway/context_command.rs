//! `/context` command handler — shows the assembled system prompt sections.
//!
//! Intercepted in pipeline.rs (like `/setup` and `/google`) because it needs
//! access to `Gateway` fields (`prompts`, `provider`, `projects`, etc.) to
//! build the prompt.

use omega_core::message::IncomingMessage;
use tracing::info;

use super::Gateway;
use crate::i18n;

impl Gateway {
    /// Handle the `/context` command: build the system prompt and show a section breakdown.
    pub(super) async fn handle_context_command(
        &self,
        incoming: &IncomingMessage,
        active_project: Option<&str>,
    ) {
        let lang = self
            .memory
            .get_fact(&incoming.sender_id, "preferred_language")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "English".to_string());

        let projects = &omega_skills::load_projects(&self.data_dir);

        // Parse optional argument: `/context full` shows the raw prompt.
        let arg = incoming
            .text
            .split_whitespace()
            .nth(1)
            .unwrap_or("")
            .to_lowercase();

        let show_full = arg == "full";

        // Build the prompt exactly as the pipeline would (all sections always injected).
        let prompt = self.build_system_prompt(incoming, active_project, projects);

        if show_full {
            // Send the raw prompt (split if needed for Telegram's 4096 char limit).
            let chunks = split_message(&prompt, 4000);
            for chunk in chunks {
                self.send_text(incoming, &chunk).await;
            }
            return;
        }

        // Build structured summary — all sections are always ON.
        let mut sections = Vec::new();

        let identity_chars = self.prompts.identity.len();
        let soul_chars = self.prompts.soul.len();
        let system_chars = self.prompts.system.len();

        let on = i18n::t("context_on", &lang);

        sections.push(format!("[{on}] Identity ({identity_chars} chars)"));
        sections.push(format!("[{on}] Soul ({soul_chars} chars)"));
        sections.push(format!("[{on}] System ({system_chars} chars)"));

        sections.push(format!(
            "[{on}] Scheduling ({} chars)",
            self.prompts.scheduling.len()
        ));

        sections.push(format!(
            "[{on}] Project rules ({} chars)",
            self.prompts.projects_rules.len()
        ));

        sections.push(format!(
            "[{on}] Builds ({} chars)",
            self.prompts.builds.len()
        ));

        sections.push(format!("[{on}] Meta ({} chars)", self.prompts.meta.len()));

        if let Some(project_name) = active_project {
            if let Some(proj) = projects.iter().find(|p| p.name == project_name) {
                sections.push(format!(
                    "[{on}] ROLE.md: {project_name} ({} chars)",
                    proj.instructions.len()
                ));
            }
        }

        let total_chars = prompt.len();
        let total_tokens = total_chars / 4;

        let sections_text = sections.join("\n  ");
        let response = format!(
            "{}\n\n  {sections_text}\n\n{} ~{total_tokens} (~{total_chars} chars)\n\n{}",
            i18n::t("context_header", &lang),
            i18n::t("context_total", &lang),
            i18n::t("context_tip", &lang),
        );

        info!(
            "[{}] /context: {} chars, ~{} tokens",
            incoming.channel, total_chars, total_tokens
        );

        self.send_text(incoming, &response).await;
    }
}

/// Split a long message into chunks that fit within `max_chars`.
fn split_message(text: &str, max_chars: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }
    let mut chunks = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        if remaining.len() <= max_chars {
            chunks.push(remaining.to_string());
            break;
        }
        // Try to split at a newline boundary.
        let split_at = remaining[..max_chars].rfind('\n').unwrap_or(max_chars);
        chunks.push(remaining[..split_at].to_string());
        remaining = remaining[split_at..].trim_start_matches('\n');
    }
    chunks
}
