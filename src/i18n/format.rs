//! Format helpers for strings with interpolation.

use super::t;

/// Format the language set confirmation.
pub fn language_set(lang: &str, new_lang: &str) -> String {
    match lang {
        "Spanish" => format!("Idioma configurado a: {new_lang}"),
        "Portuguese" => format!("Idioma definido para: {new_lang}"),
        "French" => format!("Langue d\u{00e9}finie sur: {new_lang}"),
        "German" => format!("Sprache eingestellt auf: {new_lang}"),
        "Italian" => format!("Lingua impostata su: {new_lang}"),
        "Dutch" => format!("Taal ingesteld op: {new_lang}"),
        "Russian" => format!("\u{042f}\u{0437}\u{044b}\u{043a} \u{0443}\u{0441}\u{0442}\u{0430}\u{043d}\u{043e}\u{0432}\u{043b}\u{0435}\u{043d}: {new_lang}"),
        _ => format!("Language set to: {new_lang}"),
    }
}

/// Format language show with usage hint.
pub fn language_show(lang: &str, current: &str) -> String {
    let label = t("language_label", lang);
    format!("{label} {current}\nUsage: /language <language>")
}

/// Format personality updated confirmation.
pub fn personality_updated(lang: &str, pref: &str) -> String {
    match lang {
        "Spanish" => format!("Personalidad actualizada: _{pref}_"),
        "Portuguese" => format!("Personalidade atualizada: _{pref}_"),
        "French" => format!("Personnalit\u{00e9} mise \u{00e0} jour: _{pref}_"),
        "German" => format!("Pers\u{00f6}nlichkeit aktualisiert: _{pref}_"),
        "Italian" => format!("Personalit\u{00e0} aggiornata: _{pref}_"),
        "Dutch" => format!("Persoonlijkheid bijgewerkt: _{pref}_"),
        "Russian" => format!("\u{041b}\u{0438}\u{0447}\u{043d}\u{043e}\u{0441}\u{0442}\u{044c} \u{043e}\u{0431}\u{043d}\u{043e}\u{0432}\u{043b}\u{0435}\u{043d}\u{0430}: _{pref}_"),
        _ => format!("Personality updated: _{pref}_"),
    }
}

/// Format the personality show (current preference).
pub fn personality_show(lang: &str, pref: &str) -> String {
    let header = match lang {
        "Spanish" => "Tu preferencia de personalidad:",
        "Portuguese" => "Sua prefer\u{00ea}ncia de personalidade:",
        "French" => "Votre pr\u{00e9}f\u{00e9}rence de personnalit\u{00e9}:",
        "German" => "Deine Pers\u{00f6}nlichkeitspr\u{00e4}ferenz:",
        "Italian" => "La tua preferenza di personalit\u{00e0}:",
        "Dutch" => "Je persoonlijkheidsvoorkeur:",
        "Russian" => "\u{0422}\u{0432}\u{043e}\u{0438} \u{043d}\u{0430}\u{0441}\u{0442}\u{0440}\u{043e}\u{0439}\u{043a}\u{0438} \u{043b}\u{0438}\u{0447}\u{043d}\u{043e}\u{0441}\u{0442}\u{0438}:",
        _ => "Your personality preference:",
    };
    let hint = t("personality_reset_hint", lang);
    format!("{header}\n_{pref}_\n\n{hint}")
}

/// Format the purge result.
pub fn purge_result(lang: &str, purged: usize, keys_display: &str) -> String {
    match lang {
        "Spanish" => {
            format!("{purged} datos eliminados. Claves del sistema preservadas ({keys_display}).")
        }
        "Portuguese" => {
            format!("{purged} fatos exclu\u{00ed}dos. Chaves do sistema preservadas ({keys_display}).")
        }
        "French" => format!("{purged} faits supprim\u{00e9}s. Cl\u{00e9}s syst\u{00e8}me pr\u{00e9}serv\u{00e9}es ({keys_display})."),
        "German" => {
            format!("{purged} Fakten gel\u{00f6}scht. Systemschl\u{00fc}ssel beibehalten ({keys_display}).")
        }
        "Italian" => {
            format!("{purged} fatti eliminati. Chiavi di sistema preservate ({keys_display}).")
        }
        "Dutch" => {
            format!("{purged} feiten verwijderd. Systeemsleutels behouden ({keys_display}).")
        }
        "Russian" => {
            format!("{purged} \u{0444}\u{0430}\u{043a}\u{0442}\u{043e}\u{0432} \u{0443}\u{0434}\u{0430}\u{043b}\u{0435}\u{043d}\u{043e}. \u{0421}\u{0438}\u{0441}\u{0442}\u{0435}\u{043c}\u{043d}\u{044b}\u{0435} \u{043a}\u{043b}\u{044e}\u{0447}\u{0438} \u{0441}\u{043e}\u{0445}\u{0440}\u{0430}\u{043d}\u{0435}\u{043d}\u{044b} ({keys_display}).")
        }
        _ => format!("Purged {purged} facts. System keys preserved ({keys_display})."),
    }
}

