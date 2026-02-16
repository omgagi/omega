# Memory Store

## Path
`/Users/isudoajl/ownCloud/Projects/omega/crates/omega-memory/src/store.rs`

## What Is the Memory Store?

The Memory Store is Omega's long-term memory. It is a SQLite-backed persistence layer that remembers conversations, messages, and facts about users across sessions. Without it, Omega would forget everything the moment a message is processed.

The store serves four core purposes:

1. **Conversation continuity** -- When a user sends a message, the store retrieves their recent conversation history so the AI provider knows what was said before.
2. **User personalization** -- Facts extracted from past conversations (name, preferences, timezone) are stored and injected into future prompts, making the AI feel personal and context-aware.
3. **Cross-conversation recall** -- FTS5 full-text search lets Omega find relevant details from ANY past conversation, not just the current one. When a conversation closes, its summary compresses details, but the original messages remain searchable.
4. **Context building** -- Before every AI provider call, the store assembles a rich context containing the system prompt, conversation history, user facts, summaries, and recalled past messages.

## How the Store Fits in the Pipeline

Every incoming message passes through the store twice:

```
User sends message
       |
       v
  [Auth] → [Sanitize] → [Commands]
       |
       v
  memory.build_context()          ← First touch: READ from store
       |                             (fetch conversation, history, facts, summaries)
       v
  [Provider call]
       |
       v
  memory.store_exchange()         ← Second touch: WRITE to store
       |                             (save user message + AI response)
       v
  [Audit] → [Send response]
```

## The Database

The store uses a single SQLite file located at `~/.omega/memory.db` by default (configurable in `config.toml`). The database runs in WAL (Write-Ahead Logging) mode for better concurrent read performance.

### Tables

There are five tables managed by the store:

**conversations** -- Tracks conversation threads between users and Omega.
```
id            TEXT PRIMARY KEY   -- UUID v4
channel       TEXT               -- "telegram", "whatsapp", etc.
sender_id     TEXT               -- Platform user ID
started_at    TEXT               -- When the conversation started
updated_at    TEXT               -- Last modification time
summary       TEXT (nullable)    -- AI-generated summary (set on close)
last_activity TEXT               -- Most recent message timestamp
status        TEXT               -- 'active' or 'closed'
```

**messages** -- Individual messages within conversations.
```
id              TEXT PRIMARY KEY   -- UUID v4
conversation_id TEXT               -- FK to conversations
role            TEXT               -- 'user' or 'assistant'
content         TEXT               -- Message text
timestamp       TEXT               -- When the message was stored
metadata_json   TEXT (nullable)    -- JSON metadata for assistant messages
```

**facts** -- Key-value facts about users, extracted from conversations.
```
id        TEXT PRIMARY KEY   -- UUID v4
sender_id TEXT               -- User this fact belongs to
key       TEXT               -- Fact name (e.g., "name", "timezone")
value     TEXT               -- Fact value (e.g., "Alice", "America/New_York")
```
Facts are unique per `(sender_id, key)` -- storing a fact with the same key overwrites the previous value.

**scheduled_tasks** -- Tasks and reminders created by users through natural language.
```
id           TEXT PRIMARY KEY   -- UUID v4
channel      TEXT               -- Channel that created the task
sender_id    TEXT               -- User who created the task
reply_target TEXT               -- Platform-specific delivery target (e.g., chat_id)
description  TEXT               -- What to remind the user about
due_at       TEXT               -- When the task is due (ISO 8601)
repeat       TEXT (nullable)    -- Repeat type: NULL/once, daily, weekly, monthly, weekdays
status       TEXT               -- 'pending', 'delivered', or 'cancelled'
created_at   TEXT               -- When the task was created
delivered_at TEXT (nullable)    -- When the task was last delivered
```
Tasks are indexed on `(status, due_at)` for efficient polling and on `(sender_id, status)` for the `/tasks` command.

**_migrations** -- Tracks which database migrations have been applied.

## Conversation Lifecycle

Understanding conversations is key to understanding the store. A conversation is a thread of related messages between a specific user and Omega on a specific channel.

### How Conversations Are Created and Found

When a message arrives, the store looks for an active conversation for that user and channel. Two conditions must be met:

1. The conversation's status must be `'active'`.
2. The conversation's `last_activity` must be within the last 30 minutes.

If both conditions are met, the existing conversation is used and its `last_activity` is refreshed. If either condition fails, a new conversation is created with a fresh UUID.

This means that if a user goes silent for 30 minutes and then sends a new message, they start a fresh conversation. The old one remains active in the database until the background summarizer finds and closes it.

