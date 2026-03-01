use super::super::*;

// --- LANG_SWITCH ---

#[test]
fn test_extract_lang_switch() {
    let text = "Sure, I'll speak French now.\nLANG_SWITCH: French";
    assert_eq!(extract_lang_switch(text), Some("French".to_string()));
}

#[test]
fn test_extract_lang_switch_inline() {
    let text = "Estoy usando el modelo Claude. LANG_SWITCH: espanol";
    assert_eq!(extract_lang_switch(text), Some("espanol".to_string()));
}

#[test]
fn test_extract_lang_switch_none() {
    assert!(extract_lang_switch("Just a normal response.").is_none());
}

#[test]
fn test_strip_lang_switch() {
    let text = "Sure, I'll speak French now.\nLANG_SWITCH: French";
    assert_eq!(strip_lang_switch(text), "Sure, I'll speak French now.");
}

#[test]
fn test_strip_lang_switch_inline() {
    let text = "Estoy usando el modelo Claude. LANG_SWITCH: espanol";
    assert_eq!(strip_lang_switch(text), "Estoy usando el modelo Claude.");
}

// --- PROJECT ---

#[test]
fn test_extract_project_activate() {
    let text = "I've created a project for you.\nPROJECT_ACTIVATE: real-estate";
    assert_eq!(
        extract_project_activate(text),
        Some("real-estate".to_string())
    );
}

#[test]
fn test_extract_project_activate_none() {
    assert!(extract_project_activate("Just a normal response.").is_none());
}

#[test]
fn test_extract_project_activate_empty_name() {
    assert!(extract_project_activate("PROJECT_ACTIVATE: ").is_none());
}

#[test]
fn test_has_project_deactivate() {
    let text = "Project deactivated.\nPROJECT_DEACTIVATE";
    assert!(has_project_deactivate(text));
}

#[test]
fn test_has_project_deactivate_false() {
    assert!(!has_project_deactivate("No marker here."));
}

#[test]
fn test_strip_project_markers() {
    let text = "I've set up the project.\nPROJECT_ACTIVATE: stocks\nLet me know if you need more.";
    let result = strip_project_markers(text);
    assert_eq!(
        result,
        "I've set up the project.\nLet me know if you need more."
    );
}

#[test]
fn test_strip_project_markers_deactivate() {
    let text = "Done, project deactivated.\nPROJECT_DEACTIVATE";
    let result = strip_project_markers(text);
    assert_eq!(result, "Done, project deactivated.");
}

#[test]
fn test_strip_project_markers_both() {
    let text = "Switching.\nPROJECT_DEACTIVATE\nPROJECT_ACTIVATE: new-proj\nEnjoy!";
    let result = strip_project_markers(text);
    assert_eq!(result, "Switching.\nEnjoy!");
}

// --- PERSONALITY ---

#[test]
fn test_extract_personality() {
    let text = "Sure, I'll be more casual.\nPERSONALITY: casual and friendly";
    assert_eq!(
        extract_personality(text),
        Some("casual and friendly".to_string())
    );
}

#[test]
fn test_extract_personality_none() {
    assert!(extract_personality("Just a normal response.").is_none());
}

#[test]
fn test_extract_personality_empty() {
    assert!(extract_personality("PERSONALITY: ").is_none());
}

#[test]
fn test_extract_personality_reset() {
    let text = "Back to defaults.\nPERSONALITY: reset";
    assert_eq!(extract_personality(text), Some("reset".to_string()));
}

#[test]
fn test_strip_personality() {
    let text = "Sure, I'll adjust.\nPERSONALITY: formal and precise\nLet me know.";
    assert_eq!(strip_personality(text), "Sure, I'll adjust.\nLet me know.");
}

// --- FORGET ---

#[test]
fn test_has_forget_marker() {
    let text = "Starting fresh.\nFORGET_CONVERSATION\nDone!";
    assert!(has_forget_marker(text));
}

#[test]
fn test_has_forget_marker_false() {
    assert!(!has_forget_marker("No marker here."));
}

#[test]
fn test_has_forget_marker_partial_no_match() {
    assert!(!has_forget_marker("FORGET_CONVERSATION_EXTRA"));
}

#[test]
fn test_strip_forget_marker() {
    let text = "Clearing now.\nFORGET_CONVERSATION\nAll fresh!";
    assert_eq!(strip_forget_marker(text), "Clearing now.\nAll fresh!");
}

// --- CANCEL_TASK ---

#[test]
fn test_extract_all_cancel_tasks_single() {
    let text = "I'll cancel that.\nCANCEL_TASK: a1b2c3d4";
    let ids = extract_all_cancel_tasks(text);
    assert_eq!(ids, vec!["a1b2c3d4"]);
}

#[test]
fn test_extract_all_cancel_tasks_none_found() {
    assert!(extract_all_cancel_tasks("Just a normal response.").is_empty());
}

