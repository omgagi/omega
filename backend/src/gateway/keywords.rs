//! Keyword constants and matching for conditional prompt injection.

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
    "proyecto",
    "projeto",
    "projet",
    "projekt",
    "progetto",
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

/// Check if the trimmed, lowercased message is a build-specific confirmation phrase.
pub(super) fn is_build_confirmed(msg: &str) -> bool {
    let normalized = msg.trim().to_lowercase();
    BUILD_CONFIRM_KW.iter().any(|kw| normalized == *kw)
}

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

/// Check if the message is an explicit build cancellation.
pub(super) fn is_build_cancelled(msg: &str) -> bool {
    let normalized = msg.trim().to_lowercase();
    BUILD_CANCEL_KW.iter().any(|kw| normalized == *kw)
}

/// Localized cancellation confirmation.
pub(super) fn build_cancelled_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Construcción cancelada.",
        "Portuguese" => "Construção cancelada.",
        "French" => "Construction annulée.",
        "German" => "Build abgebrochen.",
        "Italian" => "Costruzione annullata.",
        "Dutch" => "Build geannuleerd.",
        "Russian" => "Сборка отменена.",
        _ => "Build cancelled.",
    }
}

/// Maximum seconds a pending build request stays valid. After this, the user
/// must re-trigger the build keyword.
pub(super) const BUILD_CONFIRM_TTL_SECS: i64 = 120;

/// Localized confirmation prompt sent when a build keyword is detected.
/// The user has BUILD_CONFIRM_TTL_SECS to reply with a simple "yes" / "sí" / "sim" etc.
pub(super) fn build_confirm_message(lang: &str, request_preview: &str) -> String {
    match lang {
        "Spanish" => format!(
            "Detecté una solicitud de construcción:\n\n\
             _{request_preview}_\n\n\
             Esto iniciará un proceso de construcción de varias fases. \
             Responde *sí* para continuar (tienes 2 minutos)."
        ),
        "Portuguese" => format!(
            "Detectei uma solicitação de construção:\n\n\
             _{request_preview}_\n\n\
             Isso iniciará um processo de construção em várias fases. \
             Responda *sim* para continuar (você tem 2 minutos)."
        ),
        "French" => format!(
            "J'ai détecté une demande de construction :\n\n\
             _{request_preview}_\n\n\
             Cela lancera un processus de construction en plusieurs phases. \
             Réponds *oui* pour continuer (tu as 2 minutes)."
        ),
        "German" => format!(
            "Ich habe eine Build-Anfrage erkannt:\n\n\
             _{request_preview}_\n\n\
             Dies startet einen mehrstufigen Build-Prozess. \
             Antworte *ja* zum Fortfahren (du hast 2 Minuten)."
        ),
        "Italian" => format!(
            "Ho rilevato una richiesta di costruzione:\n\n\
             _{request_preview}_\n\n\
             Questo avvierà un processo di costruzione in più fasi. \
             Rispondi *sì* per continuare (hai 2 minuti)."
        ),
        "Dutch" => format!(
            "Ik heb een build-verzoek gedetecteerd:\n\n\
             _{request_preview}_\n\n\
             Dit start een meerfasig bouwproces. \
             Antwoord *ja* om door te gaan (je hebt 2 minuten)."
        ),
        "Russian" => format!(
            "Обнаружен запрос на сборку:\n\n\
             _{request_preview}_\n\n\
             Это запустит многоэтапный процесс сборки. \
             Ответьте *да* чтобы продолжить (у вас 2 минуты)."
        ),
        _ => format!(
            "I detected a build request:\n\n\
             _{request_preview}_\n\n\
             This will start a multi-phase build process. \
             Reply *yes* to proceed (you have 2 minutes)."
        ),
    }
}

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

/// Check if any keyword in the list is contained in the lowercased message.
pub(super) fn kw_match(msg_lower: &str, keywords: &[&str]) -> bool {
    keywords.iter().any(|kw| msg_lower.contains(kw))
}

pub(super) use omega_core::config::SYSTEM_FACT_KEYS;

