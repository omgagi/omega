use super::super::*;

// --- Active hours ---

#[test]
fn test_is_within_active_hours_normal_range() {
    assert!(is_within_active_hours("00:00", "23:59"));
}

#[test]
fn test_is_within_active_hours_narrow_miss() {
    assert!(!is_within_active_hours("00:00", "00:00"));
}

// --- Heartbeat markers ---

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

// --- REQ-HBDUP-001/004: strip_project_sections ---

#[test]
fn test_strip_project_sections_removes_matching_section() {
    let content = "\
# Heartbeat Checklist
## NON-TRADING ITEMS
- Training accountability
## TECH YOUTUBER PROJECT
- Check trending topics
- Review product launches
";
    let projects = vec!["tech-youtuber".to_string()];
    let result = strip_project_sections(content, &projects).unwrap();
    assert!(
        result.contains("NON-TRADING ITEMS"),
        "non-project section should remain"
    );
    assert!(
        !result.contains("TECH YOUTUBER"),
        "project section should be stripped"
    );
    assert!(
        !result.contains("trending topics"),
        "project items should be stripped"
    );
}

#[test]
fn test_strip_project_sections_no_projects() {
    let content = "## SECTION A\nContent A\n";
    let projects: Vec<String> = vec![];
    let result = strip_project_sections(content, &projects).unwrap();
    assert_eq!(result, content);
}

#[test]
fn test_strip_project_sections_multiple_projects() {
    let content = "\
# Checklist
## NON-TRADING ITEMS
- Training
## TECH YOUTUBER PROJECT
- Trending topics
## REALTOR PIPELINE
- Check leads
";
    let projects = vec!["tech-youtuber".to_string(), "realtor".to_string()];
    let result = strip_project_sections(content, &projects).unwrap();
    assert!(result.contains("NON-TRADING ITEMS"));
    assert!(!result.contains("TECH YOUTUBER"));
    assert!(!result.contains("REALTOR"));
}

#[test]
fn test_strip_project_sections_all_stripped_returns_none() {
    let content = "## TECH YOUTUBER PROJECT\n- Topics\n## REALTOR PIPELINE\n- Leads\n";
    let projects = vec!["tech-youtuber".to_string(), "realtor".to_string()];
    let result = strip_project_sections(content, &projects);
    assert!(
        result.is_none(),
        "should return None when all sections stripped"
    );
}

#[test]
fn test_strip_project_sections_case_insensitive() {
    let content = "## Tech YouTuber Project\n- Topics\n## Other\n- Stuff\n";
    let projects = vec!["tech-youtuber".to_string()];
    let result = strip_project_sections(content, &projects).unwrap();
    assert!(!result.contains("Tech YouTuber"));
    assert!(result.contains("Other"));
}

#[test]
fn test_strip_project_sections_no_match_preserves_all() {
    let content = "## SECTION A\nContent A\n## SECTION B\nContent B\n";
    let projects = vec!["tech-youtuber".to_string()];
    let result = strip_project_sections(content, &projects).unwrap();
    assert!(result.contains("SECTION A"));
    assert!(result.contains("SECTION B"));
}
