//! Embedded build agent definitions and temp file lifecycle.
//!
//! Each build phase uses a purpose-built agent loaded via `claude --agent <name>`.
//! Agent content is compiled into the binary and written as temporary files to
//! the build workspace's `.claude/agents/` directory at runtime.
//!
//! Implementation contract (defined by tests below):
//! - 7 agent constants: BUILD_ANALYST_AGENT through BUILD_DELIVERY_AGENT
//! - BUILD_AGENTS: &[(&str, &str)] mapping names to content
//! - AgentFilesGuard: RAII struct that writes on creation, removes on drop
//!
//! DEVELOPER: After implementing this module, register it in
//! `backend/src/gateway/mod.rs` by adding: `mod builds_agents;`

use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Agent constants — compiled into the binary, written as temp files at runtime
// ---------------------------------------------------------------------------

pub(super) const BUILD_ANALYST_AGENT: &str = "\
---
name: build-analyst
description: Analyzes build requests and produces structured project briefs with requirements
tools: Read, Grep, Glob
model: opus
permissionMode: bypassPermissions
maxTurns: 25
---

You are a build analyst. Analyze the user's build request and produce a structured project brief.

Do NOT ask questions. Do NOT ask the user for clarification. Make reasonable defaults for anything ambiguous.

## Output Format

You MUST output the following structured fields so downstream phases can parse them:

PROJECT_NAME: <snake_case project name>
LANGUAGE: <primary programming language>
DATABASE: <database if needed, or \"none\">
FRONTEND: <frontend framework if needed, or \"none\">
SCOPE: <one-line description of what the project does>
COMPONENTS: <comma-separated list of major components>

After these fields, write a detailed requirements section with numbered requirements (REQ-001, REQ-002, etc.) each with acceptance criteria.

## Rules

- Keep the project name short and snake_case
- Choose the most appropriate language for the task
- Be specific about COMPONENTS — list concrete modules, not vague categories
- Every requirement must have testable acceptance criteria
";

pub(super) const BUILD_ARCHITECT_AGENT: &str = "\
---
name: build-architect
description: Designs project architecture with specs and directory structure
tools: Read, Write, Bash, Glob, Grep
model: opus
permissionMode: bypassPermissions
---

You are a build architect. Design the project architecture based on the analyst's brief.

Do NOT ask questions. Do NOT ask the user for clarification. Make reasonable defaults for anything ambiguous.

## Your Tasks

1. Create the project directory structure
2. Write specs/requirements.md with numbered requirements and testable acceptance criteria
3. Write specs/architecture.md with module descriptions, interfaces, and data flow
4. Create initial config files (Cargo.toml, package.json, etc.) appropriate for the language

## Rules

- Write specs/ files that the test-writer can reference
- Every module in architecture.md must map to at least one requirement
- Include failure modes and edge cases in specs
- Keep the architecture simple — avoid over-engineering
- Use standard project layouts for the chosen language
";

pub(super) const BUILD_TEST_WRITER_AGENT: &str = "\
---
name: build-test-writer
description: Writes failing tests before implementation (TDD red phase)
tools: Read, Write, Edit, Bash, Glob, Grep
model: fast
permissionMode: bypassPermissions
---

You are a TDD test writer. Read the specs/ directory and write tests that cover every requirement.

Do NOT ask questions. Do NOT ask the user for clarification. Make reasonable defaults for anything ambiguous.

## Your Tasks

