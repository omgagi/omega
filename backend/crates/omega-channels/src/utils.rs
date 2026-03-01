//! Shared utilities for channel implementations.

/// Split a long message into chunks that respect a platform's character limit.
///
/// All slice boundaries are aligned to UTF-8 char boundaries to avoid panics
/// on multi-byte content (Cyrillic, CJK, emoji, etc.). Prefers splitting at
/// newline boundaries when possible.
pub fn split_message(text: &str, max_len: usize) -> Vec<&str> {
    if text.len() <= max_len {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = text.floor_char_boundary((start + max_len).min(text.len()));
        let break_at = if end < text.len() {
            text[start..end]
                .rfind('\n')
                .map(|i| start + i + 1)
                .unwrap_or(end)
        } else {
            end
        };
        chunks.push(&text[start..break_at]);
        start = break_at;
    }

    chunks
}
