use super::qr::{generate_qr_image, generate_qr_terminal};
use super::send::{sanitize_for_whatsapp, split_message, RETRY_DELAYS_MS};
use wacore_binary::jid::{Jid, JidExt};

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
fn test_jid_group_detection() {
    // Group JIDs use @g.us server.
    let group_jid: Jid = "120363001234567890@g.us".parse().unwrap();
    assert!(group_jid.is_group(), "g.us JID should be detected as group");

    // Personal JIDs use @s.whatsapp.net server.
    let personal_jid: Jid = "5511999887766@s.whatsapp.net".parse().unwrap();
    assert!(
        !personal_jid.is_group(),
        "s.whatsapp.net JID should not be group"
    );
}

#[test]
fn test_generate_qr_terminal() {
    let result = generate_qr_terminal("test-data");
    assert!(result.is_ok());
    let qr = result.unwrap();
    assert!(!qr.is_empty());
}

#[test]
fn test_generate_qr_image() {
    let result = generate_qr_image("test-data");
    assert!(result.is_ok());
    let png = result.unwrap();
    // PNG magic bytes.
    assert_eq!(&png[..4], &[0x89, 0x50, 0x4E, 0x47]);
}

#[test]
fn test_sanitize_headers() {
    assert_eq!(sanitize_for_whatsapp("## Hello World"), "*HELLO WORLD*");
    assert_eq!(sanitize_for_whatsapp("# Big Title"), "*BIG TITLE*");
    assert_eq!(sanitize_for_whatsapp("### Small"), "*SMALL*");
}

#[test]
fn test_sanitize_bold() {
    assert_eq!(
        sanitize_for_whatsapp("this is **bold** text"),
        "this is *bold* text"
    );
}

#[test]
fn test_sanitize_links() {
    assert_eq!(
        sanitize_for_whatsapp("check [Google](https://google.com) out"),
        "check Google (https://google.com) out"
    );
}

#[test]
fn test_sanitize_tables() {
    let input = "| Name | Age |\n|------|-----|\n| Alice | 30 |";
    let result = sanitize_for_whatsapp(input);
    assert!(result.contains("- Name | Age"), "should convert header row");
    assert!(result.contains("- Alice | 30"), "should convert data row");
    assert!(!result.contains("------"), "should remove separator row");
}

#[test]
fn test_sanitize_horizontal_rules() {
    let input = "above\n---\nbelow";
    let result = sanitize_for_whatsapp(input);
    assert_eq!(result, "above\nbelow");
}

#[test]
fn test_sanitize_passthrough() {
    // Native WhatsApp formatting should pass through unchanged.
    assert_eq!(sanitize_for_whatsapp("*bold*"), "*bold*");
    assert_eq!(sanitize_for_whatsapp("_italic_"), "_italic_");
    assert_eq!(sanitize_for_whatsapp("~strike~"), "~strike~");
    assert_eq!(sanitize_for_whatsapp("```code```"), "```code```");
}

#[test]
fn test_sanitize_preserves_plain_text() {
    let plain = "Hello, how are you doing today?";
    assert_eq!(sanitize_for_whatsapp(plain), plain);
}

#[test]
fn test_retry_delays_exponential() {
    assert_eq!(RETRY_DELAYS_MS.len(), 3, "should have 3 retry attempts");
    assert_eq!(RETRY_DELAYS_MS[0], 500, "first delay 500ms");
    assert_eq!(RETRY_DELAYS_MS[1], 1000, "second delay 1s");
    assert_eq!(RETRY_DELAYS_MS[2], 2000, "third delay 2s");
    // Verify exponential pattern: each delay is 2x the previous.
    assert_eq!(RETRY_DELAYS_MS[1], RETRY_DELAYS_MS[0] * 2);
    assert_eq!(RETRY_DELAYS_MS[2], RETRY_DELAYS_MS[1] * 2);
}
