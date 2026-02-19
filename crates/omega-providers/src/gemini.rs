//! Google Gemini API provider.
//!
//! Calls the Gemini `generateContent` endpoint. Auth via URL query param.

use async_trait::async_trait;
use omega_core::{
    context::Context,
    error::OmegaError,
    message::{MessageMetadata, OutgoingMessage},
    traits::Provider,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, warn};

const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Google Gemini API provider.
pub struct GeminiProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl GeminiProvider {
    /// Create from config values.
    pub fn from_config(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<GeminiContent>,
}

#[derive(Serialize, Deserialize)]
struct GeminiContent {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    parts: Vec<GeminiPart>,
}

#[derive(Serialize, Deserialize)]
struct GeminiPart {
    text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiResponse {
    candidates: Option<Vec<GeminiCandidate>>,
    usage_metadata: Option<GeminiUsage>,
}

#[derive(Deserialize)]
struct GeminiCandidate {
    content: Option<GeminiContent>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GeminiUsage {
    #[serde(default)]
    total_token_count: u64,
}

#[async_trait]
impl Provider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError> {
        let (system, api_messages) = context.to_api_messages();
        let effective_model = context.model.as_deref().unwrap_or(&self.model);
        let start = Instant::now();

        let system_instruction = if system.is_empty() {
            None
        } else {
            Some(GeminiContent {
                role: None,
                parts: vec![GeminiPart { text: system }],
            })
        };

        let contents: Vec<GeminiContent> = api_messages
            .iter()
            .map(|m| {
                let role = if m.role == "assistant" {
                    "model"
                } else {
                    "user"
                };
                GeminiContent {
                    role: Some(role.to_string()),
                    parts: vec![GeminiPart {
                        text: m.content.clone(),
                    }],
                }
            })
            .collect();

        let body = GeminiRequest {
            contents,
            system_instruction,
        };

        let url = format!(
            "{GEMINI_BASE_URL}/models/{effective_model}:generateContent?key={}",
            self.api_key
        );
        debug!("gemini: POST models/{effective_model}:generateContent");

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| OmegaError::Provider(format!("gemini request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(OmegaError::Provider(format!(
                "gemini returned {status}: {text}"
            )));
        }

        let parsed: GeminiResponse = resp
            .json()
            .await
            .map_err(|e| OmegaError::Provider(format!("gemini: failed to parse response: {e}")))?;

        let text = parsed
            .candidates
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .and_then(|c| c.parts.first())
            .map(|p| p.text.clone())
            .unwrap_or_else(|| "No response from Gemini.".to_string());

        let tokens = parsed.usage_metadata.as_ref().map(|u| u.total_token_count);
        let elapsed_ms = start.elapsed().as_millis() as u64;

        Ok(OutgoingMessage {
            text,
            metadata: MessageMetadata {
                provider_used: "gemini".to_string(),
                tokens_used: tokens,
                processing_time_ms: elapsed_ms,
                model: Some(effective_model.to_string()),
            },
            reply_target: None,
        })
    }

    async fn is_available(&self) -> bool {
        if self.api_key.is_empty() {
            warn!("gemini: no API key configured");
            return false;
        }
        let url = format!("{GEMINI_BASE_URL}/models?key={}", self.api_key);
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                warn!("gemini not available: {e}");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gemini_provider_name() {
        let p = GeminiProvider::from_config("AIza-test".into(), "gemini-2.0-flash".into());
        assert_eq!(p.name(), "gemini");
        assert!(p.requires_api_key());
    }

    #[test]
    fn test_gemini_request_serialization() {
        let body = GeminiRequest {
            contents: vec![GeminiContent {
                role: Some("user".into()),
                parts: vec![GeminiPart {
                    text: "Hello".into(),
                }],
            }],
            system_instruction: Some(GeminiContent {
                role: None,
                parts: vec![GeminiPart {
                    text: "Be helpful.".into(),
                }],
            }),
        };
        let json = serde_json::to_value(&body).unwrap();
        assert!(json.get("systemInstruction").is_some());
        assert_eq!(json["contents"][0]["role"], "user");
        assert_eq!(json["contents"][0]["parts"][0]["text"], "Hello");
    }

    #[test]
    fn test_gemini_request_no_system() {
        let body = GeminiRequest {
            contents: vec![GeminiContent {
                role: Some("user".into()),
                parts: vec![GeminiPart {
                    text: "Hello".into(),
                }],
            }],
            system_instruction: None,
        };
        let json = serde_json::to_value(&body).unwrap();
        assert!(json.get("systemInstruction").is_none());
    }

    #[test]
    fn test_gemini_role_mapping() {
        let api_msgs = vec![
            omega_core::context::ApiMessage {
                role: "user".into(),
                content: "Hi".into(),
            },
            omega_core::context::ApiMessage {
                role: "assistant".into(),
                content: "Hello!".into(),
            },
        ];
        let contents: Vec<GeminiContent> = api_msgs
            .iter()
            .map(|m| {
                let role = if m.role == "assistant" {
                    "model"
                } else {
                    "user"
                };
                GeminiContent {
                    role: Some(role.to_string()),
                    parts: vec![GeminiPart {
                        text: m.content.clone(),
                    }],
                }
            })
            .collect();
        assert_eq!(contents[0].role, Some("user".into()));
        assert_eq!(contents[1].role, Some("model".into()));
    }

    #[test]
    fn test_gemini_response_parsing() {
        let json = r#"{"candidates":[{"content":{"role":"model","parts":[{"text":"Hi there!"}]}}],"usageMetadata":{"totalTokenCount":25}}"#;
        let resp: GeminiResponse = serde_json::from_str(json).unwrap();
        let text = resp
            .candidates
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.content.as_ref())
            .and_then(|c| c.parts.first())
            .map(|p| p.text.clone());
        assert_eq!(text, Some("Hi there!".into()));
        assert_eq!(
            resp.usage_metadata.as_ref().map(|u| u.total_token_count),
            Some(25)
        );
    }
}
