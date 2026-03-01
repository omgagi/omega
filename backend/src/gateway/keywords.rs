//! Keyword matching functions for conditional prompt injection.
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
// Discovery session localized messages
// ---------------------------------------------------------------------------

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
        // es
        assert!(kw_match("activar el proyecto trader", PROJECTS_KW));
        assert!(kw_match("desactivar proyecto", PROJECTS_KW));
        // pt
        assert!(kw_match("ativar o projeto trader", PROJECTS_KW));
        assert!(kw_match("desativar projeto", PROJECTS_KW));
        // fr
        assert!(kw_match("activer le projet trader", PROJECTS_KW));
        assert!(kw_match("désactiver projet", PROJECTS_KW));
        // de
        assert!(kw_match("aktivieren das projekt trader", PROJECTS_KW));
        assert!(kw_match("deaktivieren projekt", PROJECTS_KW));
        // it
        assert!(kw_match("attivare il progetto trader", PROJECTS_KW));
        assert!(kw_match("disattivare progetto", PROJECTS_KW));
        // nl
        assert!(kw_match("activeren het project trader", PROJECTS_KW));
        assert!(kw_match("deactiveren project", PROJECTS_KW));
        // ru
        assert!(kw_match("активировать проект trader", PROJECTS_KW));
        assert!(kw_match("деактивировать проект", PROJECTS_KW));
        // negative
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

    // ===================================================================
    // REQ-BRAIN-012 (Should): SETUP_TTL_SECS constant
    // ===================================================================

    // Requirement: REQ-BRAIN-012 (Should)
    // Acceptance: Setup session TTL is 30 minutes (1800 seconds)
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

    // The 8 languages: English, Spanish, Portuguese, French, German, Italian, Dutch, Russian

    // Requirement: REQ-BRAIN-014 (Should)
    // Acceptance: setup_help_message has content for all 8 languages
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

    // Requirement: REQ-BRAIN-014 (Should)
    // Acceptance: setup_help_message for unknown language defaults to English
    #[test]
    fn test_setup_help_message_default_english() {
        let unknown = setup_help_message("Klingon");
        let en = setup_help_message("English");
        assert_eq!(unknown, en, "Unknown language must default to English");
    }

    // Requirement: REQ-BRAIN-014 (Should)
    // Acceptance: setup_intro_message has content for all 8 languages
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

    // Requirement: REQ-BRAIN-014 (Should)
    // Acceptance: setup_followup_message includes round number and questions
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

    // Requirement: REQ-BRAIN-014 (Should)
    // Acceptance: setup_proposal_message includes preview and confirmation prompt
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

    // Requirement: REQ-BRAIN-014 (Should)
    // Acceptance: setup_complete_message includes project name in all 8 languages
    #[test]
    fn test_setup_complete_message_all_languages() {
        let project = "realtor";

        let en = setup_complete_message("English", project);
        assert!(en.contains("*realtor*"), "English must bold project name");
        assert!(en.contains("expert") || en.contains("configured"));

        let es = setup_complete_message("Spanish", project);
        assert!(es.contains("*realtor*"));

        let pt = setup_complete_message("Portuguese", project);
        assert!(pt.contains("*realtor*"));

        let fr = setup_complete_message("French", project);
        assert!(fr.contains("*realtor*"));

        let de = setup_complete_message("German", project);
        assert!(de.contains("*realtor*"));

        let it = setup_complete_message("Italian", project);
        assert!(it.contains("*realtor*"));

        let nl = setup_complete_message("Dutch", project);
        assert!(nl.contains("*realtor*"));

        let ru = setup_complete_message("Russian", project);
        assert!(ru.contains("*realtor*"));
    }

    // Requirement: REQ-BRAIN-014 (Should)
    // Acceptance: setup_cancelled_message in all 8 languages
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

    // Requirement: REQ-BRAIN-014 (Should)
    // Acceptance: setup_expired_message in all 8 languages
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

        // All expired messages should mention /setup for re-invocation.
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

    // Requirement: REQ-BRAIN-013 (Should)
    // Acceptance: Setup confirmation/cancellation reuses BUILD_CONFIRM_KW / BUILD_CANCEL_KW
    // These are already tested above. This test validates setup-specific usage.
    #[test]
    fn test_setup_confirmation_reuses_build_keywords() {
        // The architecture specifies reusing is_build_confirmed / is_build_cancelled.
        // Verify they work for the setup context.
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

    // Requirement: REQ-BRAIN-013 (Should)
    // Acceptance: Non-confirmation reply during approval is not confirm or cancel
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

    // Requirement: REQ-BRAIN-012 (Should)
    // Acceptance: pending_setup is a system fact key (rejected by is_valid_fact)
    #[test]
    fn test_pending_setup_is_system_fact() {
        assert!(
            !is_valid_fact("pending_setup", "some value"),
            "pending_setup must be rejected by is_valid_fact as a system key"
        );
    }
}
