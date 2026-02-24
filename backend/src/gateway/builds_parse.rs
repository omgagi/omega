//! Pure parsing functions, data structures, and prompt templates for the build pipeline.

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

/// Parsed output from Phase 1 (Clarification).
///
/// Fields beyond `name` and `scope` are parsed for completeness and passed to Phase 2
/// via the raw `brief_text` string. They are available for future orchestrator logic
/// (e.g., conditional frontend phase, language-specific verification commands).
#[allow(dead_code)]
pub(super) struct ProjectBrief {
    pub(super) name: String,
    pub(super) language: String,
    pub(super) database: String,
    pub(super) frontend: bool,
    pub(super) scope: String,
    pub(super) components: Vec<String>,
}

/// Result of Phase 4 (Verification).
pub(super) enum VerificationResult {
    Pass,
    Fail(String),
}

/// Parsed output from Phase 5 (Delivery).
pub(super) struct BuildSummary {
    pub(super) project: String,
    pub(super) location: String,
    pub(super) language: String,
    pub(super) summary: String,
    pub(super) usage: String,
    pub(super) skill: Option<String>,
}

// ---------------------------------------------------------------------------
// Phase prompt templates
// ---------------------------------------------------------------------------

pub(super) const PHASE_1_PROMPT: &str = "\
You are analyzing a build request. Extract and decide:
- Project name (kebab-case, max 3 words, descriptive)
- Programming language (default: Rust unless user specifies otherwise)
- Database (default: SQLite unless user specifies otherwise)
- Scope summary (1-3 sentences of what the project does)
- Key components (list of modules/features)
- Whether a frontend is needed (yes/no)

Do NOT ask questions. Make reasonable defaults for anything ambiguous.
Output ONLY a structured brief in this exact format:

PROJECT_NAME: <name>
LANGUAGE: <language>
DATABASE: <database>
FRONTEND: <yes/no>
SCOPE: <1-3 sentence description>
COMPONENTS:
- <component 1>
- <component 2>";

pub(super) const PHASE_2_TEMPLATE: &str = "\
You are designing the architecture for a software project. You have full tool access.
Your working directory is: {project_dir}

Project brief:
{brief_text}

Your tasks:
1. Create the directory structure
2. Write specs/architecture.md with module descriptions, data flow, API design
3. Write specs/requirements.md with functional requirements
4. If Rust: initialize with cargo init and set up Cargo.toml with dependencies
5. Create stub files for each module (empty files with doc comments)

Do NOT implement any logic. Only create structure and specifications.
When done, output: ARCHITECTURE_COMPLETE";

pub(super) const PHASE_3_TEMPLATE: &str = "\
You are implementing a software project. You have full tool access.
Your working directory is: {project_dir}

Read the specifications in specs/ to understand what to build. Implement the project module by module:
1. Read specs/architecture.md and specs/requirements.md
2. Implement each module described in the architecture
3. Write tests alongside the code
4. Ensure all code compiles

Do NOT write documentation. Do NOT create skills. Focus only on working code.
When done, output: IMPLEMENTATION_COMPLETE";

pub(super) const PHASE_3_RETRY_TEMPLATE: &str = "\
You are fixing a software project that failed verification. You have full tool access.
Your working directory is: {project_dir}

The previous verification found these issues:
{failure_reason}

Read the code, fix the issues, and ensure:
1. The code compiles without errors
2. All lint warnings are fixed
3. All tests pass

When done, output: IMPLEMENTATION_COMPLETE";

pub(super) const PHASE_4_TEMPLATE: &str = "\
You are verifying a software project. You have full tool access.
Your working directory is: {project_dir}

Run the complete validation pipeline:
1. cargo build (or equivalent) — must compile with zero errors
2. cargo clippy --workspace (or equivalent linter) — fix ALL warnings
3. cargo test --workspace (or equivalent) — all tests must pass

If any step fails, fix the issue and re-run.
After all checks pass, output exactly: VERIFICATION: PASS
If you cannot fix the issues, output: VERIFICATION: FAIL followed by REASON: <brief description>";

pub(super) const PHASE_5_TEMPLATE: &str = "\
You are delivering a completed software project. You have full tool access.
Your working directory is: {project_dir}

Tasks:
1. Write user documentation in docs/ (README.md, usage guide)
2. Create a skill file at {skills_dir}/{project_name}/SKILL.md with YAML frontmatter (name, description, trigger keywords) and CLI documentation
3. Write a final summary

Output the summary in this format:
BUILD_COMPLETE
PROJECT: <name>
LOCATION: <full path>
LANGUAGE: <language>
SUMMARY: <2-3 sentences>
USAGE: <primary CLI command or entry point>
SKILL: <skill name>";

