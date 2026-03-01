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
