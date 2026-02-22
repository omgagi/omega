//! Telegram Bot API deserialization types.

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(crate) struct TgResponse<T> {
    pub ok: bool,
    pub result: Option<T>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TgUpdate {
    pub update_id: i64,
    pub message: Option<TgMessage>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct TgMessage {
    pub message_id: i64,
    pub from: Option<TgUser>,
    pub chat: TgChat,
    pub text: Option<String>,
    pub voice: Option<TgVoice>,
    pub photo: Option<Vec<TgPhotoSize>>,
    pub caption: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct TgVoice {
    pub file_id: String,
    pub duration: i64,
    pub mime_type: Option<String>,
    pub file_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TgFile {
    pub file_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct TgPhotoSize {
    pub file_id: String,
    pub width: i64,
    pub height: i64,
    pub file_size: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub(crate) struct TgUser {
    pub id: i64,
    pub first_name: String,
    pub last_name: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TgChat {
    pub id: i64,
    /// Chat type: "private", "group", "supergroup", or "channel".
    #[serde(default, rename = "type")]
    pub chat_type: String,
}
