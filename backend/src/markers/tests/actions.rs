use super::super::*;

// --- SKILL_IMPROVE ---

#[test]
fn test_extract_skill_improve() {
    let text =
        "I've updated the skill.\nSKILL_IMPROVE: google-workspace | Always search by both name and email when looking up contacts";
    let result = extract_skill_improve(text);
    assert_eq!(
        result,
        Some("SKILL_IMPROVE: google-workspace | Always search by both name and email when looking up contacts".to_string())
    );
}

#[test]
fn test_extract_skill_improve_none() {
    assert!(extract_skill_improve("No skill improve here.").is_none());
}

#[test]
fn test_parse_skill_improve_line() {
    let line = "SKILL_IMPROVE: google-workspace | Always search by both name and email";
    let (skill, lesson) = parse_skill_improve_line(line).unwrap();
    assert_eq!(skill, "google-workspace");
    assert_eq!(lesson, "Always search by both name and email");
}

#[test]
fn test_parse_skill_improve_line_with_internal_pipes() {
    let line =
        "SKILL_IMPROVE: playwright-mcp | Use page.waitForSelector('div | span') before clicking";
    let (skill, lesson) = parse_skill_improve_line(line).unwrap();
    assert_eq!(skill, "playwright-mcp");
    assert_eq!(
        lesson,
        "Use page.waitForSelector('div | span') before clicking"
    );
}

#[test]
fn test_parse_skill_improve_line_invalid() {
    assert!(parse_skill_improve_line("SKILL_IMPROVE:").is_none());
    assert!(parse_skill_improve_line("SKILL_IMPROVE: skill_only").is_none());
    assert!(parse_skill_improve_line("SKILL_IMPROVE:  | lesson").is_none());
    assert!(parse_skill_improve_line("SKILL_IMPROVE: skill |").is_none());
    assert!(parse_skill_improve_line("not a skill improve line").is_none());
}

#[test]
fn test_strip_skill_improve() {
    let text =
        "Fixed the issue.\nSKILL_IMPROVE: google-workspace | Search by name and email\nDone.";
    let result = strip_skill_improve(text);
    assert_eq!(result, "Fixed the issue.\nDone.");
}

// --- BUG_REPORT ---

#[test]
fn test_extract_bug_report() {
    let text = "I can't read my heartbeat config.\nBUG_REPORT: Cannot read own heartbeat interval";
    assert_eq!(
        extract_bug_report(text),
        Some("Cannot read own heartbeat interval".to_string())
    );
}

#[test]
fn test_extract_bug_report_inline() {
    let text = "I noticed a gap. BUG_REPORT: No introspection for MCP connections";
    assert_eq!(
        extract_bug_report(text),
        Some("No introspection for MCP connections".to_string())
    );
}

#[test]
fn test_extract_bug_report_none() {
    assert!(extract_bug_report("Just a normal response.").is_none());
}

#[test]
fn test_extract_bug_report_empty() {
    assert!(extract_bug_report("BUG_REPORT: ").is_none());
}

#[test]
fn test_strip_bug_report() {
    let text = "I noticed an issue.\nBUG_REPORT: Cannot list active MCP connections\nDone.";
    assert_eq!(strip_bug_report(text), "I noticed an issue.\nDone.");
}

#[test]
fn test_strip_bug_report_inline() {
    let text = "There's a gap here. BUG_REPORT: No runtime config access";
    assert_eq!(strip_bug_report(text), "There's a gap here.");
}

#[test]
fn test_append_bug_report_creates_file() {
    let tmp = std::env::temp_dir().join("omega_test_bug_report_create");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    append_bug_report(tmp.to_str().unwrap(), "Cannot read heartbeat interval").unwrap();

    let content = std::fs::read_to_string(tmp.join("BUG.md")).unwrap();
    assert!(content.contains("# OMEGA Bug Reports"));
    assert!(content.contains("- **Cannot read heartbeat interval**"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_append_bug_report_groups_by_date() {
    let tmp = std::env::temp_dir().join("omega_test_bug_report_group");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    append_bug_report(tmp.to_str().unwrap(), "First bug").unwrap();
    append_bug_report(tmp.to_str().unwrap(), "Second bug").unwrap();

    let content = std::fs::read_to_string(tmp.join("BUG.md")).unwrap();
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    // Only one date header for today.
    assert_eq!(
        content.matches(&format!("## {today}")).count(),
        1,
        "should have exactly one date header"
    );
    assert!(content.contains("- **First bug**"));
    assert!(content.contains("- **Second bug**"));

    let _ = std::fs::remove_dir_all(&tmp);
}

// --- ACTION_OUTCOME ---

#[test]
fn test_extract_action_outcome_success() {
    let text = "Email sent successfully.\nACTION_OUTCOME: success";
    assert_eq!(extract_action_outcome(text), Some(ActionOutcome::Success));
}

#[test]
fn test_extract_action_outcome_success_case_insensitive() {
    let text = "Done.\nACTION_OUTCOME: Success";
    assert_eq!(extract_action_outcome(text), Some(ActionOutcome::Success));
}

#[test]
fn test_extract_action_outcome_failed_with_reason() {
    let text = "Could not send.\nACTION_OUTCOME: failed | SMTP connection refused";
    assert_eq!(
        extract_action_outcome(text),
        Some(ActionOutcome::Failed("SMTP connection refused".to_string()))
    );
}

#[test]
fn test_extract_action_outcome_failed_no_reason() {
    let text = "Error occurred.\nACTION_OUTCOME: failed";
    assert_eq!(
        extract_action_outcome(text),
        Some(ActionOutcome::Failed(String::new()))
    );
}

#[test]
fn test_extract_action_outcome_none() {
    assert!(extract_action_outcome("No marker here.").is_none());
}

#[test]
fn test_extract_action_outcome_empty() {
    assert!(extract_action_outcome("ACTION_OUTCOME: ").is_none());
}

#[test]
fn test_strip_action_outcome() {
    let text = "Email sent to Adri.\nACTION_OUTCOME: success\nDone.";
    assert_eq!(strip_action_outcome(text), "Email sent to Adri.\nDone.");
}

#[test]
fn test_strip_action_outcome_failed() {
    let text = "Failed to send.\nACTION_OUTCOME: failed | timeout\nSorry.";
    assert_eq!(strip_action_outcome(text), "Failed to send.\nSorry.");
}