/// Validate a fact key/value before storing. Rejects junk patterns.
pub(super) fn is_valid_fact(key: &str, value: &str) -> bool {
    // Reject system-managed keys — only bot commands may set these.
    if SYSTEM_FACT_KEYS.contains(&key) {
        return false;
    }

    // Length limits.
    if key.len() > 50 || value.len() > 200 {
        return false;
    }

    // Key must not be numeric-only or start with a digit.
    if key.chars().next().is_none_or(|c| c.is_ascii_digit()) {
        return false;
    }

    // Value must not start with '$' (price patterns).
    if value.starts_with('$') {
        return false;
    }

    // Reject pipe-delimited table rows.
    if value.contains('|') && value.matches('|').count() >= 2 {
        return false;
    }

    // Reject values that look like prices (e.g., "0.00123", "45,678.90").
    let price_like = value
        .trim()
        .chars()
        .all(|c| c.is_ascii_digit() || c == '.' || c == ',' || c == '-');
    if price_like && !value.trim().is_empty() {
        return false;
    }

    true
}

// ---------------------------------------------------------------------------
// Discovery session constants and localized messages
// ---------------------------------------------------------------------------

/// Maximum seconds a discovery session stays valid.
pub(super) const DISCOVERY_TTL_SECS: i64 = 1800; // 30 minutes

/// Localized message sent when discovery starts (first round questions).
pub(super) fn discovery_intro_message(lang: &str, questions: &str) -> String {
    let intro = match lang {
        "Spanish" => "Antes de empezar a construir, necesito entender mejor tu idea:",
        "Portuguese" => "Antes de come\u{e7}ar a construir, preciso entender melhor sua ideia:",
        "French" => "Avant de commencer \u{e0} construire, j'ai besoin de mieux comprendre ton id\u{e9}e :",
        "German" => "Bevor ich mit dem Bauen beginne, muss ich deine Idee besser verstehen:",
        "Italian" => "Prima di iniziare a costruire, ho bisogno di capire meglio la tua idea:",
        "Dutch" => "Voordat ik begin met bouwen, moet ik je idee beter begrijpen:",
        "Russian" => "\u{41f}\u{440}\u{435}\u{436}\u{434}\u{435} \u{447}\u{435}\u{43c} \u{43d}\u{430}\u{447}\u{430}\u{442}\u{44c} \u{441}\u{431}\u{43e}\u{440}\u{43a}\u{443}, \u{43c}\u{43d}\u{435} \u{43d}\u{443}\u{436}\u{43d}\u{43e} \u{43b}\u{443}\u{447}\u{448}\u{435} \u{43f}\u{43e}\u{43d}\u{44f}\u{442}\u{44c} \u{432}\u{430}\u{448}\u{443} \u{438}\u{434}\u{435}\u{44e}:",
        _ => "Before I start building, I need to understand your idea better:",
    };
    format!("{intro}\n\n{questions}")
}

/// Localized message sent for follow-up discovery rounds (2-3).
pub(super) fn discovery_followup_message(lang: &str, questions: &str, round: u8) -> String {
    let followup = match lang {
        "Spanish" => format!("Gracias. Un par de preguntas m\u{e1}s ({round}/3):"),
        "Portuguese" => format!("Obrigado. Mais algumas perguntas ({round}/3):"),
        "French" => format!("Merci. Encore quelques questions ({round}/3) :"),
        "German" => format!("Danke. Noch ein paar Fragen ({round}/3):"),
        "Italian" => format!("Grazie. Ancora qualche domanda ({round}/3):"),
        "Dutch" => format!("Bedankt. Nog een paar vragen ({round}/3):"),
        "Russian" => format!("\u{421}\u{43f}\u{430}\u{441}\u{438}\u{431}\u{43e}. \u{415}\u{449}\u{451} \u{43d}\u{435}\u{441}\u{43a}\u{43e}\u{43b}\u{44c}\u{43a}\u{43e} \u{432}\u{43e}\u{43f}\u{440}\u{43e}\u{441}\u{43e}\u{432} ({round}/3):"),
        _ => format!("Thanks. A few more questions ({round}/3):"),
    };
    format!("{followup}\n\n{questions}")
}

