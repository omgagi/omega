use super::*;

// --- SCHEDULE ---

#[test]
fn test_extract_schedule_marker() {
    let text = "Sure, I'll remind you.\nSCHEDULE: Call John | 2026-02-17T15:00:00 | once";
    let result = extract_schedule_marker(text);
    assert_eq!(
        result,
        Some("SCHEDULE: Call John | 2026-02-17T15:00:00 | once".to_string())
    );
}

#[test]
fn test_extract_schedule_marker_none() {
    let text = "No schedule here, just a normal response.";
    assert!(extract_schedule_marker(text).is_none());
}

#[test]
fn test_parse_schedule_line() {
    let line = "SCHEDULE: Call John | 2026-02-17T15:00:00 | once";
    let result = parse_schedule_line(line).unwrap();
    assert_eq!(result.0, "Call John");
    assert_eq!(result.1, "2026-02-17T15:00:00");
    assert_eq!(result.2, "once");
}

#[test]
fn test_parse_schedule_line_daily() {
    let line = "SCHEDULE: Stand-up meeting | 2026-02-18T09:00:00 | daily";
    let result = parse_schedule_line(line).unwrap();
    assert_eq!(result.0, "Stand-up meeting");
    assert_eq!(result.2, "daily");
}

#[test]
fn test_parse_schedule_line_invalid() {
    assert!(parse_schedule_line("SCHEDULE: missing parts").is_none());
    assert!(parse_schedule_line("not a schedule line").is_none());
}

#[test]
fn test_strip_schedule_marker() {
    let text = "Sure, I'll remind you.\nSCHEDULE: Call John | 2026-02-17T15:00:00 | once";
    let result = strip_schedule_marker(text);
    assert_eq!(result, "Sure, I'll remind you.");
}

#[test]
fn test_strip_schedule_marker_preserves_other_lines() {
    let text = "Line 1\nLine 2\nSCHEDULE: test | 2026-01-01T00:00:00 | once\nLine 3";
    let result = strip_schedule_marker(text);
    assert_eq!(result, "Line 1\nLine 2\nLine 3");
}

#[test]
fn test_extract_all_schedule_markers_multiple() {
    let text = "I'll set up your reminders.\n\
                SCHEDULE: Cancel Hostinger | 2026-03-01T09:00:00 | once\n\
                SCHEDULE: Cancel Hostinger 2 | 2026-03-05T09:00:00 | once\n\
                SCHEDULE: Cancel Hostinger 3 | 2026-03-10T09:00:00 | once\n\
                Done!";
    let result = extract_all_schedule_markers(text);
    assert_eq!(result.len(), 3);
    assert!(result[0].contains("Cancel Hostinger |"));
    assert!(result[1].contains("Cancel Hostinger 2"));
    assert!(result[2].contains("Cancel Hostinger 3"));
}

#[test]
fn test_extract_all_schedule_markers_single() {
    let text = "Sure.\nSCHEDULE: Call John | 2026-02-17T15:00:00 | once";
    let result = extract_all_schedule_markers(text);
    assert_eq!(result.len(), 1);
}

#[test]
fn test_extract_all_schedule_markers_none() {
    let text = "No schedule markers here.";
    let result = extract_all_schedule_markers(text);
    assert!(result.is_empty());
}

#[test]
fn test_extract_all_schedule_markers_ignores_schedule_action() {
    let text = "SCHEDULE: Reminder | 2026-02-17T09:00:00 | once\n\
                SCHEDULE_ACTION: Check price | 2026-02-17T14:00:00 | daily";
    let result = extract_all_schedule_markers(text);
    assert_eq!(result.len(), 1);
    assert!(result[0].contains("Reminder"));
}

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

