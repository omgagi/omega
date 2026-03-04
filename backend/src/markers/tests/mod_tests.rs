use super::super::*;

// --- strip_all_remaining_markers (defined in mod.rs, exercises all submodules) ---

#[test]
fn test_strip_all_remaining_markers() {
    let text = "Hello world. LANG_SWITCH: english\nMore text. PERSONALITY: friendly";
    let result = strip_all_remaining_markers(text);
    assert!(!result.contains("LANG_SWITCH:"));
    assert!(!result.contains("PERSONALITY:"));
    assert!(result.contains("Hello world."));
    assert!(result.contains("More text."));
}

#[test]
fn test_strip_all_remaining_markers_includes_bug_report() {
    let text = "Hello. BUG_REPORT: some limitation\nMore text.";
    let result = strip_all_remaining_markers(text);
    assert!(!result.contains("BUG_REPORT:"));
    assert!(result.contains("Hello."));
    assert!(result.contains("More text."));
}

#[test]
fn test_strip_all_remaining_markers_includes_suppress_section() {
    let text = "Hello. HEARTBEAT_SUPPRESS_SECTION: TRADING\nMore text.";
    let result = strip_all_remaining_markers(text);
    assert!(!result.contains("HEARTBEAT_SUPPRESS_SECTION:"));
    assert!(result.contains("Hello."));
    assert!(result.contains("More text."));
}

#[test]
fn test_strip_all_remaining_markers_includes_unsuppress_section() {
    let text = "Hello. HEARTBEAT_UNSUPPRESS_SECTION: TRADING\nMore text.";
    let result = strip_all_remaining_markers(text);
    assert!(!result.contains("HEARTBEAT_UNSUPPRESS_SECTION:"));
}

#[test]
fn test_strip_all_remaining_markers_includes_action_outcome() {
    let text = "Done. ACTION_OUTCOME: success\nMore text.";
    let result = strip_all_remaining_markers(text);
    assert!(!result.contains("ACTION_OUTCOME:"));
    assert!(result.contains("Done."));
    assert!(result.contains("More text."));
}

// --- Orchestration: combined multi-marker scenarios ---

#[test]
fn test_strip_all_remaining_markers_combined_schedule_and_lang() {
    let text = "I'll set that up.\n\
                SCHEDULE: Call mom | 2026-03-01T15:00:00 | once\n\
                LANG_SWITCH: Spanish\n\
                Done!";
    let result = strip_all_remaining_markers(text);
    assert!(!result.contains("SCHEDULE:"));
    assert!(!result.contains("LANG_SWITCH:"));
    assert!(result.contains("I'll set that up."));
    assert!(result.contains("Done!"));
}

#[test]
fn test_strip_all_remaining_markers_combined_reward_and_lesson() {
    let text = "Great session.\n\
                REWARD: +1|trading|Profitable trade\n\
                LESSON: trading|Always check RSI before entry\n\
                Keep it up!";
    let result = strip_all_remaining_markers(text);
    assert!(!result.contains("REWARD:"));
    assert!(!result.contains("LESSON:"));
    assert!(result.contains("Great session."));
    assert!(result.contains("Keep it up!"));
}