/// Localized message sent when discovery completes and confirmation is needed.
pub(super) fn discovery_complete_message(lang: &str, brief_preview: &str) -> String {
    match lang {
        "Spanish" => format!(
            "Entendido. Esto es lo que voy a construir:\n\n\
             _{brief_preview}_\n\n\
             Responde *s\u{ed}* para comenzar la construcci\u{f3}n (tienes 2 minutos)."
        ),
        "Portuguese" => format!(
            "Entendido. Isto \u{e9} o que vou construir:\n\n\
             _{brief_preview}_\n\n\
             Responda *sim* para iniciar a constru\u{e7}\u{e3}o (voc\u{ea} tem 2 minutos)."
        ),
        "French" => format!(
            "Compris. Voici ce que je vais construire :\n\n\
             _{brief_preview}_\n\n\
             R\u{e9}ponds *oui* pour lancer la construction (tu as 2 minutes)."
        ),
        "German" => format!(
            "Verstanden. Das werde ich bauen:\n\n\
             _{brief_preview}_\n\n\
             Antworte *ja* um den Build zu starten (du hast 2 Minuten)."
        ),
        "Italian" => format!(
            "Capito. Ecco cosa costruir\u{f2}:\n\n\
             _{brief_preview}_\n\n\
             Rispondi *s\u{ec}* per avviare la costruzione (hai 2 minuti)."
        ),
        "Dutch" => format!(
            "Begrepen. Dit ga ik bouwen:\n\n\
             _{brief_preview}_\n\n\
             Antwoord *ja* om de build te starten (je hebt 2 minuten)."
        ),
        "Russian" => format!(
            "\u{41f}\u{43e}\u{43d}\u{44f}\u{43b}. \u{412}\u{43e}\u{442} \u{447}\u{442}\u{43e} \u{44f} \u{441}\u{43e}\u{431}\u{438}\u{440}\u{430}\u{44e}\u{441}\u{44c} \u{43f}\u{43e}\u{441}\u{442}\u{440}\u{43e}\u{438}\u{442}\u{44c}:\n\n\
             _{brief_preview}_\n\n\
             \u{41e}\u{442}\u{432}\u{435}\u{442}\u{44c}\u{442}\u{435} *\u{434}\u{430}* \u{447}\u{442}\u{43e}\u{431}\u{44b} \u{43d}\u{430}\u{447}\u{430}\u{442}\u{44c} \u{441}\u{431}\u{43e}\u{440}\u{43a}\u{443} (\u{443} \u{432}\u{430}\u{441} 2 \u{43c}\u{438}\u{43d}\u{443}\u{442}\u{44b})."
        ),
        _ => format!(
            "Got it. Here's what I'll build:\n\n\
             _{brief_preview}_\n\n\
             Reply *yes* to start the build (you have 2 minutes)."
        ),
    }
}

/// Localized message when discovery session expires.
pub(super) fn discovery_expired_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "La sesi\u{f3}n de descubrimiento expir\u{f3}. Env\u{ed}a tu solicitud de construcci\u{f3}n de nuevo si quieres continuar.",
        "Portuguese" => "A sess\u{e3}o de descoberta expirou. Envie sua solicita\u{e7}\u{e3}o de constru\u{e7}\u{e3}o novamente se quiser continuar.",
        "French" => "La session de d\u{e9}couverte a expir\u{e9}. Renvoie ta demande de construction si tu veux continuer.",
        "German" => "Die Discovery-Sitzung ist abgelaufen. Sende deine Build-Anfrage erneut, wenn du fortfahren m\u{f6}chtest.",
        "Italian" => "La sessione di scoperta \u{e8} scaduta. Invia di nuovo la tua richiesta di costruzione se vuoi continuare.",
        "Dutch" => "De discovery-sessie is verlopen. Stuur je build-verzoek opnieuw als je wilt doorgaan.",
        "Russian" => "\u{421}\u{435}\u{441}\u{441}\u{438}\u{44f} \u{43e}\u{431}\u{43d}\u{430}\u{440}\u{443}\u{436}\u{435}\u{43d}\u{438}\u{44f} \u{438}\u{441}\u{442}\u{435}\u{43a}\u{43b}\u{430}. \u{41e}\u{442}\u{43f}\u{440}\u{430}\u{432}\u{44c}\u{442}\u{435} \u{437}\u{430}\u{43f}\u{440}\u{43e}\u{441} \u{43d}\u{430} \u{441}\u{431}\u{43e}\u{440}\u{43a}\u{443} \u{441}\u{43d}\u{43e}\u{432}\u{430}, \u{435}\u{441}\u{43b}\u{438} \u{445}\u{43e}\u{442}\u{438}\u{442}\u{435} \u{43f}\u{440}\u{43e}\u{434}\u{43e}\u{43b}\u{436}\u{438}\u{442}\u{44c}.",
        _ => "Discovery session expired. Send your build request again if you want to continue.",
    }
}

