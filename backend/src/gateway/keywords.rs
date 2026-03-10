//! Keyword matching functions for the gateway module.
//!
//! Static keyword data arrays live in `keywords_data.rs` (split for the
//! 500-line-per-file rule). Re-exported here so the rest of the gateway
//! module sees a single flat namespace.

pub(super) use super::keywords_data::*;

/// Check if the trimmed, lowercased message is a build-specific confirmation phrase.
pub(super) fn is_build_confirmed(msg: &str) -> bool {
    let normalized = msg.trim().to_lowercase();
    BUILD_CONFIRM_KW.iter().any(|kw| normalized == *kw)
}

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

/// Check if any keyword in the list is contained in the lowercased message.
///
/// Used for the WhatsApp help intercept (HELP_KW) — the only remaining
/// keyword-match gate in the pipeline.
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
// Setup session localized messages
// ---------------------------------------------------------------------------

/// Localized help message for `/setup` command (shown when no description provided).
pub(super) fn setup_help_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Usa `/setup` seguido de una descripci\u{f3}n de tu negocio para que *OMEGA \u{3a9}* se configure como experto en tu dominio.\n\nEjemplo: `/setup Soy agente inmobiliario en Lisboa`",
        "Portuguese" => "Use `/setup` seguido de uma descri\u{e7}\u{e3}o do seu neg\u{f3}cio para que *OMEGA \u{3a9}* se configure como especialista no seu dom\u{ed}nio.\n\nExemplo: `/setup Sou corretor de im\u{f3}veis em Lisboa`",
        "French" => "Utilisez `/setup` suivi d'une description de votre activit\u{e9} pour que *OMEGA \u{3a9}* se configure comme expert de votre domaine.\n\nExemple : `/setup Je suis agent immobilier \u{e0} Lisbonne`",
        "German" => "Verwende `/setup` gefolgt von einer Beschreibung deines Gesch\u{e4}fts, damit *OMEGA \u{3a9}* sich als Experte f\u{fc}r deine Dom\u{e4}ne konfiguriert.\n\nBeispiel: `/setup Ich bin Immobilienmakler in Lissabon`",
        "Italian" => "Usa `/setup` seguito da una descrizione della tua attivit\u{e0} per configurare *OMEGA \u{3a9}* come esperto nel tuo dominio.\n\nEsempio: `/setup Sono un agente immobiliare a Lisbona`",
        "Dutch" => "Gebruik `/setup` gevolgd door een beschrijving van je bedrijf om *OMEGA \u{3a9}* te configureren als domeinexpert.\n\nVoorbeeld: `/setup Ik ben makelaar in Lissabon`",
        "Russian" => "\u{418}\u{441}\u{43f}\u{43e}\u{43b}\u{44c}\u{437}\u{443}\u{439}\u{442}\u{435} `/setup` \u{441} \u{43e}\u{43f}\u{438}\u{441}\u{430}\u{43d}\u{438}\u{435}\u{43c} \u{432}\u{430}\u{448}\u{435}\u{433}\u{43e} \u{431}\u{438}\u{437}\u{43d}\u{435}\u{441}\u{430}, \u{447}\u{442}\u{43e}\u{431}\u{44b} *OMEGA \u{3a9}* \u{43d}\u{430}\u{441}\u{442}\u{440}\u{43e}\u{438}\u{43b}\u{441}\u{44f} \u{43a}\u{430}\u{43a} \u{44d}\u{43a}\u{441}\u{43f}\u{435}\u{440}\u{442} \u{432} \u{432}\u{430}\u{448}\u{435}\u{439} \u{43e}\u{431}\u{43b}\u{430}\u{441}\u{442}\u{438}.\n\n\u{41f}\u{440}\u{438}\u{43c}\u{435}\u{440}: `/setup \u{42f} \u{440}\u{438}\u{44d}\u{43b}\u{442}\u{43e}\u{440} \u{432} \u{41b}\u{438}\u{441}\u{441}\u{430}\u{431}\u{43e}\u{43d}\u{435}`",
        _ => "Use `/setup` followed by a description of your business so *OMEGA \u{3a9}* configures itself as your domain expert.\n\nExample: `/setup I'm a realtor in Lisbon`",
    }
}