/// Format the project activated confirmation.
pub fn project_activated(lang: &str, name: &str) -> String {
    match lang {
        "Spanish" => format!("Proyecto '{name}' activado. Conversaci\u{00f3}n borrada."),
        "Portuguese" => format!("Projeto '{name}' ativado. Conversa apagada."),
        "French" => format!("Projet '{name}' activ\u{00e9}. Conversation effac\u{00e9}e."),
        "German" => format!("Projekt '{name}' aktiviert. Gespr\u{00e4}ch gel\u{00f6}scht."),
        "Italian" => format!("Progetto '{name}' attivato. Conversazione cancellata."),
        "Dutch" => format!("Project '{name}' geactiveerd. Gesprek gewist."),
        "Russian" => format!("\u{041f}\u{0440}\u{043e}\u{0435}\u{043a}\u{0442} '{name}' \u{0430}\u{043a}\u{0442}\u{0438}\u{0432}\u{0438}\u{0440}\u{043e}\u{0432}\u{0430}\u{043d}. \u{0420}\u{0430}\u{0437}\u{0433}\u{043e}\u{0432}\u{043e}\u{0440} \u{0443}\u{0434}\u{0430}\u{043b}\u{0451}\u{043d}."),
        _ => format!("Project '{name}' activated. Conversation cleared."),
    }
}

/// Format the project not found message.
pub fn project_not_found(lang: &str, name: &str) -> String {
    match lang {
        "Spanish" => {
            format!("Proyecto '{name}' no encontrado. Usa /projects para ver los disponibles.")
        }
        "Portuguese" => {
            format!("Projeto '{name}' n\u{00e3}o encontrado. Use /projects para ver os dispon\u{00ed}veis.")
        }
        "French" => {
            format!("Projet '{name}' introuvable. Utilisez /projects pour voir les disponibles.")
        }
        "German" => {
            format!("Projekt '{name}' nicht gefunden. Verwende /projects f\u{00fc}r verf\u{00fc}gbare Projekte.")
        }
        "Italian" => {
            format!("Progetto '{name}' non trovato. Usa /projects per vedere quelli disponibili.")
        }
        "Dutch" => {
            format!("Project '{name}' niet gevonden. Gebruik /projects om beschikbare te zien.")
        }
        "Russian" => {
            format!("\u{041f}\u{0440}\u{043e}\u{0435}\u{043a}\u{0442} '{name}' \u{043d}\u{0435} \u{043d}\u{0430}\u{0439}\u{0434}\u{0435}\u{043d}. \u{0418}\u{0441}\u{043f}\u{043e}\u{043b}\u{044c}\u{0437}\u{0443}\u{0439}\u{0442}\u{0435} /projects \u{0434}\u{043b}\u{044f} \u{043f}\u{0440}\u{043e}\u{0441}\u{043c}\u{043e}\u{0442}\u{0440}\u{0430} \u{0434}\u{043e}\u{0441}\u{0442}\u{0443}\u{043f}\u{043d}\u{044b}\u{0445}.")
        }
        _ => format!("Project '{name}' not found. Use /projects to see available projects."),
    }
}

/// Format the active project display.
pub fn active_project(lang: &str, name: &str) -> String {
    let hint = t("project_deactivate_hint", lang);
    match lang {
        "Spanish" => format!("Proyecto activo: {name}\n{hint}"),
        "Portuguese" => format!("Projeto ativo: {name}\n{hint}"),
        "French" => format!("Projet actif: {name}\n{hint}"),
        "German" => format!("Aktives Projekt: {name}\n{hint}"),
        "Italian" => format!("Progetto attivo: {name}\n{hint}"),
        "Dutch" => format!("Actief project: {name}\n{hint}"),
        "Russian" => format!("\u{0410}\u{043a}\u{0442}\u{0438}\u{0432}\u{043d}\u{044b}\u{0439} \u{043f}\u{0440}\u{043e}\u{0435}\u{043a}\u{0442}: {name}\n{hint}"),
        _ => format!("Active project: {name}\n{hint}"),
    }
}

/// Format the "Scheduled N tasks:" header.
pub fn tasks_confirmed(lang: &str, n: usize) -> String {
    match lang {
        "Spanish" => format!("\u{2713} {n} tareas programadas:"),
        "Portuguese" => format!("\u{2713} {n} tarefas agendadas:"),
        "French" => format!("\u{2713} {n} t\u{00e2}ches planifi\u{00e9}es:"),
        "German" => format!("\u{2713} {n} Aufgaben geplant:"),
        "Italian" => format!("\u{2713} {n} attivit\u{00e0} pianificate:"),
        "Dutch" => format!("\u{2713} {n} taken gepland:"),
        "Russian" => format!("\u{2713} {n} \u{0437}\u{0430}\u{0434}\u{0430}\u{0447} \u{0437}\u{0430}\u{043f}\u{043b}\u{0430}\u{043d}\u{0438}\u{0440}\u{043e}\u{0432}\u{0430}\u{043d}\u{043e}:"),
        _ => format!("\u{2713} Scheduled {n} tasks:"),
    }
}