// ---------------------------------------------------------------------------
// Pure parsing functions (testable without mocking)
// ---------------------------------------------------------------------------

/// Parse structured output from Phase 1 into a `ProjectBrief`.
pub(super) fn parse_project_brief(text: &str) -> Option<ProjectBrief> {
    let get_field = |key: &str| -> Option<String> {
        text.lines()
            .find(|line| line.starts_with(&format!("{key}:")))
            .map(|line| line[key.len() + 1..].trim().to_string())
    };

    let name = get_field("PROJECT_NAME")?;
    if name.is_empty()
        || name.contains('/')
        || name.contains('\\')
        || name.contains("..")
        || name.starts_with('.')
    {
        return None;
    }

    let language = get_field("LANGUAGE").unwrap_or_else(|| "Rust".to_string());
    let database = get_field("DATABASE").unwrap_or_else(|| "SQLite".to_string());
    let frontend = get_field("FRONTEND")
        .map(|v| v.to_lowercase().starts_with('y'))
        .unwrap_or(false);
    let scope = get_field("SCOPE").unwrap_or_else(|| "A software project.".to_string());

    let components: Vec<String> = text
        .lines()
        .skip_while(|line| !line.starts_with("COMPONENTS:"))
        .skip(1)
        .take_while(|line| line.starts_with("- "))
        .map(|line| line[2..].trim().to_string())
        .collect();

    Some(ProjectBrief {
        name,
        language,
        database,
        frontend,
        scope,
        components,
    })
}

/// Parse Phase 4 verification output into a pass/fail result.
pub(super) fn parse_verification_result(text: &str) -> VerificationResult {
    if text.contains("VERIFICATION: PASS") {
        VerificationResult::Pass
    } else if let Some(reason_line) = text.lines().find(|l| l.starts_with("REASON:")) {
        VerificationResult::Fail(reason_line["REASON:".len()..].trim().to_string())
    } else if text.contains("VERIFICATION: FAIL") {
        VerificationResult::Fail("Verification failed (no reason provided)".to_string())
    } else {
        // No marker found — treat as failure to avoid silently passing a broken build.
        VerificationResult::Fail("No verification marker found in response".to_string())
    }
}

/// Parse Phase 5 delivery output into a `BuildSummary`.
pub(super) fn parse_build_summary(text: &str) -> Option<BuildSummary> {
    if !text.contains("BUILD_COMPLETE") {
        return None;
    }

    let get_field = |key: &str| -> Option<String> {
        text.lines()
            .find(|line| line.starts_with(&format!("{key}:")))
            .map(|line| line[key.len() + 1..].trim().to_string())
    };

    Some(BuildSummary {
        project: get_field("PROJECT").unwrap_or_default(),
        location: get_field("LOCATION").unwrap_or_default(),
        language: get_field("LANGUAGE").unwrap_or_default(),
        summary: get_field("SUMMARY").unwrap_or_default(),
        usage: get_field("USAGE").unwrap_or_default(),
        skill: get_field("SKILL").filter(|s| !s.is_empty()),
    })
}

