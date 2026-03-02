//! Localized messages for the `/google` credential setup flow.
//!
//! Extracted into its own file to respect the 500-line-per-file rule.
//! All functions are `pub(super)` -- consumed by `google_auth.rs`.

/// Step 1: Initial prompt asking for client_id.
/// Includes overwrite warning if `existing` is true.
pub(super) fn google_step1_message(lang: &str, existing: bool) -> String {
    let warning = if existing {
        match lang {
            "Spanish" => "\n\n**Nota:** Ya tienes credenciales de Google configuradas. Este proceso las sobrescribira.",
            "Portuguese" => "\n\n**Nota:** Voce ja tem credenciais do Google configuradas. Este processo as substituira.",
            "French" => "\n\n**Note :** Vous avez deja des identifiants Google configures. Ce processus les ecrasera.",
            "German" => "\n\n**Hinweis:** Du hast bereits Google-Zugangsdaten konfiguriert. Dieser Vorgang wird sie uberschreiben.",
            "Italian" => "\n\n**Nota:** Hai gia credenziali Google configurate. Questo processo le sovrascrivera.",
            "Dutch" => "\n\n**Let op:** Je hebt al Google-inloggegevens geconfigureerd. Dit proces zal ze overschrijven.",
            "Russian" => "\n\n**Примечание:** У вас уже настроены учетные данные Google. Этот процесс их перезапишет.",
            _ => "\n\n**Note:** You already have Google credentials configured. This process will overwrite them.",
        }
    } else {
        ""
    };

    let base = match lang {
        "Spanish" => "Configuracion de cuenta Google\n\nPor favor, envia tu **Client ID** de Google OAuth.\n\nPuedes escribir *cancel* en cualquier momento para cancelar.",
        "Portuguese" => "Configuracao de conta Google\n\nPor favor, envie seu **Client ID** do Google OAuth.\n\nVoce pode escrever *cancel* a qualquer momento para cancelar.",
        "French" => "Configuration du compte Google\n\nVeuillez envoyer votre **Client ID** Google OAuth.\n\nVous pouvez ecrire *cancel* a tout moment pour annuler.",
        "German" => "Google-Konto einrichten\n\nBitte sende deine **Client ID** von Google OAuth.\n\nDu kannst jederzeit *cancel* schreiben, um abzubrechen.",
        "Italian" => "Configurazione account Google\n\nPer favore, invia il tuo **Client ID** di Google OAuth.\n\nPuoi scrivere *cancel* in qualsiasi momento per annullare.",
        "Dutch" => "Google-account instellen\n\nStuur alsjeblieft je **Client ID** van Google OAuth.\n\nJe kunt op elk moment *cancel* typen om te annuleren.",
        "Russian" => "Настройка аккаунта Google\n\nПожалуйста, отправьте ваш **Client ID** Google OAuth.\n\nВы можете написать *cancel* в любой момент для отмены.",
        _ => "Google Account Setup\n\nPlease send your Google OAuth **Client ID**.\n\nYou can type *cancel* at any time to abort.",
    };

    format!("{base}{warning}")
}

/// Step 2: Received client_id, asking for client_secret.
pub(super) fn google_step2_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Client ID recibido. Ahora envia tu **Client Secret**.",
        "Portuguese" => "Client ID recebido. Agora envie seu **Client Secret**.",
        "French" => "Client ID recu. Maintenant envoyez votre **Client Secret**.",
        "German" => "Client ID erhalten. Jetzt sende dein **Client Secret**.",
        "Italian" => "Client ID ricevuto. Ora invia il tuo **Client Secret**.",
        "Dutch" => "Client ID ontvangen. Stuur nu je **Client Secret**.",
        "Russian" => "Client ID получен. Теперь отправьте ваш **Client Secret**.",
        _ => "Client ID received. Now send your **Client Secret**.",
    }
}

/// Step 3: Received client_secret, asking for refresh_token.
pub(super) fn google_step3_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Client Secret recibido. Ahora envia tu **Refresh Token**.",
        "Portuguese" => "Client Secret recebido. Agora envie seu **Refresh Token**.",
        "French" => "Client Secret recu. Maintenant envoyez votre **Refresh Token**.",
        "German" => "Client Secret erhalten. Jetzt sende dein **Refresh Token**.",
        "Italian" => "Client Secret ricevuto. Ora invia il tuo **Refresh Token**.",
        "Dutch" => "Client Secret ontvangen. Stuur nu je **Refresh Token**.",
        "Russian" => "Client Secret получен. Теперь отправьте ваш **Refresh Token**.",
        _ => "Client Secret received. Now send your **Refresh Token**.",
    }
}

