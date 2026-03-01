//! Build pipeline iteration loops and inter-step validation.
//!
//! Contains the unified corrective loop (replaces separate QA/reviewer loops),
//! topology-driven pre-phase validation, legacy phase validation for backward
//! compatibility, and chain state persistence for failure recovery.

use super::builds_parse::*;
use super::builds_topology::{Phase, RetryConfig, ValidationConfig, ValidationType};
use super::Gateway;
use omega_core::message::IncomingMessage;
use std::path::Path;
use tracing::warn;

impl Gateway {
    /// Run a topology-driven corrective loop: verify -> fix -> re-verify.
    ///
    /// Parameters come from the topology's RetryConfig for the phase.
    /// The verify agent is the phase's own agent. The fix agent comes from retry config.
    /// Dispatches to QA or reviewer parsing based on the phase name.
    pub(super) async fn run_corrective_loop(
        &self,
        incoming: &IncomingMessage,
        state: &super::builds::OrchestratorState,
        user_lang: &str,
        phase: &Phase,
        retry: &RetryConfig,
        model: &str,
    ) -> Result<(), String> {
        let project_dir_str = state.project_dir_str.as_deref().unwrap_or("");
        // TODO(phase-2): add loop_style field to RetryConfig instead of name-based dispatch
        // NOTE: Tracked as P2-26 — phase-2 i18n not yet implemented, English-only fallback used.
        //       Currently dispatches QA vs review by checking phase.name == "qa"; should use
        //       an explicit loop_style enum on RetryConfig (e.g. LoopStyle::Qa / LoopStyle::Review).
        let is_qa = phase.name == "qa";

        let verify_prompt = if is_qa {
            format!(
                "Validate the project in {project_dir_str}. Run build, lint, tests. \
                 Report VERIFICATION: PASS or FAIL."
            )
        } else {
            format!(
                "Review the code in {project_dir_str} for bugs, security, quality. \
                 Report REVIEW: PASS or FAIL."
            )
        };

        for attempt in 1..=retry.max {
            let verification = match self
                .run_build_phase(&phase.agent, &verify_prompt, model, phase.max_turns)
                .await
            {
                Ok(text) => {
                    if is_qa {
                        match parse_verification_result(&text) {
                            VerificationResult::Pass => Ok(()),
                            VerificationResult::Fail(r) => Err(r),
                        }
                    } else {
                        match parse_review_result(&text) {
                            ReviewResult::Pass => Ok(()),
                            ReviewResult::Fail(r) => Err(r),
                        }
                    }
                }
                Err(e) => Err(e),
            };

            match verification {
                Ok(()) => {
                    // Send pass message.
                    let msg = if is_qa {
                        qa_pass_message(user_lang, attempt)
                    } else {
                        review_pass_message(user_lang, attempt)
                    };
                    self.send_text(incoming, &msg).await;
                    return Ok(());
                }
                Err(reason) => {
                    if attempt < retry.max {
                        // Send retry message.
                        if is_qa {
                            self.send_text(
                                incoming,
                                &qa_retry_message(user_lang, attempt, &reason),
                            )
                            .await;
                        } else {
                            self.send_text(incoming, &review_retry_message(user_lang, &reason))
                                .await;
                        }

                        // Invoke fix agent.
                        let fix_prompt = if is_qa {
                            format!(
                                "Read the tests and specs/ in {project_dir_str}. \
                                 The QA verification (attempt {attempt}/{}) found these issues:\n\
                                 {reason}\n\
                                 Fix the issues and ensure all tests pass. Begin.",
                                retry.max
                            )
                        } else {
                            format!(
                                "Read the code in {project_dir_str}. \
                                 The code review found these issues:\n{reason}\n\
                                 Fix only the issues listed above. Begin."
                            )
                        };

                        if let Err(e) = self
                            .run_build_phase(&retry.fix_agent, &fix_prompt, model, None)
                            .await
                        {
                            let label = if is_qa {
                                format!("Developer fix failed on attempt {attempt}: {e}")
                            } else {
                                format!("Developer fix for review failed: {e}")
                            };
                            return Err(label);
                        }
                    } else {
                        return Err(reason);
                    }
                }
            }
        }
        Err("loop terminated without resolution".to_string())
    }

