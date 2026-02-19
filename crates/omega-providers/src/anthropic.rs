//! Anthropic API provider.
//!
//! Calls the Anthropic Messages API directly (not via Claude Code CLI).

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

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic Messages API provider.
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
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
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    #[serde(skip_serializing_if = "String::is_empty")]
    system: String,
    messages: Vec<AnthropicMessage>,
}

#[derive(Serialize, Deserialize)]
struct AnthropicMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Option<Vec<AnthropicContentBlock>>,
    model: Option<String>,
    usage: Option<AnthropicUsage>,
}

#[derive(Deserialize)]
struct AnthropicContentBlock {
    #[serde(default)]
    text: String,
}

#[derive(Deserialize)]
struct AnthropicUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    fn requires_api_key(&self) -> bool {
        true
    }

    async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError> {
        let (system, api_messages) = context.to_api_messages();
        let effective_model = context.model.as_deref().unwrap_or(&self.model);
        let start = Instant::now();

        let messages: Vec<AnthropicMessage> = api_messages
            .iter()
            .map(|m| AnthropicMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect();

        let body = AnthropicRequest {
            model: effective_model.to_string(),
            max_tokens: 8192,
            system,
            messages,
        };

        debug!("anthropic: POST {ANTHROPIC_API_URL} model={effective_model}");

        let resp = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| OmegaError::Provider(format!("anthropic request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(OmegaError::Provider(format!(
                "anthropic returned {status}: {text}"
            )));
        }

        let parsed: AnthropicResponse = resp.json().await.map_err(|e| {
            OmegaError::Provider(format!("anthropic: failed to parse response: {e}"))
        })?;

        let text = parsed
            .content
            .as_ref()
            .and_then(|blocks| blocks.first())
            .map(|b| b.text.clone())
            .unwrap_or_else(|| "No response from Anthropic.".to_string());

        let tokens = parsed
            .usage
            .as_ref()
            .map(|u| u.input_tokens + u.output_tokens);

        let elapsed_ms = start.elapsed().as_millis() as u64;

        Ok(OutgoingMessage {
            text,
            metadata: MessageMetadata {
                provider_used: "anthropic".to_string(),
                tokens_used: tokens,
                processing_time_ms: elapsed_ms,
                model: parsed.model,
            },
            reply_target: None,
        })
    }

    async fn is_available(&self) -> bool {
        if self.api_key.is_empty() {
            warn!("anthropic: no API key configured");
            return false;
        }
        // No lightweight health endpoint; we trust the key is valid.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_provider_name() {
        let p =
            AnthropicProvider::from_config("sk-ant-test".into(), "claude-sonnet-4-20250514".into());
        assert_eq!(p.name(), "anthropic");
        assert!(p.requires_api_key());
    }

    #[test]
    fn test_anthropic_request_serialization() {
        let body = AnthropicRequest {
            model: "claude-sonnet-4-20250514".into(),
            max_tokens: 8192,
            system: "Be helpful.".into(),
            messages: vec![AnthropicMessage {
                role: "user".into(),
                content: "Hello".into(),
            }],
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["model"], "claude-sonnet-4-20250514");
        assert_eq!(json["max_tokens"], 8192);
        assert_eq!(json["system"], "Be helpful.");
        assert_eq!(json["messages"][0]["role"], "user");
    }

    #[test]
    fn test_anthropic_request_empty_system_omitted() {
        let body = AnthropicRequest {
            model: "claude-sonnet-4-20250514".into(),
            max_tokens: 8192,
            system: String::new(),
            messages: vec![AnthropicMessage {
                role: "user".into(),
                content: "Hello".into(),
            }],
        };
        let json = serde_json::to_value(&body).unwrap();
        assert!(json.get("system").is_none());
    }

    #[test]
    fn test_anthropic_response_parsing() {
        let json = r#"{"content":[{"type":"text","text":"Hello!"}],"model":"claude-sonnet-4-20250514","usage":{"input_tokens":10,"output_tokens":5}}"#;
        let resp: AnthropicResponse = serde_json::from_str(json).unwrap();
        let text = resp
            .content
            .as_ref()
            .and_then(|b| b.first())
            .map(|b| b.text.clone());
        assert_eq!(text, Some("Hello!".into()));
        assert_eq!(
            resp.usage
                .as_ref()
                .map(|u| u.input_tokens + u.output_tokens),
            Some(15)
        );
    }
}
