//! Message sending utilities — sanitization, chunking, and retry logic.

use omega_core::error::OmegaError;
use tracing::{error, warn};
use wacore_binary::jid::Jid;
use whatsapp_rust::client::Client;

/// Retry delays for exponential backoff: 500ms, 1s, 2s.
pub(super) const RETRY_DELAYS_MS: [u64; 3] = [500, 1000, 2000];

/// Send a WhatsApp message with retry and exponential backoff.
///
/// Attempts up to 3 times with delays of 500ms, 1s, 2s between retries.
/// Clones the message for each retry attempt.
pub(super) async fn retry_send(
    client: &Client,
    jid: &Jid,
    msg: waproto::whatsapp::Message,
) -> Result<String, OmegaError> {
    let mut last_err = None;

    for (attempt, delay_ms) in RETRY_DELAYS_MS.iter().enumerate() {
        match client.send_message(jid.clone(), msg.clone()).await {
            Ok(msg_id) => return Ok(msg_id),
            Err(e) => {
                let attempt_num = attempt + 1;
                if attempt_num < RETRY_DELAYS_MS.len() {
                    warn!(
                        "whatsapp send attempt {attempt_num}/{} failed: {e}, retrying in {delay_ms}ms",
                        RETRY_DELAYS_MS.len()
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(*delay_ms)).await;
                } else {
                    error!(
                        "whatsapp send attempt {attempt_num}/{} failed: {e}, giving up",
                        RETRY_DELAYS_MS.len()
                    );
                }
                last_err = Some(e);
            }
        }
    }

    Err(OmegaError::Channel(format!(
        "whatsapp send failed after {} attempts: {}",
        RETRY_DELAYS_MS.len(),
        last_err.map(|e| e.to_string()).unwrap_or_default()
    )))
}

/// Convert Markdown formatting to WhatsApp-native formatting.
///
/// - `## Header` -> `*HEADER*` (bold uppercase)
/// - `**bold**` -> `*bold*`
/// - `[text](url)` -> `text (url)`
/// - `| col | col |` table rows -> `- col | col` bullets
/// - `---` horizontal rules -> removed
pub(super) fn sanitize_for_whatsapp(text: &str) -> String {
    let mut out = String::with_capacity(text.len());

    for line in text.lines() {
        let trimmed = line.trim();

        // Remove horizontal rules.
        if trimmed.chars().all(|c| c == '-' || c == ' ') && trimmed.matches('-').count() >= 3 {
            continue;
        }

        // Convert markdown headers to bold uppercase.
        if let Some(header) = trimmed.strip_prefix("### ") {
            out.push_str(&format!("*{}*", header.trim().to_uppercase()));
            out.push('\n');
            continue;
        }
        if let Some(header) = trimmed.strip_prefix("## ") {
            out.push_str(&format!("*{}*", header.trim().to_uppercase()));
            out.push('\n');
            continue;
        }
        if let Some(header) = trimmed.strip_prefix("# ") {
            out.push_str(&format!("*{}*", header.trim().to_uppercase()));
            out.push('\n');
            continue;
        }

        // Convert table rows (skip separator rows like |---|---|).
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let inner = &trimmed[1..trimmed.len() - 1];
            // Skip separator rows.
            if inner
                .chars()
                .all(|c| c == '-' || c == '|' || c == ' ' || c == ':')
            {
                continue;
            }
            let cols: Vec<&str> = inner.split('|').map(|s| s.trim()).collect();
            out.push_str("- ");
            out.push_str(&cols.join(" | "));
            out.push('\n');
            continue;
        }

        let mut result = line.to_string();

        // Convert markdown links: [text](url) -> text (url)
        while let Some(start_bracket) = result.find('[') {
            if let Some(end_bracket) = result[start_bracket..].find("](") {
                let abs_end_bracket = start_bracket + end_bracket;
                if let Some(end_paren) = result[abs_end_bracket + 2..].find(')') {
                    let abs_end_paren = abs_end_bracket + 2 + end_paren;
                    let link_text = &result[start_bracket + 1..abs_end_bracket];
                    let url = &result[abs_end_bracket + 2..abs_end_paren];
                    let replacement = format!("{link_text} ({url})");
                    result.replace_range(start_bracket..=abs_end_paren, &replacement);
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        // Convert **bold** to *bold* (WhatsApp native).
        while let Some(start_pos) = result.find("**") {
            if let Some(end_pos) = result[start_pos + 2..].find("**") {
                let abs_end = start_pos + 2 + end_pos;
                let inner_text = result[start_pos + 2..abs_end].to_string();
                result.replace_range(start_pos..abs_end + 2, &format!("*{inner_text}*"));
            } else {
                break;
            }
        }

        out.push_str(&result);
        out.push('\n');
    }

    // Remove trailing newline if the original didn't have one.
    if !text.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }

    out
}