/// Step 4: Received refresh_token, asking for email.
pub(super) fn google_step4_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => {
            "Refresh Token recibido. Por ultimo, envia tu **direccion de email** de Gmail."
        }
        "Portuguese" => {
            "Refresh Token recebido. Por ultimo, envie seu **endereco de email** do Gmail."
        }
        "French" => "Refresh Token recu. Enfin, envoyez votre **adresse email** Gmail.",
        "German" => "Refresh Token erhalten. Zuletzt sende deine **E-Mail-Adresse** von Gmail.",
        "Italian" => "Refresh Token ricevuto. Infine, invia il tuo **indirizzo email** di Gmail.",
        "Dutch" => "Refresh Token ontvangen. Tot slot, stuur je **e-mailadres** van Gmail.",
        "Russian" => {
            "Refresh Token получен. Наконец, отправьте ваш **адрес электронной почты** Gmail."
        }
        _ => "Refresh Token received. Finally, send your **Gmail email address**.",
    }
}

/// Completion: All credentials stored successfully.
pub(super) fn google_complete_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Credenciales de Google guardadas correctamente.",
        "Portuguese" => "Credenciais do Google salvas com sucesso.",
        "French" => "Identifiants Google enregistres avec succes.",
        "German" => "Google-Zugangsdaten erfolgreich gespeichert.",
        "Italian" => "Credenziali Google salvate con successo.",
        "Dutch" => "Google-inloggegevens succesvol opgeslagen.",
        "Russian" => "Учетные данные Google успешно сохранены.",
        _ => "Google credentials saved successfully.",
    }
}

/// Cancellation confirmation.
pub(super) fn google_cancelled_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Configuracion de Google cancelada.",
        "Portuguese" => "Configuracao do Google cancelada.",
        "French" => "Configuration Google annulee.",
        "German" => "Google-Einrichtung abgebrochen.",
        "Italian" => "Configurazione Google annullata.",
        "Dutch" => "Google-instelling geannuleerd.",
        "Russian" => "Настройка Google отменена.",
        _ => "Google setup cancelled.",
    }
}

/// Session expired (10 min TTL).
pub(super) fn google_expired_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => {
            "La sesion de configuracion de Google ha expirado. Usa /google para reiniciar."
        }
        "Portuguese" => "A sessao de configuracao do Google expirou. Use /google para reiniciar.",
        "French" => {
            "La session de configuration Google a expire. Utilisez /google pour recommencer."
        }
        "German" => {
            "Die Google-Einrichtungssitzung ist abgelaufen. Verwende /google, um neu zu starten."
        }
        "Italian" => {
            "La sessione di configurazione Google e scaduta. Usa /google per ricominciare."
        }
        "Dutch" => {
            "De Google-installatiesessie is verlopen. Gebruik /google om opnieuw te beginnen."
        }
        "Russian" => "Сессия настройки Google истекла. Используйте /google, чтобы начать заново.",
        _ => "Google setup session expired. Use /google to start again.",
    }
}

/// Concurrent session guard -- active session already exists.
pub(super) fn google_conflict_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Ya tienes una sesion de configuracion de Google activa. Completala o escribe *cancel* para cancelarla.",
        "Portuguese" => "Voce ja tem uma sessao de configuracao do Google ativa. Complete-a ou escreva *cancel* para cancela-la.",
        "French" => "Vous avez deja une session de configuration Google active. Terminez-la ou ecrivez *cancel* pour l'annuler.",
        "German" => "Du hast bereits eine aktive Google-Einrichtungssitzung. Schliesse sie ab oder schreibe *cancel*, um sie abzubrechen.",
        "Italian" => "Hai gia una sessione di configurazione Google attiva. Completala o scrivi *cancel* per annullarla.",
        "Dutch" => "Je hebt al een actieve Google-installatiesessie. Voltooi deze of typ *cancel* om te annuleren.",
        "Russian" => "У вас уже есть активная сессия настройки Google. Завершите её или напишите *cancel* для отмены.",
        _ => "You already have an active Google setup session. Complete it or type *cancel* to abort.",
    }
}

/// Validation error: empty input.
pub(super) fn google_empty_input_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "La entrada no puede estar vacia. Por favor, envia un valor.",
        "Portuguese" => "A entrada nao pode estar vazia. Por favor, envie um valor.",
        "French" => "L'entree ne peut pas etre vide. Veuillez envoyer une valeur.",
        "German" => "Die Eingabe darf nicht leer sein. Bitte sende einen Wert.",
        "Italian" => "L'input non puo essere vuoto. Per favore, invia un valore.",
        "Dutch" => "De invoer mag niet leeg zijn. Stuur alsjeblieft een waarde.",
        "Russian" => "Ввод не может быть пустым. Пожалуйста, отправьте значение.",
        _ => "Input cannot be empty. Please send a value.",
    }
}