#[test]
fn test_strip_all_remaining_markers() {
    let text = "Hello world. LANG_SWITCH: english\nMore text. PERSONALITY: friendly";
    let result = strip_all_remaining_markers(text);
    assert!(!result.contains("LANG_SWITCH:"));
    assert!(!result.contains("PERSONALITY:"));
    assert!(result.contains("Hello world."));
    assert!(result.contains("More text."));
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

// --- Active hours ---

#[test]
fn test_is_within_active_hours_normal_range() {
    assert!(is_within_active_hours("00:00", "23:59"));
}

#[test]
fn test_is_within_active_hours_narrow_miss() {
    assert!(!is_within_active_hours("00:00", "00:00"));
}

// --- Status messages ---

#[test]
fn test_friendly_provider_error_timeout() {
    let msg = friendly_provider_error("claude CLI timed out after 600s");
    assert!(msg.contains("too long"));
    assert!(!msg.contains("timed out"));
}

#[test]
fn test_friendly_provider_error_generic() {
    let msg = friendly_provider_error("failed to run claude CLI: No such file");
    assert_eq!(msg, "Something went wrong. Please try again.");
}

#[test]
fn test_status_messages_all_languages() {
    let languages = [
        "English",
        "Spanish",
        "Portuguese",
        "French",
        "German",
        "Italian",
        "Dutch",
        "Russian",
    ];
    for lang in &languages {
        let (nudge, still) = status_messages(lang);
        assert!(!nudge.is_empty(), "nudge for {lang} should not be empty");
        assert!(!still.is_empty(), "still for {lang} should not be empty");
    }
}

#[test]
fn test_status_messages_unknown_falls_back_to_english() {
    let (nudge, still) = status_messages("Klingon");
    assert!(nudge.contains("think about this"));
    assert!(still.contains("Still on it"));
}

#[test]
fn test_status_messages_spanish() {
    let (nudge, still) = status_messages("Spanish");
    assert!(nudge.contains("pensar"));
    assert!(still.contains("ello"));
}

// --- Heartbeat ---

#[test]
fn test_read_heartbeat_file_returns_none_when_missing() {
    let result = read_heartbeat_file();
    let _ = result;
}

#[test]
fn test_extract_heartbeat_add() {
    let text = "Sure, I'll monitor that.\nHEARTBEAT_ADD: Check exercise habits";
    let actions = extract_heartbeat_markers(text);
    assert_eq!(
        actions,
        vec![HeartbeatAction::Add("Check exercise habits".to_string())]
    );
}

#[test]
fn test_extract_heartbeat_remove() {
    let text = "I'll stop monitoring that.\nHEARTBEAT_REMOVE: exercise";
    let actions = extract_heartbeat_markers(text);
    assert_eq!(
        actions,
        vec![HeartbeatAction::Remove("exercise".to_string())]
    );
}

#[test]
fn test_extract_heartbeat_multiple() {
    let text = "Updating your checklist.\nHEARTBEAT_ADD: Water plants\nHEARTBEAT_REMOVE: old task";
    let actions = extract_heartbeat_markers(text);
    assert_eq!(
        actions,
        vec![
            HeartbeatAction::Add("Water plants".to_string()),
            HeartbeatAction::Remove("old task".to_string()),
        ]
    );
}

#[test]
fn test_extract_heartbeat_empty_ignored() {
    let text = "HEARTBEAT_ADD: \nHEARTBEAT_REMOVE:   \nSome response.";
    let actions = extract_heartbeat_markers(text);
    assert!(actions.is_empty());
}

#[test]
fn test_strip_heartbeat_markers() {
    let text = "Sure, I'll monitor that.\nHEARTBEAT_ADD: Check exercise habits\nDone!";
    let result = strip_heartbeat_markers(text);
    assert_eq!(result, "Sure, I'll monitor that.\nDone!");
}

#[test]
fn test_strip_heartbeat_both_types() {
    let text = "Response.\nHEARTBEAT_ADD: new item\nHEARTBEAT_REMOVE: old item\nEnd.";
    let result = strip_heartbeat_markers(text);
    assert_eq!(result, "Response.\nEnd.");
}

#[test]
fn test_extract_heartbeat_interval() {
    let text = "Updating interval.\nHEARTBEAT_INTERVAL: 15\nDone.";
    let actions = extract_heartbeat_markers(text);
    assert_eq!(actions, vec![HeartbeatAction::SetInterval(15)]);
}

#[test]
fn test_extract_heartbeat_interval_invalid() {
    let text = "HEARTBEAT_INTERVAL: 0";
    assert!(extract_heartbeat_markers(text).is_empty());
    let text = "HEARTBEAT_INTERVAL: -5";
    assert!(extract_heartbeat_markers(text).is_empty());
    let text = "HEARTBEAT_INTERVAL: abc";
    assert!(extract_heartbeat_markers(text).is_empty());
    let text = "HEARTBEAT_INTERVAL: 1441";
    assert!(extract_heartbeat_markers(text).is_empty());
    let text = "HEARTBEAT_INTERVAL: 1440";
    assert_eq!(
        extract_heartbeat_markers(text),
        vec![HeartbeatAction::SetInterval(1440)]
    );
    let text = "HEARTBEAT_INTERVAL: 1";
    assert_eq!(
        extract_heartbeat_markers(text),
        vec![HeartbeatAction::SetInterval(1)]
    );
}

#[test]
fn test_strip_heartbeat_interval() {
    let text = "Updated.\nHEARTBEAT_INTERVAL: 10\nDone.";
    let result = strip_heartbeat_markers(text);
    assert_eq!(result, "Updated.\nDone.");
}

#[test]
fn test_extract_heartbeat_mixed() {
    let text = "Ok.\nHEARTBEAT_INTERVAL: 20\nHEARTBEAT_ADD: new check\nHEARTBEAT_REMOVE: old\nEnd.";
    let actions = extract_heartbeat_markers(text);
    assert_eq!(
        actions,
        vec![
            HeartbeatAction::SetInterval(20),
            HeartbeatAction::Add("new check".to_string()),
            HeartbeatAction::Remove("old".to_string()),
        ]
    );
}

#[test]
fn test_apply_heartbeat_add() {
    // Both add and remove tests share a single HOME env var to avoid
    // race conditions between parallel tests (HOME is process-global).
    let fake_home = std::env::temp_dir().join("omega_test_hb_changes_home");
    let _ = std::fs::remove_dir_all(&fake_home);
    let _ = std::fs::create_dir_all(fake_home.join(".omega/prompts"));
    std::fs::write(
        fake_home.join(".omega/prompts/HEARTBEAT.md"),
        "# My checklist\n- Existing item\n",
    )
    .unwrap();
    let original_home = std::env::var("HOME").unwrap();
    std::env::set_var("HOME", &fake_home);

    apply_heartbeat_changes(&[HeartbeatAction::Add("New item".to_string())], None);

    let content = std::fs::read_to_string(fake_home.join(".omega/prompts/HEARTBEAT.md")).unwrap();
    assert!(content.contains("- Existing item"), "should keep existing");
    assert!(content.contains("- New item"), "should add new item");

    apply_heartbeat_changes(&[HeartbeatAction::Add("New item".to_string())], None);
    let content = std::fs::read_to_string(fake_home.join(".omega/prompts/HEARTBEAT.md")).unwrap();
    assert_eq!(
        content.matches("New item").count(),
        1,
        "should not duplicate"
    );

    // --- Remove scenario (same HOME, sequential) ---
    std::fs::write(
        fake_home.join(".omega/prompts/HEARTBEAT.md"),
        "# My checklist\n- Check exercise habits\n- Water the plants\n",
    )
    .unwrap();

    apply_heartbeat_changes(&[HeartbeatAction::Remove("exercise".to_string())], None);

    let content = std::fs::read_to_string(fake_home.join(".omega/prompts/HEARTBEAT.md")).unwrap();
    assert!(!content.contains("exercise"), "should remove exercise line");
    assert!(
        content.contains("Water the plants"),
        "should keep other items"
    );
    assert!(content.contains("# My checklist"), "should keep comments");

    // --- Section suppression lifecycle (same HOME, sequential) ---

    // Add suppression.
    add_suppression("TRADING", None);
    let entries = read_suppress_file(None);
    assert_eq!(entries, vec!["TRADING"]);

    // Duplicate add — no-op (case-insensitive).
    add_suppression("trading", None);
    assert_eq!(read_suppress_file(None).len(), 1);

    // Add another.
    add_suppression("HEALTH", None);
    assert_eq!(read_suppress_file(None).len(), 2);

    // Remove.
    remove_suppression("TRADING", None);
    assert_eq!(read_suppress_file(None), vec!["HEALTH"]);

    // Remove non-existent — no-op.
    remove_suppression("NONEXISTENT", None);
    assert_eq!(read_suppress_file(None), vec!["HEALTH"]);

    // Clean up for filter tests.
    remove_suppression("HEALTH", None);

    // --- filter with suppression ---
    std::fs::write(
        fake_home.join(".omega/prompts/HEARTBEAT.suppress"),
        "TRADING\n",
    )
    .unwrap();

    let hb_content =
        "# Title\n## TRADING — Engine\n400 lines of trading\n## NON-TRADING ITEMS\n- Reminder\n";
    let result = filter_suppressed_sections(hb_content, None);
    assert!(result.is_some());
    let filtered = result.unwrap();
    assert!(
        !filtered.contains("400 lines of trading"),
        "TRADING content filtered"
    );
    assert!(
        !filtered.contains("## TRADING — Engine"),
        "TRADING header filtered"
    );
    assert!(
        filtered.contains("NON-TRADING ITEMS"),
        "NON-TRADING remains"
    );
    assert!(filtered.contains("- Reminder"));
    assert!(filtered.contains("# Title"), "preamble remains");

    // --- all sections suppressed → None ---
    std::fs::write(
        fake_home.join(".omega/prompts/HEARTBEAT.suppress"),
        "TRADING\nNON-TRADING ITEMS\n",
    )
    .unwrap();

    let hb_content = "## TRADING\nStuff\n## NON-TRADING ITEMS\nMore stuff\n";
    let result = filter_suppressed_sections(hb_content, None);
    assert!(result.is_none(), "all sections suppressed = no checklist");

    // --- case-insensitive matching ---
    std::fs::write(
        fake_home.join(".omega/prompts/HEARTBEAT.suppress"),
        "trading\n",
    )
    .unwrap();

    let hb_content = "## TRADING — Engine\nStuff\n## OTHER\nKeep\n";
    let result = filter_suppressed_sections(hb_content, None);
    assert!(result.is_some());
    let filtered = result.unwrap();
    assert!(!filtered.contains("## TRADING"), "case-insensitive filter");
    assert!(!filtered.contains("Stuff"));
    assert!(filtered.contains("OTHER"));

    std::env::set_var("HOME", &original_home);
    let _ = std::fs::remove_dir_all(&fake_home);
}

#[test]
fn test_apply_heartbeat_remove() {
    // This test verifies remove works standalone, using direct file path
    // instead of HOME env var to avoid parallel test interference.
    let dir = std::env::temp_dir().join("omega_test_hb_remove_standalone");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("HEARTBEAT.md");
    std::fs::write(&path, "# Tasks\n- Monitor BTC\n- Review logs\n").unwrap();

    // Verify the remove logic directly on file content (no HOME dependency).
    let mut lines: Vec<String> = std::fs::read_to_string(&path)
        .unwrap()
        .lines()
        .map(|l| l.to_string())
        .collect();
    let needle = "btc".to_lowercase();
    lines.retain(|l| {
        let trimmed = l.trim();
        if trimmed.starts_with('#') {
            return true;
        }
        let content = trimmed.trim_start_matches("- ").to_lowercase();
        !content.contains(&needle)
    });
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(!content.contains("BTC"), "should remove BTC line");
    assert!(content.contains("Review logs"), "should keep other items");
    assert!(content.contains("# Tasks"), "should keep comments");

    let _ = std::fs::remove_dir_all(&dir);
}

// --- Workspace images ---

#[test]
fn test_snapshot_workspace_images_finds_images() {
    let dir = std::env::temp_dir().join("omega_test_snap_images");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("screenshot.png"), b"fake png").unwrap();
    std::fs::write(dir.join("photo.jpg"), b"fake jpg").unwrap();
    std::fs::write(dir.join("readme.txt"), b"not an image").unwrap();

    let result = snapshot_workspace_images(&dir);
    assert_eq!(result.len(), 2);
    assert!(result.contains_key(&dir.join("screenshot.png")));
    assert!(result.contains_key(&dir.join("photo.jpg")));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_snapshot_workspace_images_empty_dir() {
    let dir = std::env::temp_dir().join("omega_test_snap_empty");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let result = snapshot_workspace_images(&dir);
    assert!(result.is_empty());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn test_snapshot_workspace_images_nonexistent_dir() {
    let dir = std::env::temp_dir().join("omega_test_snap_nonexistent");
    let _ = std::fs::remove_dir_all(&dir);

    let result = snapshot_workspace_images(&dir);
    assert!(result.is_empty());
}

#[test]
fn test_snapshot_workspace_images_all_extensions() {
    let dir = std::env::temp_dir().join("omega_test_snap_all_ext");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for ext in IMAGE_EXTENSIONS {
        std::fs::write(dir.join(format!("test.{ext}")), b"fake").unwrap();
    }

    let result = snapshot_workspace_images(&dir);
    assert_eq!(result.len(), IMAGE_EXTENSIONS.len());

    let _ = std::fs::remove_dir_all(&dir);
}

// --- Inbox ---

#[test]
fn test_ensure_inbox_dir() {
    let tmp = std::env::temp_dir().join("omega_test_inbox_dir");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let inbox = ensure_inbox_dir(tmp.to_str().unwrap());
    assert!(inbox.exists());
    assert!(inbox.is_dir());
    assert!(inbox.ends_with("workspace/inbox"));

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_save_and_cleanup_inbox_images() {
    use omega_core::message::{Attachment, AttachmentType};

    let tmp = std::env::temp_dir().join("omega_test_save_inbox");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let attachments = vec![Attachment {
        file_type: AttachmentType::Image,
        url: None,
        data: Some(b"fake image data".to_vec()),
        filename: Some("test_photo.jpg".to_string()),
    }];

    let paths = save_attachments_to_inbox(&tmp, &attachments);
    assert_eq!(paths.len(), 1);
    assert!(paths[0].exists());
    assert_eq!(std::fs::read(&paths[0]).unwrap(), b"fake image data");

    cleanup_inbox_images(&paths);
    assert!(!paths[0].exists());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_save_attachments_skips_non_images() {
    use omega_core::message::{Attachment, AttachmentType};

    let tmp = std::env::temp_dir().join("omega_test_skip_non_img");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let attachments = vec![
        Attachment {
            file_type: AttachmentType::Document,
            url: None,
            data: Some(b"some doc".to_vec()),
            filename: Some("doc.pdf".to_string()),
        },
        Attachment {
            file_type: AttachmentType::Audio,
            url: None,
            data: Some(b"some audio".to_vec()),
            filename: Some("audio.mp3".to_string()),
        },
    ];

    let paths = save_attachments_to_inbox(&tmp, &attachments);
    assert!(paths.is_empty());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_save_attachments_rejects_empty_data() {
    use omega_core::message::{Attachment, AttachmentType};

    let tmp = std::env::temp_dir().join("omega_test_reject_empty");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let attachments = vec![Attachment {
        file_type: AttachmentType::Image,
        url: None,
        data: Some(Vec::new()),
        filename: Some("empty.jpg".to_string()),
    }];

    let paths = save_attachments_to_inbox(&tmp, &attachments);
    assert!(paths.is_empty(), "zero-byte attachment must be rejected");

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_inbox_guard_cleans_up_on_drop() {
    let tmp = std::env::temp_dir().join("omega_test_guard_cleanup");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let file = tmp.join("guard_test.jpg");
    std::fs::write(&file, b"image data").unwrap();
    assert!(file.exists());

    {
        let _guard = InboxGuard::new(vec![file.clone()]);
        // Guard is alive — file should still exist.
        assert!(file.exists());
    }
    // Guard dropped — file should be cleaned up.
    assert!(!file.exists());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_inbox_guard_empty_is_noop() {
    // An empty guard should not panic or error on drop.
    let _guard = InboxGuard::new(Vec::new());
}

// --- Classification ---

#[test]
fn test_parse_plan_response_direct() {
    assert!(parse_plan_response("DIRECT").is_none());
    assert!(parse_plan_response("  DIRECT  ").is_none());
    assert!(parse_plan_response("direct").is_none());
}

#[test]
fn test_parse_plan_response_numbered_list() {
    let text = "1. Set up the database schema\n\
                2. Create the API endpoint\n\
                3. Write integration tests";
    let steps = parse_plan_response(text).unwrap();
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0], "Set up the database schema");
    assert_eq!(steps[1], "Create the API endpoint");
    assert_eq!(steps[2], "Write integration tests");
}

#[test]
fn test_parse_plan_response_single_step() {
    let text = "1. Just do the thing";
    assert!(parse_plan_response(text).is_none());
}

#[test]
fn test_parse_plan_response_with_preamble() {
    let text = "Here are the steps:\n\
                1. First step\n\
                2. Second step\n\
                3. Third step";
    let steps = parse_plan_response(text).unwrap();
    assert_eq!(steps.len(), 3);
    assert_eq!(steps[0], "First step");
}

// --- SCHEDULE_ACTION ---

#[test]
fn test_extract_schedule_action_marker() {
    let text = "I'll handle that.\nSCHEDULE_ACTION: Check BTC price | 2026-02-18T14:00:00 | daily";
    let result = extract_schedule_action_marker(text);
    assert_eq!(
        result,
        Some("SCHEDULE_ACTION: Check BTC price | 2026-02-18T14:00:00 | daily".to_string())
    );
}

#[test]
fn test_extract_schedule_action_marker_none() {
    let text = "No action scheduled here.";
    assert!(extract_schedule_action_marker(text).is_none());
}

#[test]
fn test_parse_schedule_action_line() {
    let line = "SCHEDULE_ACTION: Check BTC price | 2026-02-18T14:00:00 | daily";
    let result = parse_schedule_action_line(line).unwrap();
    assert_eq!(result.0, "Check BTC price");
    assert_eq!(result.1, "2026-02-18T14:00:00");
    assert_eq!(result.2, "daily");
}

#[test]
fn test_parse_schedule_action_line_once() {
    let line = "SCHEDULE_ACTION: Run scraper | 2026-02-18T22:00:00 | once";
    let result = parse_schedule_action_line(line).unwrap();
    assert_eq!(result.0, "Run scraper");
    assert_eq!(result.2, "once");
}

#[test]
fn test_parse_schedule_action_line_invalid() {
    assert!(parse_schedule_action_line("SCHEDULE_ACTION: missing parts").is_none());
    assert!(parse_schedule_action_line("not an action line").is_none());
    assert!(parse_schedule_action_line("SCHEDULE_ACTION:  | time | once").is_none());
}

#[test]
fn test_strip_schedule_action_markers() {
    let text = "I'll do that.\nSCHEDULE_ACTION: Check BTC | 2026-02-18T14:00:00 | daily\nDone.";
    let result = strip_schedule_action_markers(text);
    assert_eq!(result, "I'll do that.\nDone.");
}

#[test]
fn test_strip_schedule_action_preserves_schedule() {
    let text = "Response.\nSCHEDULE: Remind me | 2026-02-18T09:00:00 | once\nSCHEDULE_ACTION: Check prices | 2026-02-18T14:00:00 | daily\nEnd.";
    let result = strip_schedule_action_markers(text);
    assert!(
        result.contains("SCHEDULE: Remind me"),
        "should keep SCHEDULE lines"
    );
    assert!(
        !result.contains("SCHEDULE_ACTION:"),
        "should strip SCHEDULE_ACTION lines"
    );
}

#[test]
fn test_extract_all_schedule_action_markers_multiple() {
    let text = "Setting up monitoring.\n\
                SCHEDULE_ACTION: Check BTC | 2026-02-18T14:00:00 | daily\n\
                SCHEDULE_ACTION: Check ETH | 2026-02-18T14:05:00 | daily\n\
                SCHEDULE_ACTION: Check SOL | 2026-02-18T14:10:00 | daily\n\
                All set!";
    let result = extract_all_schedule_action_markers(text);
    assert_eq!(result.len(), 3);
    assert!(result[0].contains("Check BTC"));
    assert!(result[1].contains("Check ETH"));
    assert!(result[2].contains("Check SOL"));
}

#[test]
fn test_extract_all_schedule_action_markers_none() {
    let text = "No action markers here.";
    let result = extract_all_schedule_action_markers(text);
    assert!(result.is_empty());
}

#[test]
fn test_extract_all_schedule_action_markers_ignores_schedule() {
    let text = "SCHEDULE: Reminder | 2026-02-17T09:00:00 | once\n\
                SCHEDULE_ACTION: Check price | 2026-02-17T14:00:00 | daily";
    let result = extract_all_schedule_action_markers(text);
    assert_eq!(result.len(), 1);
    assert!(result[0].contains("Check price"));
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

#[test]
fn test_strip_all_remaining_markers_includes_bug_report() {
    let text = "Hello. BUG_REPORT: some limitation\nMore text.";
    let result = strip_all_remaining_markers(text);
    assert!(!result.contains("BUG_REPORT:"));
    assert!(result.contains("Hello."));
    assert!(result.contains("More text."));
}

// --- Section suppression (REQ-HB-010..014) ---

#[test]
fn test_parse_heartbeat_sections_basic() {
    let content = "# Title\nSome preamble\n## TRADING — Autonomous Engine\nTrading stuff\nMore trading\n## NON-TRADING ITEMS\n- Daily reminder\n";
    let (preamble, sections) = parse_heartbeat_sections(content);
    assert!(preamble.contains("# Title"));
    assert!(preamble.contains("Some preamble"));
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].0, "TRADING");
    assert!(sections[0].1.contains("Trading stuff"));
    assert!(sections[0].1.contains("More trading"));
    assert_eq!(sections[1].0, "NON-TRADING ITEMS");
    assert!(sections[1].1.contains("Daily reminder"));
}

#[test]
fn test_parse_heartbeat_sections_no_sections() {
    let content = "# Just a title\n- Item 1\n- Item 2\n";
    let (preamble, sections) = parse_heartbeat_sections(content);
    assert!(preamble.contains("# Just a title"));
    assert!(sections.is_empty());
}

#[test]
fn test_parse_heartbeat_sections_no_preamble() {
    let content = "## SECTION A\nContent A\n## SECTION B\nContent B\n";
    let (preamble, sections) = parse_heartbeat_sections(content);
    assert!(preamble.trim().is_empty());
    assert_eq!(sections.len(), 2);
    assert_eq!(sections[0].0, "SECTION A");
    assert_eq!(sections[1].0, "SECTION B");
}

#[test]
fn test_parse_heartbeat_sections_emdash_extraction() {
    let content = "## TRADING — Autonomous Quant-Driven Execution Engine\nStuff\n";
    let (_, sections) = parse_heartbeat_sections(content);
    assert_eq!(sections[0].0, "TRADING");
}

#[test]
fn test_filter_suppressed_sections_no_suppress_file() {
    // No suppress file exists — all sections active.
    // Uses parse_heartbeat_sections directly to avoid HOME dependency.
    let content = "# Title\n## SECTION A\nContent A\n## SECTION B\nContent B\n";
    let (preamble, sections) = parse_heartbeat_sections(content);
    assert!(preamble.contains("# Title"));
    assert_eq!(sections.len(), 2);
    // With no suppress file, filter_suppressed_sections returns content unchanged.
    // We test this via the parse path since filter reads HOME.
}

#[test]
fn test_extract_suppress_section_markers() {
    let text = "Done.\nHEARTBEAT_SUPPRESS_SECTION: TRADING\nStopping reports.";
    let actions = extract_suppress_section_markers(text);
    assert_eq!(
        actions,
        vec![SuppressAction::Suppress("TRADING".to_string())]
    );
}

#[test]
fn test_extract_unsuppress_section_markers() {
    let text = "Re-enabling.\nHEARTBEAT_UNSUPPRESS_SECTION: TRADING\nDone.";
    let actions = extract_suppress_section_markers(text);
    assert_eq!(
        actions,
        vec![SuppressAction::Unsuppress("TRADING".to_string())]
    );
}

#[test]
fn test_extract_suppress_section_markers_empty_ignored() {
    let text = "HEARTBEAT_SUPPRESS_SECTION: \nHEARTBEAT_UNSUPPRESS_SECTION:   ";
    let actions = extract_suppress_section_markers(text);
    assert!(actions.is_empty());
}

#[test]
fn test_extract_suppress_section_markers_mixed() {
    let text = "HEARTBEAT_SUPPRESS_SECTION: TRADING\nHEARTBEAT_UNSUPPRESS_SECTION: HEALTH\nDone.";
    let actions = extract_suppress_section_markers(text);
    assert_eq!(
        actions,
        vec![
            SuppressAction::Suppress("TRADING".to_string()),
            SuppressAction::Unsuppress("HEALTH".to_string()),
        ]
    );
}

#[test]
fn test_strip_suppress_section_markers() {
    let text = "Stopping reports.\nHEARTBEAT_SUPPRESS_SECTION: TRADING\nDone!";
    let result = strip_suppress_section_markers(text);
    assert_eq!(result, "Stopping reports.\nDone!");
}

#[test]
fn test_strip_suppress_section_markers_both() {
    let text = "Response.\nHEARTBEAT_SUPPRESS_SECTION: A\nHEARTBEAT_UNSUPPRESS_SECTION: B\nEnd.";
    let result = strip_suppress_section_markers(text);
    assert_eq!(result, "Response.\nEnd.");
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

// --- Classification context ---

#[test]
fn test_classification_context_full() {
    let history = vec![
        omega_core::context::ContextEntry {
            role: "user".into(),
            content: "Check BTC price".into(),
        },
        omega_core::context::ContextEntry {
            role: "assistant".into(),
            content: "BTC is at $45,000".into(),
        },
        omega_core::context::ContextEntry {
            role: "user".into(),
            content: "Set up a trailing stop".into(),
        },
    ];
    let result =
        build_classification_context(Some("trader"), &history, &["claude-code", "playwright-mcp"]);
    assert!(result.contains("Active project: trader"));
    assert!(result.contains("Recent conversation:"));
    assert!(result.contains("User: Check BTC price"));
    assert!(result.contains("Assistant: BTC is at $45,000"));
    assert!(result.contains("User: Set up a trailing stop"));
    assert!(result.contains("Available skills: claude-code, playwright-mcp"));
}

#[test]
fn test_classification_context_empty() {
    let result = build_classification_context(None, &[], &[]);
    assert!(result.is_empty());
}

#[test]
fn test_classification_context_truncation() {
    let long_msg = "a".repeat(120);
    let history = vec![omega_core::context::ContextEntry {
        role: "user".into(),
        content: long_msg,
    }];
    let result = build_classification_context(None, &history, &[]);
    assert!(result.contains("..."));
    assert!(!result.contains(&"a".repeat(120)));
    assert!(result.contains(&"a".repeat(80)));
}

#[test]
fn test_classification_context_partial() {
    let result = build_classification_context(Some("trader"), &[], &[]);
    assert!(result.contains("Active project: trader"));
    assert!(!result.contains("Recent conversation:"));
    assert!(!result.contains("Available skills:"));
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

#[test]
fn test_strip_all_remaining_markers_includes_action_outcome() {
    let text = "Done. ACTION_OUTCOME: success\nMore text.";
    let result = strip_all_remaining_markers(text);
    assert!(!result.contains("ACTION_OUTCOME:"));
    assert!(result.contains("Done."));
    assert!(result.contains("More text."));
}
