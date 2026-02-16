use crate::{
    context::Context,
    error::OmegaError,
    message::{IncomingMessage, OutgoingMessage},
};
use async_trait::async_trait;

/// AI Provider trait — the brain.
///
/// Every AI backend (Claude Code, Anthropic API, OpenAI, Ollama, etc.)
/// implements this trait to provide a uniform interface.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Human-readable provider name.
    fn name(&self) -> &str;

    /// Whether this provider requires an API key to function.
    fn requires_api_key(&self) -> bool;

    /// Send a conversation context to the provider and get a response.
    async fn complete(&self, context: &Context) -> Result<OutgoingMessage, OmegaError>;

    /// Check if the provider is available and ready.
    async fn is_available(&self) -> bool;
}

/// Messaging Channel trait — the nervous system.
///
/// Every messaging platform (Telegram, WhatsApp, etc.) implements this
/// trait to receive and send messages.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Human-readable channel name.
    fn name(&self) -> &str;

    /// Start listening for incoming messages.
    /// Returns a receiver that yields incoming messages.
    async fn start(&self) -> Result<tokio::sync::mpsc::Receiver<IncomingMessage>, OmegaError>;

    /// Send a response back through this channel.
    async fn send(&self, message: OutgoingMessage) -> Result<(), OmegaError>;

    /// Send a typing indicator to show the bot is processing.
    async fn send_typing(&self, _target: &str) -> Result<(), OmegaError> {
        Ok(())
    }

    /// Send a photo (PNG bytes) with an optional caption.
    async fn send_photo(
        &self,
        _target: &str,
        _image: &[u8],
        _caption: &str,
    ) -> Result<(), OmegaError> {
        Ok(())
    }

    /// Graceful shutdown.
    async fn stop(&self) -> Result<(), OmegaError>;
}
