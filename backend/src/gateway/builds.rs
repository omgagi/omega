//! Multi-phase build orchestrator — decomposes build requests into 7 isolated phases
//! using purpose-built agents via `claude --agent <name>`.
//!
//! Phase pipeline:
//! 1. Analyst    — analyze request, produce ProjectBrief
//! 2. Architect  — design architecture, create specs/
//! 3. Test Writer — write failing tests (TDD red)
//! 4. Developer  — implement to pass tests (TDD green)
//! 5. QA         — validate quality (up to 3 iterations, fatal)
//! 6. Reviewer   — audit code (up to 2 iterations, fatal)
//! 7. Delivery   — docs, SKILL.md, build summary
//!
//! Safety controls:
//! - Inter-step validation before phases 3, 4, 5
//! - QA loop: 3 iterations (QA → developer fix → re-QA)
//! - Review loop: 2 iterations (review → developer fix → re-review)
//! - Chain state persisted on failure for recovery inspection

use super::builds_agents::AgentFilesGuard;
use super::builds_parse::*;
use super::Gateway;
use omega_core::{config::shellexpand, context::Context, message::IncomingMessage};
use omega_memory::audit::{AuditEntry, AuditStatus};
use std::path::PathBuf;
use tracing::warn;

// ---------------------------------------------------------------------------
// Gateway methods
// ---------------------------------------------------------------------------

impl Gateway {
    /// Main build orchestrator — runs 7 sequential agent phases for a build request.
    pub(super) async fn handle_build_request(
        &self,
        incoming: &IncomingMessage,
        typing_handle: Option<tokio::task::JoinHandle<()>>,
    ) {
        // Resolve user language for localized messages.
        let user_lang = self
            .memory
            .get_fact(&incoming.sender_id, "preferred_language")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "English".to_string());

        // Write agent files to workspace root BEFORE any phase runs.
        // The CLI subprocess runs with cwd = ~/.omega/workspace/, so agent files
        // must be at ~/.omega/workspace/.claude/agents/ for --agent discovery.
        let workspace_dir = PathBuf::from(shellexpand(&self.data_dir)).join("workspace");
        let _agent_guard = match AgentFilesGuard::write(&workspace_dir).await {
            Ok(guard) => guard,
            Err(e) => {
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(incoming, &format!("Failed to write agent files: {e}"))
                    .await;
                return;
            }
        };

        // Phase 1: Analyst — analyze build request and produce a brief.
        self.send_text(incoming, &phase_message(&user_lang, 1, "analyzing"))
            .await;

        let brief_text = match self
            .run_build_phase(
                "build-analyst",
                &incoming.text,
                &self.model_complex,
                Some(25),
            )
            .await
        {
            Ok(text) => text,
            Err(e) => {
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(
                    incoming,
                    &format!("Could not analyze your build request: {e}"),
                )
                .await;
                return;
            }
        };

        // Phase 1 output is parsed below — no need to log in production.

        let brief = match parse_project_brief(&brief_text) {
            Some(b) => b,
            None => {
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(
                    incoming,
                    "Could not parse the build brief. Please try rephrasing.",
                )
                .await;
                return;
            }
        };

        let project_dir = PathBuf::from(shellexpand(&self.data_dir))
            .join("workspace/builds")
            .join(&brief.name);
        let project_dir_str = project_dir.display().to_string();

        if let Err(e) = tokio::fs::create_dir_all(&project_dir).await {
            if let Some(h) = typing_handle {
                h.abort();
            }
            self.send_text(
                incoming,
                &format!("Failed to create project directory: {e}"),
            )
            .await;
            return;
        }

        self.send_text(
            incoming,
            &format!(
                "Building `{}` \u{2014} {}. I'll keep you posted.",
                brief.name, brief.scope
            ),
        )
        .await;

        // Phase 2: Architect — design architecture and create specs.
        self.send_text(incoming, &phase_message(&user_lang, 2, "designing"))
            .await;

        let architect_prompt = format!(
            "Project brief:\n{brief_text}\nBegin architecture design in {project_dir_str}."
        );
        if let Err(e) = self
            .run_build_phase(
                "build-architect",
                &architect_prompt,
                &self.model_complex,
                None,
            )
            .await
        {
            if let Some(h) = typing_handle {
                h.abort();
            }
            self.send_text(incoming, &format!("Architecture phase failed: {e}"))
                .await;
            return;
        }