    /// Run the QA verification loop — up to 3 iterations of QA + developer fix.
    ///
    /// Kept for backward compatibility — production code uses run_corrective_loop().
    #[allow(dead_code)]
    pub(super) async fn run_qa_loop(
        &self,
        incoming: &IncomingMessage,
        project_dir_str: &str,
        user_lang: &str,
    ) -> Result<(), String> {
        let qa_prompt = format!(
            "Validate the project in {project_dir_str}. Run build, lint, tests. \
             Report VERIFICATION: PASS or FAIL."
        );

        for attempt in 1..=3u32 {
            let verification = match self
                .run_build_phase("build-qa", &qa_prompt, &self.model_complex, None)
                .await
            {
                Ok(text) => parse_verification_result(&text),
                Err(e) => VerificationResult::Fail(e),
            };

            match verification {
                VerificationResult::Pass => {
                    self.send_text(incoming, &qa_pass_message(user_lang, attempt))
                        .await;
                    return Ok(());
                }
                VerificationResult::Fail(reason) => {
                    if attempt < 3 {
                        self.send_text(incoming, &qa_retry_message(user_lang, attempt, &reason))
                            .await;

                        // Re-invoke developer with failure context.
                        let fix_prompt = format!(
                            "Read the tests and specs/ in {project_dir_str}. \
                             The QA verification (attempt {attempt}/3) found these issues:\n\
                             {reason}\n\
                             Fix the issues and ensure all tests pass. Begin."
                        );
                        if let Err(e) = self
                            .run_build_phase(
                                "build-developer",
                                &fix_prompt,
                                &self.model_complex,
                                None,
                            )
                            .await
                        {
                            return Err(format!("Developer fix failed on attempt {attempt}: {e}"));
                        }
                    } else {
                        return Err(reason);
                    }
                }
            }
        }
        Err("loop terminated without resolution".to_string())
    }

    /// Run the code review loop — up to 2 iterations of review + developer fix.
    ///
    /// Kept for backward compatibility — production code uses run_corrective_loop().
    #[allow(dead_code)]
    pub(super) async fn run_review_loop(
        &self,
        incoming: &IncomingMessage,
        project_dir_str: &str,
        user_lang: &str,
    ) -> Result<(), String> {
        let reviewer_prompt = format!(
            "Review the code in {project_dir_str} for bugs, security, quality. \
             Report REVIEW: PASS or FAIL."
        );

        for attempt in 1..=2u32 {
            let review = match self
                .run_build_phase(
                    "build-reviewer",
                    &reviewer_prompt,
                    &self.model_complex,
                    None,
                )
                .await
            {
                Ok(text) => parse_review_result(&text),
                Err(e) => ReviewResult::Fail(e),
            };

            match review {
                ReviewResult::Pass => {
                    self.send_text(incoming, &review_pass_message(user_lang, attempt))
                        .await;
                    return Ok(());
                }
                ReviewResult::Fail(reason) => {
                    if attempt < 2 {
                        self.send_text(incoming, &review_retry_message(user_lang, &reason))
                            .await;

                        // Re-invoke developer scoped to review findings.
                        let fix_prompt = format!(
                            "Read the code in {project_dir_str}. \
                             The code review found these issues:\n{reason}\n\
                             Fix only the issues listed above. Begin."
                        );
                        if let Err(e) = self
                            .run_build_phase(
                                "build-developer",
                                &fix_prompt,
                                &self.model_complex,
                                None,
                            )
                            .await
                        {
                            return Err(format!("Developer fix for review failed: {e}"));
                        }
                    } else {
                        return Err(reason);
                    }
                }
            }
        }
        Err("loop terminated without resolution".to_string())
    }

    /// Maximum recursion depth for filesystem traversal.
    const MAX_SCAN_DEPTH: u32 = 10;