/// Localized intro message when setup starts (first round questions).
pub(super) fn setup_intro_message(lang: &str, questions: &str) -> String {
    let intro = match lang {
        "Spanish" => "Para configurar *OMEGA \u{3a9}* como tu experto, necesito entender mejor tu negocio:",
        "Portuguese" => "Para configurar *OMEGA \u{3a9}* como seu especialista, preciso entender melhor seu neg\u{f3}cio:",
        "French" => "Pour configurer *OMEGA \u{3a9}* comme votre expert, j'ai besoin de mieux comprendre votre activit\u{e9} :",
        "German" => "Um *OMEGA \u{3a9}* als deinen Experten einzurichten, muss ich dein Gesch\u{e4}ft besser verstehen:",
        "Italian" => "Per configurare *OMEGA \u{3a9}* come tuo esperto, ho bisogno di capire meglio la tua attivit\u{e0}:",
        "Dutch" => "Om *OMEGA \u{3a9}* als jouw expert in te stellen, moet ik je bedrijf beter begrijpen:",
        "Russian" => "\u{427}\u{442}\u{43e}\u{431}\u{44b} \u{43d}\u{430}\u{441}\u{442}\u{440}\u{43e}\u{438}\u{442}\u{44c} *OMEGA \u{3a9}* \u{43a}\u{430}\u{43a} \u{432}\u{430}\u{448}\u{435}\u{433}\u{43e} \u{44d}\u{43a}\u{441}\u{43f}\u{435}\u{440}\u{442}\u{430}, \u{43c}\u{43d}\u{435} \u{43d}\u{443}\u{436}\u{43d}\u{43e} \u{43b}\u{443}\u{447}\u{448}\u{435} \u{43f}\u{43e}\u{43d}\u{44f}\u{442}\u{44c} \u{432}\u{430}\u{448} \u{431}\u{438}\u{437}\u{43d}\u{435}\u{441}:",
        _ => "To configure *OMEGA \u{3a9}* as your expert, I need to understand your business better:",
    };
    format!("{intro}\n\n{questions}")
}

/// Localized follow-up message for setup rounds 2-3.
pub(super) fn setup_followup_message(lang: &str, questions: &str, round: u8) -> String {
    let followup = match lang {
        "Spanish" => format!("Gracias. Unas preguntas m\u{e1}s ({round}/3):"),
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

/// Localized message when setup proposal is ready for user review.
pub(super) fn setup_proposal_message(lang: &str, preview: &str) -> String {
    match lang {
        "Spanish" => format!(
            "Esto es lo que voy a configurar:\n\n\
             {preview}\n\n\
             Responde *s\u{ed}* para crear todo, o describe cambios que quieras."
        ),
        "Portuguese" => format!(
            "Isto \u{e9} o que vou configurar:\n\n\
             {preview}\n\n\
             Responda *sim* para criar tudo, ou descreva altera\u{e7}\u{f5}es que deseja."
        ),
        "French" => format!(
            "Voici ce que je vais configurer :\n\n\
             {preview}\n\n\
             R\u{e9}ponds *oui* pour tout cr\u{e9}er, ou d\u{e9}cris les changements souhait\u{e9}s."
        ),
        "German" => format!(
            "Das werde ich einrichten:\n\n\
             {preview}\n\n\
             Antworte *ja* um alles zu erstellen, oder beschreibe gew\u{fc}nschte \u{c4}nderungen."
        ),
        "Italian" => format!(
            "Ecco cosa configurer\u{f2}:\n\n\
             {preview}\n\n\
             Rispondi *s\u{ec}* per creare tutto, o descrivi le modifiche desiderate."
        ),
        "Dutch" => format!(
            "Dit ga ik instellen:\n\n\
             {preview}\n\n\
             Antwoord *ja* om alles aan te maken, of beschrijf gewenste wijzigingen."
        ),
        "Russian" => format!(
            "\u{412}\u{43e}\u{442} \u{447}\u{442}\u{43e} \u{44f} \u{43d}\u{430}\u{441}\u{442}\u{440}\u{43e}\u{44e}:\n\n\
             {preview}\n\n\
             \u{41e}\u{442}\u{432}\u{435}\u{442}\u{44c}\u{442}\u{435} *\u{434}\u{430}* \u{447}\u{442}\u{43e}\u{431}\u{44b} \u{441}\u{43e}\u{437}\u{434}\u{430}\u{442}\u{44c} \u{432}\u{441}\u{451}, \u{438}\u{43b}\u{438} \u{43e}\u{43f}\u{438}\u{448}\u{438}\u{442}\u{435} \u{436}\u{435}\u{43b}\u{430}\u{435}\u{43c}\u{44b}\u{435} \u{438}\u{437}\u{43c}\u{435}\u{43d}\u{435}\u{43d}\u{438}\u{44f}."
        ),
        _ => format!(
            "Here's what I'll set up:\n\n\
             {preview}\n\n\
             Reply *yes* to create everything, or describe changes you'd like."
        ),
    }
}

/// Localized message when setup completes successfully.
pub(super) fn setup_complete_message(lang: &str, project: &str) -> String {
    crate::i18n::project_activated(lang, project)
}

/// Localized message when setup is cancelled.
pub(super) fn setup_cancelled_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Setup cancelado.",
        "Portuguese" => "Setup cancelado.",
        "French" => "Setup annul\u{e9}.",
        "German" => "Setup abgebrochen.",
        "Italian" => "Setup annullato.",
        "Dutch" => "Setup geannuleerd.",
        "Russian" => "\u{41d}\u{430}\u{441}\u{442}\u{440}\u{43e}\u{439}\u{43a}\u{430} \u{43e}\u{442}\u{43c}\u{435}\u{43d}\u{435}\u{43d}\u{430}.",
        _ => "Setup cancelled.",
    }
}