/// Validation error: invalid email format.
pub(super) fn google_invalid_email_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Formato de email invalido. Por favor, envia una direccion de email valida.",
        "Portuguese" => "Formato de email invalido. Por favor, envie um endereco de email valido.",
        "French" => "Format d'email invalide. Veuillez envoyer une adresse email valide.",
        "German" => "Ungultiges E-Mail-Format. Bitte sende eine gultige E-Mail-Adresse.",
        "Italian" => "Formato email non valido. Per favore, invia un indirizzo email valido.",
        "Dutch" => "Ongeldig e-mailformaat. Stuur alsjeblieft een geldig e-mailadres.",
        "Russian" => {
            "Неверный формат электронной почты. Пожалуйста, отправьте действительный адрес."
        }
        _ => "Invalid email format. Please send a valid email address.",
    }
}

/// Error: failed to start session (store_fact failure).
pub(super) fn google_start_error_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "No se pudo iniciar la configuracion de Google. Intentalo de nuevo.",
        "Portuguese" => "Nao foi possivel iniciar a configuracao do Google. Tente novamente.",
        "French" => "Impossible de demarrer la configuration Google. Reessayez.",
        "German" => "Google-Einrichtung konnte nicht gestartet werden. Versuche es erneut.",
        "Italian" => "Impossibile avviare la configurazione Google. Riprova.",
        "Dutch" => "Kan Google-instelling niet starten. Probeer het opnieuw.",
        "Russian" => "Не удалось начать настройку Google. Попробуйте снова.",
        _ => "Failed to start Google setup. Please try again.",
    }
}

/// Error: failed to store credential in current step.
pub(super) fn google_store_error_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "No se pudo guardar el valor. Intentalo de nuevo.",
        "Portuguese" => "Nao foi possivel salvar o valor. Tente novamente.",
        "French" => "Impossible de sauvegarder la valeur. Reessayez.",
        "German" => "Der Wert konnte nicht gespeichert werden. Versuche es erneut.",
        "Italian" => "Impossibile salvare il valore. Riprova.",
        "Dutch" => "Kan de waarde niet opslaan. Probeer het opnieuw.",
        "Russian" => "Не удалось сохранить значение. Попробуйте снова.",
        _ => "Failed to save the value. Please try again.",
    }
}

/// Error: missing credential data at completion.
pub(super) fn google_missing_data_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "Faltan datos de credenciales. Usa /google para reiniciar.",
        "Portuguese" => "Dados de credenciais ausentes. Use /google para reiniciar.",
        "French" => "Donnees d'identification manquantes. Utilisez /google pour recommencer.",
        "German" => "Fehlende Zugangsdaten. Verwende /google, um neu zu starten.",
        "Italian" => "Dati delle credenziali mancanti. Usa /google per ricominciare.",
        "Dutch" => "Ontbrekende inloggegevens. Gebruik /google om opnieuw te beginnen.",
        "Russian" => "Отсутствуют данные учетных данных. Используйте /google, чтобы начать заново.",
        _ => "Missing credential data. Use /google to start again.",
    }
}

/// Error: failed to write google.json.
pub(super) fn google_write_error_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "No se pudieron guardar las credenciales de Google. Usa /google para reiniciar.",
        "Portuguese" => "Nao foi possivel salvar as credenciais do Google. Use /google para reiniciar.",
        "French" => "Impossible de sauvegarder les identifiants Google. Utilisez /google pour recommencer.",
        "German" => "Google-Zugangsdaten konnten nicht gespeichert werden. Verwende /google, um neu zu starten.",
        "Italian" => "Impossibile salvare le credenziali Google. Usa /google per ricominciare.",
        "Dutch" => "Kan Google-inloggegevens niet opslaan. Gebruik /google om opnieuw te beginnen.",
        "Russian" => "Не удалось сохранить учетные данные Google. Используйте /google, чтобы начать заново.",
        _ => "Failed to save Google credentials. Use /google to start again.",
    }
}

