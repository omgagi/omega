//! Static keyword data arrays for conditional prompt injection.
//!
//! Extracted from `keywords.rs` to respect the 500-line-per-file rule.
//! All arrays are `pub(super)` — consumed by `keywords.rs` functions and
//! the broader gateway module.

/// Maximum number of retries for failed action tasks.
pub(super) const MAX_ACTION_RETRIES: u32 = 3;

// --- Keyword lists for conditional prompt injection ---

/// Keywords that trigger the scheduling context section.
pub(super) const SCHEDULING_KW: &[&str] = &[
    "remind",
    "schedule",
    "alarm",
    "timer",
    "tomorrow",
    "next week",
    "daily",
    "weekly",
    "monthly",
    "weekday",
    "cancel",
    "update task",
    "recurring",
    "every morning",
    "every day",
    "every evening",
    "appointment",
    "due",
    "at noon",
    "recuerda",
    "recuérd",
    "recordar",
    "alarma",
    "agendar",
    "lembr",
    "rappel",
    "erinner",
    "ricorda",
    "herinner",
];

/// Keywords that trigger semantic recall (FTS5 related past messages).
pub(super) const RECALL_KW: &[&str] = &[
    "remember",
    "last time",
    "you said",
    "earlier",
    "before",
    "we talked",
    "we discussed",
    "you told",
    "you mentioned",
    "yesterday",
    "last week",
    "recuerd",
    "dijiste",
    "lembr",
    "você disse",
    "souvien",
    "erinnerst",
    "ricord",
    "herinner",
];

/// Keywords that trigger pending tasks injection.
pub(super) const TASKS_KW: &[&str] = &[
    "task",
    "reminder",
    "pending",
    "scheduled",
    "what's coming",
    "what's scheduled",
    "my tasks",
    "my reminders",
    "tarea",
    "recordatorio",
    "pendiente",
    "tarefa",
    "lembrete",
    "tâche",
    "aufgabe",
    "compito",
    "taak",
];

/// Keywords that trigger the projects context section.
pub(super) const PROJECTS_KW: &[&str] = &[
    "project",
    "activate",
    "deactivate",
    // es
    "proyecto",
    "activar",
    "desactivar",
    // pt
    "projeto",
    "ativar",
    "desativar",
    // fr
    "projet",
    "activer",
    "désactiver",
    // de
    "projekt",
    "aktivieren",
    "deaktivieren",
    // it
    "progetto",
    "attivare",
    "disattivare",
    // nl
    "projecten",
    "activeren",
    "deactiveren",
    // ru
    "проект",
    "активировать",
    "деактивировать",
];

/// Keywords that trigger user profile injection into the system prompt.
pub(super) const PROFILE_KW: &[&str] = &[
    "who am i",
    "my name",
    "about me",
    "my profile",
    "my facts",
    "what do you know",
    "quién soy",
    "mi nombre",
    "sobre mí",
    "quem sou",
    "meu nome",
    "sobre mim",
    "qui suis",
    "mon nom",
    "wer bin ich",
    "mein name",
    "chi sono",
    "mio nome",
    "wie ben ik",
    "mijn naam",
    "кто я",
];

/// Keywords that trigger recent outcomes injection.
pub(super) const OUTCOMES_KW: &[&str] = &[
    "how did i",
    "how am i doing",
    "reward",
    "outcome",
    "feedback",
    "performance",
    "cómo lo hice",
    "resultado",
    "como me saí",
    "desempenho",
    "comment j'ai",
    "résultat",
    "wie habe ich",
    "ergebnis",
    "come ho fatto",
    "risultato",
    "hoe deed ik",
    "resultaat",
];

/// Keywords that trigger the builds context section.
pub(super) const BUILDS_KW: &[&str] = &[
    "build me",
    "build a ",
    "build an ",
    "scaffold",
    "code me",
    "code a ",
    "code an ",
    "develop a",
    "develop an",
    "make me a",
    "write me a",
    "new tool",
    "new app",
    "new service",
    "new api",
    "new library",
    "new cli",
    // Common English typos (mobile keyboard, missing/swapped letters)
    "buil me",
    "buil a ",
    "buil an ",
    "buidl",
    "bulid",
    "biuld",
    "buld ",
    "scafold",
    "scaffol ",
    "devlop",
    "develp",
    "develo a",
    "mak me a",
    "writ me a",
    "wrte me a",
    // Spanish
    "constrúyeme",
    "construye un",
    "hazme un",
    "hazme una",
    "desarroll",
    "codifica",
    // Spanish typos
    "contruyeme",
    "construyem",
    "hasme un",
    // Portuguese
    "construa um",
    "crie um",
    "desenvolva",
    // Portuguese typos
    "contrua um",
    "desevolva",
    // French
    "construis",
    "développe",
    "code-moi",
    "crée un",
    "crée une",
    "nouvel outil",
    "nouvelle app",
    // French typos
    "developpe",
    "cree un",
    "cree une",
    // German
    "baue mir",
    "erstelle",
    "entwickle",
    "programmier",
    "neues tool",
    "neue app",
    // German typos
    "erstele",
    "enwickle",
    // Italian
    "costruisci",
    "sviluppa",
    "programma un",
    "crea un",
    "crea una",
    "nuovo strumento",
    "nuova app",
    // Italian typos
    "costruici",
    "svilupa",
    // Dutch
    "bouw me",
    "maak me",
    "ontwikkel",
    "codeer",
    "nieuwe tool",
    "nieuwe app",
    // Dutch typos
    "ontwikel",
    "bouw mij",
    // Russian
    "построй",
    "создай",
    "разработай",
    "напиши мне",
    "новый инструмент",
    "новое приложение",
    // Russian typos
    "пострй",
    "сздай",
    "разрабтай",
];

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

/// Keywords that trigger the meta context section.
pub(super) const META_KW: &[&str] = &[
    "skill",
    "improve",
    "bug",
    "limitation",
    "whatsapp",
    "qr",
    "pair",
    "personality",
    "forget",
    "purge",
];

/// Maximum seconds a discovery session stays valid.
pub(super) const DISCOVERY_TTL_SECS: i64 = 1800; // 30 minutes

/// Maximum seconds a setup session stays valid.
pub(super) const SETUP_TTL_SECS: i64 = 1800; // 30 minutes