/// Localized message when setup session expires.
pub(super) fn setup_expired_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "La sesi\u{f3}n de setup expir\u{f3}. Usa /setup de nuevo si quieres continuar.",
        "Portuguese" => "A sess\u{e3}o de setup expirou. Use /setup novamente se quiser continuar.",
        "French" => "La session de setup a expir\u{e9}. Utilisez /setup \u{e0} nouveau si vous voulez continuer.",
        "German" => "Die Setup-Sitzung ist abgelaufen. Verwende /setup erneut, wenn du fortfahren m\u{f6}chtest.",
        "Italian" => "La sessione di setup \u{e8} scaduta. Usa /setup di nuovo se vuoi continuare.",
        "Dutch" => "De setup-sessie is verlopen. Gebruik /setup opnieuw als je wilt doorgaan.",
        "Russian" => "\u{421}\u{435}\u{441}\u{441}\u{438}\u{44f} \u{43d}\u{430}\u{441}\u{442}\u{440}\u{43e}\u{439}\u{43a}\u{438} \u{438}\u{441}\u{442}\u{435}\u{43a}\u{43b}\u{430}. \u{418}\u{441}\u{43f}\u{43e}\u{43b}\u{44c}\u{437}\u{443}\u{439}\u{442}\u{435} /setup \u{441}\u{43d}\u{43e}\u{432}\u{430}, \u{435}\u{441}\u{43b}\u{438} \u{445}\u{43e}\u{442}\u{438}\u{442}\u{435} \u{43f}\u{440}\u{43e}\u{434}\u{43e}\u{43b}\u{436}\u{438}\u{442}\u{44c}.",
        _ => "Setup session expired. Use /setup again if you want to continue.",
    }
}

