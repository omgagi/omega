//! Shared Whisper transcription — used by both Telegram and WhatsApp channels.

use omega_core::error::OmegaError;
use serde::Deserialize;

/// Whisper API response.
#[derive(Deserialize)]
struct WhisperResponse {
    text: String,
}

/// Transcribe audio bytes via OpenAI Whisper API.
pub async fn transcribe_whisper(
    client: &reqwest::Client,
    api_key: &str,
    audio_bytes: &[u8],
) -> Result<String, OmegaError> {
    let part = reqwest::multipart::Part::bytes(audio_bytes.to_vec())
        .file_name("voice.ogg")
        .mime_str("audio/ogg")
        .map_err(|e| OmegaError::Channel(format!("whisper mime error: {e}")))?;

    let form = reqwest::multipart::Form::new()
        .text("model", "whisper-1")
        .part("file", part);

    let resp = client
        .post("https://api.openai.com/v1/audio/transcriptions")
        .bearer_auth(api_key)
        .multipart(form)
        .send()
        .await
        .map_err(|e| OmegaError::Channel(format!("whisper request failed: {e}")))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(OmegaError::Channel(format!(
            "whisper API error {status}: {body}"
        )));
    }

    let result: WhisperResponse = resp
        .json()
        .await
        .map_err(|e| OmegaError::Channel(format!("whisper response parse failed: {e}")))?;

    Ok(result.text)
}
