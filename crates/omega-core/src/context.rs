use serde::{Deserialize, Serialize};

/// A single entry in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    /// "user" or "assistant".
    pub role: String,
    /// The message content.
    pub content: String,
}

/// Conversation context passed to an AI provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Context {
    /// System prompt prepended to every request.
    pub system_prompt: String,
    /// Conversation history (oldest first).
    pub history: Vec<ContextEntry>,
    /// The current user message.
    pub current_message: String,
}

impl Context {
    /// Create a new context with just a current message and default system prompt.
    pub fn new(message: &str) -> Self {
        Self {
            system_prompt: default_system_prompt(),
            history: Vec::new(),
            current_message: message.to_string(),
        }
    }

    /// Flatten the context into a single prompt string for providers
    /// that accept a single text input (e.g. Claude Code CLI).
    pub fn to_prompt_string(&self) -> String {
        let mut parts = Vec::new();

        if !self.system_prompt.is_empty() {
            parts.push(format!("[System]\n{}", self.system_prompt));
        }

        for entry in &self.history {
            let role = if entry.role == "user" {
                "User"
            } else {
                "Assistant"
            };
            parts.push(format!("[{}]\n{}", role, entry.content));
        }

        parts.push(format!("[User]\n{}", self.current_message));

        parts.join("\n\n")
    }
}

/// Default system prompt for the Omega agent.
fn default_system_prompt() -> String {
    "You are Omega, a personal AI assistant running on the user's own server. \
     You are helpful, concise, and action-oriented."
        .to_string()
}