1. Read specs/requirements.md and specs/architecture.md
2. Write test files covering each numbered requirement
3. Tests must reference requirement IDs in comments (e.g. // REQ-001)
4. All tests must fail initially — this is the TDD red phase
5. Run the tests to confirm they fail (expected at this stage)

## Rules

- Must requirements get exhaustive test coverage
- Should requirements get at least one test each
- Tests must be self-contained and independent
- Use the project's standard testing framework
- Write unit tests, not integration tests (those come later in QA)
- Every test must have a clear assertion — no empty test bodies
";

pub(super) const BUILD_DEVELOPER_AGENT: &str = "\
---
name: build-developer
description: Implements minimum code to pass all tests (TDD green phase)
tools: Read, Write, Edit, Bash, Glob, Grep
model: fast
permissionMode: bypassPermissions
---

You are a TDD developer. Read the tests and specs, then implement the minimum code to pass all tests.

Do NOT ask questions. Do NOT ask the user for clarification. Make reasonable defaults for anything ambiguous.

## Your Tasks

1. Read the test files first to understand what must be implemented
2. Read specs/ for architectural context
3. Implement module by module until all tests pass
4. Run tests after each module to verify progress
5. Refactor if needed while keeping tests green

## Rules

- No file may exceed 500 lines (excluding tests)
- Implement the minimum code to pass tests — no gold-plating
- Follow the project's established conventions
- Each module must be self-contained with clear interfaces
- Run all tests at the end to confirm everything passes
";

pub(super) const BUILD_QA_AGENT: &str = "\
---
name: build-qa
description: Validates project quality by running build, lint, and tests
tools: Read, Write, Edit, Bash, Glob, Grep
model: fast
permissionMode: bypassPermissions
---

You are a QA validator. Validate the project by running the full build, linter, and test suite.

Do NOT ask questions. Do NOT ask the user for clarification. Make reasonable defaults for anything ambiguous.

## Your Tasks

1. Run the project build (cargo build, npm run build, etc.)
2. Run the linter if configured
3. Run the full test suite
4. Check that all acceptance criteria from specs/requirements.md are met
5. Report results in the required format

## Output Format

You MUST end your response with one of:
- VERIFICATION: PASS — if all checks pass
- VERIFICATION: FAIL — followed by a description of what failed

Example:
VERIFICATION: PASS

Or:
VERIFICATION: FAIL
3 tests failing in module auth: test_login_invalid, test_token_expired, test_refresh_missing

## Rules

- Run actual commands, do not simulate results
- Report ALL failures, not just the first one
- Be specific about which tests or checks failed
";

pub(super) const BUILD_REVIEWER_AGENT: &str = "\
---
name: build-reviewer
description: Reviews code for bugs, security issues, and quality
tools: Read, Grep, Glob, Bash
model: fast
permissionMode: bypassPermissions
maxTurns: 50
---

You are a code reviewer. Audit the project for bugs, security issues, and code quality.

Do NOT ask questions. Do NOT ask the user for clarification. Make reasonable defaults for anything ambiguous.

## Your Tasks

1. Read all source files and review for correctness
2. Check for security vulnerabilities (injection, auth bypass, etc.)
3. Check for performance issues (N+1 queries, unbounded allocations, etc.)
4. Verify code follows project conventions
5. Check that specs/ and docs/ are consistent with the code
6. Report results in the required format

## Output Format

You MUST end your response with one of:
- REVIEW: PASS — if the code meets quality standards
- REVIEW: FAIL — followed by specific findings

Example:
REVIEW: PASS

Or:
REVIEW: FAIL
- security: SQL injection in query_builder.rs line 45
- bug: off-by-one error in pagination.rs line 120

## Rules

- Be thorough but pragmatic — this is a build, not a production audit
- Focus on correctness and security over style
- Do NOT modify any files — you are read-only
";

pub(super) const BUILD_DELIVERY_AGENT: &str = "\
---
name: build-delivery
description: Creates documentation, README, and SKILL.md for the completed project
tools: Read, Write, Edit, Bash, Glob, Grep
model: fast
permissionMode: bypassPermissions
---

You are a delivery agent. Create final documentation and the SKILL.md registration file.

Do NOT ask questions. Do NOT ask the user for clarification. Make reasonable defaults for anything ambiguous.

## Your Tasks

1. Write or update README.md with project description, setup, and usage
2. Write docs/ files if the project warrants them
3. Create the SKILL.md file in the skills directory for OMEGA registration
4. Produce a final build summary

## Build Summary Format

You MUST end your response with a build summary block:

BUILD_SUMMARY:
PROJECT: <project name>
LOCATION: <absolute path to project>
LANGUAGE: <primary language>
USAGE: <one-line command to run/use the project>
SKILL: <skill name if SKILL.md was created>
SUMMARY: <2-3 sentence description of what was built>

## Rules

- README must be clear enough for a new developer to get started
- SKILL.md must follow OMEGA's skill format
- Include all necessary setup steps in documentation
";

/// Name-to-content mapping for all 7 build agents.
pub(super) const BUILD_AGENTS: &[(&str, &str)] = &[
    ("build-analyst", BUILD_ANALYST_AGENT),
    ("build-architect", BUILD_ARCHITECT_AGENT),
    ("build-test-writer", BUILD_TEST_WRITER_AGENT),
    ("build-developer", BUILD_DEVELOPER_AGENT),
    ("build-qa", BUILD_QA_AGENT),
    ("build-reviewer", BUILD_REVIEWER_AGENT),
    ("build-delivery", BUILD_DELIVERY_AGENT),
];

// ---------------------------------------------------------------------------
// Agent file lifecycle — RAII guard
// ---------------------------------------------------------------------------

/// RAII guard that writes agent `.md` files on creation and removes them on drop.
///
/// Usage:
/// ```ignore
/// let _guard = AgentFilesGuard::write(&project_dir).await?;
/// // ... invoke claude --agent <name> ...
/// // Drop cleans up automatically, even on panic.
/// ```
pub(super) struct AgentFilesGuard {
    agents_dir: PathBuf,
}

impl AgentFilesGuard {
    /// Write all build agent files to `<project_dir>/.claude/agents/`.
    pub async fn write(project_dir: &Path) -> std::io::Result<Self> {
        let agents_dir = project_dir.join(".claude").join("agents");
        tokio::fs::create_dir_all(&agents_dir).await?;
        for (name, content) in BUILD_AGENTS {
            let path = agents_dir.join(format!("{name}.md"));
            tokio::fs::write(&path, content).await?;
        }
        Ok(Self { agents_dir })
    }
}

impl Drop for AgentFilesGuard {
    fn drop(&mut self) {
        // Remove the agents directory and all files within it.
        let _ = std::fs::remove_dir_all(&self.agents_dir);
        // Remove the parent .claude/ directory if it is now empty.
        if let Some(claude_dir) = self.agents_dir.parent() {
            let _ = std::fs::remove_dir(claude_dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ===================================================================
    // REQ-BAP-002 (Must): Embedded agent content — 7 agent definitions
    // ===================================================================

    // Requirement: REQ-BAP-002 (Must)
    // Acceptance: all 7 build agent definitions compiled into the binary
    #[test]
    fn test_build_agents_has_exactly_7_entries() {
        assert_eq!(
            BUILD_AGENTS.len(),
            7,
            "BUILD_AGENTS must contain exactly 7 agent definitions"
        );
    }

    // Requirement: REQ-BAP-002 (Must)
    // Acceptance: correct agent names in the mapping
    #[test]
    fn test_build_agents_correct_names() {
        let expected_names = [
            "build-analyst",
            "build-architect",
            "build-test-writer",
            "build-developer",
            "build-qa",
            "build-reviewer",
            "build-delivery",
        ];
        let actual_names: Vec<&str> = BUILD_AGENTS.iter().map(|(name, _)| *name).collect();
        assert_eq!(
            actual_names, expected_names,
            "Agent names must match expected 7-phase pipeline order"
        );
    }

    // Requirement: REQ-BAP-002 (Must)
    // Acceptance: no .md files shipped on disk; content accessible via constants
    #[test]
    fn test_build_agent_constants_are_non_empty() {
        assert!(!BUILD_ANALYST_AGENT.is_empty(), "BUILD_ANALYST_AGENT must not be empty");
        assert!(!BUILD_ARCHITECT_AGENT.is_empty(), "BUILD_ARCHITECT_AGENT must not be empty");
        assert!(!BUILD_TEST_WRITER_AGENT.is_empty(), "BUILD_TEST_WRITER_AGENT must not be empty");
        assert!(!BUILD_DEVELOPER_AGENT.is_empty(), "BUILD_DEVELOPER_AGENT must not be empty");
        assert!(!BUILD_QA_AGENT.is_empty(), "BUILD_QA_AGENT must not be empty");
        assert!(!BUILD_REVIEWER_AGENT.is_empty(), "BUILD_REVIEWER_AGENT must not be empty");
        assert!(!BUILD_DELIVERY_AGENT.is_empty(), "BUILD_DELIVERY_AGENT must not be empty");
    }

    // Requirement: REQ-BAP-002 (Must)
    // Acceptance: each agent has YAML frontmatter
    #[test]
    fn test_build_agents_have_yaml_frontmatter() {
        for (name, content) in BUILD_AGENTS {
            assert!(
                content.starts_with("---"),
                "Agent '{name}' must start with YAML frontmatter delimiter '---'"
            );
            // Must have a closing --- delimiter.
            let after_open = &content[3..];
            assert!(
                after_open.contains("\n---"),
                "Agent '{name}' must have closing YAML frontmatter delimiter '---'"
            );
        }
    }

    // Requirement: REQ-BAP-002 (Must)
    // Acceptance: each agent frontmatter contains required keys
    #[test]
    fn test_build_agents_frontmatter_required_keys() {
        let required_keys = ["name:", "description:", "tools:", "model:", "permissionMode:"];
        for (agent_name, content) in BUILD_AGENTS {
            // Extract frontmatter (between first --- and second ---).
            let after_open = &content[3..];
            let close_idx = after_open
                .find("\n---")
                .unwrap_or_else(|| panic!("Agent '{agent_name}' missing closing ---"));
            let frontmatter = &after_open[..close_idx];

            for key in &required_keys {
                assert!(
                    frontmatter.contains(key),
                    "Agent '{agent_name}' frontmatter must contain '{key}'"
                );
            }
        }
    }

    // Requirement: REQ-BAP-002 (Must)
    // Acceptance: frontmatter name matches the mapping key
    #[test]
    fn test_build_agents_frontmatter_name_matches_key() {
        for (agent_name, content) in BUILD_AGENTS {
            let after_open = &content[3..];
            let close_idx = after_open.find("\n---").unwrap();
            let frontmatter = &after_open[..close_idx];

            // Find the "name:" line and extract value.
            let name_line = frontmatter
                .lines()
                .find(|l| l.starts_with("name:"))
                .unwrap_or_else(|| panic!("Agent '{agent_name}' has no name: line"));
            let name_value = name_line["name:".len()..].trim();
            assert_eq!(
                name_value, *agent_name,
                "Agent frontmatter name '{name_value}' must match mapping key '{agent_name}'"
            );
        }
    }

    // ===================================================================
    // REQ-BAP-014 (Must): Permission bypass in build agents
    // ===================================================================

    // Requirement: REQ-BAP-014 (Must)
    // Acceptance: build agents use bypassPermissions
    #[test]
    fn test_build_agents_permission_bypass() {
        for (name, content) in BUILD_AGENTS {
            assert!(
                content.contains("permissionMode: bypassPermissions"),
                "Agent '{name}' must have permissionMode: bypassPermissions"
            );
        }
    }

    // ===================================================================
    // REQ-BAP-011 (Must): Non-interactive build agents
    // ===================================================================

    // Requirement: REQ-BAP-011 (Must)
    // Acceptance: "Do NOT ask questions" in every agent
    #[test]
    fn test_build_agents_non_interactive() {
        for (name, content) in BUILD_AGENTS {
            let lower = content.to_lowercase();
            assert!(
                lower.contains("do not ask question")
                    || lower.contains("don't ask question")
                    || lower.contains("never ask question")
                    || lower.contains("do not ask the user")
                    || lower.contains("never ask the user"),
                "Agent '{name}' must contain non-interactive instruction \
                 (e.g. 'Do NOT ask questions')"
            );
        }
    }

    // Requirement: REQ-BAP-011 (Must)
    // Acceptance: "Make reasonable defaults for anything ambiguous"
    #[test]
    fn test_build_agents_reasonable_defaults_instruction() {
        for (name, content) in BUILD_AGENTS {
            let lower = content.to_lowercase();
            assert!(
                lower.contains("reasonable default")
                    || lower.contains("sensible default")
                    || lower.contains("make default")
                    || lower.contains("assume reasonable"),
                "Agent '{name}' must instruct making reasonable defaults for ambiguity"
            );
        }
    }

    // ===================================================================
    // REQ-BAP-012 (Must): Analyst output format
    // ===================================================================

    // Requirement: REQ-BAP-012 (Must)
    // Acceptance: analyst agent instructions include parseable output format
    #[test]
    fn test_analyst_agent_output_format() {
        let content = BUILD_ANALYST_AGENT;
        // Must instruct the analyst to output in the structured format
        // compatible with parse_project_brief().
        assert!(
            content.contains("PROJECT_NAME"),
            "Analyst agent must reference PROJECT_NAME output format"
        );
        assert!(
            content.contains("LANGUAGE"),
            "Analyst agent must reference LANGUAGE output format"
        );
        assert!(
            content.contains("SCOPE"),
            "Analyst agent must reference SCOPE output format"
        );
        assert!(
            content.contains("COMPONENTS"),
            "Analyst agent must reference COMPONENTS output format"
        );
    }

    // ===================================================================
    // REQ-BAP-021 (Should): Agent tool restrictions per role
    // ===================================================================

    // Requirement: REQ-BAP-021 (Should)
    // Acceptance: Analyst has restricted tools (Read, Grep, Glob)
    #[test]
    fn test_analyst_agent_restricted_tools() {
        let after_open = &BUILD_ANALYST_AGENT[3..];
        let close_idx = after_open.find("\n---").unwrap();
        let frontmatter = &after_open[..close_idx];
        let tools_line = frontmatter
            .lines()
            .find(|l| l.starts_with("tools:"))
            .expect("Analyst must have tools: in frontmatter");
        // Analyst should NOT have Write or Edit tools.
        assert!(
            !tools_line.contains("Write"),
            "Analyst should not have Write tool"
        );
        assert!(
            !tools_line.contains("Edit"),
            "Analyst should not have Edit tool"
        );
        // Should have Read.
        assert!(
            tools_line.contains("Read"),
            "Analyst must have Read tool"
        );
    }

    // Requirement: REQ-BAP-021 (Should)
    // Acceptance: Reviewer has restricted tools (Read, Grep, Glob, Bash)
    #[test]
    fn test_reviewer_agent_restricted_tools() {
        let after_open = &BUILD_REVIEWER_AGENT[3..];
        let close_idx = after_open.find("\n---").unwrap();
        let frontmatter = &after_open[..close_idx];
        let tools_line = frontmatter
            .lines()
            .find(|l| l.starts_with("tools:"))
            .expect("Reviewer must have tools: in frontmatter");
        // Reviewer should NOT have Write or Edit tools.
        assert!(
            !tools_line.contains("Write"),
            "Reviewer should not have Write tool"
        );
        assert!(
            !tools_line.contains("Edit"),
            "Reviewer should not have Edit tool"
        );
        // Should have Read and Bash.
        assert!(tools_line.contains("Read"), "Reviewer must have Read tool");
        assert!(tools_line.contains("Bash"), "Reviewer must have Bash tool");
    }

    // Requirement: REQ-BAP-021 (Should)
    // Acceptance: Developer/Test-writer/QA/Delivery have full tools
    #[test]
    fn test_developer_agents_have_full_tools() {
        let full_tool_agents = [
            ("build-test-writer", BUILD_TEST_WRITER_AGENT),
            ("build-developer", BUILD_DEVELOPER_AGENT),
            ("build-qa", BUILD_QA_AGENT),
            ("build-delivery", BUILD_DELIVERY_AGENT),
        ];
        for (name, content) in full_tool_agents {
            let after_open = &content[3..];
            let close_idx = after_open.find("\n---").unwrap();
            let frontmatter = &after_open[..close_idx];
            let tools_line = frontmatter
                .lines()
                .find(|l| l.starts_with("tools:"))
                .unwrap_or_else(|| panic!("Agent '{name}' must have tools:"));
            assert!(
                tools_line.contains("Read"),
                "Agent '{name}' must have Read tool"
            );
            assert!(
                tools_line.contains("Write"),
                "Agent '{name}' must have Write tool"
            );
            assert!(
                tools_line.contains("Edit"),
                "Agent '{name}' must have Edit tool"
            );
            assert!(
                tools_line.contains("Bash"),
                "Agent '{name}' must have Bash tool"
            );
        }
    }

    // ===================================================================
    // REQ-BAP-025 (Could): maxTurns in frontmatter
    // ===================================================================

    // Requirement: REQ-BAP-025 (Could)
    // Acceptance: analyst has maxTurns: 25 in frontmatter
    #[test]
    fn test_analyst_agent_max_turns() {
        let after_open = &BUILD_ANALYST_AGENT[3..];
        let close_idx = after_open.find("\n---").unwrap();
        let frontmatter = &after_open[..close_idx];
        assert!(
            frontmatter.contains("maxTurns:"),
            "Analyst agent should have maxTurns in frontmatter"
        );
    }

    // ===================================================================
    // REQ-BAP-001 (Must): Agent file lifecycle — AgentFilesGuard
    // ===================================================================

    // Requirement: REQ-BAP-001 (Must)
    // Acceptance: Agent files written to <project_dir>/.claude/agents/ before phase invocation
    #[tokio::test]
    async fn test_agent_files_guard_writes_all_agent_files() {
        let tmp = std::env::temp_dir().join("__omega_test_agents_write__");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let guard = AgentFilesGuard::write(&tmp).await.unwrap();
        let agents_dir = tmp.join(".claude").join("agents");

        assert!(agents_dir.exists(), ".claude/agents/ directory must exist");

        // Verify all 7 agent files were written.
        for (name, _content) in BUILD_AGENTS {
            let file_path = agents_dir.join(format!("{name}.md"));
            assert!(
                file_path.exists(),
                "Agent file '{name}.md' must exist after write"
            );
            let file_content = std::fs::read_to_string(&file_path).unwrap();
            assert!(
                !file_content.is_empty(),
                "Agent file '{name}.md' must not be empty"
            );
            assert!(
                file_content.starts_with("---"),
                "Agent file '{name}.md' must start with YAML frontmatter"
            );
        }

        // Cleanup.
        drop(guard);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // Requirement: REQ-BAP-001 (Must)
    // Acceptance: Agent file content matches the embedded constant
    #[tokio::test]
    async fn test_agent_files_guard_content_matches_constants() {
        let tmp = std::env::temp_dir().join("__omega_test_agents_content__");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let guard = AgentFilesGuard::write(&tmp).await.unwrap();
        let agents_dir = tmp.join(".claude").join("agents");

        for (name, expected_content) in BUILD_AGENTS {
            let file_path = agents_dir.join(format!("{name}.md"));
            let actual_content = std::fs::read_to_string(&file_path).unwrap();
            assert_eq!(
                actual_content, *expected_content,
                "File content for '{name}.md' must match BUILD_AGENTS constant"
            );
        }

        drop(guard);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // Requirement: REQ-BAP-001 (Must)
    // Acceptance: cleanup runs even on panic (RAII guard pattern) — test Drop
    #[tokio::test]
    async fn test_agent_files_guard_drop_cleans_up() {
        let tmp = std::env::temp_dir().join("__omega_test_agents_drop__");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let agents_dir = tmp.join(".claude").join("agents");

        {
            let _guard = AgentFilesGuard::write(&tmp).await.unwrap();
            assert!(agents_dir.exists(), "agents/ must exist while guard is alive");
            // Guard goes out of scope here — Drop should clean up.
        }

        assert!(
            !agents_dir.exists(),
            ".claude/agents/ must be removed after guard is dropped"
        );

        // Also verify .claude/ directory is removed (if empty).
        let claude_dir = tmp.join(".claude");
        assert!(
            !claude_dir.exists(),
            ".claude/ should be removed if empty after guard drop"
        );

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // Requirement: REQ-BAP-001 (Must)
    // Failure mode: project_dir doesn't exist
    #[tokio::test]
    async fn test_agent_files_guard_creates_directory_hierarchy() {
        let tmp = std::env::temp_dir().join("__omega_test_agents_nested__");
        let _ = std::fs::remove_dir_all(&tmp);
        // Do NOT create tmp — the guard must create the full path.
        std::fs::create_dir_all(&tmp).unwrap();

        let nested = tmp.join("deep").join("nested").join("project");
        // nested doesn't exist yet.
        assert!(!nested.exists());

        // Guard should create_dir_all internally.
        let guard = AgentFilesGuard::write(&nested).await.unwrap();
        let agents_dir = nested.join(".claude").join("agents");
        assert!(
            agents_dir.exists(),
            "Guard must create full directory hierarchy"
        );

        drop(guard);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // Requirement: REQ-BAP-001 (Must)
    // Edge case: overwrite behavior when .claude/agents/ already exists
    #[tokio::test]
    async fn test_agent_files_guard_overwrites_existing_files() {
        let tmp = std::env::temp_dir().join("__omega_test_agents_overwrite__");
        let _ = std::fs::remove_dir_all(&tmp);
        let agents_dir = tmp.join(".claude").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        // Write a pre-existing file with stale content.
        let stale_file = agents_dir.join("build-analyst.md");
        std::fs::write(&stale_file, "stale content").unwrap();

        let guard = AgentFilesGuard::write(&tmp).await.unwrap();

        // File should be overwritten with correct content.
        let content = std::fs::read_to_string(&stale_file).unwrap();
        assert_ne!(content, "stale content", "Must overwrite existing files");
        assert!(
            content.starts_with("---"),
            "Overwritten content must be valid agent definition"
        );

        drop(guard);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // Requirement: REQ-BAP-001 (Must)
    // Edge case: multiple guards for the same directory
    #[tokio::test]
    async fn test_agent_files_guard_second_write_succeeds() {
        let tmp = std::env::temp_dir().join("__omega_test_agents_double__");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let guard1 = AgentFilesGuard::write(&tmp).await.unwrap();
        drop(guard1); // Clean up first.

        // Second write should succeed even though directory was removed.
        let guard2 = AgentFilesGuard::write(&tmp).await.unwrap();
        let agents_dir = tmp.join(".claude").join("agents");
        assert!(agents_dir.exists());

        drop(guard2);
        let _ = std::fs::remove_dir_all(&tmp);
    }

    // Requirement: REQ-BAP-001 (Must)
    // Edge case: guard Drop doesn't panic if files already removed
    #[tokio::test]
    async fn test_agent_files_guard_drop_idempotent() {
        let tmp = std::env::temp_dir().join("__omega_test_agents_idempotent__");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let guard = AgentFilesGuard::write(&tmp).await.unwrap();
        let agents_dir = tmp.join(".claude").join("agents");

        // Manually delete the directory before drop.
        std::fs::remove_dir_all(&agents_dir).unwrap();

        // Drop should NOT panic.
        drop(guard);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    // ===================================================================
    // REQ-BAP-019 (Should): QA outputs parseable VERIFICATION marker
    // ===================================================================

    // Requirement: REQ-BAP-019 (Should)
    // Acceptance: QA agent instructions include VERIFICATION: PASS/FAIL output
    #[test]
    fn test_qa_agent_verification_output_format() {
        let content = BUILD_QA_AGENT;
        assert!(
            content.contains("VERIFICATION: PASS") || content.contains("VERIFICATION:"),
            "QA agent must instruct outputting VERIFICATION: PASS/FAIL"
        );
    }

    // ===================================================================
    // REQ-BAP-020 (Should): Reviewer outputs parseable REVIEW marker
    // ===================================================================

    // Requirement: REQ-BAP-020 (Should)
    // Acceptance: Reviewer agent outputs REVIEW: PASS/FAIL
    #[test]
    fn test_reviewer_agent_review_output_format() {
        let content = BUILD_REVIEWER_AGENT;
        assert!(
            content.contains("REVIEW: PASS") || content.contains("REVIEW:"),
            "Reviewer agent must instruct outputting REVIEW: PASS/FAIL"
        );
    }

    // ===================================================================
    // REQ-BAP-016 (Should): Architect creates TDD-ready specs
    // ===================================================================

    // Requirement: REQ-BAP-016 (Should)
    // Acceptance: architect agent mentions specs/ and testable criteria
    #[test]
    fn test_architect_agent_tdd_specs() {
        let content = BUILD_ARCHITECT_AGENT;
        assert!(
            content.contains("specs/") || content.contains("specs\\"),
            "Architect agent must reference specs/ directory"
        );
        assert!(
            content.to_lowercase().contains("test")
                || content.to_lowercase().contains("acceptance"),
            "Architect agent must mention testable/acceptance criteria"
        );
    }

    // ===================================================================
    // REQ-BAP-017 (Should): Test writer references specs
    // ===================================================================

    // Requirement: REQ-BAP-017 (Should)
    // Acceptance: test-writer reads specs/ and writes tests
    #[test]
    fn test_test_writer_agent_references_specs() {
        let content = BUILD_TEST_WRITER_AGENT;
        assert!(
            content.contains("specs/") || content.contains("specs\\"),
            "Test-writer agent must reference specs/ directory"
        );
        assert!(
            content.to_lowercase().contains("fail"),
            "Test-writer agent must mention tests failing initially (TDD red phase)"
        );
    }

    // ===================================================================
    // REQ-BAP-018 (Should): Developer reads tests first
    // ===================================================================

    // Requirement: REQ-BAP-018 (Should)
    // Acceptance: developer reads tests before implementing
    #[test]
    fn test_developer_agent_reads_tests_first() {
        let content = BUILD_DEVELOPER_AGENT;
        assert!(
            content.to_lowercase().contains("test"),
            "Developer agent must reference tests"
        );
    }

    // ===================================================================
    // REQ-BAP-018 (Should): 500-line file limit
    // ===================================================================

    // Requirement: REQ-BAP-018 (Should)
    // Acceptance: 500-line file limit enforced in developer agent
    #[test]
    fn test_developer_agent_500_line_limit() {
        let content = BUILD_DEVELOPER_AGENT;
        assert!(
            content.contains("500") || content.contains("file limit")
                || content.contains("line limit"),
            "Developer agent should enforce 500-line file limit"
        );
    }
}
