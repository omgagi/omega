use serde::{Deserialize, Serialize};

/// Channel configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ChannelConfig {
    pub telegram: Option<TelegramConfig>,
    pub whatsapp: Option<WhatsAppConfig>,
}

/// Telegram bot config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub bot_token: String,
    #[serde(default)]
    pub allowed_users: Vec<i64>,
    /// OpenAI API key for Whisper voice transcription. Presence = voice enabled.
    #[serde(default)]
    pub whisper_api_key: Option<String>,
}

/// WhatsApp channel config.
///
/// Session data is stored at `{data_dir}/whatsapp_session/`.
/// Pairing is done by scanning a QR code (like WhatsApp Web).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    #[serde(default)]
    pub enabled: bool,
    /// Allowed phone numbers (e.g. `["5511999887766"]`). Empty = allow all.
    #[serde(default)]
    pub allowed_users: Vec<String>,
    /// OpenAI API key for Whisper voice transcription. Presence = voice enabled.
    #[serde(default)]
    pub whisper_api_key: Option<String>,
}
