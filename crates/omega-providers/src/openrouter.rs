//! OpenRouter proxy provider.
//!
//! Reuses OpenAI's request/response types. Only the base URL and provider name differ.

use async_trait::async_trait;
use omega_core::{
    context::Context,
    error::OmegaError,
    message::{MessageMetadata, OutgoingMessage},
    traits::Provider,
};
use std::time::Instant;
use tracing::{debug, warn};

use crate::openai::{build_openai_messages, ChatCompletionRequest, ChatCompletionResponse};

const OPENROUTER_BASE_URL: &str = "https://openrouter.ai/api/v1";

/// OpenRouter provider — routes requests to many models via the OpenAI-compatible API.
pub struct OpenRouterProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenRouterProvider {
    /// Create from config values.
    pub fn from_config(api_key: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key,
            model,
        }
    }
}

#[async_trait]
impl Provider for OpenRouterProvider {
    fn name(&self) -> &str {
        "openrouter"
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

        let url = format!("{OPENROUTER_BASE_URL}/chat/completions");
        debug!("openrouter: POST {url} model={effective_model}");

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| OmegaError::Provider(format!("openrouter request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(OmegaError::Provider(format!(
                "openrouter returned {status}: {text}"
            )));
        }

        let parsed: ChatCompletionResponse = resp.json().await.map_err(|e| {
            OmegaError::Provider(format!("openrouter: failed to parse response: {e}"))
        })?;

        let text = parsed
            .choices
            .as_ref()
            .and_then(|c| c.first())
            .and_then(|c| c.message.as_ref())
            .map(|m| m.content.clone())
            .unwrap_or_else(|| "No response from OpenRouter.".to_string());

        let tokens = parsed.usage.as_ref().and_then(|u| u.total_tokens);
        let elapsed_ms = start.elapsed().as_millis() as u64;

        Ok(OutgoingMessage {
            text,
            metadata: MessageMetadata {
                provider_used: "openrouter".to_string(),
                tokens_used: tokens,
                processing_time_ms: elapsed_ms,
                model: parsed.model,
            },
            reply_target: None,
        })
    }

    async fn is_available(&self) -> bool {
        if self.api_key.is_empty() {
            warn!("openrouter: no API key configured");
            return false;
        }
        let url = format!("{OPENROUTER_BASE_URL}/models");
        match self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
        {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                warn!("openrouter not available: {e}");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openrouter_provider_name() {
        let p = OpenRouterProvider::from_config(
            "sk-or-test".into(),
            "anthropic/claude-sonnet-4".into(),
        );
        assert_eq!(p.name(), "openrouter");
        assert!(p.requires_api_key());
    }

    #[test]
    fn test_openrouter_base_url() {
        assert_eq!(OPENROUTER_BASE_URL, "https://openrouter.ai/api/v1");
    }
}
