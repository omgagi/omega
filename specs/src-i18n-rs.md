# Specification: backend/src/i18n/ (Directory Module)

## File Path
`backend/src/i18n/` (6-file directory module: mod.rs, commands.rs, confirmations.rs, format.rs, labels.rs, tests.rs)

## Purpose
Localized strings for bot command responses. All user-facing text in command handlers is internationalized through this module rather than hardcoded in English.

## Supported Languages (8)
English (fallback), Spanish, Portuguese, French, German, Italian, Dutch, Russian.

## Public API

### `t(key, lang) -> &'static str`
Static string lookup. Searches labels, confirmations, and commands submodules in order. Falls back to English for unknown languages. Returns `"???"` for unknown keys.

### `format_*()` helpers (re-exported from `format.rs`)
Interpolated string builders for responses that include dynamic values:
- `language_set(lang, new_lang)` -- language change confirmation
- `language_show(lang, current)` -- current language display with usage hint
- `personality_updated(lang, pref)` -- personality change confirmation
- `personality_show(lang, pref)` -- current personality display with reset hint
- `purge_result(lang, purged, keys_display)` -- purge count with preserved keys
- `humanize_project_name(name)` -- convert kebab-case directory name to Title Case (`realtor` -> `Realtor`, `real-estate` -> `Real Estate`)
- `project_persona_greeting(lang, persona)` -- persona-branded greeting in 8 languages: "Hi, I'm *OMEGA Ω {persona}*, what can I do for you?"
- `project_activated(lang, name)` -- project activation with persona greeting (auto-derives display name via `humanize_project_name`)
- `project_not_found(lang, name)` -- project not found with hint
- `active_project(lang, name)` -- active project display with deactivate hint
- `tasks_confirmed(lang, n)` -- scheduled N tasks header
- `tasks_cancelled_confirmed(lang, n)` -- cancelled N tasks header
- `tasks_updated_confirmed(lang, n)` -- updated N tasks header
- `task_save_failed(lang, n)` -- task save failure message

## Submodule Structure

| File | Responsibility |
|------|----------------|
| `mod.rs` | `t()` function, submodule declarations, re-exports `format::*` |
| `labels.rs` | `lookup(key, lang)` -- headers (status, memory, history, etc.), labels (uptime, provider, etc.), empty states (no_facts, no_history, etc.) |
| `commands.rs` | `lookup(key, lang)` -- `help_*` command description strings for `/help` output |
| `confirmations.rs` | `lookup(key, lang)` -- action confirmations (task_cancelled, conversation_cleared, etc.), status strings (available, missing_deps, etc.), heartbeat/skill/bug labels |
| `format.rs` | `format_*()` helpers that accept dynamic values and return `String` |
| `tests.rs` | Coverage: all keys have English fallback, all 8 languages differ from English for sample keys, unknown language falls back to English, unknown key returns `"???"`, format helpers produce expected output, all help keys contain their command name |

## Design

- All `lookup()` functions are `pub(super)` -- only accessible within the `i18n` module.
- The `t()` function in `mod.rs` chains lookups across submodules: labels -> confirmations -> commands.
- Static strings use match arms per language with `_` wildcard for English fallback.
- Format helpers call `t()` internally when they need static label fragments.
- No external dependencies -- pure Rust string matching.