        // Verify specs/architecture.md was created.
        let arch_file = project_dir.join("specs/architecture.md");
        if !arch_file.exists() {
            if let Some(h) = typing_handle {
                h.abort();
            }
            self.send_text(
                incoming,
                "Architecture phase completed but no specs were generated. Build stopped.",
            )
            .await;
            return;
        }

        self.send_text(incoming, "Architecture defined.").await;

        // Phase 3: Test Writer — write failing tests (TDD red phase).
        if let Some(err) = Gateway::validate_phase_output(&project_dir, "test-writer") {
            if let Some(h) = typing_handle {
                h.abort();
            }
            self.send_text(incoming, &err).await;
            let cs = Self::chain_state(
                &brief.name,
                &project_dir_str,
                &["analyst", "architect"],
                "test-writer (validation)",
                err,
            );
            Gateway::save_chain_state(&project_dir, &cs).await;
            return;
        }

        self.send_text(incoming, &phase_message(&user_lang, 3, "testing"))
            .await;

        let test_writer_prompt =
            format!("Read specs/ in {project_dir_str} and write failing tests. Begin.");
        if let Err(e) = self
            .run_build_phase(
                "build-test-writer",
                &test_writer_prompt,
                &self.model_complex,
                None,
            )
            .await
        {
            if let Some(h) = typing_handle {
                h.abort();
            }
            self.send_text(
                incoming,
                &format!(
                    "Test writing phase failed: {e}. Partial results in `{}`.",
                    brief.name
                ),
            )
            .await;
            return;
        }

        self.send_text(incoming, "Tests written.").await;

        // Phase 4: Developer — implement to pass tests (TDD green phase).
        if let Some(err) = Gateway::validate_phase_output(&project_dir, "developer") {
            if let Some(h) = typing_handle {
                h.abort();
            }
            self.send_text(incoming, &err).await;
            let cs = Self::chain_state(
                &brief.name,
                &project_dir_str,
                &["analyst", "architect", "test-writer"],
                "developer (validation)",
                err,
            );
            Gateway::save_chain_state(&project_dir, &cs).await;
            return;
        }

        self.send_text(incoming, &phase_message(&user_lang, 4, "implementing"))
            .await;

        let developer_prompt = format!(
            "Read the tests and specs/ in {project_dir_str}. Implement until all tests pass. Begin."
        );
        if let Err(e) = self
            .run_build_phase(
                "build-developer",
                &developer_prompt,
                &self.model_complex,
                None,
            )
            .await
        {
            if let Some(h) = typing_handle {
                h.abort();
            }
            self.send_text(
                incoming,
                &format!(
                    "Implementation phase failed: {e}. Partial results in `{}`.",
                    brief.name
                ),
            )
            .await;
            return;
        }
        self.send_text(incoming, "Implementation complete \u{2014} verifying...")
            .await;

        // Phase 5: QA — validate quality (up to 3 iterations).
        if let Some(err) = Gateway::validate_phase_output(&project_dir, "qa") {
            if let Some(h) = typing_handle {
                h.abort();
            }
            self.send_text(incoming, &err).await;
            let cs = Self::chain_state(
                &brief.name,
                &project_dir_str,
                &["analyst", "architect", "test-writer", "developer"],
                "qa (validation)",
                err,
            );
            Gateway::save_chain_state(&project_dir, &cs).await;
            return;
        }

        self.send_text(incoming, &phase_message(&user_lang, 5, "validating"))
            .await;

        match self
            .run_qa_loop(incoming, &project_dir_str, &user_lang)
            .await
        {
            Ok(()) => {}
            Err(reason) => {
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(
                    incoming,
                    &qa_exhausted_message(&user_lang, &reason, &project_dir_str),
                )
                .await;
                self.audit_build(incoming, &brief.name, "failed", &reason)
                    .await;
                let cs = Self::chain_state(
                    &brief.name,
                    &project_dir_str,
                    &["analyst", "architect", "test-writer", "developer"],
                    "qa",
                    reason,
                );
                Gateway::save_chain_state(&project_dir, &cs).await;
                return;
            }
        }

        // Phase 6: Reviewer — code review (up to 2 iterations, fatal).
        self.send_text(incoming, &phase_message(&user_lang, 6, "reviewing"))
            .await;

        match self
            .run_review_loop(incoming, &project_dir_str, &user_lang)
            .await
        {
            Ok(()) => {}
            Err(reason) => {
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(
                    incoming,
                    &review_exhausted_message(&user_lang, &reason, &project_dir_str),
                )
                .await;
                self.audit_build(incoming, &brief.name, "failed", &reason)
                    .await;
                let cs = Self::chain_state(
                    &brief.name,
                    &project_dir_str,
                    &["analyst", "architect", "test-writer", "developer", "qa"],
                    "reviewer",
                    reason,
                );
                Gateway::save_chain_state(&project_dir, &cs).await;
                return;
            }
        }

