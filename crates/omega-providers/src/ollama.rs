//! Ollama local model provider.
//!
//! Connects to a locally running Ollama server. No API key required.

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

/// Ollama provider backed by a local server.
pub struct OllamaProvider {
    client: reqwest::Client,
    base_url: String,
    model: String,
}

impl OllamaProvider {
    /// Create from config values.
    pub fn from_config(base_url: String, model: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url,
            model,
        }
    }
}

#[derive(Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaChatMessage>,
    stream: bool,
}

#[derive(Serialize, Deserialize)]
struct OllamaChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct OllamaChatResponse {
    message: Option<OllamaChatMessage>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    eval_count: Option<u64>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
}

#[async_trait]
impl Provider for OllamaProvider {
    fn name(&self) -> &str {
        "ollama"
    }

    fn requires_api_key(&self) -> bool {
        false
    }

    async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError> {
        let (system, api_messages) = context.to_api_messages();
        let effective_model = context.model.as_deref().unwrap_or(&self.model);
        let start = Instant::now();

        let mut messages = Vec::with_capacity(api_messages.len() + 1);
        if !system.is_empty() {
            messages.push(OllamaChatMessage {
                role: "system".to_string(),
                content: system,
            });
        }
        for m in &api_messages {
            messages.push(OllamaChatMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            });
        }

        let body = OllamaChatRequest {
            model: effective_model.to_string(),
            messages,
            stream: false,
        };

        let url = format!("{}/api/chat", self.base_url.trim_end_matches('/'));
        debug!("ollama: POST {url} model={effective_model}");

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| OmegaError::Provider(format!("ollama request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(OmegaError::Provider(format!(
                "ollama returned {status}: {text}"
            )));
        }

        let parsed: OllamaChatResponse = resp
            .json()
            .await
            .map_err(|e| OmegaError::Provider(format!("ollama: failed to parse response: {e}")))?;

        let text = parsed
            .message
            .map(|m| m.content)
            .unwrap_or_else(|| "No response from Ollama.".to_string());

        let tokens = match (parsed.eval_count, parsed.prompt_eval_count) {
            (Some(e), Some(p)) => Some(e + p),
            (Some(e), None) => Some(e),
            _ => None,
        };

        let elapsed_ms = start.elapsed().as_millis() as u64;

        Ok(OutgoingMessage {
            text,
            metadata: MessageMetadata {
                provider_used: "ollama".to_string(),
                tokens_used: tokens,
                processing_time_ms: elapsed_ms,
                model: parsed.model,
            },
            reply_target: None,
        })
    }

    async fn is_available(&self) -> bool {
        let url = format!("{}/api/tags", self.base_url.trim_end_matches('/'));
        match self.client.get(&url).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(e) => {
                warn!("ollama not available: {e}");
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ollama_provider_name() {
        let p = OllamaProvider::from_config("http://localhost:11434".into(), "llama3".into());
        assert_eq!(p.name(), "ollama");
        assert!(!p.requires_api_key());
    }

    #[test]
    fn test_ollama_request_serialization() {
        let body = OllamaChatRequest {
            model: "llama3".into(),
            messages: vec![
                OllamaChatMessage {
                    role: "system".into(),
                    content: "Be helpful.".into(),
                },
                OllamaChatMessage {
                    role: "user".into(),
                    content: "Hello".into(),
                },
            ],
            stream: false,
        };
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["model"], "llama3");
        assert!(!json["stream"].as_bool().unwrap());
        assert_eq!(json["messages"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn test_ollama_response_parsing() {
        let json = r#"{"message":{"role":"assistant","content":"Hi there!"},"model":"llama3","eval_count":42,"prompt_eval_count":10}"#;
        let resp: OllamaChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.message.unwrap().content, "Hi there!");
        assert_eq!(resp.model, Some("llama3".into()));
        assert_eq!(resp.eval_count, Some(42));
        assert_eq!(resp.prompt_eval_count, Some(10));
    }
}
