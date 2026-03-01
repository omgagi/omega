# Internationalization (backend/src/i18n/)

## Overview

The `i18n` module provides localized strings for all user-facing bot responses. Every command response, confirmation message, and status label is internationalized through this module rather than hardcoded in English.

**Supported languages (8):** English (fallback), Spanish, Portuguese, French, German, Italian, Dutch, Russian.

## How It Works

### Static String Lookup

The primary interface is `t(key, lang)`:

```rust
use crate::i18n;

let msg = i18n::t("task_confirmed", "Spanish");
// Returns: "Programado"
```

The function searches three submodules in order: labels, confirmations, commands. If the key is not found in any, it returns `"???"`. If the language is not recognized, it falls back to English.

### Format Helpers

For strings that include dynamic values (counts, names, etc.), use the `format_*()` helpers:

```rust
use crate::i18n;

let msg = i18n::tasks_confirmed("French", 3);
// Returns: "3 taches programmees"

let msg = i18n::project_activated("German", "trading-bot");
// Returns: "Projekt aktiviert: trading-bot"
```

## Module Structure

| File | Purpose |
|------|---------|
| `mod.rs` | `t()` function, submodule declarations, re-exports `format::*` |
| `labels.rs` | Headers (status, memory, history), labels (uptime, provider), empty states (no_facts, no_history) |
| `commands.rs` | `help_*` command descriptions for `/help` output |
| `confirmations.rs` | Action confirmations (task_cancelled, conversation_cleared), status strings, heartbeat/skill/bug labels |
| `format.rs` | `format_*()` helpers that accept dynamic values and return `String` |
| `tests.rs` | Coverage for all keys, all 8 languages, fallback behavior |

## Available Format Helpers

| Function | Parameters | Purpose |
|----------|-----------|---------|
| `language_set(lang, new_lang)` | Language change confirmation |
| `language_show(lang, current)` | Current language display with hint |
| `personality_updated(lang, pref)` | Personality change confirmation |
| `personality_show(lang, pref)` | Current personality display with hint |
| `purge_result(lang, purged, keys)` | Purge count with preserved keys |
| `project_activated(lang, name)` | Project activation confirmation |
| `project_not_found(lang, name)` | Project not found with hint |
| `active_project(lang, name)` | Active project display with deactivate hint |
| `tasks_confirmed(lang, n)` | "Scheduled N tasks" header |
| `tasks_cancelled_confirmed(lang, n)` | "Cancelled N tasks" header |
| `tasks_updated_confirmed(lang, n)` | "Updated N tasks" header |
| `task_save_failed(lang, n)` | Task save failure message |

## Adding a New Key

1. Decide which submodule it belongs to (labels, confirmations, or commands)
2. Add a match arm in the corresponding `lookup()` function with all 8 languages
3. Use `_` wildcard for the English default
4. Add a test in `tests.rs` to verify the key exists

## Adding a New Language

Not currently supported without code changes. Each `lookup()` function uses match arms per language name string. A new language would require adding a match arm in every key across all submodules.

## Design Decisions

- **No external i18n crate** -- Pure Rust string matching keeps the module dependency-free and fast.
- **Static strings via match** -- All translations are compile-time constants (except format helpers which allocate).
- **Layered lookup** -- `t()` chains across submodules so keys don't need to be globally unique within a single file, only within their category.
- **English fallback** -- Unknown languages silently fall back to English rather than erroring.
