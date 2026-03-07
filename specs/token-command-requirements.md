# /token Command — Requirements

> Slash command that shows estimated context token usage in the current conversation session.

## User Story

As an Omega user, I want to see how many context tokens have accumulated in my current conversation so I can decide whether to `/forget` and start fresh before hitting context limits.

## Requirements

### REQ-TOKEN-001 (Must): Command registration

**Description:** Register `/token` as a built-in command in the `Command` enum and `parse()` method.

**Acceptance criteria:**
- Given a message starting with `/token`, when `Command::parse()` is called, then it returns `Some(Command::Token)`
- Given a message `/token@omega_bot`, when parsed, then the `@botname` suffix is stripped and `Token` is returned

### REQ-TOKEN-002 (Must): Command handler

**Description:** Implement `handle_token()` in `commands/status.rs` that queries the memory store for current conversation token usage and returns a formatted response.

**Acceptance criteria:**
- Given the user sends `/token`, when the handler runs, then it returns a localized message showing: message count, estimated token count (prefixed with `~`)
- Given no active conversation exists, when the handler runs, then it returns a "no active conversation" message

### REQ-TOKEN-003 (Must): Memory query — read-only conversation lookup

**Description:** Add `get_active_conversation_id()` to the Store that finds the current active conversation WITHOUT creating one (unlike `get_or_create_conversation()`).

**Acceptance criteria:**
- Given an active conversation exists within the timeout window, when called, then it returns `Ok(Some(conversation_id))`
- Given no active conversation exists, when called, then it returns `Ok(None)` (no side effects, no INSERT)

### REQ-TOKEN-004 (Must): Memory query — token estimation

**Description:** Add `get_conversation_token_estimate()` to the Store that sums message content lengths in the current conversation and returns `(message_count, estimated_tokens)`.

**Acceptance criteria:**
- Given a conversation with messages, when called, then `estimated_tokens = total_chars / 4` (integer division)
- Given an empty conversation, when called, then it returns `(0, 0)`

### REQ-TOKEN-005 (Must): i18n — all 8 languages

**Description:** Add localized labels for the `/token` response and help text in all 8 languages.

**Labels needed:**
- `token_header` — "Context usage" header
- `token_messages` — "Messages:" label
- `token_estimated` — "Estimated tokens:" label
- `token_no_conversation` — "No active conversation" message
- `help_token` — Help text for `/help` output

**Acceptance criteria:**
- All 5 labels are translated in: English, Spanish, Portuguese, French, German, Italian, Dutch, Russian

### REQ-TOKEN-006 (Must): Help text update

**Description:** Add `/token` to the `/help` output.

**Acceptance criteria:**
- Given the user sends `/help`, when the response is rendered, then `/token` appears with its description

### REQ-TOKEN-007 (Must): Tests

**Description:** Unit tests for the new functionality.

**Test cases:**
- `Command::parse("/token")` returns `Some(Command::Token)`
- `Command::parse("/token@bot")` returns `Some(Command::Token)`
- `get_active_conversation_id()` returns `None` when no conversation exists
- `get_active_conversation_id()` returns `Some(id)` for an active conversation
- `get_conversation_token_estimate()` returns correct counts
- `get_conversation_token_estimate()` returns `(0, 0)` for empty conversation
- Token estimation formula: `chars / 4`

### REQ-TOKEN-008 (Must): CommandContext passthrough

**Description:** The handler needs `channel` and `sender_id` (already in `CommandContext`) plus `active_project` to find the right conversation. `active_project` is already available in the pipeline.

**Acceptance criteria:**
- The `/token` handler receives all data needed to query the correct conversation (channel, sender_id, active_project)

## Impact Analysis

| Component | Impact | Risk |
|-----------|--------|------|
| `commands/mod.rs` | Add enum variant + parse arm + handle arm | Low |
| `commands/status.rs` | Add handler function (~20 lines) | Low |
| `omega-memory/store/conversations.rs` | Add 2 read-only query methods (~30 lines) | Low |
| `i18n/labels.rs` | Add 4 label translations | Low |
| `i18n/commands.rs` | Add 1 help text translation | Low |
| `pipeline.rs` | No changes needed (standard command dispatch) | None |

## Traceability Matrix

| Req ID | Spec | Test | Code |
|--------|------|------|------|
| REQ-TOKEN-001 | This doc | commands/tests.rs | commands/mod.rs |
| REQ-TOKEN-002 | This doc | commands/tests.rs | commands/status.rs |
| REQ-TOKEN-003 | This doc | store/tests.rs | store/conversations.rs |
| REQ-TOKEN-004 | This doc | store/tests.rs | store/conversations.rs |
| REQ-TOKEN-005 | This doc | i18n/tests.rs | i18n/labels.rs, i18n/commands.rs |
| REQ-TOKEN-006 | This doc | commands/tests.rs | commands/status.rs |
| REQ-TOKEN-007 | This doc | — | — |
| REQ-TOKEN-008 | This doc | commands/tests.rs | commands/mod.rs |

## Won't Do (Deferred)

- **Natural language awareness** — The AI can already tell users about `/token` when asked about context. No system prompt injection needed.
- **Context budget indicator** — Would require model context window sizes, not currently tracked.
- **Real token counts** — Claude Code CLI returns `None` for `tokens_used`. Estimation is sufficient.