        // Phase 7: Delivery — docs, SKILL.md, build summary.
        self.send_text(incoming, &phase_message(&user_lang, 7, "delivering"))
            .await;

        let skills_dir = PathBuf::from(shellexpand(&self.data_dir)).join("skills");
        let skills_dir_str = skills_dir.display().to_string();
        let delivery_prompt = format!(
            "Create docs and skill file for {} in {project_dir_str}. Skills dir: {skills_dir_str}.",
            brief.name
        );

        let delivery_text = match self
            .run_build_phase(
                "build-delivery",
                &delivery_prompt,
                &self.model_complex,
                None,
            )
            .await
        {
            Ok(text) => text,
            Err(e) => {
                // Build succeeded but delivery failed — still report success.
                if let Some(h) = typing_handle {
                    h.abort();
                }
                self.send_text(
                    incoming,
                    &format!(
                        "Build complete but delivery had issues: {e}\n\
                         Project: `{}`",
                        brief.name
                    ),
                )
                .await;
                self.audit_build(incoming, &brief.name, "partial", &e).await;
                return;
            }
        };

        // Parse and send final summary.
        let final_msg = if let Some(summary) = parse_build_summary(&delivery_text) {
            format!(
                "Build complete!\n\n\
                 *{}*\n\
                 {}\n\n\
                 Location: `{}`\n\
                 Language: {}\n\
                 Usage: `{}`{}",
                summary.project,
                summary.summary,
                summary.location,
                summary.language,
                summary.usage,
                summary
                    .skill
                    .as_ref()
                    .map(|s| format!("\nSkill: {s}"))
                    .unwrap_or_default(),
            )
        } else {
            format!(
                "Build complete!\n\n\
                 Project `{}` is ready.",
                brief.name,
            )
        };

        if let Some(h) = typing_handle {
            h.abort();
        }
        self.send_text(incoming, &final_msg).await;
        self.audit_build(incoming, &brief.name, "success", "").await;
    }

    /// Build a `ChainState` for the current project at a given failure point.
    fn chain_state(
        name: &str,
        dir: &str,
        completed: &[&str],
        failed: &str,
        reason: String,
    ) -> ChainState {
        ChainState {
            project_name: name.to_string(),
            project_dir: dir.to_string(),
            completed_phases: completed.iter().map(|s| (*s).to_string()).collect(),
            failed_phase: Some(failed.to_string()),
            failure_reason: Some(reason),
        }
    }

    /// Generic phase runner with retry logic (3 attempts, 2s delay).
    ///
    /// Each phase gets a fresh Context with `agent_name` set and no session_id.
    /// The agent file provides the system prompt; only the user message is sent via `-p`.
    pub(super) async fn run_build_phase(
        &self,
        agent_name: &str,
        user_message: &str,
        model: &str,
        max_turns: Option<u32>,
    ) -> Result<String, String> {
        let mut ctx = Context::new(user_message);
        ctx.system_prompt = String::new();
        ctx.agent_name = Some(agent_name.to_string());
        ctx.model = Some(model.to_string());
        // Explicit max_turns prevents auto-resume from losing agent context.
        ctx.max_turns = Some(max_turns.unwrap_or(100));

        for attempt in 1..=3u32 {
            match self.provider.complete(&ctx).await {
                Ok(resp) => return Ok(resp.text),
                Err(e) => {
                    warn!("build phase '{agent_name}' attempt {attempt}/3 failed: {e}");
                    if attempt < 3 {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }
        Err(format!("phase '{agent_name}' failed after 3 attempts"))
    }

    /// Log an audit entry for a build operation.
    async fn audit_build(
        &self,
        incoming: &IncomingMessage,
        project: &str,
        status: &str,
        detail: &str,
    ) {
        let _ = self
            .audit
            .log(&AuditEntry {
                channel: incoming.channel.clone(),
                sender_id: incoming.sender_id.clone(),
                sender_name: incoming.sender_name.clone(),
                input_text: format!("[BUILD:{project}] {}", incoming.text),
                output_text: Some(format!("[{status}] {detail}")),
                provider_used: Some(self.provider.name().to_string()),
                model: None,
                processing_ms: None,
                status: if status == "success" {
                    AuditStatus::Ok
                } else {
                    AuditStatus::Error
                },
                denial_reason: None,
            })
            .await;
    }
}
