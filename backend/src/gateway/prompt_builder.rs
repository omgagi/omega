//! System prompt construction — always injects all context sections.
//!
//! Previous versions gated sections (scheduling, projects, meta, heartbeat)
//! behind keyword matching on the user message. This caused false negatives
//! (missed intent) and false positives (irrelevant context). All sections are
//! now always injected — the token cost is small and reliability wins.

use std::sync::atomic::Ordering;

use omega_core::message::IncomingMessage;

use super::Gateway;
use crate::markers::*;

impl Gateway {
    /// Build the system prompt with all context sections always injected.
    pub(super) fn build_system_prompt(
        &self,
        incoming: &IncomingMessage,
        active_project: Option<&str>,
        projects: &[omega_skills::Project],
    ) -> String {
        let mut prompt = format!(
            "{}\n\n{}\n\n{}",
            self.prompts.identity, self.prompts.soul, self.prompts.system
        );

        prompt.push_str(&format!(
            "\n\nYou are running on provider '{}', model '{}'.",
            self.provider.name(),
            self.model_fast
        ));
        prompt.push_str(&format!(
            "\nCurrent time: {}",
            chrono::Local::now().format("%Y-%m-%d %H:%M %Z")
        ));

        match incoming.channel.as_str() {
            "whatsapp" => prompt.push_str(
                "\n\nPlatform: WhatsApp. Avoid markdown tables and headers — use bold (*text*) and bullet lists instead.",
            ),
            "telegram" => prompt.push_str(
                "\n\nPlatform: Telegram. Markdown is supported (bold, italic, code blocks).",
            ),
            _ => {}
        }

        // Always-on project awareness (compact hint, ~40-50 tokens)
        if !projects.is_empty() {
            let names: Vec<&str> = projects.iter().map(|p| p.name.as_str()).collect();
            let active_note = match active_project {
                Some(ap) => format!(" (active: {ap})"),
                None => String::new(),
            };
            prompt.push_str(&format!(
                "\n\nAvailable projects: [{}]{}. When conversation aligns with a project domain, activate it. For new recurring domains, suggest creating a project (~/.omega/projects/<name>/ROLE.md). User commands: /projects, /project <name>, /project off.",
                names.join(", "),
                active_note,
            ));
        } else {
            prompt.push_str(
                "\n\nNo projects yet. When the user works in a recurring domain (trading, real estate, fitness...), suggest creating a project (~/.omega/projects/<name>/ROLE.md). User commands: /projects, /project <name>, /project off."
            );
        }

        prompt.push_str("\n\n");
        prompt.push_str(&self.prompts.scheduling);
        prompt.push_str("\n\n");
        prompt.push_str(&self.prompts.projects_rules);
        prompt.push_str("\n\n");
        prompt.push_str(&self.prompts.builds);
        prompt.push_str("\n\n");
        prompt.push_str(&self.prompts.meta);

        // Active project ROLE.md — always injected when a project is active
        if let Some(project_name) = active_project {
            if let Some(proj) = projects.iter().find(|p| p.name == project_name) {
                prompt.push_str(&format!(
                    "\n\n---\n\n[Active project: {project_name}]\n{}",
                    proj.instructions
                ));
                // Inject project-declared skills
                if !proj.skills.is_empty() {
                    let project_skills: Vec<_> = self
                        .skills
                        .iter()
                        .filter(|s| proj.skills.contains(&s.name))
                        .collect();
                    if !project_skills.is_empty() {
                        prompt.push_str("\n\n[Project skills]");
                        for s in &project_skills {
                            let status = if s.available {
                                "installed"
                            } else {
                                "not installed"
                            };
                            prompt.push_str(&format!(
                                "\n- {} [{}]: {} → Read {}",
                                s.name,
                                status,
                                s.description,
                                s.path.display()
                            ));
                        }
                    }
                }
            }
        }

        if self.heartbeat_config.enabled {
            let checklist = match active_project {
                Some(proj) => read_project_heartbeat_file(proj),
                None => read_heartbeat_file(),
            };
            if let Some(checklist) = checklist {
                prompt
                    .push_str("\n\nCurrent heartbeat checklist (items monitored periodically):\n");
                prompt.push_str(&checklist);
            }
            let mins = self.heartbeat_interval.load(Ordering::Relaxed);
            prompt.push_str(&format!(
                "\n\nHeartbeat pulse: every {mins} minutes. You can report this when asked and change it with HEARTBEAT_INTERVAL: <1-1440>."
            ));
        }

        prompt
    }
}