/// Localized message when user already has an active setup session.
pub(super) fn setup_conflict_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Ya tienes una sesi\u{f3}n de setup activa. Termina o cancela la actual primero.",
        "Portuguese" => "Voc\u{ea} j\u{e1} tem uma sess\u{e3}o de setup ativa. Termine ou cancele a atual primeiro.",
        "French" => "Vous avez d\u{e9}j\u{e0} une session de setup active. Terminez ou annulez la session en cours d'abord.",
        "German" => "Du hast bereits eine aktive Setup-Sitzung. Beende oder brich die aktuelle zuerst ab.",
        "Italian" => "Hai gi\u{e0} una sessione di setup attiva. Termina o annulla quella attuale prima.",
        "Dutch" => "Je hebt al een actieve setup-sessie. Rond de huidige af of annuleer deze eerst.",
        "Russian" => "\u{423} \u{432}\u{430}\u{441} \u{443}\u{436}\u{435} \u{435}\u{441}\u{442}\u{44c} \u{430}\u{43a}\u{442}\u{438}\u{432}\u{43d}\u{430}\u{44f} \u{441}\u{435}\u{441}\u{441}\u{438}\u{44f} \u{43d}\u{430}\u{441}\u{442}\u{440}\u{43e}\u{439}\u{43a}\u{438}. \u{417}\u{430}\u{432}\u{435}\u{440}\u{448}\u{438}\u{442}\u{435} \u{438}\u{43b}\u{438} \u{43e}\u{442}\u{43c}\u{435}\u{43d}\u{438}\u{442}\u{435} \u{0442}\u{0435}\u{043a}\u{0443}\u{0449}\u{0443}\u{044e} \u{441}\u{43d}\u{430}\u{447}\u{430}\u{43b}\u{430}.",
        _ => "You already have an active setup session. Finish or cancel the current one first.",
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

    // --- WhatsApp help keyword tests ---

    #[test]
    fn test_kw_match_help_keywords() {
        assert!(kw_match("what commands do you have", HELP_KW));
        assert!(kw_match("what can you do", HELP_KW));
        assert!(kw_match("qué puedes hacer", HELP_KW));
        assert!(kw_match("was kannst du", HELP_KW));
        assert!(!kw_match("hello omega", HELP_KW));
        assert!(!kw_match("good morning", HELP_KW));
    }

    // --- Build confirm/cancel tests ---

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
        assert!(!is_build_confirmed("no"));
        assert!(!is_build_confirmed("nah"));
        assert!(!is_build_confirmed("cancel"));
        assert!(!is_build_confirmed("nein"));
        assert!(!is_build_confirmed("non"));
        assert!(!is_build_confirmed("нет"));
        assert!(!is_build_confirmed("yes please build it now"));
        assert!(!is_build_confirmed("build me a tool"));
        assert!(!is_build_confirmed(""));
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

    // ===================================================================
    // REQ-BRAIN-012 (Should): SETUP_TTL_SECS constant
    // ===================================================================

    #[test]
    fn test_setup_ttl_secs_value() {
        assert_eq!(
            SETUP_TTL_SECS, 1800,
            "SETUP_TTL_SECS must be 1800 (30 minutes)"
        );
    }

    // ===================================================================
    // REQ-BRAIN-014 (Should): Localized setup messages -- all 8 languages
    // ===================================================================

    #[test]
    fn test_setup_help_message_all_languages() {
        let en = setup_help_message("English");
        assert!(en.contains("/setup"), "English help must mention /setup");
        assert!(
            en.contains("Example") || en.contains("example"),
            "English help must have example"
        );

        let es = setup_help_message("Spanish");
        assert!(es.contains("/setup"), "Spanish help must mention /setup");
        assert!(es.contains("Ejemplo"), "Spanish help must have example");

        let pt = setup_help_message("Portuguese");
        assert!(pt.contains("/setup"), "Portuguese help must mention /setup");
        assert!(pt.contains("Exemplo"), "Portuguese help must have example");

        let fr = setup_help_message("French");
        assert!(fr.contains("/setup"), "French help must mention /setup");
        assert!(fr.contains("Exemple"), "French help must have example");

        let de = setup_help_message("German");
        assert!(de.contains("/setup"), "German help must mention /setup");
        assert!(de.contains("Beispiel"), "German help must have example");

        let it = setup_help_message("Italian");
        assert!(it.contains("/setup"), "Italian help must mention /setup");
        assert!(it.contains("Esempio"), "Italian help must have example");

        let nl = setup_help_message("Dutch");
        assert!(nl.contains("/setup"), "Dutch help must mention /setup");
        assert!(nl.contains("Voorbeeld"), "Dutch help must have example");

        let ru = setup_help_message("Russian");
        assert!(ru.contains("/setup"), "Russian help must mention /setup");
    }

    #[test]
    fn test_setup_help_message_default_english() {
        let unknown = setup_help_message("Klingon");
        let en = setup_help_message("English");
        assert_eq!(unknown, en, "Unknown language must default to English");
    }

    #[test]
    fn test_setup_intro_message_all_languages() {
        let questions = "1. What type of business?\n2. What location?";

        let en = setup_intro_message("English", questions);
        assert!(en.contains(questions), "Must include questions");
        assert!(en.contains("OMEGA"), "English intro must mention OMEGA");

        let es = setup_intro_message("Spanish", questions);
        assert!(es.contains(questions), "Must include questions");
        assert!(es.contains("OMEGA"), "Spanish intro must mention OMEGA");

        let pt = setup_intro_message("Portuguese", questions);
        assert!(pt.contains(questions));

        let fr = setup_intro_message("French", questions);
        assert!(fr.contains(questions));

        let de = setup_intro_message("German", questions);
        assert!(de.contains(questions));

        let it = setup_intro_message("Italian", questions);
        assert!(it.contains(questions));

        let nl = setup_intro_message("Dutch", questions);
        assert!(nl.contains(questions));

        let ru = setup_intro_message("Russian", questions);
        assert!(ru.contains(questions));
    }

    #[test]
    fn test_setup_followup_message_all_languages() {
        let questions = "1. More details about your clients?";
        let round: u8 = 2;

        let en = setup_followup_message("English", questions, round);
        assert!(en.contains("2/3"), "Must show round 2/3");
        assert!(en.contains(questions), "Must include questions");

        let es = setup_followup_message("Spanish", questions, round);
        assert!(es.contains("2/3"));

        let pt = setup_followup_message("Portuguese", questions, round);
        assert!(pt.contains("2/3"));

        let fr = setup_followup_message("French", questions, round);
        assert!(fr.contains("2/3"));

        let de = setup_followup_message("German", questions, round);
        assert!(de.contains("2/3"));

        let it = setup_followup_message("Italian", questions, round);
        assert!(it.contains("2/3"));

        let nl = setup_followup_message("Dutch", questions, round);
        assert!(nl.contains("2/3"));

        let ru = setup_followup_message("Russian", questions, round);
        assert!(ru.contains("2/3"));
    }

    #[test]
    fn test_setup_proposal_message_all_languages() {
        let preview = "Project: realtor\nDomain: Real estate";

        let en = setup_proposal_message("English", preview);
        assert!(en.contains(preview), "Must include proposal preview");
        assert!(en.contains("*yes*"), "English must prompt for yes");

        let es = setup_proposal_message("Spanish", preview);
        assert!(es.contains(preview));

        let pt = setup_proposal_message("Portuguese", preview);
        assert!(pt.contains(preview));

        let fr = setup_proposal_message("French", preview);
        assert!(fr.contains(preview));

        let de = setup_proposal_message("German", preview);
        assert!(de.contains(preview));

        let it = setup_proposal_message("Italian", preview);
        assert!(it.contains(preview));

        let nl = setup_proposal_message("Dutch", preview);
        assert!(nl.contains(preview));

        let ru = setup_proposal_message("Russian", preview);
        assert!(ru.contains(preview));
    }

    #[test]
    fn test_setup_complete_message_all_languages() {
        let project = "realtor";
        let langs = [
            "English",
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ];
        for lang in langs {
            let msg = setup_complete_message(lang, project);
            assert!(
                msg.contains("OMEGA \u{03a9} Realtor"),
                "{lang} setup message should contain persona: {msg}"
            );
        }
    }

    #[test]
    fn test_setup_cancelled_message_all_languages() {
        assert!(
            setup_cancelled_message("English").contains("cancelled")
                || setup_cancelled_message("English").contains("Setup")
        );
        assert!(!setup_cancelled_message("Spanish").is_empty());
        assert!(!setup_cancelled_message("Portuguese").is_empty());
        assert!(!setup_cancelled_message("French").is_empty());
        assert!(!setup_cancelled_message("German").is_empty());
        assert!(!setup_cancelled_message("Italian").is_empty());
        assert!(!setup_cancelled_message("Dutch").is_empty());
        assert!(!setup_cancelled_message("Russian").is_empty());
    }

    #[test]
    fn test_setup_expired_message_all_languages() {
        let en = setup_expired_message("English");
        assert!(
            en.contains("expired") || en.contains("/setup"),
            "English must mention expiry"
        );

        assert!(!setup_expired_message("Spanish").is_empty());
        assert!(!setup_expired_message("Portuguese").is_empty());
        assert!(!setup_expired_message("French").is_empty());
        assert!(!setup_expired_message("German").is_empty());
        assert!(!setup_expired_message("Italian").is_empty());
        assert!(!setup_expired_message("Dutch").is_empty());
        assert!(!setup_expired_message("Russian").is_empty());

        for lang in [
            "English",
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ] {
            assert!(
                setup_expired_message(lang).contains("/setup"),
                "{lang} expired message must mention /setup"
            );
        }
    }

    #[test]
    fn test_setup_confirmation_reuses_build_keywords() {
        assert!(
            is_build_confirmed("yes"),
            "Setup confirmation must accept 'yes'"
        );
        assert!(
            is_build_confirmed("si"),
            "Setup confirmation must accept Spanish 'si'"
        );
        assert!(
            is_build_cancelled("no"),
            "Setup cancellation must accept 'no'"
        );
        assert!(
            is_build_cancelled("cancelar"),
            "Setup cancellation must accept Spanish 'cancelar'"
        );
    }

    #[test]
    fn test_setup_modification_is_neither_confirm_nor_cancel() {
        let modification = "I want to add restaurant management too";
        assert!(
            !is_build_confirmed(modification),
            "Modification request must not be treated as confirmation"
        );
        assert!(
            !is_build_cancelled(modification),
            "Modification request must not be treated as cancellation"
        );
    }

    #[test]
    fn test_pending_setup_is_system_fact() {
        assert!(
            !is_valid_fact("pending_setup", "some value"),
            "pending_setup must be rejected by is_valid_fact as a system key"
        );
    }
}