    /// Validate that a previous phase produced expected output before the next phase runs.
    ///
    /// Returns `Some(error_message)` if validation fails, `None` if OK.
    /// Kept for backward compatibility — production code uses run_validation().
    #[allow(dead_code)]
    pub(super) fn validate_phase_output(project_dir: &Path, next_phase: &str) -> Option<String> {
        match next_phase {
            "test-writer" => {
                // Before test-writer: specs/architecture.md must exist.
                if !project_dir.join("specs/architecture.md").exists() {
                    Some("Cannot start test-writer: specs/architecture.md not found".to_string())
                } else {
                    None
                }
            }
            "developer" => {
                // Before developer: at least one test file should exist.
                let has_tests =
                    Self::has_files_matching(project_dir, &["test", "spec", "_test."], 0);
                if !has_tests {
                    Some("Cannot start developer: no test files found in project".to_string())
                } else {
                    None
                }
            }
            "qa" => {
                // Before QA: at least one source file should exist.
                let has_sources = Self::has_files_matching(
                    project_dir,
                    &[
                        ".rs", ".py", ".js", ".ts", ".go", ".java", ".rb", ".c", ".cpp",
                    ],
                    0,
                );
                if !has_sources {
                    Some("Cannot start QA: no source files found in project".to_string())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Check if any file in the directory tree contains one of the given substrings in its name.
    ///
    /// Stops recursing at `MAX_SCAN_DEPTH` to prevent stack overflow. Skips symlinks
    /// to prevent infinite loops from symlink cycles.
    fn has_files_matching(dir: &Path, patterns: &[&str], depth: u32) -> bool {
        if depth >= Self::MAX_SCAN_DEPTH {
            return false;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return false;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let path = entry.path();
            // Skip symlinks to prevent infinite loops from cycles.
            if path.is_symlink() {
                continue;
            }
            if path.is_dir() {
                // Skip hidden directories and common non-source dirs.
                if !name.starts_with('.')
                    && name != "node_modules"
                    && name != "target"
                    && Self::has_files_matching(&path, patterns, depth + 1)
                {
                    return true;
                }
            } else if patterns.iter().any(|p| name.contains(p)) {
                return true;
            }
        }
        false
    }

    /// Run pre-phase validation from topology config.
    /// Returns Some(error_message) on failure, None on success.
    ///
    /// REQ-TOP-007: Parameterized pre/post-phase validation from topology config.
    pub(super) fn run_validation(project_dir: &Path, config: &ValidationConfig) -> Option<String> {
        match config.validation_type {
            ValidationType::FileExists => {
                for path in &config.paths {
                    if !project_dir.join(path).exists() {
                        return Some(format!(
                            "Pre-validation failed: required file '{path}' not found"
                        ));
                    }
                }
                None
            }
            ValidationType::FilePatterns => {
                let has_match = Self::has_files_matching(
                    project_dir,
                    &config
                        .patterns
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>(),
                    0,
                );
                if !has_match {
                    return Some(format!(
                        "Pre-validation failed: no files matching patterns {:?}",
                        config.patterns
                    ));
                }
                None
            }
        }
    }

    /// Save chain state to `docs/.workflow/chain-state.md` in the project directory.
    ///
    /// Best-effort — logs a warning on I/O error but does not propagate.
    pub(super) async fn save_chain_state(project_dir: &Path, state: &ChainState) {
        let workflow_dir = project_dir.join("docs").join(".workflow");
        if let Err(e) = tokio::fs::create_dir_all(&workflow_dir).await {
            warn!("Failed to create chain-state directory: {e}");
            return;
        }

        let completed = if state.completed_phases.is_empty() {
            "  (none)".to_string()
        } else {
            state
                .completed_phases
                .iter()
                .map(|p| format!("  - {p}"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let failed = state.failed_phase.as_deref().unwrap_or("(none)");
        let reason = state.failure_reason.as_deref().unwrap_or("(none)");
        let topology = state.topology_name.as_deref().unwrap_or("(none)");

        let content = format!(
            "# Chain State — {}\n\n\
             Project: {}\n\
             Directory: {}\n\
             Topology: {topology}\n\n\
             ## Completed Phases\n{completed}\n\n\
             ## Failed Phase\n{failed}\n\n\
             ## Failure Reason\n{reason}\n",
            state.project_name, state.project_name, state.project_dir
        );

        let path = workflow_dir.join("chain-state.md");
        if let Err(e) = tokio::fs::write(&path, content).await {
            warn!("Failed to write chain-state: {e}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_validate_phase_output_test_writer_needs_specs() {
        let dir = PathBuf::from("/nonexistent/project");
        let result = Gateway::validate_phase_output(&dir, "test-writer");
        assert!(result.is_some());
        assert!(result.unwrap().contains("specs/architecture.md"));
    }

    #[test]
    fn test_validate_phase_output_unknown_phase_passes() {
        let dir = PathBuf::from("/nonexistent/project");
        let result = Gateway::validate_phase_output(&dir, "delivery");
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_save_chain_state_writes_file() {
        let tmp = tempfile::tempdir().unwrap();
        let state = ChainState {
            project_name: "test-proj".to_string(),
            project_dir: tmp.path().display().to_string(),
            completed_phases: vec!["analyst".to_string(), "architect".to_string()],
            failed_phase: Some("qa".to_string()),
            failure_reason: Some("3 tests failing".to_string()),
            topology_name: None,
        };
        Gateway::save_chain_state(tmp.path(), &state).await;

        let path = tmp.path().join("docs/.workflow/chain-state.md");
        assert!(path.exists(), "chain-state.md should be created");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("test-proj"));
        assert!(content.contains("analyst"));
        assert!(content.contains("architect"));
        assert!(content.contains("qa"));
        assert!(content.contains("3 tests failing"));
    }

    #[test]
    fn test_validate_phase_output_developer_needs_tests() {
        let dir = PathBuf::from("/nonexistent/project");
        let result = Gateway::validate_phase_output(&dir, "developer");
        assert!(result.is_some());
        assert!(result.unwrap().contains("no test files"));
    }

    #[test]
    fn test_validate_phase_output_qa_needs_sources() {
        let dir = PathBuf::from("/nonexistent/project");
        let result = Gateway::validate_phase_output(&dir, "qa");
        assert!(result.is_some());
        assert!(result.unwrap().contains("no source files"));
    }

    #[test]
    fn test_has_files_matching_finds_test_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("test_main.rs"), "// test").unwrap();
        assert!(Gateway::has_files_matching(tmp.path(), &["test"], 0));
    }

    #[test]
    fn test_has_files_matching_recurses_into_subdirs() {
        let tmp = tempfile::tempdir().unwrap();
        let sub = tmp.path().join("src");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("lib.rs"), "// code").unwrap();
        assert!(Gateway::has_files_matching(tmp.path(), &[".rs"], 0));
    }

    #[test]
    fn test_has_files_matching_skips_hidden_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let hidden = tmp.path().join(".hidden");
        std::fs::create_dir_all(&hidden).unwrap();
        std::fs::write(hidden.join("test.rs"), "// test").unwrap();
        assert!(!Gateway::has_files_matching(tmp.path(), &["test"], 0));
    }

    // ===================================================================
    // REQ-TOP-007 (Must): run_validation() — topology-driven pre-phase validation
    // ===================================================================

    #[test]
    fn test_run_validation_file_exists_passes() {
        let tmp = tempfile::tempdir().unwrap();
        let specs_dir = tmp.path().join("specs");
        std::fs::create_dir_all(&specs_dir).unwrap();
        std::fs::write(specs_dir.join("architecture.md"), "# Architecture").unwrap();

        let config = ValidationConfig {
            validation_type: ValidationType::FileExists,
            paths: vec!["specs/architecture.md".to_string()],
            patterns: vec![],
        };

        let result = Gateway::run_validation(tmp.path(), &config);
        assert!(
            result.is_none(),
            "FileExists validation should pass when file exists: got {:?}",
            result
        );
    }

    #[test]
    fn test_run_validation_file_exists_fails() {
        let tmp = tempfile::tempdir().unwrap();

        let config = ValidationConfig {
            validation_type: ValidationType::FileExists,
            paths: vec!["specs/architecture.md".to_string()],
            patterns: vec![],
        };

        let result = Gateway::run_validation(tmp.path(), &config);
        assert!(
            result.is_some(),
            "FileExists validation should fail when file is missing"
        );
        let err = result.unwrap();
        assert!(
            err.contains("specs/architecture.md"),
            "Error should name the missing file: got '{err}'"
        );
    }

    #[test]
    fn test_run_validation_file_exists_multiple_paths() {
        let tmp = tempfile::tempdir().unwrap();
        let specs_dir = tmp.path().join("specs");
        std::fs::create_dir_all(&specs_dir).unwrap();
        std::fs::write(specs_dir.join("architecture.md"), "# Arch").unwrap();

        let config = ValidationConfig {
            validation_type: ValidationType::FileExists,
            paths: vec![
                "specs/architecture.md".to_string(),
                "specs/requirements.md".to_string(),
            ],
            patterns: vec![],
        };

        let result = Gateway::run_validation(tmp.path(), &config);
        assert!(
            result.is_some(),
            "Should fail when any required file is missing"
        );
        assert!(result.unwrap().contains("requirements.md"));
    }

    #[test]
    fn test_run_validation_file_patterns_passes() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("test_main.rs"), "// test").unwrap();

        let config = ValidationConfig {
            validation_type: ValidationType::FilePatterns,
            paths: vec![],
            patterns: vec!["test".to_string(), "spec".to_string(), "_test.".to_string()],
        };

        let result = Gateway::run_validation(tmp.path(), &config);
        assert!(
            result.is_none(),
            "FilePatterns should pass when matching files exist: got {:?}",
            result
        );
    }

    #[test]
    fn test_run_validation_file_patterns_fails() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("readme.md"), "# Readme").unwrap();

        let config = ValidationConfig {
            validation_type: ValidationType::FilePatterns,
            paths: vec![],
            patterns: vec!["test".to_string(), "spec".to_string()],
        };

        let result = Gateway::run_validation(tmp.path(), &config);
        assert!(
            result.is_some(),
            "FilePatterns should fail when no matching files exist"
        );
        let err = result.unwrap();
        assert!(
            err.contains("no files matching"),
            "Error should mention 'no files matching': got '{err}'"
        );
    }

    #[test]
    fn test_run_validation_file_patterns_recursive() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("lib.rs"), "// code").unwrap();

