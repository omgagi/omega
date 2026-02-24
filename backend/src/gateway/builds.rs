//! Multi-phase build orchestrator — decomposes build requests into 5 isolated phases.

use super::builds_parse::*;
use super::Gateway;
use omega_core::{
    config::shellexpand,
    context::Context,
    message::IncomingMessage,
};
use omega_memory::audit::{AuditEntry, AuditStatus};
use std::path::PathBuf;
use tracing::warn;

// ---------------------------------------------------------------------------
// Gateway methods
// ---------------------------------------------------------------------------

impl Gateway {
    /// Main build orchestrator — runs 5 sequential phases for a build request.
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

        // Phase 1: Clarification
        self.send_text(incoming, &phase_message(&user_lang, 1, "analyzing"))
            .await;

        let brief_text = match self
            .run_build_phase(
                PHASE_1_PROMPT,
                &incoming.text,
                &self.model_complex,
                Some(vec![]),
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

        // Phase 2: Architecture
        let phase2_prompt = PHASE_2_TEMPLATE
            .replace("{project_dir}", &project_dir_str)
            .replace("{brief_text}", &brief_text);
        if let Err(e) = self
            .run_build_phase(
                &phase2_prompt,
                "Begin architecture design.",
                &self.model_complex,
                None,
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

        // Phase 3: Implementation
        let phase3_prompt = PHASE_3_TEMPLATE.replace("{project_dir}", &project_dir_str);
        if let Err(e) = self
            .run_build_phase(
                &phase3_prompt,
                "Begin implementation.",
                &self.model_fast,
                None,
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
                    "Implementation phase failed: {e}. Partial results at {project_dir_str}"
                ),
            )
            .await;
            return;
        }
        self.send_text(incoming, "Implementation complete \u{2014} verifying...")
            .await;

        // Phase 4: Verification (with one retry loop)
        let phase4_prompt = PHASE_4_TEMPLATE.replace("{project_dir}", &project_dir_str);
        let verification = match self
            .run_build_phase(
                &phase4_prompt,
                "Begin verification.",
                &self.model_fast,
                None,
                None,
            )
            .await
        {
            Ok(text) => parse_verification_result(&text),
            Err(e) => VerificationResult::Fail(e),
        };

        match verification {
            VerificationResult::Pass => {
                self.send_text(incoming, "All checks passed.").await;
            }
            VerificationResult::Fail(reason) => {
                // One retry: re-implement then re-verify.
                self.send_text(incoming, "Verification found issues \u{2014} fixing...")
                    .await;

                let retry_prompt = PHASE_3_RETRY_TEMPLATE
                    .replace("{project_dir}", &project_dir_str)
                    .replace("{failure_reason}", &reason);
                if let Err(e) = self
                    .run_build_phase(
                        &retry_prompt,
                        "Fix the verification issues.",
                        &self.model_fast,
                        None,
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
                            "Failed to fix issues: {e}. Partial results at {project_dir_str}"
                        ),
                    )
                    .await;
                    return;
                }

                let retry_verification = match self
                    .run_build_phase(
                        &phase4_prompt,
                        "Re-verify after fixes.",
                        &self.model_fast,
                        None,
                        None,
                    )
                    .await
                {
                    Ok(text) => parse_verification_result(&text),
                    Err(e) => VerificationResult::Fail(e),
                };

                match retry_verification {
                    VerificationResult::Pass => {
                        self.send_text(incoming, "All checks passed after fixes.")
                            .await;
                    }
                    VerificationResult::Fail(reason) => {
                        if let Some(h) = typing_handle {
                            h.abort();
                        }
                        self.send_text(
                            incoming,
                            &format!(
                                "Build verification failed after retry: {reason}\n\
                                 Partial results at `{project_dir_str}`"
                            ),
                        )
                        .await;
                        self.audit_build(incoming, &brief.name, "failed", &reason)
                            .await;
                        return;
                    }
                }
            }
        }

        // Phase 5: Delivery
        let skills_dir = PathBuf::from(shellexpand(&self.data_dir)).join("skills");
        let skills_dir_str = skills_dir.display().to_string();
        let phase5_prompt = PHASE_5_TEMPLATE
            .replace("{project_dir}", &project_dir_str)
            .replace("{skills_dir}", &skills_dir_str)
            .replace("{project_name}", &brief.name);

        let delivery_text = match self
            .run_build_phase(
                &phase5_prompt,
                "Begin delivery.",
                &self.model_fast,
                None,
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
                         Project is at `{project_dir_str}`"
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
                 Project `{}` is ready at `{}`.",
                brief.name, project_dir_str,
            )
        };

        if let Some(h) = typing_handle {
            h.abort();
        }
        self.send_text(incoming, &final_msg).await;
        self.audit_build(incoming, &brief.name, "success", "").await;
    }

    /// Generic phase runner with retry logic (3 attempts, 2s delay).
    ///
    /// The working directory is communicated to the AI via the system prompt text
    /// (not via OS-level cwd). The provider runs in `~/.omega/workspace/` and the
    /// AI uses the path in the prompt to operate on the correct project directory.
    async fn run_build_phase(
        &self,
        system_prompt: &str,
        user_message: &str,
        model: &str,
        allowed_tools: Option<Vec<String>>,
        max_turns: Option<u32>,
    ) -> Result<String, String> {
        let mut ctx = Context::new(user_message);
        ctx.system_prompt = system_prompt.to_string();
        ctx.model = Some(model.to_string());
        ctx.allowed_tools = allowed_tools;
        if let Some(mt) = max_turns {
            ctx.max_turns = Some(mt);
        }

        for attempt in 1..=3u32 {
            match self.provider.complete(&ctx).await {
                Ok(resp) => return Ok(resp.text),
                Err(e) => {
                    warn!("build phase attempt {attempt}/3 failed: {e}");
                    if attempt < 3 {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                }
            }
        }
        Err("phase failed after 3 attempts".to_string())
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