### The 30-Minute Timeout

The timeout is a compile-time constant (`CONVERSATION_TIMEOUT_MINUTES = 30`). It is not configurable via `config.toml`. This timeout controls two things:

- **Conversation boundaries** -- Messages more than 30 minutes apart belong to different conversations.
- **Idle detection** -- The background summarizer finds conversations idle for 30+ minutes and closes them.

### How Conversations Are Closed

There are three ways a conversation can be closed:

**1. Background summarizer** (normal path) -- Every 60 seconds, the gateway's background task finds idle conversations. For each one, it asks the AI to generate a summary and extract user facts, then closes the conversation with the summary stored.

**2. /forget command** (user-initiated) -- The user explicitly asks to forget the current conversation. The conversation is closed immediately without a summary.

**3. Shutdown** (graceful exit) -- When Omega shuts down (Ctrl+C), all active conversations are summarized and closed before exit. This prevents data loss.

### Lifecycle Diagram

```
New message from user (no active conversation)
       |
       v
  [CREATE] ─── status='active', new UUID
       |
       |    User sends more messages
       |    (each within 30min of the last)
       v
  [ACTIVE] ─── last_activity refreshed on each message
       |
       |    30+ minutes of silence
       v
  [IDLE] ─── background summarizer detects it
       |
       v
  [SUMMARIZE] ─── AI generates summary, extracts facts
       |
       v
  [CLOSED] ─── status='closed', summary stored
```

## Context Building

Context building is the most important operation in the store. It happens once per incoming message and produces the `Context` struct that the AI provider uses to generate a response.

### What Goes Into a Context

A context has three parts:

1. **System prompt** -- Instructions telling the AI who it is and how to behave. Dynamically enriched with user facts and conversation summaries.
2. **History** -- The last N messages from the current conversation, in chronological order. N defaults to 50, configurable via `max_context_messages`.
3. **Current message** -- The user's latest message.

### How Context Is Built

```rust
let context = store.build_context(&incoming).await?;
```

Behind the scenes, this does:

1. **Find or create the conversation** for this user and channel.
2. **Fetch recent messages** from the conversation (up to `max_context_messages`).
3. **Fetch user facts** -- all stored facts for this sender (name, preferences, etc.).
4. **Fetch recent summaries** -- the 3 most recent closed conversation summaries.
5. **Search past messages** -- FTS5 full-text search finds up to 5 relevant messages from other conversations.
6. **Build the system prompt** -- weave facts, summaries, and recalled messages into the base prompt.

### The System Prompt

The system prompt is not static. It is dynamically composed based on what Omega knows about the user. Here is an example of what it might look like:

```
You are Omega, a personal AI agent running on the owner's infrastructure.
You are NOT a chatbot. You are an agent that DOES things.

Rules:
- When asked to DO something, DO IT. Don't explain how.
- Answer concisely. No preamble.
- Speak the same language the user uses.
- Reference past conversations naturally when relevant.
- Never apologize unnecessarily.

Known facts about this user:
- name: Ivan
- timezone: America/Chicago
- language: Spanish

Recent conversation history:
- [2024-01-15 14:30:00] User asked about deploying a Rust service.
- [2024-01-14 09:15:00] User discussed SQLite performance tuning.

Related past context:
- [2024-01-10 16:00:00] User: I need to set up nginx reverse proxy for port 8080...
- [2024-01-08 11:30:00] User: The SSL cert is at /etc/letsencrypt/live/example.com...

IMPORTANT: Always respond in Spanish.
```

The "Known facts", "Recent conversation history", and "Related past context" sections are only included when data is available. Recalled messages are truncated to 200 characters to avoid bloating the prompt. The language directive (e.g., "IMPORTANT: Always respond in Spanish.") is always present, using the user's stored `preferred_language` fact or auto-detecting from the first message via stop-word heuristics for 7 languages (Spanish, Portuguese, French, German, Italian, Dutch, Russian), defaulting to English.

### Resilience

If facts, summaries, or recalled messages cannot be loaded (e.g., database error), the context is still built -- it just lacks personalization or recalled context. The store uses `unwrap_or_default()` for these queries, ensuring that a transient database issue does not prevent the user from getting a response.

## Facts Management

Facts are key-value pairs that Omega learns about users over time. They are extracted automatically during conversation summarization and can be managed by users via bot commands.

### How Facts Are Extracted

When a conversation is closed by the background summarizer, the gateway asks the AI to extract facts from the conversation transcript. The AI returns lines like:

```
name: Alice
timezone: America/New_York
interested_in: Rust programming
```