        let config = ValidationConfig {
            validation_type: ValidationType::FilePatterns,
            paths: vec![],
            patterns: vec![".rs".to_string()],
        };

        let result = Gateway::run_validation(tmp.path(), &config);
        assert!(
            result.is_none(),
            "FilePatterns should find files in subdirectories"
        );
    }

    #[test]
    fn test_run_validation_parity_test_writer() {
        let tmp = tempfile::tempdir().unwrap();

        let config = ValidationConfig {
            validation_type: ValidationType::FileExists,
            paths: vec!["specs/architecture.md".to_string()],
            patterns: vec![],
        };

        let old_result = Gateway::validate_phase_output(tmp.path(), "test-writer");
        let new_result = Gateway::run_validation(tmp.path(), &config);

        assert!(
            old_result.is_some() && new_result.is_some(),
            "Both old and new validation should fail on missing specs/architecture.md"
        );
    }

    #[test]
    fn test_run_validation_parity_developer() {
        let tmp = tempfile::tempdir().unwrap();

        let config = ValidationConfig {
            validation_type: ValidationType::FilePatterns,
            paths: vec![],
            patterns: vec!["test".to_string(), "spec".to_string(), "_test.".to_string()],
        };

        let old_result = Gateway::validate_phase_output(tmp.path(), "developer");
        let new_result = Gateway::run_validation(tmp.path(), &config);

        assert!(
            old_result.is_some() && new_result.is_some(),
            "Both old and new validation should fail on missing test files"
        );
    }

    #[test]
    fn test_run_validation_file_exists_empty_paths() {
        let tmp = tempfile::tempdir().unwrap();

        let config = ValidationConfig {
            validation_type: ValidationType::FileExists,
            paths: vec![],
            patterns: vec![],
        };

        let result = Gateway::run_validation(tmp.path(), &config);
        assert!(
            result.is_none(),
            "Empty paths list should pass validation (no requirements)"
        );
    }

    #[test]
    fn test_run_validation_file_patterns_empty_patterns() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("file.rs"), "code").unwrap();

        let config = ValidationConfig {
            validation_type: ValidationType::FilePatterns,
            paths: vec![],
            patterns: vec![],
        };

        let result = Gateway::run_validation(tmp.path(), &config);
        assert!(
            result.is_some(),
            "Empty patterns list should fail validation (nothing to match)"
        );
    }

    // ===================================================================
    // REQ-TOP-015 (Could): Chain state includes topology_name field
    // ===================================================================

    #[tokio::test]
    async fn test_chain_state_includes_topology_name() {
        let tmp = tempfile::tempdir().unwrap();
        let state = ChainState {
            project_name: "topo-test".to_string(),
            project_dir: tmp.path().display().to_string(),
            completed_phases: vec!["analyst".to_string()],
            failed_phase: Some("architect".to_string()),
            failure_reason: Some("specs missing".to_string()),
            topology_name: Some("development".to_string()),
        };

        assert_eq!(state.topology_name.as_deref(), Some("development"));

        Gateway::save_chain_state(tmp.path(), &state).await;

        let path = tmp.path().join("docs/.workflow/chain-state.md");
        assert!(path.exists(), "chain-state.md should be created");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.contains("development"),
            "chain-state.md should include topology name 'development': got '{}'",
            &content[..content.len().min(200)]
        );
    }

    #[tokio::test]
    async fn test_chain_state_topology_name_optional() {
        let tmp = tempfile::tempdir().unwrap();
        let state = ChainState {
            project_name: "no-topo".to_string(),
            project_dir: tmp.path().display().to_string(),
            completed_phases: vec![],
            failed_phase: None,
            failure_reason: None,
            topology_name: None,
        };

        assert!(state.topology_name.is_none());

        Gateway::save_chain_state(tmp.path(), &state).await;

        let path = tmp.path().join("docs/.workflow/chain-state.md");
        assert!(path.exists(), "chain-state.md should still be created");
    }
}