#[test]
fn test_strip_all_remaining_markers_all_types_at_once() {
    let text = "Response.\n\
                SCHEDULE: Task | 2026-03-01T09:00:00 | once\n\
                SCHEDULE_ACTION: Check price | 2026-03-01T14:00:00 | daily\n\
                LANG_SWITCH: French\n\
                PERSONALITY: formal\n\
                CANCEL_TASK: abc123\n\
                UPDATE_TASK: def456 | New desc | | daily\n\
                PROJECT_ACTIVATE: trader\n\
                HEARTBEAT_ADD: Monitor BTC\n\
                HEARTBEAT_REMOVE: Old item\n\
                HEARTBEAT_INTERVAL: 15\n\
                SKILL_IMPROVE: google-workspace | Search contacts better\n\
                BUG_REPORT: Cannot introspect MCP\n\
                REWARD: +1|trading|Good trade\n\
                LESSON: trading|Set stop-loss\n\
                BUILD_PROPOSAL: Dashboard for BTC\n\
                GOOGLE_SETUP\n\
                End.";
    let result = strip_all_remaining_markers(text);
    assert!(result.contains("Response."));
    assert!(result.contains("End."));
    // Verify ALL marker types are stripped.
    assert!(!result.contains("SCHEDULE:"), "SCHEDULE not stripped");
    assert!(
        !result.contains("SCHEDULE_ACTION:"),
        "SCHEDULE_ACTION not stripped"
    );
    assert!(!result.contains("LANG_SWITCH:"), "LANG_SWITCH not stripped");
    assert!(!result.contains("PERSONALITY:"), "PERSONALITY not stripped");
    assert!(!result.contains("CANCEL_TASK:"), "CANCEL_TASK not stripped");
    assert!(!result.contains("UPDATE_TASK:"), "UPDATE_TASK not stripped");
    assert!(
        !result.contains("PROJECT_ACTIVATE:"),
        "PROJECT_ACTIVATE not stripped"
    );
    assert!(
        !result.contains("HEARTBEAT_ADD:"),
        "HEARTBEAT_ADD not stripped"
    );
    assert!(
        !result.contains("HEARTBEAT_REMOVE:"),
        "HEARTBEAT_REMOVE not stripped"
    );
    assert!(
        !result.contains("HEARTBEAT_INTERVAL:"),
        "HEARTBEAT_INTERVAL not stripped"
    );
    assert!(
        !result.contains("SKILL_IMPROVE:"),
        "SKILL_IMPROVE not stripped"
    );
    assert!(!result.contains("BUG_REPORT:"), "BUG_REPORT not stripped");
    assert!(!result.contains("REWARD:"), "REWARD not stripped");
    assert!(!result.contains("LESSON:"), "LESSON not stripped");
    assert!(
        !result.contains("BUILD_PROPOSAL:"),
        "BUILD_PROPOSAL not stripped"
    );
    assert!(
        !result.contains("GOOGLE_SETUP"),
        "GOOGLE_SETUP not stripped"
    );
}

// --- extract_inline_marker_value edge cases ---

#[test]
fn test_extract_inline_marker_value_at_line_start() {
    let text = "Hello.\nLANG_SWITCH: French\nBye.";
    assert_eq!(
        extract_inline_marker_value(text, "LANG_SWITCH:"),
        Some("French".to_string())
    );
}

#[test]
fn test_extract_inline_marker_value_inline() {
    let text = "Some text LANG_SWITCH: Spanish in the middle.";
    assert_eq!(
        extract_inline_marker_value(text, "LANG_SWITCH:"),
        Some("Spanish in the middle.".to_string())
    );
}

#[test]
fn test_extract_inline_marker_value_empty() {
    assert!(extract_inline_marker_value("LANG_SWITCH: ", "LANG_SWITCH:").is_none());
}

#[test]
fn test_extract_inline_marker_value_not_present() {
    assert!(extract_inline_marker_value("No marker here.", "LANG_SWITCH:").is_none());
}

// --- strip_inline_marker edge cases ---

#[test]
fn test_strip_inline_marker_at_line_start() {
    let text = "Hello.\nLANG_SWITCH: French\nBye.";
    let result = strip_inline_marker(text, "LANG_SWITCH:");
    assert_eq!(result, "Hello.\nBye.");
}

#[test]
fn test_strip_inline_marker_inline_position() {
    let text = "Some response. LANG_SWITCH: Spanish";
    let result = strip_inline_marker(text, "LANG_SWITCH:");
    assert_eq!(result, "Some response.");
}

#[test]
fn test_strip_inline_marker_multiple_occurrences() {
    let text = "Text. REWARD: +1|a|b\nMore. REWARD: -1|c|d\nEnd.";
    let result = strip_inline_marker(text, "REWARD:");
    assert!(!result.contains("REWARD:"));
}
