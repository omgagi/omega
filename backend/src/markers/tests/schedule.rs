use super::super::*;

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