Each line is parsed as a `key: value` pair and stored in the database. If a fact with the same key already exists for that user, the value is updated (upsert).

### How Facts Are Used

Facts are injected into the system prompt during context building. This gives the AI long-term memory about the user without needing to store full conversation history indefinitely.

### Managing Facts

Users can interact with their facts through bot commands:

- **/facts** -- View all stored facts.
- **/forget** -- Delete specific facts or all facts.

### Fact Upsert Behavior

```rust
store.store_fact("user_123", "name", "Alice").await?;
// Database: sender_id="user_123", key="name", value="Alice"

store.store_fact("user_123", "name", "Bob").await?;
// Database: sender_id="user_123", key="name", value="Bob"  (updated, not duplicated)
```

The `UNIQUE(sender_id, key)` constraint with `ON CONFLICT DO UPDATE` ensures clean upsert behavior.

## Storing Exchanges

After the AI provider returns a response, the gateway saves both the user's message and the AI's response:

```rust
store.store_exchange(&incoming, &response).await?;
```

This inserts two rows into the `messages` table:
1. The user's message with `role = 'user'`.
2. The assistant's response with `role = 'assistant'` and serialized metadata (provider name, model, processing time).

Both messages are linked to the same conversation via `conversation_id`.

### Why Metadata Is Stored

The assistant message includes JSON metadata recording which provider and model generated the response, and how long it took. This is useful for debugging, performance analysis, and auditing which AI models are being used.

## Scheduled Tasks

The store manages the lifecycle of scheduled tasks -- reminders and recurring items created by users through natural language. The gateway's scheduler loop uses these methods to poll and deliver tasks.

### Task Lifecycle

```
User says "remind me to call John at 3pm"
       |
       v
  [Provider includes SCHEDULE: marker in response]
       |
       v
  [Gateway extracts marker]
       |
       v
  store.create_task() ─── status='pending', due_at set
       |
       |    Scheduler polls every 60s
       v
  store.get_due_tasks() ─── finds tasks where due_at <= now
       |
       v
  [Channel delivers reminder message]
       |
       v
  store.complete_task()
       |
       ├── One-shot: status → 'delivered', delivered_at set
       └── Recurring: due_at advanced, status stays 'pending'
```

### Store Methods

The five scheduler-related methods on the store:

- **`create_task()`** -- Inserts a new task with a UUID, setting `status = 'pending'`.
- **`get_due_tasks()`** -- Queries for all pending tasks where `due_at <= datetime('now')`.
- **`complete_task()`** -- For one-shot tasks, marks as `'delivered'`. For recurring tasks, advances `due_at` to the next occurrence.
- **`get_tasks_for_sender()`** -- Returns all pending tasks for a given user (used by the `/tasks` command).
- **`cancel_task()`** -- Matches a task by ID prefix and sender, setting `status = 'cancelled'` (used by the `/cancel` command).

## Memory Statistics and Introspection

The store provides several methods for system introspection, used primarily by bot commands:

### /status Command
```rust
let (conversations, messages, facts) = store.get_memory_stats(sender_id).await?;
let db_bytes = store.db_size().await?;
```

Returns the total number of conversations, messages, and facts for a user, plus the database file size.

### /memory or /history Command
```rust
let history = store.get_history(channel, sender_id, 10).await?;
```

Returns the most recent closed conversations with their summaries and timestamps.

### /facts Command
```rust
let facts = store.get_facts(sender_id).await?;
```

Returns all stored facts for the user.

## Migrations

The store uses a simple, custom migration system. SQL migration files are embedded at compile time and executed on first run. A `_migrations` table tracks which migrations have been applied.

### Migration Order

1. **001_init** -- Creates the `conversations`, `messages`, and `facts` tables.
2. **002_audit_log** -- Creates the `audit_log` table.
3. **003_memory_enhancement** -- Adds conversation boundaries (status, last_activity, summary) and re-creates facts with sender-scoped uniqueness.
4. **004_fts5_recall** -- Creates FTS5 full-text search index on user messages for cross-conversation recall.
5. **005_scheduled_tasks** -- Creates the `scheduled_tasks` table with indexes for the task queue.

### Handling Pre-Existing Databases

If the database was created before migration tracking was added, the store detects this by checking whether the `conversations` table already has a `summary` column. If it does, all migrations are marked as applied without re-running them. This prevents errors when upgrading from an older version.

## Common Patterns

### Pattern: Build Context Then Store Exchange

The most common usage pattern in the gateway:

