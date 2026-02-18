# Specification: omega-core/src/context.rs

## Path
`/Users/isudoajl/ownCloud/Projects/omega/crates/omega-core/src/context.rs`

## Purpose
Defines the conversation context data structures that carry a system prompt, conversation history, and the current user message through the Omega message pipeline. Every AI provider receives a `Context` to generate a response. This module also provides the logic to flatten a multi-part context into a single prompt string for providers that accept plain text input (such as the Claude Code CLI).

## Data Structures

### `ContextEntry`

A single turn in a conversation (one message from either the user or the assistant).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextEntry {
    /// "user" or "assistant".
    pub role: String,
    /// The message content.
    pub content: String,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `role` | `String` | The speaker: `"user"` or `"assistant"`. |
| `content` | `String` | The text of the message. |

**Traits derived:** `Debug`, `Clone`, `Serialize`, `Deserialize`.

---

### `McpServer`

An MCP server declared by a skill, activated dynamically based on message trigger matching.

| Field | Type | Description |
|-------|------|-------------|
| `name` | `String` | Server name (used as the key in Claude settings and for `--allowedTools` patterns). |
| `command` | `String` | Command to launch the server (e.g., `"npx"`). |
| `args` | `Vec<String>` | Command-line arguments (e.g., `["@playwright/mcp", "--headless"]`). |

**Traits derived:** `Debug`, `Clone`, `Serialize`, `Deserialize`, `Default`.

---

### `Context`

The complete conversation context passed to an AI provider for a single request.

```rust
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
    /// Optional model override for this request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}
```

| Field | Type | Description |
|-------|------|-------------|
| `system_prompt` | `String` | Instructions prepended to every provider call. Configures the AI's persona and behavioral guidelines. |
| `history` | `Vec<ContextEntry>` | Previous conversation turns, ordered oldest-first (chronological). Populated by the memory store or left empty for one-shot requests. |
| `current_message` | `String` | The user's latest message that the provider must respond to. |
| `mcp_servers` | `Vec<McpServer>` | MCP servers to activate for this request. Populated by skill trigger matching in the gateway. Default: empty. |
| `model` | `Option<String>` | Optional model override for this request. When `Some`, the provider uses this model instead of its default. Set by the gateway's classify-and-route logic. Default: `None`. |

**Traits derived:** `Debug`, `Clone`, `Serialize`, `Deserialize`.

## Methods

### `Context::new(message: &str) -> Self`

**Signature:**
```rust
pub fn new(message: &str) -> Self
```

**Purpose:** Create a minimal context with only a current message and the default system prompt. History is left empty.

**Parameters:**

| Parameter | Type | Description |
|-----------|------|-------------|
| `message` | `&str` | The user message to wrap in a context. |

**Returns:** A new `Context` where:
- `system_prompt` is set to the value returned by `default_system_prompt()`.
- `history` is an empty `Vec`.
- `current_message` is a clone of `message`.
- `mcp_servers` is an empty `Vec`.
- `model` is `None`.

**Usage sites:**
- `src/main.rs` -- the `omega ask` CLI command creates a one-shot context for a single prompt with no history.
- `src/gateway.rs` -- the `summarize_conversation()` function creates throwaway contexts for summarization and fact-extraction prompts.

---

### `Context::to_prompt_string(&self) -> String`

**Signature:**
```rust
pub fn to_prompt_string(&self) -> String
```

**Purpose:** Flatten the structured context into a single plain-text prompt string. Designed for providers that accept a single text input rather than structured message arrays (e.g., the Claude Code CLI which passes the prompt as a command-line argument).

**Parameters:** None (operates on `&self`).

**Returns:** A `String` with sections separated by double newlines (`\n\n`).

**Output format:**

```
[System]
<system_prompt>

[User]
<history entry 1 content>

[Assistant]
<history entry 2 content>

...

[User]
<current_message>
```

**Detailed behavior:**

1. If `self.system_prompt` is non-empty, emit `[System]\n{system_prompt}`.
2. For each entry in `self.history`, emit `[User]\n{content}` or `[Assistant]\n{content}` depending on the `role` field. Any role that is not `"user"` is rendered as `"Assistant"`.
3. Always emit `[User]\n{current_message}` at the end.
4. All parts are joined with `"\n\n"`.

**Edge cases:**
- If `system_prompt` is empty, the `[System]` section is omitted entirely.
- If `history` is empty, only the system prompt (if present) and the current message appear.
- Roles other than `"user"` (including typos or future roles) all map to `"Assistant"`.

**Usage sites:**
- `crates/omega-providers/src/claude_code.rs` -- the Claude Code provider calls `context.to_prompt_string()` to produce the single prompt string passed to the `claude -p` CLI command.

## Private Functions

### `default_system_prompt() -> String`

**Signature:**
```rust
fn default_system_prompt() -> String
```

**Purpose:** Returns the hardcoded default system prompt used when constructing a context via `Context::new()`.

**Returns:**
```
"You are OMEGA Ω, a personal AI assistant running on the user's own server. You are helpful, concise, and action-oriented."
```

**Note:** This default prompt is only used for one-shot contexts created with `Context::new()`. The gateway pipeline uses `memory.build_context()`, which constructs an enriched system prompt containing user facts and conversation summaries via the `build_system_prompt()` function in `omega-memory`.

## Dependencies

| Crate | Usage |
|-------|-------|
| `serde` | `Serialize` and `Deserialize` derives for both structs. |

No internal omega crate dependencies. This module is a leaf in the dependency graph, consumed by `omega-core::traits`, `omega-memory`, `omega-providers`, and the binary crate.

## How Context Is Built

There are two distinct construction paths:

### Path 1: One-shot (`Context::new`)

Used for CLI commands and internal gateway tasks (summarization, fact extraction). Produces a minimal context with the default system prompt and no history.

```
Context::new(prompt)
  --> system_prompt = default_system_prompt()
  --> history = []
  --> current_message = prompt
  --> model = None
```

**Call sites:** `src/main.rs:161`, `src/gateway.rs:166`, `src/gateway.rs:182`.

### Path 2: Memory-enriched (`Store::build_context`)

Used by the gateway when processing real user messages. Produces a full context with conversation history, user facts, and recent summaries baked into the system prompt.

```
store.build_context(&incoming)
  --> get_or_create_conversation(channel, sender_id)
  --> SELECT role, content FROM messages WHERE conversation_id = ? (newest N, reversed to chronological)
  --> get_facts(sender_id)
  --> get_recent_summaries(channel, sender_id, 3)
  --> build_system_prompt(facts, summaries, text)
  --> Context { system_prompt, history, current_message }
```

**Call site:** `src/gateway.rs:345`.

## How Context Is Consumed

### Provider Trait

The `Provider::complete(&self, context: &Context)` method receives the context. Each provider implementation decides how to use it:

| Provider | Consumption Method |
|----------|-------------------|
| Claude Code CLI | Calls `context.to_prompt_string()` to produce a single flat string, passed as the `-p` argument to the `claude` subprocess. |
| (Future) Anthropic API | Would use `system_prompt` as the system message and `history` + `current_message` as the structured messages array. |
| (Future) OpenAI API | Same structured approach as Anthropic. |

## Position in the Message Pipeline

```
Incoming Message
       |
       v
[Auth] --> [Sanitize] --> [Command Check] --> [Typing]
                                                  |
                                                  v
                                         [memory.build_context()]
                                                  |
                                                  v
                                              Context {
                                                system_prompt: "enriched...",
                                                history: [ContextEntry, ...],
                                                current_message: "user text"
                                              }
                                                  |
                                                  v
                                         [provider.complete(&context)]
                                                  |
                                                  v
                                           OutgoingMessage
```

## Serialization

Both `Context` and `ContextEntry` derive `Serialize` and `Deserialize`. This allows contexts to be:
- Serialized to JSON for logging, debugging, or future API transport.
- Deserialized from stored representations if context caching is added.

Currently, serialization is not explicitly used in the codebase but is available for future use.

## Tests

- `test_mcp_server_serde_roundtrip`: McpServer serializes and deserializes correctly (round-trip).
- `test_context_new_has_empty_mcp_servers`: `Context::new()` initializes `mcp_servers` to an empty `Vec`.
- `test_context_with_mcp_servers_serde`: Context with populated `mcp_servers` serializes and deserializes correctly.
- `test_context_deserialize_without_mcp_servers`: Deserializing a Context JSON without the `mcp_servers` field succeeds with an empty default (backwards compatibility).

## Invariants

1. `current_message` is always non-empty when passed to a provider.
2. `history` entries are in chronological order (oldest first).
3. Every `ContextEntry.role` is either `"user"` or `"assistant"`.
4. The `[User]` section for `current_message` is always the last section in `to_prompt_string()` output.
5. `to_prompt_string()` never produces an empty string (at minimum it contains the current message).
