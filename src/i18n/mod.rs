//! Internationalization — localized strings for bot command responses.
//!
//! Uses a simple `t(key, lang)` function for static strings and
//! `format_*()` helpers for strings with interpolation.
//! Supported languages: English (fallback), Spanish, Portuguese, French,
//! German, Italian, Dutch, Russian.

mod commands;
mod confirmations;
mod format;
mod labels;

#[cfg(test)]
mod tests;

pub use format::*;

/// Return a localized static string for `key` in the given `lang`.
/// Falls back to English for unknown keys or unsupported languages.
pub fn t(key: &str, lang: &str) -> &'static str {
    if let Some(v) = labels::lookup(key, lang) {
        return v;
    }
    if let Some(v) = confirmations::lookup(key, lang) {
        return v;
    }
    if let Some(v) = commands::lookup(key, lang) {
        return v;
    }
    "???"
}
