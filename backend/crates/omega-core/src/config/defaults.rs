//! Default value functions used by serde for config deserialization.

pub fn default_name() -> String {
    "OMEGA \u{03a9}".to_string()
}

pub fn default_data_dir() -> String {
    "~/.omega".to_string()
}

pub fn default_log_level() -> String {
    "info".to_string()
}

pub fn default_provider() -> String {
    "claude-code".to_string()
}

pub fn default_true() -> bool {
    true
}

pub fn default_deny_message() -> String {
    "Access denied. You are not authorized to use this agent.".to_string()
}

pub fn default_max_turns() -> u32 {
    25
}

pub fn default_allowed_tools() -> Vec<String> {
    vec![]
}

pub fn default_anthropic_model() -> String {
    "claude-sonnet-4-20250514".to_string()
}

pub fn default_openai_model() -> String {
    "gpt-4o".to_string()
}

pub fn default_openai_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

pub fn default_ollama_base_url() -> String {
    "http://localhost:11434".to_string()
}

pub fn default_ollama_model() -> String {
    "llama3".to_string()
}

pub fn default_gemini_model() -> String {
    "gemini-2.0-flash".to_string()
}

pub fn default_memory_backend() -> String {
    "sqlite".to_string()
}

pub fn default_db_path() -> String {
    "~/.omega/data/memory.db".to_string()
}

pub fn default_max_context() -> usize {
    50
}

pub fn default_heartbeat_interval() -> u64 {
    30
}

pub fn default_poll_interval() -> u64 {
    60
}

pub fn default_api_host() -> String {
    "127.0.0.1".to_string()
}

pub fn default_api_port() -> u16 {
    3000
}

pub fn default_timeout_secs() -> u64 {
    3600
}

pub fn default_max_resume_attempts() -> u32 {
    5
}

pub fn default_model() -> String {
    "claude-sonnet-4-6".to_string()
}

pub fn default_model_complex() -> String {
    "claude-opus-4-6".to_string()
}
