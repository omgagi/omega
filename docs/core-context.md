# Conversation Context

## Path
`/Users/isudoajl/ownCloud/Projects/omega/crates/omega-core/src/context.rs`

## What Is a Context?

When you send a message to Omega, the AI provider does not just see your latest message in isolation. It receives a **Context** -- a structured bundle that includes:

- **A system prompt** telling the AI who it is and how to behave.
- **Conversation history** so the AI knows what was said before.
- **Your current message** that it needs to respond to.

The `Context` struct is the container for all of this information. It is created once per incoming message and handed to the AI provider, which uses it to generate a response.

## The Two Structs

### ContextEntry

A single message in the conversation history. Each entry has a `role` (either `"user"` or `"assistant"`) and the `content` of that message.

```rust
pub struct ContextEntry {
    pub role: String,    // "user" or "assistant"
    pub content: String, // the message text
}
```

Think of this as one line in a chat transcript.

### Context

The full package sent to the AI provider.

```rust
pub struct Context {
    pub system_prompt: String,        // instructions for the AI
    pub history: Vec<ContextEntry>,   // previous messages (oldest first)
    pub current_message: String,      // the message to respond to
}
```

Together, these three fields give the provider everything it needs to generate a relevant, contextual reply.

## How Context Flows Through the System

There are two ways a context gets created, depending on where in Omega it is needed.

### 1. Simple Contexts (CLI and Internal Tasks)

When you run `omega ask "What time is it?"` from the command line, or when the gateway needs to summarize an idle conversation, a simple context is created with `Context::new()`:

```rust
let context = Context::new("What time is it?");
```

This produces a context with:
- The default system prompt: *"You are OMEGA Ω, a personal AI assistant running on the user's own server. You are helpful, concise, and action-oriented."*
- Empty history (no previous messages).
- Your message as the current message.

Simple contexts are lightweight and disposable. They are used when there is no conversation to continue -- just a one-off question.

### 2. Enriched Contexts (Gateway Pipeline)

When a message arrives through Telegram or another channel, the gateway builds a much richer context using the memory store:

```rust
let context = self.memory.build_context(&incoming, &self.prompts.system).await?;
```

This does significantly more work behind the scenes:

1. **Finds or creates a conversation** for this user and channel.
2. **Loads recent history** from the database (the last N messages in this conversation, in chronological order).
3. **Fetches stored facts** about the user (name, preferences, location, etc. -- things Omega has learned from past conversations).
4. **Retrieves recent conversation summaries** from the user's previous interactions.
5. **Builds a custom system prompt** that weaves in the facts and summaries, making the AI aware of who the user is and what they have discussed before.

The result is a context that feels personal. The AI knows the user's name, remembers their preferences, and can reference earlier conversations.

## How Providers Use the Context

Once built, the context is passed to the AI provider via the `complete()` method:

```rust
let response = provider.complete(&context).await?;
```

Different providers consume the context differently.

### Claude Code CLI Provider

The Claude Code CLI accepts a single text prompt on the command line. It does not natively understand structured message arrays. So the provider calls `to_prompt_string()` to flatten the context into plain text:

```rust
let prompt = context.to_prompt_string();
// prompt is now a single string passed to `claude -p`
```

The flattened output looks like this:

```
[System]
You are OMEGA Ω, a personal AI assistant...

[User]
What's the weather like?

[Assistant]
I don't have real-time weather data, but...

[User]
What about tomorrow?
```

Each section is labeled with `[System]`, `[User]`, or `[Assistant]` and separated by blank lines. The current message is always the last `[User]` section.

### Future API-Based Providers

Providers that talk to the Anthropic or OpenAI APIs will use the context fields directly, mapping `system_prompt` to the API's system message parameter and `history` + `current_message` to the structured messages array. The `to_prompt_string()` method would not be needed for these providers.

## The Big Picture

Here is where context fits in the full message pipeline:

```
User sends "Hello" on Telegram
       |
       v
  [Auth check] -- Is this user allowed?
       |
       v
  [Sanitize] -- Clean the input
       |
       v
  [Command check] -- Is it a /command? If yes, handle it and stop.
       |
       v
  [Build context] -- Query memory for history, facts, summaries
       |              Build enriched system prompt
       |              Package everything into a Context
       v
  [Provider call] -- Send the Context to Claude Code CLI
       |              Provider flattens it with to_prompt_string()
       |              AI generates a response
       v
  [Store exchange] -- Save this Q&A pair to the database
       |               (so it appears in future history)
       v
  [Send response] -- Deliver the answer back to Telegram
```

The context is the bridge between memory and reasoning. Without it, the AI would have no history, no personalization, and no behavioral guidance.

## Why the Default System Prompt Exists

When `Context::new()` is called, it uses a built-in default prompt:

> You are OMEGA Ω, a personal AI assistant running on the user's own server. You are helpful, concise, and action-oriented.

This prompt is intentionally short. It is only used for quick, one-shot calls (CLI usage, internal summarization tasks). The enriched context built by the memory store replaces this with a longer, personalized prompt that includes user facts and conversation summaries.

## Practical Examples

### One-Shot CLI Query

```
omega ask "Explain Rust ownership"

Context {
    system_prompt: "You are OMEGA Ω, a personal AI assistant...",
    history: [],
    current_message: "Explain Rust ownership",
}
```

### Mid-Conversation Telegram Message

```
Context {
    system_prompt: "You are OMEGA Ω, a personal AI assistant running on
        the user's own server. You are helpful, concise, and
        action-oriented.

        ## Known facts about this user
        - name: Alice
        - timezone: America/New_York

        ## Recent conversation summaries
        - User asked about Rust async patterns. Omega provided examples.",

    history: [
        ContextEntry { role: "user",      content: "How do I use tokio?" },
        ContextEntry { role: "assistant",  content: "Tokio is an async runtime..." },
    ],

    current_message: "Can you show me a spawn example?",
}
```

### Internal Summarization Task

```
Context {
    system_prompt: "You are OMEGA Ω, a personal AI assistant...",
    history: [],
    current_message: "Summarize this conversation in 1-2 sentences.
        Be factual and concise. Do not add commentary.

        User: How do I use tokio?
        Assistant: Tokio is an async runtime for Rust...",
}
```

## Key Design Decisions

**Why are history entries stored oldest-first?**
Because that is the natural reading order. The AI reads from top to bottom, so the earliest message should come first. The memory store fetches newest-first from the database and reverses the order before building the context.

**Why does `to_prompt_string()` use `[System]`/`[User]`/`[Assistant]` labels?**
These labels make the role boundaries explicit in plain text. Without them, the AI could confuse who said what, especially in long conversations.

**Why is `Serialize`/`Deserialize` derived?**
Both structs derive serde traits for future flexibility -- JSON logging, context caching, or transport over APIs. This is not actively used today but costs nothing to maintain.

**Why is the system prompt a `String` and not a configuration reference?**
Because different call sites produce different prompts. The CLI uses the default, the gateway builds an enriched one from facts and summaries, and internal tasks use task-specific prompts. A string field gives each caller full control.