```rust
// Before provider call: build rich context from memory
let context = store.build_context(&incoming).await?;

// Call the AI provider
let response = provider.complete(&context).await?;

// After provider call: save the exchange for future context
store.store_exchange(&incoming, &response).await?;
```

This pattern ensures that each new message benefits from the full conversation history, and that each exchange is persisted for future messages.

### Pattern: Summarize and Close

Used by the background summarizer and shutdown handler:

```rust
// Get all messages from the conversation
let messages = store.get_conversation_messages(conversation_id).await?;

// Ask AI to summarize (handled in gateway.rs, not in store)
let summary = provider.complete(&Context::new(&summarize_prompt)).await?;

// Ask AI to extract facts (handled in gateway.rs)
let facts_response = provider.complete(&Context::new(&facts_prompt)).await?;
// Parse and store each fact
store.store_fact(sender_id, key, value).await?;

// Close the conversation with the summary
store.close_conversation(conversation_id, &summary).await?;
```

### Pattern: User-Initiated Reset

When a user sends `/forget`:

```rust
// Close the active conversation without summarization
let closed = store.close_current_conversation(channel, sender_id).await?;

// Optionally delete facts
let deleted = store.delete_facts(sender_id, None).await?;  // all facts
let deleted = store.delete_facts(sender_id, Some("name")).await?;  // specific fact
```

The next message from the user will create a fresh conversation with no history from the forgotten one.

## Configuration

The store is configured via the `[memory]` section in `config.toml`:

```toml
[memory]
backend = "sqlite"           # Only SQLite is supported
db_path = "~/.omega/memory.db"  # Path to the database file
max_context_messages = 50    # Max messages to include in context
```

The `~` in `db_path` is expanded to the user's home directory at runtime.

## Performance Characteristics

### Database Operations

| Operation | Typical Latency | Notes |
|-----------|----------------|-------|
| `build_context()` | 10-50ms | 3-4 queries (conversation lookup, messages, facts, summaries). |
| `store_exchange()` | 5-20ms | 1 conversation lookup + 2 inserts. |
| `store_fact()` | 1-5ms | Single upsert. |
| `get_facts()` | 1-5ms | Single indexed query. |
| `find_idle_conversations()` | 1-10ms | Index scan on status + last_activity. |
| `close_conversation()` | 1-5ms | Single update. |
| `db_size()` | <1ms | Two PRAGMA queries. |

### Connection Pool

The pool allows up to 4 concurrent connections. Since the gateway processes messages sequentially on the main thread, contention is rare. The extra connections are useful for concurrent access from the background summarizer and audit logger.

### WAL Mode

The database runs in WAL (Write-Ahead Logging) mode, which allows concurrent readers while a writer is active. This prevents the background summarizer from blocking the main message pipeline during writes.

## Design Decisions

### Why SQLite?

SQLite is the right fit for Omega because:
- **Zero configuration** -- No separate database server to install or manage.
- **Single file** -- The entire database is one file at `~/.omega/memory.db`. Easy to back up, move, or delete.
- **Embedded** -- Linked directly into the Omega binary. No network latency.
- **Sufficient scale** -- Omega is a personal assistant, not a multi-tenant SaaS. SQLite handles the expected load with ease.

### Why 30-Minute Timeout?

30 minutes strikes a balance between:
- **Too short** (5 min) -- Normal pauses during work would split conversations unnecessarily.
- **Too long** (2 hours) -- Unrelated messages hours apart would be grouped together.

The timeout is a compile-time constant. If it needs to become configurable, it would be added to `MemoryConfig`.

### Why Suppress Fact/Summary Errors in build_context()?

Facts and summaries are "nice to have" enrichments. If the database has a transient issue reading them, the AI should still get the conversation history and current message. A slightly less personalized response is better than no response at all.

### Why Upsert for Facts?

Facts evolve over time. A user might mention their timezone once and then correct it later. The upsert pattern (`INSERT ... ON CONFLICT DO UPDATE`) ensures that the most recent value always wins without requiring the caller to check for existing records.

### Why UUID v4 for All Primary Keys?

UUIDs avoid the need for auto-increment sequences and make it trivial to generate IDs in application code without a database round-trip. They also ensure global uniqueness even if data is ever merged from multiple sources.

### Why is Language Detection a Heuristic?

A proper language detection library would add a dependency for a feature that is only used to set a user preference on first contact. The simple stop-word counting heuristic (2+ matches = detected language) is good enough for multilingual users and adds zero dependencies. If the heuristic guesses wrong, the user can always correct it with `/language <lang>` or by asking Omega to "speak in French" in a regular message.