#[test]
fn test_extract_all_cancel_tasks_empty_value() {
    assert!(extract_all_cancel_tasks("CANCEL_TASK: ").is_empty());
}

#[test]
fn test_extract_all_cancel_tasks_multiple() {
    let text =
        "Cancelling all.\nCANCEL_TASK: aaa111\nCANCEL_TASK: bbb222\nCANCEL_TASK: ccc333\nDone.";
    let ids = extract_all_cancel_tasks(text);
    assert_eq!(ids, vec!["aaa111", "bbb222", "ccc333"]);
}

#[test]
fn test_extract_all_cancel_tasks_skips_empty() {
    let text = "CANCEL_TASK: abc\nCANCEL_TASK: \nCANCEL_TASK: def";
    let ids = extract_all_cancel_tasks(text);
    assert_eq!(ids, vec!["abc", "def"]);
}

#[test]
fn test_strip_cancel_task() {
    let text = "Cancelled.\nCANCEL_TASK: abc123\nDone.";
    assert_eq!(strip_cancel_task(text), "Cancelled.\nDone.");
}

// --- UPDATE_TASK ---

#[test]
fn test_extract_all_update_tasks_single_line() {
    let text =
        "I've updated that.\nUPDATE_TASK: a1b2c3d4 | New description | 2026-03-01T09:00:00 | daily";
    let lines = extract_all_update_tasks(text);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].contains("a1b2c3d4"));
}

#[test]
fn test_extract_all_update_tasks_none_found() {
    assert!(extract_all_update_tasks("Just a normal response.").is_empty());
}

#[test]
fn test_parse_update_task_line_all_fields() {
    let line = "UPDATE_TASK: abc123 | New desc | 2026-03-01T09:00:00 | daily";
    let (id, desc, due_at, repeat) = parse_update_task_line(line).unwrap();
    assert_eq!(id, "abc123");
    assert_eq!(desc, Some("New desc".to_string()));
    assert_eq!(due_at, Some("2026-03-01T09:00:00".to_string()));
    assert_eq!(repeat, Some("daily".to_string()));
}

#[test]
fn test_parse_update_task_line_empty_fields() {
    let line = "UPDATE_TASK: abc123 | | | daily";
    let (id, desc, due_at, repeat) = parse_update_task_line(line).unwrap();
    assert_eq!(id, "abc123");
    assert!(desc.is_none());
    assert!(due_at.is_none());
    assert_eq!(repeat, Some("daily".to_string()));
}

#[test]
fn test_parse_update_task_line_only_description() {
    let line = "UPDATE_TASK: abc123 | Updated reminder text | | ";
    let (id, desc, due_at, repeat) = parse_update_task_line(line).unwrap();
    assert_eq!(id, "abc123");
    assert_eq!(desc, Some("Updated reminder text".to_string()));
    assert!(due_at.is_none());
    assert!(repeat.is_none());
}

#[test]
fn test_parse_update_task_line_invalid() {
    assert!(parse_update_task_line("UPDATE_TASK: missing pipes").is_none());
    assert!(parse_update_task_line("not an update line").is_none());
    assert!(parse_update_task_line("UPDATE_TASK:  | desc | time | once").is_none());
}

#[test]
fn test_extract_all_update_tasks_multiple() {
    let text =
        "Updating.\nUPDATE_TASK: aaa | New A | | daily\nUPDATE_TASK: bbb | New B | | weekly\nDone.";
    let lines = extract_all_update_tasks(text);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("aaa"));
    assert!(lines[1].contains("bbb"));
}

#[test]
fn test_strip_update_task() {
    let text = "Updated.\nUPDATE_TASK: abc123 | | | daily\nDone.";
    assert_eq!(strip_update_task(text), "Updated.\nDone.");
}

// --- PURGE_FACTS ---

#[test]
fn test_has_purge_marker() {
    let text = "Deleting everything.\nPURGE_FACTS\nClean slate.";
    assert!(has_purge_marker(text));
}

#[test]
fn test_has_purge_marker_false() {
    assert!(!has_purge_marker("No purge here."));
}

#[test]
fn test_has_purge_marker_partial_no_match() {
    assert!(!has_purge_marker("PURGE_FACTS_EXTRA"));
}

#[test]
fn test_strip_purge_marker() {
    let text = "All gone.\nPURGE_FACTS\nStarting over.";
    assert_eq!(strip_purge_marker(text), "All gone.\nStarting over.");
}

// --- WHATSAPP_QR ---

#[test]
fn test_has_whatsapp_qr_marker() {
    let text = "Let me set that up.\nWHATSAPP_QR\nDone.";
    assert!(has_whatsapp_qr_marker(text));
}

#[test]
fn test_strip_whatsapp_qr_marker() {
    let text = "Setting up.\nWHATSAPP_QR\nAll done.";
    assert_eq!(strip_whatsapp_qr_marker(text), "Setting up.\nAll done.");
}