/// Error: unknown step in state machine.
pub(super) fn google_unknown_step_message(lang: &str) -> &'static str {
    match lang {
        "Spanish" => "La configuracion de Google encontro un estado inesperado. Usa /google para reiniciar.",
        "Portuguese" => "A configuracao do Google encontrou um estado inesperado. Use /google para reiniciar.",
        "French" => "La configuration Google a rencontre un etat inattendu. Utilisez /google pour recommencer.",
        "German" => "Die Google-Einrichtung hat einen unerwarteten Zustand erreicht. Verwende /google, um neu zu starten.",
        "Italian" => "La configurazione Google ha riscontrato uno stato imprevisto. Usa /google per ricominciare.",
        "Dutch" => "De Google-instelling heeft een onverwachte status bereikt. Gebruik /google om opnieuw te beginnen.",
        "Russian" => "Настройка Google столкнулась с неожиданным состоянием. Используйте /google, чтобы начать заново.",
        _ => "Google setup encountered an unexpected state. Use /google to start again.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_LANGUAGES: &[&str] = &[
        "English",
        "Spanish",
        "Portuguese",
        "French",
        "German",
        "Italian",
        "Dutch",
        "Russian",
    ];

    // ===================================================================
    // REQ-GAUTH-013 (Should): Localized messages for all 8 languages
    // ===================================================================

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: google_step1_message returns non-empty for all 8 languages (no overwrite)
    #[test]
    fn test_google_step1_message_all_languages_no_overwrite() {
        for lang in ALL_LANGUAGES {
            let msg = google_step1_message(lang, false);
            assert!(
                !msg.is_empty(),
                "google_step1_message({lang}, false) must not be empty"
            );
        }
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: google_step1_message returns non-empty for all 8 languages (with overwrite)
    #[test]
    fn test_google_step1_message_all_languages_with_overwrite() {
        for lang in ALL_LANGUAGES {
            let msg = google_step1_message(lang, true);
            assert!(
                !msg.is_empty(),
                "google_step1_message({lang}, true) must not be empty"
            );
        }
    }

    // Requirement: REQ-GAUTH-014 (Should)
    // Acceptance: Overwrite warning is included when existing=true
    #[test]
    fn test_google_step1_message_overwrite_warning_present() {
        let msg_with = google_step1_message("English", true);
        let msg_without = google_step1_message("English", false);
        assert!(
            msg_with.len() > msg_without.len(),
            "Message with overwrite warning must be longer than without"
        );
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: google_step2_message returns non-empty for all 8 languages
    #[test]
    fn test_google_step2_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_step2_message(lang);
            assert!(
                !msg.is_empty(),
                "google_step2_message({lang}) must not be empty"
            );
        }
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: google_step3_message returns non-empty for all 8 languages
    #[test]
    fn test_google_step3_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_step3_message(lang);
            assert!(
                !msg.is_empty(),
                "google_step3_message({lang}) must not be empty"
            );
        }
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: google_step4_message returns non-empty for all 8 languages
    #[test]
    fn test_google_step4_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_step4_message(lang);
            assert!(
                !msg.is_empty(),
                "google_step4_message({lang}) must not be empty"
            );
        }
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: google_complete_message returns non-empty for all 8 languages
    #[test]
    fn test_google_complete_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_complete_message(lang);
            assert!(
                !msg.is_empty(),
                "google_complete_message({lang}) must not be empty"
            );
        }
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: google_cancelled_message returns non-empty for all 8 languages
    #[test]
    fn test_google_cancelled_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_cancelled_message(lang);
            assert!(
                !msg.is_empty(),
                "google_cancelled_message({lang}) must not be empty"
            );
        }
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: google_expired_message returns non-empty for all 8 languages
    #[test]
    fn test_google_expired_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_expired_message(lang);
            assert!(
                !msg.is_empty(),
                "google_expired_message({lang}) must not be empty"
            );
        }
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: google_conflict_message returns non-empty for all 8 languages
    #[test]
    fn test_google_conflict_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_conflict_message(lang);
            assert!(
                !msg.is_empty(),
                "google_conflict_message({lang}) must not be empty"
            );
        }
    }

    // Requirement: REQ-GAUTH-017 (Should)
    // Acceptance: google_empty_input_message returns non-empty for all 8 languages
    #[test]
    fn test_google_empty_input_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_empty_input_message(lang);
            assert!(
                !msg.is_empty(),
                "google_empty_input_message({lang}) must not be empty"
            );
        }
    }

    // Requirement: REQ-GAUTH-017 (Should)
    // Acceptance: google_invalid_email_message returns non-empty for all 8 languages
    #[test]
    fn test_google_invalid_email_message_all_languages() {
        for lang in ALL_LANGUAGES {
            let msg = google_invalid_email_message(lang);
            assert!(
                !msg.is_empty(),
                "google_invalid_email_message({lang}) must not be empty"
            );
        }
    }

    // ===================================================================
    // REQ-GAUTH-013 (Should): English is the default fallback
    // ===================================================================

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: Unknown language defaults to English for all functions
    #[test]
    fn test_step1_message_default_english() {
        let unknown = google_step1_message("Klingon", false);
        let en = google_step1_message("English", false);
        assert_eq!(
            unknown, en,
            "Unknown language must default to English for step1"
        );
    }

    #[test]
    fn test_step2_message_default_english() {
        let unknown = google_step2_message("Klingon");
        let en = google_step2_message("English");
        assert_eq!(
            unknown, en,
            "Unknown language must default to English for step2"
        );
    }

    #[test]
    fn test_step3_message_default_english() {
        let unknown = google_step3_message("Klingon");
        let en = google_step3_message("English");
        assert_eq!(
            unknown, en,
            "Unknown language must default to English for step3"
        );
    }

    #[test]
    fn test_step4_message_default_english() {
        let unknown = google_step4_message("Klingon");
        let en = google_step4_message("English");
        assert_eq!(
            unknown, en,
            "Unknown language must default to English for step4"
        );
    }

    #[test]
    fn test_complete_message_default_english() {
        let unknown = google_complete_message("Klingon");
        let en = google_complete_message("English");
        assert_eq!(
            unknown, en,
            "Unknown language must default to English for complete"
        );
    }

    #[test]
    fn test_cancelled_message_default_english() {
        let unknown = google_cancelled_message("Klingon");
        let en = google_cancelled_message("English");
        assert_eq!(
            unknown, en,
            "Unknown language must default to English for cancelled"
        );
    }

    #[test]
    fn test_expired_message_default_english() {
        let unknown = google_expired_message("Klingon");
        let en = google_expired_message("English");
        assert_eq!(
            unknown, en,
            "Unknown language must default to English for expired"
        );
    }

    #[test]
    fn test_conflict_message_default_english() {
        let unknown = google_conflict_message("Klingon");
        let en = google_conflict_message("English");
        assert_eq!(
            unknown, en,
            "Unknown language must default to English for conflict"
        );
    }

    #[test]
    fn test_empty_input_message_default_english() {
        let unknown = google_empty_input_message("Klingon");
        let en = google_empty_input_message("English");
        assert_eq!(
            unknown, en,
            "Unknown language must default to English for empty_input"
        );
    }

    #[test]
    fn test_invalid_email_message_default_english() {
        let unknown = google_invalid_email_message("Klingon");
        let en = google_invalid_email_message("English");
        assert_eq!(
            unknown, en,
            "Unknown language must default to English for invalid_email"
        );
    }

    // ===================================================================
    // REQ-GAUTH-013 (Should): Each language returns distinct text
    // ===================================================================

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: Spanish messages differ from English
    #[test]
    fn test_spanish_differs_from_english() {
        // At least some messages should be translated (not all identical to English)
        let en = google_complete_message("English");
        let es = google_complete_message("Spanish");
        assert_ne!(en, es, "Spanish complete message must differ from English");
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: French messages differ from English
    #[test]
    fn test_french_differs_from_english() {
        let en = google_cancelled_message("English");
        let fr = google_cancelled_message("French");
        assert_ne!(en, fr, "French cancelled message must differ from English");
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: German messages differ from English
    #[test]
    fn test_german_differs_from_english() {
        let en = google_expired_message("English");
        let de = google_expired_message("German");
        assert_ne!(en, de, "German expired message must differ from English");
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: Russian messages differ from English
    #[test]
    fn test_russian_differs_from_english() {
        let en = google_conflict_message("English");
        let ru = google_conflict_message("Russian");
        assert_ne!(en, ru, "Russian conflict message must differ from English");
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: Portuguese messages differ from English
    #[test]
    fn test_portuguese_differs_from_english() {
        let en = google_empty_input_message("English");
        let pt = google_empty_input_message("Portuguese");
        assert_ne!(
            en, pt,
            "Portuguese empty_input message must differ from English"
        );
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: Italian messages differ from English
    #[test]
    fn test_italian_differs_from_english() {
        let en = google_invalid_email_message("English");
        let it = google_invalid_email_message("Italian");
        assert_ne!(
            en, it,
            "Italian invalid_email message must differ from English"
        );
    }

    // Requirement: REQ-GAUTH-013 (Should)
    // Acceptance: Dutch messages differ from English
    #[test]
    fn test_dutch_differs_from_english() {
        let en = google_step2_message("English");
        let nl = google_step2_message("Dutch");
        assert_ne!(en, nl, "Dutch step2 message must differ from English");
    }
}