/// Localized message when user cancels discovery.
pub(super) fn discovery_cancelled_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Descubrimiento cancelado.",
        "Portuguese" => "Descoberta cancelada.",
        "French" => "D\u{e9}couverte annul\u{e9}e.",
        "German" => "Discovery abgebrochen.",
        "Italian" => "Scoperta annullata.",
        "Dutch" => "Discovery geannuleerd.",
        "Russian" => "\u{41e}\u{431}\u{43d}\u{430}\u{440}\u{443}\u{436}\u{435}\u{43d}\u{438}\u{435} \u{43e}\u{442}\u{43c}\u{435}\u{43d}\u{435}\u{43d}\u{43e}.",
        _ => "Discovery cancelled.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Fact validation tests ---

    #[test]
    fn test_is_valid_fact_accepts_good_facts() {
        assert!(is_valid_fact("name", "Juan"));
        assert!(is_valid_fact("occupation", "software engineer"));
        assert!(is_valid_fact("timezone", "Europe/Madrid"));
        assert!(is_valid_fact("interests", "trading, hiking, Rust"));
        assert!(is_valid_fact("communication_style", "direct and concise"));
    }

    #[test]
    fn test_is_valid_fact_rejects_numeric_keys() {
        assert!(!is_valid_fact("1", "some value"));
        assert!(!is_valid_fact("42", "another value"));
        assert!(!is_valid_fact("3. step three", "do something"));
    }

    #[test]
    fn test_is_valid_fact_rejects_price_values() {
        assert!(!is_valid_fact("target", "$150.00"));
        assert!(!is_valid_fact("price", "0.00123"));
        assert!(!is_valid_fact("level", "45,678.90"));
    }

    #[test]
    fn test_is_valid_fact_rejects_pipe_delimited() {
        assert!(!is_valid_fact("data", "BTC | 45000 | bullish"));
        assert!(!is_valid_fact("row", "col1 | col2 | col3"));
    }

    #[test]
    fn test_is_valid_fact_rejects_oversized() {
        let long_key = "a".repeat(51);
        assert!(!is_valid_fact(&long_key, "value"));
        let long_value = "b".repeat(201);
        assert!(!is_valid_fact("key", &long_value));
    }

    #[test]
    fn test_is_valid_fact_rejects_system_keys() {
        assert!(!is_valid_fact("welcomed", "true"));
        assert!(!is_valid_fact("preferred_language", "en"));
        assert!(!is_valid_fact("active_project", "trader"));
        assert!(!is_valid_fact("personality", "direct, results-oriented"));
    }

    // --- Keyword detection tests ---

    #[test]
    fn test_kw_match_scheduling() {
        assert!(kw_match("remind me tomorrow", SCHEDULING_KW));
        assert!(kw_match("schedule a meeting", SCHEDULING_KW));
        assert!(kw_match("set an alarm for 5pm", SCHEDULING_KW));
        assert!(kw_match("cancel my reminder", SCHEDULING_KW));
        assert!(!kw_match("good morning", SCHEDULING_KW));
        assert!(!kw_match("how are you today", SCHEDULING_KW));
    }

    #[test]
    fn test_kw_match_recall() {
        assert!(kw_match("do you remember what we discussed", RECALL_KW));
        assert!(kw_match("you told me last time", RECALL_KW));
        assert!(kw_match("what did you mention yesterday", RECALL_KW));
        assert!(!kw_match("hello omega", RECALL_KW));
    }

    #[test]
    fn test_kw_match_tasks() {
        assert!(kw_match("show my tasks", TASKS_KW));
        assert!(kw_match("what's scheduled for today", TASKS_KW));
        assert!(kw_match("any pending reminders", TASKS_KW));
        assert!(!kw_match("good morning", TASKS_KW));
    }

    #[test]
    fn test_kw_match_projects() {
        assert!(kw_match("activate the trader project", PROJECTS_KW));
        assert!(kw_match("deactivate project", PROJECTS_KW));
        assert!(!kw_match("hello there", PROJECTS_KW));
    }

    #[test]
    fn test_kw_match_meta() {
        assert!(kw_match("improve this skill", META_KW));
        assert!(kw_match("report a bug", META_KW));
        assert!(kw_match("set up whatsapp", META_KW));
        assert!(kw_match("change my personality", META_KW));
        assert!(!kw_match("good morning", META_KW));
    }

    #[test]
    fn test_kw_match_profile() {
        assert!(kw_match("who am i exactly", PROFILE_KW));
        assert!(kw_match("tell me about me", PROFILE_KW));
        assert!(kw_match("what do you know about me", PROFILE_KW));
        assert!(kw_match("quién soy yo", PROFILE_KW));
        assert!(kw_match("wer bin ich eigentlich", PROFILE_KW));
        assert!(kw_match("кто я такой", PROFILE_KW));
        assert!(!kw_match("good morning", PROFILE_KW));
        assert!(!kw_match("hello omega", PROFILE_KW));
    }

    #[test]
    fn test_kw_match_outcomes() {
        assert!(kw_match("how did i do today", OUTCOMES_KW));
        assert!(kw_match("how am i doing overall", OUTCOMES_KW));
        assert!(kw_match("show my performance", OUTCOMES_KW));
        assert!(kw_match("any feedback for me", OUTCOMES_KW));
        assert!(kw_match("cómo lo hice hoy", OUTCOMES_KW));
        assert!(kw_match("wie habe ich abgeschnitten", OUTCOMES_KW));
        assert!(!kw_match("good morning", OUTCOMES_KW));
        assert!(!kw_match("hello omega", OUTCOMES_KW));
    }

    #[test]
    fn test_kw_match_builds() {
        // Positive matches — English
        assert!(kw_match("build me a cli tool", BUILDS_KW));
        assert!(kw_match("scaffold a new api", BUILDS_KW));
        assert!(kw_match("code me a scraper", BUILDS_KW));
        assert!(kw_match("build a price tracker", BUILDS_KW));
        assert!(kw_match("build an invoice tool", BUILDS_KW));
        assert!(kw_match("develop a monitoring service", BUILDS_KW));
        assert!(kw_match("write me a parser", BUILDS_KW));
        assert!(kw_match("make me a dashboard", BUILDS_KW));
        assert!(kw_match("i want a new tool for scraping", BUILDS_KW));
        assert!(kw_match("create a new app please", BUILDS_KW));
        assert!(kw_match("i need a new cli", BUILDS_KW));
        // Spanish
        assert!(kw_match("hazme un scraper", BUILDS_KW));
        assert!(kw_match("construye un api", BUILDS_KW));
        // Portuguese
        assert!(kw_match("construa um serviço", BUILDS_KW));
        assert!(kw_match("desenvolva um bot", BUILDS_KW));
        // French
        assert!(kw_match("développe un outil", BUILDS_KW));
        assert!(kw_match("crée un scraper", BUILDS_KW));
        // German
        assert!(kw_match("baue mir ein tool", BUILDS_KW));
        assert!(kw_match("erstelle einen scraper", BUILDS_KW));
        // Italian
        assert!(kw_match("costruisci un api", BUILDS_KW));
        assert!(kw_match("sviluppa un bot", BUILDS_KW));
        // Dutch
        assert!(kw_match("bouw me een tool", BUILDS_KW));
        assert!(kw_match("ontwikkel een scraper", BUILDS_KW));
        // Russian
        assert!(kw_match("создай мне скрейпер", BUILDS_KW));
        assert!(kw_match("построй инструмент", BUILDS_KW));
        // Typos — must still trigger
        assert!(kw_match("buil me a test tool", BUILDS_KW));
        assert!(kw_match("buidl a scraper", BUILDS_KW));
        assert!(kw_match("bulid me an api", BUILDS_KW));
        assert!(kw_match("scafold a project", BUILDS_KW));
        assert!(kw_match("devlop a service", BUILDS_KW));
        assert!(kw_match("mak me a dashboard", BUILDS_KW));
        assert!(kw_match("writ me a parser", BUILDS_KW));
        assert!(kw_match("hasme un scraper", BUILDS_KW));
        assert!(kw_match("cree un outil", BUILDS_KW));
        assert!(kw_match("erstele mir ein tool", BUILDS_KW));
        assert!(kw_match("costruici un api", BUILDS_KW));
        assert!(kw_match("ontwikel een scraper", BUILDS_KW));
        // Negative matches — must NOT trigger
        assert!(!kw_match("the building is tall", BUILDS_KW));
        assert!(!kw_match("my code review", BUILDS_KW));
        assert!(!kw_match("good morning", BUILDS_KW));
        assert!(!kw_match("check the build logs", BUILDS_KW));
        assert!(!kw_match("coding standards look good", BUILDS_KW));
    }

    #[test]
    fn test_kw_match_multilingual() {
        // Spanish — "recordar" and "alarma" trigger scheduling
        assert!(kw_match("puedes recordar esto", SCHEDULING_KW));
        assert!(kw_match("pon una alarma", SCHEDULING_KW));
        assert!(kw_match("agendar una reunión", SCHEDULING_KW));
        // Portuguese — "lembr" prefix matches "lembre", "lembrar", "lembrete"
        assert!(kw_match("lembre-me amanhã", SCHEDULING_KW));
        assert!(kw_match("lembro que você disse", RECALL_KW));
    }

    #[test]
    fn test_build_confirm_all_languages() {
        // English
        assert!(is_build_confirmed("yes"));
        assert!(is_build_confirmed("Yes")); // case insensitive
        assert!(is_build_confirmed("  yeah  ")); // trimmed
        assert!(is_build_confirmed("yep"));
        assert!(is_build_confirmed("go"));
        assert!(is_build_confirmed("do it"));
        assert!(is_build_confirmed("go ahead"));
        // Spanish
        assert!(is_build_confirmed("sí"));
        assert!(is_build_confirmed("si"));
        assert!(is_build_confirmed("dale"));
        assert!(is_build_confirmed("hazlo"));
        // Portuguese
        assert!(is_build_confirmed("sim"));
        assert!(is_build_confirmed("vai"));
        assert!(is_build_confirmed("bora"));
        // French
        assert!(is_build_confirmed("oui"));
        assert!(is_build_confirmed("ouais"));
        assert!(is_build_confirmed("vas-y"));
        // German
        assert!(is_build_confirmed("ja"));
        assert!(is_build_confirmed("jawohl"));
        assert!(is_build_confirmed("mach es"));
        // Italian
        assert!(is_build_confirmed("sì"));
        assert!(is_build_confirmed("fallo"));
        // Dutch
        assert!(is_build_confirmed("doe het"));
        assert!(is_build_confirmed("ga door"));
        // Russian
        assert!(is_build_confirmed("да"));
        assert!(is_build_confirmed("давай"));
        assert!(is_build_confirmed("поехали"));
    }

    #[test]
    fn test_build_confirm_rejects_non_confirmations() {
        // Rejections
        assert!(!is_build_confirmed("no"));
        assert!(!is_build_confirmed("nah"));
        assert!(!is_build_confirmed("cancel"));
        assert!(!is_build_confirmed("nein"));
        assert!(!is_build_confirmed("non"));
        assert!(!is_build_confirmed("нет"));
        // Sentences (not exact match)
        assert!(!is_build_confirmed("yes please build it now"));
        assert!(!is_build_confirmed("build me a tool"));
        assert!(!is_build_confirmed(""));
    }

    #[test]
    fn test_build_confirm_message_all_languages() {
        let en = build_confirm_message("English", "build me a tool");
        assert!(en.contains("build request"));
        assert!(en.contains("*yes*"));
        assert!(en.contains("2 minutes"));

        let es = build_confirm_message("Spanish", "hazme un scraper");
        assert!(es.contains("solicitud de construcción"));
        assert!(es.contains("*sí*"));

        let pt = build_confirm_message("Portuguese", "construa um bot");
        assert!(pt.contains("solicitação de construção"));
        assert!(pt.contains("*sim*"));

        let fr = build_confirm_message("French", "crée un outil");
        assert!(fr.contains("demande de construction"));
        assert!(fr.contains("*oui*"));

        let de = build_confirm_message("German", "baue mir ein tool");
        assert!(de.contains("Build-Anfrage"));
        assert!(de.contains("*ja*"));

        let it = build_confirm_message("Italian", "costruisci un api");
        assert!(it.contains("richiesta di costruzione"));
        assert!(it.contains("*sì*"));

        let nl = build_confirm_message("Dutch", "bouw me een tool");
        assert!(nl.contains("build-verzoek"));
        assert!(nl.contains("*ja*"));

        let ru = build_confirm_message("Russian", "создай скрейпер");
        assert!(ru.contains("запрос на сборку"));
        assert!(ru.contains("*да*"));
    }

    #[test]
    fn test_build_cancel_all_languages() {
        // English
        assert!(is_build_cancelled("no"));
        assert!(is_build_cancelled("No")); // case insensitive
        assert!(is_build_cancelled("  nope  ")); // trimmed
        assert!(is_build_cancelled("cancel"));
        assert!(is_build_cancelled("nevermind"));
        assert!(is_build_cancelled("never mind"));
        // Spanish
        assert!(is_build_cancelled("cancelar"));
        assert!(is_build_cancelled("olvídalo"));
        assert!(is_build_cancelled("olvidalo"));
        // Portuguese
        assert!(is_build_cancelled("não"));
        assert!(is_build_cancelled("nao"));
        assert!(is_build_cancelled("esquece"));
        // French
        assert!(is_build_cancelled("non"));
        assert!(is_build_cancelled("annuler"));
        assert!(is_build_cancelled("laisse tomber"));
        // German
        assert!(is_build_cancelled("nein"));
        assert!(is_build_cancelled("abbrechen"));
        // Italian
        assert!(is_build_cancelled("annulla"));
        assert!(is_build_cancelled("lascia stare"));
        // Dutch
        assert!(is_build_cancelled("nee"));
        assert!(is_build_cancelled("laat maar"));
        // Russian
        assert!(is_build_cancelled("нет"));
        assert!(is_build_cancelled("отмена"));
        assert!(is_build_cancelled("не надо"));
    }

    #[test]
    fn test_build_cancel_rejects_non_cancellations() {
        assert!(!is_build_cancelled("yes"));
        assert!(!is_build_cancelled("no thanks but maybe later"));
        assert!(!is_build_cancelled("build me a tool"));
        assert!(!is_build_cancelled(""));
    }

    #[test]
    fn test_build_cancelled_message_all_languages() {
        assert!(build_cancelled_message("English").contains("cancelled"));
        assert!(build_cancelled_message("Spanish").contains("cancelada"));
        assert!(build_cancelled_message("Portuguese").contains("cancelada"));
        assert!(build_cancelled_message("French").contains("annulée"));
        assert!(build_cancelled_message("German").contains("abgebrochen"));
        assert!(build_cancelled_message("Italian").contains("annullata"));
        assert!(build_cancelled_message("Dutch").contains("geannuleerd"));
        assert!(build_cancelled_message("Russian").contains("отменена"));
    }

    #[test]
    fn test_is_valid_fact_rejects_pending_build_request() {
        assert!(!is_valid_fact("pending_build_request", "build me a tool"));
    }
}
