//! Static keyword data arrays for the gateway module.
//!
//! Extracted from `keywords.rs` to respect the 500-line-per-file rule.
//! All arrays are `pub(super)` — consumed by `keywords.rs` functions and
//! the broader gateway module.
//!
//! Note: Conditional prompt injection keyword arrays (SCHEDULING_KW, RECALL_KW,
//! TASKS_KW, PROJECTS_KW, META_KW, PROFILE_KW, OUTCOMES_KW) were removed in
//! the always-inject refactor. All context sections are now injected
//! unconditionally — no keyword gating.

/// Maximum number of retries for failed action tasks.
pub(super) const MAX_ACTION_RETRIES: u32 = 3;

/// Simple confirmation words for build requests (lowercased).
/// Safe because they are only checked during the 2-minute TTL window after
/// OMEGA explicitly asked for confirmation — outside that window, "yes" is just "yes".
pub(super) const BUILD_CONFIRM_KW: &[&str] = &[
    // English
    "yes",
    "yeah",
    "yep",
    "y",
    "go",
    "do it",
    "go ahead",
    "start",
    // Spanish
    "sí",
    "si",
    "dale",
    "hazlo",
    "adelante",
    // Portuguese
    "sim",
    "vai",
    "bora",
    // French
    "oui",
    "ouais",
    "vas-y",
    // German
    "ja",
    "jawohl",
    "los",
    "mach es",
    // Italian
    "sì",
    "vai",
    "fallo",
    // Dutch
    "ja",
    "doe het",
    "ga door",
    // Russian
    "да",
    "давай",
    "поехали",
];

/// Explicit cancellation words — immediately close the confirmation window.
pub(super) const BUILD_CANCEL_KW: &[&str] = &[
    // English
    "no",
    "nah",
    "nope",
    "n",
    "cancel",
    "stop",
    "nevermind",
    "never mind",
    // Spanish
    "no",
    "cancelar",
    "olvídalo",
    "olvidalo",
    // Portuguese
    "não",
    "nao",
    "cancelar",
    "esquece",
    // French
    "non",
    "annuler",
    "laisse tomber",
    // German
    "nein",
    "abbrechen",
    "lass es",
    // Italian
    "no",
    "annulla",
    "lascia stare",
    // Dutch
    "nee",
    "annuleer",
    "laat maar",
    // Russian
    "нет",
    "отмена",
    "не надо",
];

/// Maximum seconds a pending build request stays valid. After this, the user
/// must re-trigger the build keyword.
pub(super) const BUILD_CONFIRM_TTL_SECS: i64 = 120;

/// Keywords that trigger the help/commands intercept on WhatsApp.
/// On Telegram, the autocomplete menu shows all commands natively.
/// On WhatsApp there is no autocomplete, so we detect natural-language
/// help requests and return the `/help` output directly.
pub(super) const HELP_KW: &[&str] = &[
    // English — broad substrings that catch natural variations
    "what commands",
    "which commands",
    "what can you do",
    "what do you do",
    "your commands",
    "available commands",
    "show commands",
    "list commands",
    "your functions",
    "what functions",
    "your features",
    "what features",
    "how do i use you",
    "how does this work",
    // Spanish
    "qué comandos",
    "que comandos",
    "cuáles comandos",
    "cuales comandos",
    "qué puedes hacer",
    "que puedes hacer",
    "tus comandos",
    "tus funciones",
    "qué funciones",
    "que funciones",
    // Portuguese
    "quais comandos",
    "que comandos",
    "o que você faz",
    "o que voce faz",
    "seus comandos",
    "teus comandos",
    "que funções",
    "que funcoes",
    // French
    "quelles commandes",
    "que peux-tu faire",
    "tes commandes",
    "les commandes",
    // German
    "welche befehle",
    "was kannst du",
    "deine befehle",
    // Italian
    "quali comandi",
    "cosa puoi fare",
    "i tuoi comandi",
    // Dutch
    "welke commando",
    "wat kan je",
    "wat kun je",
    // Russian
    "какие команды",
    "что ты умеешь",
    "что ты можешь",
    "твои команды",
];

/// Maximum seconds a setup session stays valid.
pub(super) const SETUP_TTL_SECS: i64 = 1800; // 30 minutes

/// Maximum seconds a Google auth session stays valid.
pub(super) const GOOGLE_AUTH_TTL_SECS: i64 = 1800; // 30 minutes
