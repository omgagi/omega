use super::super::*;

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