/// Format the "Cancelled N tasks:" header.
pub fn tasks_cancelled_confirmed(lang: &str, n: usize) -> String {
    match lang {
        "Spanish" => format!("\u{2713} {n} tareas canceladas:"),
        "Portuguese" => format!("\u{2713} {n} tarefas canceladas:"),
        "French" => format!("\u{2713} {n} t\u{00e2}ches annul\u{00e9}es:"),
        "German" => format!("\u{2713} {n} Aufgaben storniert:"),
        "Italian" => format!("\u{2713} {n} attivit\u{00e0} annullate:"),
        "Dutch" => format!("\u{2713} {n} taken geannuleerd:"),
        "Russian" => format!("\u{2713} {n} \u{0437}\u{0430}\u{0434}\u{0430}\u{0447} \u{043e}\u{0442}\u{043c}\u{0435}\u{043d}\u{0435}\u{043d}\u{043e}:"),
        _ => format!("\u{2713} Cancelled {n} tasks:"),
    }
}

/// Format the "Updated N tasks:" header.
pub fn tasks_updated_confirmed(lang: &str, n: usize) -> String {
    match lang {
        "Spanish" => format!("\u{2713} {n} tareas actualizadas:"),
        "Portuguese" => format!("\u{2713} {n} tarefas atualizadas:"),
        "French" => format!("\u{2713} {n} t\u{00e2}ches mises \u{00e0} jour:"),
        "German" => format!("\u{2713} {n} Aufgaben aktualisiert:"),
        "Italian" => format!("\u{2713} {n} attivit\u{00e0} aggiornate:"),
        "Dutch" => format!("\u{2713} {n} taken bijgewerkt:"),
        "Russian" => format!("\u{2713} {n} \u{0437}\u{0430}\u{0434}\u{0430}\u{0447} \u{043e}\u{0431}\u{043d}\u{043e}\u{0432}\u{043b}\u{0435}\u{043d}\u{043e}:"),
        _ => format!("\u{2713} Updated {n} tasks:"),
    }
}

/// Format the task save failure message.
pub fn task_save_failed(lang: &str, n: usize) -> String {
    match lang {
        "Spanish" => format!("\u{2717} Error al guardar {n} tarea(s). Int\u{00e9}ntalo de nuevo."),
        "Portuguese" => format!("\u{2717} Falha ao salvar {n} tarefa(s). Tente novamente."),
        "French" => format!("\u{2717} \u{00c9}chec de l'enregistrement de {n} t\u{00e2}che(s). R\u{00e9}essayez."),
        "German" => {
            format!("\u{2717} {n} Aufgabe(n) konnten nicht gespeichert werden. Bitte erneut versuchen.")
        }
        "Italian" => format!("\u{2717} Impossibile salvare {n} attivit\u{00e0}. Riprova."),
        "Dutch" => format!("\u{2717} {n} ta(a)k(en) opslaan mislukt. Probeer opnieuw."),
        "Russian" => format!("\u{2717} \u{041d}\u{0435} \u{0443}\u{0434}\u{0430}\u{043b}\u{043e}\u{0441}\u{044c} \u{0441}\u{043e}\u{0445}\u{0440}\u{0430}\u{043d}\u{0438}\u{0442}\u{044c} {n} \u{0437}\u{0430}\u{0434}\u{0430}\u{0447}(\u{0443}). \u{041f}\u{043e}\u{043f}\u{0440}\u{043e}\u{0431}\u{0443}\u{0439}\u{0442}\u{0435} \u{0441}\u{043d}\u{043e}\u{0432}\u{0430}."),
        _ => format!("\u{2717} Failed to save {n} task(s). Please try again."),
    }
}

/// Format the heartbeat interval updated notification.
pub fn heartbeat_interval_updated(lang: &str, mins: u64) -> String {
    match lang {
        "Spanish" => format!("\u{23f1}\u{fe0f} Intervalo de pulso actualizado a {mins} minutos."),
        "Portuguese" => format!("\u{23f1}\u{fe0f} Intervalo de pulso atualizado para {mins} minutos."),
        "French" => format!("\u{23f1}\u{fe0f} Intervalle de pouls mis \u{00e0} jour \u{00e0} {mins} minutes."),
        "German" => format!("\u{23f1}\u{fe0f} Pulsintervall auf {mins} Minuten aktualisiert."),
        "Italian" => format!("\u{23f1}\u{fe0f} Intervallo del battito aggiornato a {mins} minuti."),
        "Dutch" => format!("\u{23f1}\u{fe0f} Hartslag-interval bijgewerkt naar {mins} minuten."),
        "Russian" => format!("\u{23f1}\u{fe0f} \u{0418}\u{043d}\u{0442}\u{0435}\u{0440}\u{0432}\u{0430}\u{043b} \u{043f}\u{0443}\u{043b}\u{044c}\u{0441}\u{0430} \u{043e}\u{0431}\u{043d}\u{043e}\u{0432}\u{043b}\u{0451}\u{043d} \u{0434}\u{043e} {mins} \u{043c}\u{0438}\u{043d}\u{0443}\u{0442}."),
        _ => format!("\u{23f1}\u{fe0f} Heartbeat interval updated to {mins} minutes."),
    }
}
