//! Tests for the Telegram channel module.

use super::types::*;
use crate::utils::split_message;

#[test]
fn test_split_short_message() {
    let chunks = split_message("hello", 4096);
    assert_eq!(chunks, vec!["hello"]);
}

#[test]
fn test_split_long_message() {
    let text = "a\n".repeat(3000);
    let chunks = split_message(&text, 4096);
    assert!(chunks.len() >= 2);
    for chunk in &chunks {
        assert!(chunk.len() <= 4096);
    }
}

#[test]
fn test_tg_chat_group_detection() {
    let group: TgChat = serde_json::from_str(r#"{"id": -100123, "type": "group"}"#).unwrap();
    assert_eq!(group.chat_type, "group");

    let supergroup: TgChat =
        serde_json::from_str(r#"{"id": -100456, "type": "supergroup"}"#).unwrap();
    assert_eq!(supergroup.chat_type, "supergroup");

    let private: TgChat = serde_json::from_str(r#"{"id": 789, "type": "private"}"#).unwrap();
    assert_eq!(private.chat_type, "private");

    // is_group check
    assert!(matches!(group.chat_type.as_str(), "group" | "supergroup"));
    assert!(matches!(
        supergroup.chat_type.as_str(),
        "group" | "supergroup"
    ));
    assert!(!matches!(
        private.chat_type.as_str(),
        "group" | "supergroup"
    ));
}

#[test]
fn test_tg_chat_type_defaults_when_missing() {
    let chat: TgChat = serde_json::from_str(r#"{"id": 123}"#).unwrap();
    assert_eq!(chat.chat_type, "");
    // Missing type should not be detected as group.
    assert!(!matches!(chat.chat_type.as_str(), "group" | "supergroup"));
}

#[test]
fn test_tg_message_with_voice() {
    let json = r#"{
        "message_id": 1,
        "chat": {"id": 100, "type": "private"},
        "voice": {
            "file_id": "abc123",
            "duration": 5,
            "mime_type": "audio/ogg",
            "file_size": 12345
        }
    }"#;
    let msg: TgMessage = serde_json::from_str(json).unwrap();
    assert!(msg.text.is_none());
    assert!(msg.voice.is_some());
    let voice = msg.voice.unwrap();
    assert_eq!(voice.file_id, "abc123");
    assert_eq!(voice.duration, 5);
    assert_eq!(voice.mime_type.as_deref(), Some("audio/ogg"));
}

#[test]
fn test_tg_message_with_photo() {
    let json = r#"{
        "message_id": 3,
        "chat": {"id": 100, "type": "private"},
        "photo": [
            {"file_id": "small", "width": 90, "height": 90, "file_size": 1000},
            {"file_id": "medium", "width": 320, "height": 320, "file_size": 5000},
            {"file_id": "large", "width": 800, "height": 800, "file_size": 20000}
        ],
        "caption": "Check this out"
    }"#;
    let msg: TgMessage = serde_json::from_str(json).unwrap();
    assert!(msg.text.is_none());
    assert!(msg.voice.is_none());
    let photos = msg.photo.unwrap();
    assert_eq!(photos.len(), 3);
    assert_eq!(photos.last().unwrap().file_id, "large");
    assert_eq!(photos.last().unwrap().width, 800);
    assert_eq!(msg.caption.as_deref(), Some("Check this out"));
}

#[test]
fn test_tg_message_with_photo_no_caption() {
    let json = r#"{
        "message_id": 4,
        "chat": {"id": 100, "type": "private"},
        "photo": [
            {"file_id": "only", "width": 640, "height": 480}
        ]
    }"#;
    let msg: TgMessage = serde_json::from_str(json).unwrap();
    assert!(msg.photo.is_some());
    assert!(msg.caption.is_none());
    let photos = msg.photo.unwrap();
    assert_eq!(photos.len(), 1);
    assert_eq!(photos[0].file_id, "only");
    assert!(photos[0].file_size.is_none());
}

#[test]
fn test_tg_message_text_only() {
    let json = r#"{
        "message_id": 2,
        "chat": {"id": 100, "type": "private"},
        "text": "hello"
    }"#;
    let msg: TgMessage = serde_json::from_str(json).unwrap();
    assert_eq!(msg.text.as_deref(), Some("hello"));
    assert!(msg.voice.is_none());
}

#[test]
fn test_split_message_multibyte() {
    // Each Cyrillic 'Б' is 2 bytes in UTF-8. 100 chars = 200 bytes.
    let text = "\u{0411}".repeat(100);
    assert_eq!(text.len(), 200);
    // max_len=151 lands at byte 151, which is inside a 2-byte char (chars end at even byte offsets)
    let chunks = split_message(&text, 151);
    assert!(!chunks.is_empty());
    for chunk in &chunks {
        // Each chunk must be valid UTF-8 and not exceed max_len + 1 byte (char boundary adjustment)
        assert!(chunk.len() <= 152);
    }
}

#[test]
fn test_split_message_emoji_boundary() {
    // Each 🌍 is 4 bytes. 50 emojis = 200 bytes.
    let text = "\u{1f30d}".repeat(50);
    assert_eq!(text.len(), 200);
    // max_len=10 means 2.5 emojis per chunk; byte 10 falls inside the 3rd emoji
    let chunks = split_message(&text, 10);
    assert!(!chunks.is_empty());
    // Verify we got all the content back
    let reassembled: String = chunks.iter().copied().collect();
    assert_eq!(reassembled, text);
}
