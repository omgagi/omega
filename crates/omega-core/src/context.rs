use serde::{Deserialize, Serialize};

/// Controls which optional context blocks are loaded and injected.
///
/// Used by the gateway to skip expensive DB queries (semantic recall, pending tasks)
/// when the user's message doesn't need them — reducing token overhead by ~55%.
pub struct ContextNeeds {
    /// Load semantic recall (FTS5 related past messages).
    pub recall: bool,
    /// Load and inject pending scheduled tasks.
    pub pending_tasks: bool,
}

impl Default for ContextNeeds {
    fn default() -> Self {
        Self {
            recall: true,
            pending_tasks: true,
        }
    }
}

/// A single entry in the conversation history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    /// "user" or "assistant".
    pub role: String,
    /// The message content.
    pub content: String,
}

/// An MCP server declared by a skill.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpServer {
    /// Server name (used as the key in Claude settings).
    pub name: String,
    /// Command to launch the server.
    pub command: String,
    /// Command-line arguments.
    pub args: Vec<String>,
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
    /// MCP servers to activate for this request.
    #[serde(default)]
    pub mcp_servers: Vec<McpServer>,
    /// Override the provider's default max_turns. When `Some`, the provider
    /// uses this value instead of its configured default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    /// Override the provider's default allowed tools. When `Some`, the provider
    /// uses this list instead of its configured default. `Some(vec![])` = no tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,
    /// Override the provider's default model. When `Some`, the provider passes
    /// `--model` with this value instead of its configured default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// A structured message for API-based providers (OpenAI, Anthropic, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiMessage {
    /// "user" or "assistant".
    pub role: String,
    /// The message content.
    pub content: String,
}

impl Context {
    /// Create a new context with just a current message and default system prompt.
    pub fn new(message: &str) -> Self {
        Self {
            system_prompt: default_system_prompt(),
            history: Vec::new(),
            current_message: message.to_string(),
            mcp_servers: Vec::new(),
            max_turns: None,
            allowed_tools: None,
            model: None,
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

    /// Convert context to structured API messages.
    ///
    /// Returns `(system_prompt, messages)` — the system prompt is separated
    /// because Anthropic and Gemini require it outside the messages array.
    pub fn to_api_messages(&self) -> (String, Vec<ApiMessage>) {
        let mut messages = Vec::with_capacity(self.history.len() + 1);

        for entry in &self.history {
            messages.push(ApiMessage {
                role: entry.role.clone(),
                content: entry.content.clone(),
            });
        }

        messages.push(ApiMessage {
            role: "user".to_string(),
            content: self.current_message.clone(),
        });

        (self.system_prompt.clone(), messages)
    }
}

/// Default system prompt for the Omega agent.
fn default_system_prompt() -> String {
    "You are OMEGA Ω, a personal AI assistant running on the user's own server. \
     You are helpful, concise, and action-oriented."
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_serde_round_trip() {
        let server = McpServer {
            name: "playwright".into(),
            command: "npx".into(),
            args: vec!["@playwright/mcp".into(), "--headless".into()],
        };
        let json = serde_json::to_string(&server).unwrap();
        let deserialized: McpServer = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "playwright");
        assert_eq!(deserialized.command, "npx");
        assert_eq!(deserialized.args, vec!["@playwright/mcp", "--headless"]);
    }

    #[test]
    fn test_context_new_has_empty_mcp_servers() {
        let ctx = Context::new("hello");
        assert!(ctx.mcp_servers.is_empty());
    }

    #[test]
    fn test_context_with_mcp_servers_serde() {
        let ctx = Context {
            system_prompt: "test".into(),
            history: Vec::new(),
            current_message: "browse google.com".into(),
            mcp_servers: vec![McpServer {
                name: "playwright".into(),
                command: "npx".into(),
                args: vec!["@playwright/mcp".into()],
            }],
            max_turns: None,
            allowed_tools: None,
            model: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let deserialized: Context = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.mcp_servers.len(), 1);
        assert_eq!(deserialized.mcp_servers[0].name, "playwright");
    }

    #[test]
    fn test_context_deserialize_without_mcp_servers() {
        // Old JSON without mcp_servers field should still deserialize.
        let json = r#"{"system_prompt":"test","history":[],"current_message":"hi"}"#;
        let ctx: Context = serde_json::from_str(json).unwrap();
        assert!(ctx.mcp_servers.is_empty());
    }

    #[test]
    fn test_to_api_messages_basic() {
        let ctx = Context::new("hello");
        let (system, messages) = ctx.to_api_messages();
        assert!(!system.is_empty());
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "hello");
    }

    #[test]
    fn test_to_api_messages_with_history() {
        let ctx = Context {
            system_prompt: "Be helpful.".into(),
            history: vec![
                ContextEntry {
                    role: "user".into(),
                    content: "Hi".into(),
                },
                ContextEntry {
                    role: "assistant".into(),
                    content: "Hello!".into(),
                },
            ],
            current_message: "How are you?".into(),
            mcp_servers: Vec::new(),
            max_turns: None,
            allowed_tools: None,
            model: None,
        };
        let (system, messages) = ctx.to_api_messages();
        assert_eq!(system, "Be helpful.");
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, "user");
        assert_eq!(messages[0].content, "Hi");
        assert_eq!(messages[1].role, "assistant");
        assert_eq!(messages[1].content, "Hello!");
        assert_eq!(messages[2].role, "user");
        assert_eq!(messages[2].content, "How are you?");
    }
}