/// Localized phase progress message.
pub(super) fn phase_message(lang: &str, phase: u8, action: &str) -> String {
    match lang {
        "Spanish" => match phase {
            1 => "Analizando tu solicitud de construcci\u{f3}n...".to_string(),
            5 => "Preparando la entrega...".to_string(),
            _ => format!("Fase {phase}: {action}..."),
        },
        "Portuguese" => match phase {
            1 => "Analisando sua solicita\u{e7}\u{e3}o de constru\u{e7}\u{e3}o...".to_string(),
            5 => "Preparando a entrega...".to_string(),
            _ => format!("Fase {phase}: {action}..."),
        },
        _ => match phase {
            1 => "Analyzing your build request...".to_string(),
            5 => "Preparing delivery...".to_string(),
            _ => format!("Phase {phase}: {action}..."),
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_project_brief_valid() {
        let text = "PROJECT_NAME: price-tracker\nLANGUAGE: Rust\nDATABASE: SQLite\nFRONTEND: no\nSCOPE: A CLI tool that tracks cryptocurrency prices.\nCOMPONENTS:\n- price fetcher\n- storage engine\n- alert system";
        let brief = parse_project_brief(text).unwrap();
        assert_eq!(brief.name, "price-tracker");
        assert_eq!(brief.language, "Rust");
        assert_eq!(brief.database, "SQLite");
        assert!(!brief.frontend);
        assert!(brief.scope.contains("cryptocurrency"));
        assert_eq!(brief.components.len(), 3);
    }

    #[test]
    fn test_parse_project_brief_minimal() {
        let text = "PROJECT_NAME: my-tool\nSCOPE: Does stuff";
        let brief = parse_project_brief(text).unwrap();
        assert_eq!(brief.name, "my-tool");
        assert_eq!(brief.language, "Rust"); // default
        assert_eq!(brief.database, "SQLite"); // default
        assert!(!brief.frontend); // default
    }

    #[test]
    fn test_parse_project_brief_missing_name() {
        let text = "LANGUAGE: Python\nSCOPE: A web scraper";
        assert!(parse_project_brief(text).is_none());
    }

    #[test]
    fn test_parse_project_brief_empty_name() {
        let text = "PROJECT_NAME: \nLANGUAGE: Rust";
        assert!(parse_project_brief(text).is_none());
    }

    #[test]
    fn test_parse_project_brief_path_traversal_rejected() {
        assert!(parse_project_brief("PROJECT_NAME: ../../../etc\nSCOPE: evil").is_none());
        assert!(parse_project_brief("PROJECT_NAME: foo/bar\nSCOPE: evil").is_none());
        assert!(parse_project_brief("PROJECT_NAME: .hidden\nSCOPE: evil").is_none());
        assert!(parse_project_brief("PROJECT_NAME: foo\\bar\nSCOPE: evil").is_none());
    }

    #[test]
    fn test_parse_project_brief_with_frontend() {
        let text = "PROJECT_NAME: dashboard\nFRONTEND: yes\nSCOPE: A web dashboard";
        let brief = parse_project_brief(text).unwrap();
        assert!(brief.frontend);
    }

    #[test]
    fn test_parse_project_brief_components_parsing() {
        let text =
            "PROJECT_NAME: my-app\nCOMPONENTS:\n- auth module\n- api layer\n- database\nSome other text";
        let brief = parse_project_brief(text).unwrap();
        assert_eq!(
            brief.components,
            vec!["auth module", "api layer", "database"]
        );
    }

    #[test]
    fn test_parse_verification_pass() {
        let text = "All tests passed.\n\nVERIFICATION: PASS";
        assert!(matches!(
            parse_verification_result(text),
            VerificationResult::Pass
        ));
    }

    #[test]
    fn test_parse_verification_fail_with_reason() {
        let text = "VERIFICATION: FAIL\nREASON: cargo test failed with 3 errors";
        match parse_verification_result(text) {
            VerificationResult::Fail(reason) => assert!(reason.contains("3 errors")),
            _ => panic!("expected Fail"),
        }
    }

    #[test]
    fn test_parse_verification_fail_no_reason() {
        let text = "VERIFICATION: FAIL";
        match parse_verification_result(text) {
            VerificationResult::Fail(reason) => assert!(reason.contains("no reason")),
            _ => panic!("expected Fail"),
        }
    }

    #[test]
    fn test_parse_verification_no_marker_implicit_fail() {
        let text = "Fixed all issues. Everything compiles now.";
        match parse_verification_result(text) {
            VerificationResult::Fail(reason) => {
                assert!(reason.contains("No verification marker"))
            }
            _ => panic!("expected Fail when no marker present"),
        }
    }

    #[test]
    fn test_parse_build_summary_valid() {
        let text = "BUILD_COMPLETE\nPROJECT: price-tracker\nLOCATION: /home/user/.omega/workspace/builds/price-tracker\nLANGUAGE: Rust\nSUMMARY: A CLI tool for tracking crypto prices with alerts.\nUSAGE: price-tracker watch BTC\nSKILL: price-tracker";
        let summary = parse_build_summary(text).unwrap();
        assert_eq!(summary.project, "price-tracker");
        assert!(summary.location.contains("price-tracker"));
        assert_eq!(summary.language, "Rust");
        assert!(summary.summary.contains("crypto"));
        assert_eq!(summary.usage, "price-tracker watch BTC");
        assert_eq!(summary.skill, Some("price-tracker".to_string()));
    }

    #[test]
    fn test_parse_build_summary_no_marker() {
        let text = "Here's what I built: a price tracker tool.";
        assert!(parse_build_summary(text).is_none());
    }

    #[test]
    fn test_parse_build_summary_no_skill() {
        let text = "BUILD_COMPLETE\nPROJECT: one-off\nLOCATION: /tmp/one-off\nLANGUAGE: Python\nSUMMARY: A quick script\nUSAGE: python main.py\nSKILL: ";
        let summary = parse_build_summary(text).unwrap();
        assert_eq!(summary.skill, None); // empty string filtered out
    }

    #[test]
    fn test_phase_message_english() {
        let msg = phase_message("English", 1, "analyzing");
        assert!(msg.contains("Analyzing"));
    }

    #[test]
    fn test_phase_message_spanish() {
        let msg = phase_message("Spanish", 1, "analyzing");
        assert!(msg.contains("Analizando"));
    }
}
