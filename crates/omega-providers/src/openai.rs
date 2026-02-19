//! OpenAI-compatible API provider.
//!
//! Works with OpenAI's API and any compatible endpoint.
//! Exports `pub(crate)` types reused by the OpenRouter provider.

use async_trait::async_trait;
use omega_core::{
    context::{ApiMessage, Context},
    error::OmegaError,
    message::{MessageMetadata, OutgoingMessage},
    traits::Provider,
};
use serde::{Deserialize, Serialize};
use std::time::Instant;
use tracing::{debug, warn};

/// OpenAI-compatible provider.
pub struct OpenAiProvider {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
}

impl OpenAiProvider {
    /// Create from config values.
    pub fn from_config(base_url: String, api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            api_key,
            model,
        }
    }
}

/// Build OpenAI-format messages from context (system as a message role).
pub(crate) fn build_openai_messages(system: &str, api_messages: &[ApiMessage]) -> Vec<ChatMessage> {
    let mut messages = Vec::with_capacity(api_messages.len() + 1);
    if !system.is_empty() {
        messages.push(ChatMessage {
            role: "system".to_string(),
            content: system.to_string(),
        });
    }
    for m in api_messages {
        messages.push(ChatMessage {
            role: m.role.clone(),
            content: m.content.clone(),
        });
    }
    messages
}

#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub(crate) struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
}

#[derive(Deserialize)]
pub(crate) struct ChatCompletionResponse {
    pub choices: Option<Vec<ChatChoice>>,
    pub model: Option<String>,
    pub usage: Option<ChatUsage>,
}

#[derive(Deserialize)]
pub(crate) struct ChatChoice {
    pub message: Option<ChatMessage>,
}

#[derive(Deserialize)]
pub(crate) struct ChatUsage {
    pub total_tokens: Option<u64>,
}

#[async_trait]
impl Provider for OpenAiProvider {
    fn name(&self) -> &str {
        "openai"
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError> {
        let (system, api_messages) = context.to_api_messages();
        let effective_model = context.model.as_deref().unwrap_or(&self.model);
        let start = Instant::now();

        let messages = build_openai_messages(&system, &api_messages);
        let body = ChatCompletionRequest {
            model: effective_model.to_string(),
            messages,
        };

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        debug!("openai: POST {url} model={effective_model}");

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| OmegaError::Provider(format!("openai request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(OmegaError::Provider(format!(
                "openai returned {status}: {text}"
            )));
        }

        let parsed: ChatCompletionResponse = resp
            .json()
            .await
            .map_err(|e| OmegaError::Provider(format!("openai: failed to parse response: {e}")))?;

        let text = parsed
            .choices
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.clone())
            .unwrap_or_else(|| "No response from OpenAI.".to_string());

        let tokens = parsed.usage.as_ref().and_then(|u| u.total_tokens);
        let elapsed_ms = start.elapsed().as_millis() as u64;

        Ok(OutgoingMessage {
            text,
            metadata: MessageMetadata {
                provider_used: "openai".to_string(),
                tokens_used: tokens,
                processing_time_ms: elapsed_ms,
                model: parsed.model,
            },
            reply_target: None,
        })
    }

    async fn is_available(&self) -> bool {
        if self.api_key.is_empty() {
            warn!("openai: no API key configured");
            return false;
        }
        // Basic check: try to list models.
        let url = format!("{}/models", self.base_url.trim_end_matches('/'));
        match self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                warn!("openai not available: {e}");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openai_provider_name() {
        let p = OpenAiProvider::from_config(
            "https://api.openai.com/v1".into(),
            "sk-test".into(),
            "gpt-4o".into(),
        );
        assert_eq!(p.name(), "openai");
        assert!(p.requires_api_key());
    }

    #[test]
    fn test_build_openai_messages() {
        let api_msgs = vec![
            ApiMessage {
                role: "user".into(),
                content: "Hi".into(),
            },
            ApiMessage {
                role: "assistant".into(),
                content: "Hello!".into(),
            },
            ApiMessage {
                role: "user".into(),
                content: "How?".into(),
            },
        ];
        let messages = build_openai_messages("Be helpful.", &api_msgs);
        assert_eq!(messages.len(), 4);
        assert_eq!(messages[0].role, "system");
        assert_eq!(messages[0].content, "Be helpful.");
        assert_eq!(messages[3].role, "user");
    }

    #[test]
    fn test_build_openai_messages_empty_system() {
        let api_msgs = vec![ApiMessage {
            role: "user".into(),
            content: "Hi".into(),
        }];
        let messages = build_openai_messages("", &api_msgs);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
    }

    #[test]
    fn test_openai_response_parsing() {
        let json = r#"{"choices":[{"message":{"role":"assistant","content":"Hello!"},"finish_reason":"stop"}],"model":"gpt-4o","usage":{"total_tokens":42,"prompt_tokens":10,"completion_tokens":32}}"#;
        let resp: ChatCompletionResponse = serde_json::from_str(json).unwrap();
        let text = resp
            .choices
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.clone());
        assert_eq!(text, Some("Hello!".into()));
        assert_eq!(resp.usage.as_ref().and_then(|u| u.total_tokens), Some(42));
    }
}
