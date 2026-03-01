//! Localized i18n messages for the build pipeline.
//!
//! Extracted from `builds_parse.rs` to keep that module under the 500-line limit.
//! Contains phase progress messages (8 languages x 7 phases), QA pass/retry/exhausted
//! messages, and review pass/retry/exhausted messages.

// ---------------------------------------------------------------------------
// Phase progress messages
// ---------------------------------------------------------------------------

/// Localized phase progress message for the 7-phase build pipeline.
///
/// | Phase | Agent          | English                 |
/// |-------|----------------|-------------------------|
/// | 1     | Analyst        | Analyzing requirements  |
/// | 2     | Architect      | Designing architecture  |
/// | 3     | Test Writer    | Writing tests           |
/// | 4     | Developer      | Implementing code       |
/// | 5     | QA             | Validating quality      |
/// | 6     | Reviewer       | Reviewing code          |
/// | 7     | Delivery       | Preparing delivery      |
pub(super) fn phase_message(lang: &str, phase: u8, action: &str) -> String {
    let msg = match lang {
        "Spanish" => match phase {
            1 => "Analizando requisitos...".to_string(),
            2 => "Dise\u{f1}ando arquitectura...".to_string(),
            3 => "Escribiendo pruebas...".to_string(),
            4 => "Implementando c\u{f3}digo...".to_string(),
            5 => "Validando calidad...".to_string(),
            6 => "Revisando c\u{f3}digo...".to_string(),
            7 => "Preparando entrega...".to_string(),
            _ => format!("Fase {phase}: {action}..."),
        },
        "Portuguese" => match phase {
            1 => "Analisando requisitos...".to_string(),
            2 => "Projetando arquitetura...".to_string(),
            3 => "Escrevendo testes...".to_string(),
            4 => "Implementando c\u{f3}digo...".to_string(),
            5 => "Validando qualidade...".to_string(),
            6 => "Revisando c\u{f3}digo...".to_string(),
            7 => "Preparando entrega...".to_string(),
            _ => format!("Fase {phase}: {action}..."),
        },
        "French" => match phase {
            1 => "Analyse des exigences...".to_string(),
            2 => "Conception de l'architecture...".to_string(),
            3 => "R\u{e9}daction des tests...".to_string(),
            4 => "Impl\u{e9}mentation du code...".to_string(),
            5 => "Validation de la qualit\u{e9}...".to_string(),
            6 => "R\u{e9}vision du code...".to_string(),
            7 => "Pr\u{e9}paration de la livraison...".to_string(),
            _ => format!("Phase {phase}\u{a0}: {action}..."),
        },
        "German" => match phase {
            1 => "Analysiere Anforderungen...".to_string(),
            2 => "Architektur entwerfen...".to_string(),
            3 => "Tests schreiben...".to_string(),
            4 => "Code implementieren...".to_string(),
            5 => "Qualit\u{e4}t validieren...".to_string(),
            6 => "Code \u{fc}berpr\u{fc}fen...".to_string(),
            7 => "Lieferung vorbereiten...".to_string(),
            _ => format!("Phase {phase}: {action}..."),
        },
        "Italian" => match phase {
            1 => "Analisi dei requisiti...".to_string(),
            2 => "Progettazione dell'architettura...".to_string(),
            3 => "Scrittura dei test...".to_string(),
            4 => "Implementazione del codice...".to_string(),
            5 => "Validazione della qualit\u{e0}...".to_string(),
            6 => "Revisione del codice...".to_string(),
            7 => "Preparazione della consegna...".to_string(),
            _ => format!("Fase {phase}: {action}..."),
        },
        "Dutch" => match phase {
            1 => "Vereisten analyseren...".to_string(),
            2 => "Architectuur ontwerpen...".to_string(),
            3 => "Tests schrijven...".to_string(),
            4 => "Code implementeren...".to_string(),
            5 => "Kwaliteit valideren...".to_string(),
            6 => "Code reviewen...".to_string(),
            7 => "Levering voorbereiden...".to_string(),
            _ => format!("Fase {phase}: {action}..."),
        },
        "Russian" => match phase {
            1 => "\u{410}\u{43d}\u{430}\u{43b}\u{438}\u{437} \u{442}\u{440}\u{435}\u{431}\u{43e}\u{432}\u{430}\u{43d}\u{438}\u{439}...".to_string(),
            2 => "\u{41f}\u{440}\u{43e}\u{435}\u{43a}\u{442}\u{438}\u{440}\u{43e}\u{432}\u{430}\u{43d}\u{438}\u{435} \u{430}\u{440}\u{445}\u{438}\u{442}\u{435}\u{43a}\u{442}\u{443}\u{440}\u{44b}...".to_string(),
            3 => "\u{41d}\u{430}\u{43f}\u{438}\u{441}\u{430}\u{43d}\u{438}\u{435} \u{442}\u{435}\u{441}\u{442}\u{43e}\u{432}...".to_string(),
            4 => "\u{420}\u{435}\u{430}\u{43b}\u{438}\u{437}\u{430}\u{446}\u{438}\u{44f} \u{43a}\u{43e}\u{434}\u{430}...".to_string(),
            5 => "\u{41f}\u{440}\u{43e}\u{432}\u{435}\u{440}\u{43a}\u{430} \u{43a}\u{430}\u{447}\u{435}\u{441}\u{442}\u{432}\u{430}...".to_string(),
            6 => "\u{41e}\u{431}\u{437}\u{43e}\u{440} \u{43a}\u{43e}\u{434}\u{430}...".to_string(),
            7 => "\u{41f}\u{43e}\u{434}\u{433}\u{43e}\u{442}\u{43e}\u{432}\u{43a}\u{430} \u{43a} \u{434}\u{43e}\u{441}\u{442}\u{430}\u{432}\u{43a}\u{435}...".to_string(),
            _ => format!("\u{424}\u{430}\u{437}\u{430} {phase}: {action}..."),
        },
        // English and any unknown language
        _ => match phase {
            1 => "Analyzing requirements...".to_string(),
            2 => "Designing architecture...".to_string(),
            3 => "Writing tests...".to_string(),
            4 => "Implementing code...".to_string(),
            5 => "Validating quality...".to_string(),
            6 => "Reviewing code...".to_string(),
            7 => "Preparing delivery...".to_string(),
            _ => format!("Phase {phase}: {action}..."),
        },
    };
    format!("\u{2699}\u{fe0f} {msg}")
}

/// Localized phase progress message, keyed by phase name (string).
///
/// Maps topology phase names to the same i18n strings as the old
/// phase_message(u8) function. Unknown phase names use the generic fallback.
pub(super) fn phase_message_by_name(lang: &str, phase_name: &str) -> String {
    // Map phase name to the legacy phase number for reuse.
    let phase_num = match phase_name {
        "analyst" => 1,
        "architect" => 2,
        "test-writer" => 3,
        "developer" => 4,
        "qa" => 5,
        "reviewer" => 6,
        "delivery" => 7,
        _ => 0,
    };

    if phase_num > 0 {
        // Delegate to existing function for known phases.
        phase_message(lang, phase_num, phase_name)
    } else {
        // Generic fallback for unknown/custom phase names.
        let action = phase_name.replace('-', " ");
        format!("\u{2699}\u{fe0f} {action}...")
    }
}

// ---------------------------------------------------------------------------
// QA i18n messages
// ---------------------------------------------------------------------------

/// Localized QA pass message, includes attempt count when > 1.
pub(super) fn qa_pass_message(lang: &str, attempt: u32) -> String {
    let suffix = if attempt > 1 {
        format!(" (attempt {attempt})")
    } else {
        String::new()
    };
    let base = match lang {
        "Spanish" => "Todas las verificaciones pasaron",
        "Portuguese" => "Todas as verificações passaram",
        "French" => "Toutes les vérifications réussies",
        "German" => "Alle Prüfungen bestanden",
        "Italian" => "Tutte le verifiche superate",
        "Dutch" => "Alle controles geslaagd",
        "Russian" => "Все проверки пройдены",
        _ => "All checks passed",
    };
    format!("{base}{suffix}.")
}

/// Localized QA retry message -- sent when verification finds issues and developer is re-invoked.
// TODO(phase-2): parameterize retry count from topology instead of hardcoded /3
// NOTE: Tracked as P2-23 — phase-2 i18n not yet implemented, English-only fallback used.
//       Currently hardcodes "/3" in all languages; should read max_retries from RetryConfig.
pub(super) fn qa_retry_message(lang: &str, attempt: u32, reason: &str) -> String {
    match lang {
        "Spanish" => {
            format!("Verificación {attempt}/3 encontró problemas — corrigiendo...\n{reason}")
        }
        "Portuguese" => {
            format!("Verificação {attempt}/3 encontrou problemas — corrigindo...\n{reason}")
        }
        "French" => {
            format!("Vérification {attempt}/3 a trouvé des problèmes — correction...\n{reason}")
        }
        "German" => {
            format!("Prüfung {attempt}/3 hat Probleme gefunden — wird behoben...\n{reason}")
        }
        "Italian" => format!("Verifica {attempt}/3 ha trovato problemi — correzione...\n{reason}"),
        "Dutch" => format!("Controle {attempt}/3 vond problemen — wordt opgelost...\n{reason}"),
        "Russian" => format!("Проверка {attempt}/3 обнаружила проблемы — исправляю...\n{reason}"),
        _ => format!("Verification {attempt}/3 found issues — fixing...\n{reason}"),
    }
}

/// Localized QA exhausted message -- sent when all 3 QA iterations fail.
// TODO(phase-2): parameterize retry count from topology instead of hardcoded 3
// NOTE: Tracked as P2-24 — phase-2 i18n not yet implemented, English-only fallback used.
//       Currently hardcodes "3" in all languages; should read max_retries from RetryConfig.
pub(super) fn qa_exhausted_message(lang: &str, reason: &str, dir: &str) -> String {
    match lang {
        "Spanish" => format!("La verificación falló después de 3 intentos: {reason}\nResultados parciales en `{dir}`"),
        "Portuguese" => format!("A verificação falhou após 3 tentativas: {reason}\nResultados parciais em `{dir}`"),
        "French" => format!("La vérification a échoué après 3 tentatives : {reason}\nRésultats partiels dans `{dir}`"),
        "German" => format!("Verifizierung nach 3 Versuchen fehlgeschlagen: {reason}\nTeilergebnisse in `{dir}`"),
        "Italian" => format!("La verifica è fallita dopo 3 tentativi: {reason}\nRisultati parziali in `{dir}`"),
        "Dutch" => format!("Verificatie mislukt na 3 pogingen: {reason}\nGedeeltelijke resultaten in `{dir}`"),
        "Russian" => format!("Проверка не пройдена после 3 попыток: {reason}\nЧастичные результаты в `{dir}`"),
        _ => format!("Build verification failed after 3 iterations: {reason}\nPartial results at `{dir}`"),
    }
}

// ---------------------------------------------------------------------------
// Review i18n messages
// ---------------------------------------------------------------------------

/// Localized review pass message.
pub(super) fn review_pass_message(lang: &str, attempt: u32) -> String {
    let suffix = if attempt > 1 {
        format!(" (attempt {attempt})")
    } else {
        String::new()
    };
    let base = match lang {
        "Spanish" => "Revisión de código aprobada",
        "Portuguese" => "Revisão de código aprovada",
        "French" => "Revue de code réussie",
        "German" => "Code-Review bestanden",
        "Italian" => "Revisione del codice superata",
        "Dutch" => "Code review geslaagd",
        "Russian" => "Обзор кода пройден",
        _ => "Code review passed",
    };
    format!("{base}{suffix}.")
}

/// Localized review retry message -- sent when review finds issues and developer is re-invoked.
pub(super) fn review_retry_message(lang: &str, reason: &str) -> String {
    match lang {
        "Spanish" => format!("La revisión encontró problemas — corrigiendo...\n{reason}"),
        "Portuguese" => format!("A revisão encontrou problemas — corrigindo...\n{reason}"),
        "French" => format!("La revue a trouvé des problèmes — correction...\n{reason}"),
        "German" => format!("Review hat Probleme gefunden — wird behoben...\n{reason}"),
        "Italian" => format!("La revisione ha trovato problemi — correzione...\n{reason}"),
        "Dutch" => format!("Review vond problemen — wordt opgelost...\n{reason}"),
        "Russian" => format!("Обзор обнаружил проблемы — исправляю...\n{reason}"),
        _ => format!("Review found issues — fixing...\n{reason}"),
    }
}

/// Localized review exhausted message -- sent when both review iterations fail.
// TODO(phase-2): parameterize retry count from topology instead of hardcoded 2
// NOTE: Tracked as P2-25 — phase-2 i18n not yet implemented, English-only fallback used.
//       Currently hardcodes "2" in all languages; should read max_retries from RetryConfig.
pub(super) fn review_exhausted_message(lang: &str, reason: &str, dir: &str) -> String {
    match lang {
        "Spanish" => format!(
            "La revisión falló después de 2 intentos: {reason}\nResultados parciales en `{dir}`"
        ),
        "Portuguese" => {
            format!("A revisão falhou após 2 tentativas: {reason}\nResultados parciais em `{dir}`")
        }
        "French" => format!(
            "La revue a échoué après 2 tentatives : {reason}\nRésultats partiels dans `{dir}`"
        ),
        "German" => format!(
            "Code-Review nach 2 Versuchen fehlgeschlagen: {reason}\nTeilergebnisse in `{dir}`"
        ),
        "Italian" => format!(
            "La revisione è fallita dopo 2 tentativi: {reason}\nRisultati parziali in `{dir}`"
        ),
        "Dutch" => {
            format!("Review mislukt na 2 pogingen: {reason}\nGedeeltelijke resultaten in `{dir}`")
        }
        "Russian" => {
            format!("Обзор не пройден после 2 попыток: {reason}\nЧастичные результаты в `{dir}`")
        }
        _ => format!("Code review failed after 2 iterations: {reason}\nPartial results at `{dir}`"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_message_english() {
        let msg = phase_message("English", 1, "analyzing");
        assert!(msg.contains("Analyzing"));
    }

    #[test]
    fn test_phase_message_spanish() {
        let msg = phase_message("Spanish", 1, "analyzing");
        assert!(msg.contains("Analizando"));
    }

    #[test]
    fn test_phase_message_all_languages() {
        // Phase 1 (Analyst) — each language has a custom string
        assert!(phase_message("Portuguese", 1, "").contains("Analisando"));
        assert!(phase_message("French", 1, "").contains("Analyse"));
        assert!(phase_message("German", 1, "").contains("Analysiere"));
        assert!(phase_message("Italian", 1, "").contains("Analisi"));
        assert!(phase_message("Dutch", 1, "").contains("analyseren"));
        assert!(phase_message("Russian", 1, "").contains("Анализ"));

        // Phase 7 (Delivery) — moved from phase 5 in the 7-phase pipeline
        assert!(phase_message("French", 7, "").contains("livraison"));
        assert!(phase_message("German", 7, "").contains("Lieferung"));
        assert!(phase_message("Italian", 7, "").contains("consegna"));
        assert!(phase_message("Dutch", 7, "").contains("Levering"));
        assert!(phase_message("Russian", 7, "").contains("доставке"));

        // Phase 3 (Test Writer) — now has custom messages per language
        assert!(phase_message("French", 3, "").contains("tests"));
        assert!(phase_message("German", 3, "").contains("Tests"));
        assert!(phase_message("Italian", 3, "").contains("test"));
        assert!(phase_message("Dutch", 3, "").contains("Tests"));
        assert!(phase_message("Russian", 3, "").contains("тестов"));
    }

    // =======================================================================
    // REQ-BAP-009 (Must): Localized progress messages for all 7 phases
    // =======================================================================
    //
    // The current phase_message() handles phases 1 and 5 with custom messages
    // and uses a generic format for others. For the 7-phase pipeline, the
    // developer must extend this to handle all 7 phases with meaningful
    // localized messages.
    //
    // These tests will FAIL until the developer extends phase_message().

    // Requirement: REQ-BAP-009 (Must)
    // Acceptance: Each phase transition sends a dedicated localized message
    // (not just the generic "Phase N: action..." format).
    //
    // IMPORTANT: These tests call phase_message with an EMPTY action string
    // to ensure the function produces meaningful per-phase messages from its
    // own content, not by echoing the action parameter back.
    #[test]
    fn test_phase_message_7_phases_english_custom_messages() {
        // Phase 1: Analyst — already has custom message
        let msg1 = phase_message("English", 1, "");
        assert!(
            msg1.contains("Analyzing"),
            "Phase 1 must have custom English message: got '{msg1}'"
        );
        // Phase 2: Architect — needs custom message about architecture/design
        let msg2 = phase_message("English", 2, "");
        assert!(
            msg2.contains("architecture")
                || msg2.contains("Architecture")
                || msg2.contains("design")
                || msg2.contains("Design"),
            "Phase 2 must have custom English message about architecture: got '{msg2}'"
        );
        // Phase 3: Test Writer — needs custom message about tests
        let msg3 = phase_message("English", 3, "");
        assert!(
            msg3.contains("test") || msg3.contains("Test"),
            "Phase 3 must have custom English message about tests: got '{msg3}'"
        );
        // Phase 4: Developer — needs custom message about implementation
        let msg4 = phase_message("English", 4, "");
        assert!(
            msg4.contains("implement")
                || msg4.contains("Implement")
                || msg4.contains("build")
                || msg4.contains("Build")
                || msg4.contains("cod")
                || msg4.contains("Cod"),
            "Phase 4 must have custom English message about implementation: got '{msg4}'"
        );
        // Phase 5: QA — needs custom message about verification/quality
        let msg5 = phase_message("English", 5, "");
        assert!(
            msg5.contains("verif")
                || msg5.contains("Verif")
                || msg5.contains("quality")
                || msg5.contains("Quality")
                || msg5.contains("check")
                || msg5.contains("Check")
                || msg5.contains("test")
                || msg5.contains("Test"),
            "Phase 5 must have custom English message about QA: got '{msg5}'"
        );
        // Phase 6: Reviewer — needs custom message about review
        let msg6 = phase_message("English", 6, "");
        assert!(
            msg6.contains("review")
                || msg6.contains("Review")
                || msg6.contains("audit")
                || msg6.contains("Audit"),
            "Phase 6 must have custom English message about review: got '{msg6}'"
        );
        // Phase 7: Delivery — needs custom message about delivery
        let msg7 = phase_message("English", 7, "");
        assert!(
            msg7.contains("deliver")
                || msg7.contains("Deliver")
                || msg7.contains("Preparing")
                || msg7.contains("delivery"),
            "Phase 7 must have custom English message about delivery: got '{msg7}'"
        );
    }

    // Requirement: REQ-BAP-009 (Must)
    // Acceptance: All 8 languages supported for all 7 phases
    #[test]
    fn test_phase_message_7_phases_all_languages_non_empty() {
        let languages = [
            "English",
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ];
        for lang in &languages {
            for phase in 1..=7 {
                let msg = phase_message(lang, phase, "action");
                assert!(
                    !msg.is_empty(),
                    "Phase {phase} message for {lang} must not be empty"
                );
            }
        }
    }

    // Requirement: REQ-BAP-009 (Must)
    // Acceptance: Spanish messages for all 7 phases
    #[test]
    fn test_phase_message_7_phases_spanish() {
        let msg1 = phase_message("Spanish", 1, "");
        assert!(msg1.contains("Analizando"), "Spanish phase 1: got '{msg1}'");

        let msg7 = phase_message("Spanish", 7, "");
        assert!(
            msg7.contains("entrega") || msg7.contains("Entrega") || msg7.contains("Preparando"),
            "Spanish phase 7 should mention delivery: got '{msg7}'"
        );
    }

    // Requirement: REQ-BAP-009 (Must)
    // Edge case: unknown language falls back to English-like behavior
    #[test]
    fn test_phase_message_unknown_language_all_phases() {
        for phase in 1..=7 {
            let msg = phase_message("Klingon", phase, "action");
            assert!(
                !msg.is_empty(),
                "Unknown language should still produce a message for phase {phase}"
            );
        }
    }

    // Requirement: REQ-BAP-009 (Must)
    // Edge case: phase 0 and phase 8 (out of range)
    #[test]
    fn test_phase_message_out_of_range_phases() {
        // Phase 0 and 8+ should produce a reasonable generic message.
        let msg0 = phase_message("English", 0, "unknown");
        assert!(
            !msg0.is_empty(),
            "Phase 0 should produce some output (generic fallback)"
        );
        let msg8 = phase_message("English", 8, "unknown");
        assert!(
            !msg8.is_empty(),
            "Phase 8 should produce some output (generic fallback)"
        );
    }

    // Requirement: REQ-BAP-009 (Must)
    // Regression: In the 7-phase pipeline, phase 7 is delivery.
    // The custom message must not depend on the action parameter.
    #[test]
    fn test_phase_message_delivery_phase_english() {
        // Phase 7 is delivery in the new pipeline.
        // Use EMPTY action to ensure the function has its own custom message.
        let msg = phase_message("English", 7, "");
        assert!(
            msg.contains("deliver")
                || msg.contains("Deliver")
                || msg.contains("Preparing")
                || msg.contains("delivery"),
            "Phase 7 (delivery) must have a custom English message: got '{msg}'"
        );
    }

    // ===================================================================
    // QA i18n messages
    // ===================================================================

    #[test]
    fn test_qa_pass_message_first_attempt() {
        let msg = qa_pass_message("English", 1);
        assert!(msg.contains("All checks passed"));
        assert!(!msg.contains("attempt"));
    }

    #[test]
    fn test_qa_pass_message_retry_attempt() {
        let msg = qa_pass_message("English", 2);
        assert!(msg.contains("All checks passed"));
        assert!(msg.contains("attempt 2"));
    }

    #[test]
    fn test_qa_pass_message_all_languages() {
        let languages = [
            "English",
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ];
        for lang in &languages {
            let msg = qa_pass_message(lang, 1);
            assert!(
                !msg.is_empty(),
                "qa_pass_message for {lang} must not be empty"
            );
        }
    }

    #[test]
    fn test_qa_retry_message_english() {
        let msg = qa_retry_message("English", 1, "tests failing");
        assert!(msg.contains("1/3"));
        assert!(msg.contains("tests failing"));
    }

    #[test]
    fn test_qa_exhausted_message_english() {
        let msg = qa_exhausted_message("English", "3 tests failing", "/tmp/build");
        assert!(msg.contains("3 iterations"));
        assert!(msg.contains("/tmp/build"));
    }

    // ===================================================================
    // Review i18n messages
    // ===================================================================

    #[test]
    fn test_review_pass_message_first_attempt() {
        let msg = review_pass_message("English", 1);
        assert!(msg.contains("Code review passed"));
        assert!(!msg.contains("attempt"));
    }

    #[test]
    fn test_review_pass_message_retry_attempt() {
        let msg = review_pass_message("English", 2);
        assert!(msg.contains("attempt 2"));
    }

    #[test]
    fn test_review_pass_message_all_languages() {
        let languages = [
            "English",
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ];
        for lang in &languages {
            let msg = review_pass_message(lang, 1);
            assert!(
                !msg.is_empty(),
                "review_pass_message for {lang} must not be empty"
            );
        }
    }

    #[test]
    fn test_review_retry_message_english() {
        let msg = review_retry_message("English", "security issue found");
        assert!(msg.contains("Review found issues"));
        assert!(msg.contains("security issue found"));
    }

    #[test]
    fn test_review_exhausted_message_english() {
        let msg = review_exhausted_message("English", "bugs remain", "/tmp/build");
        assert!(msg.contains("2 iterations"));
        assert!(msg.contains("/tmp/build"));
    }

    // ===================================================================
    // REQ-TOP-008 (Should): phase_message_by_name() -- localized messages
    //                        keyed by phase name string instead of u8
    // ===================================================================

    // Requirement: REQ-TOP-008 (Should)
    // Acceptance: phase_message_by_name maps known phase names to i18n strings
    #[test]
    fn test_phase_message_by_name_analyst_english() {
        let msg = phase_message_by_name("English", "analyst");
        assert!(
            msg.contains("Analyzing"),
            "phase_message_by_name('analyst') must produce same message as phase 1: got '{msg}'"
        );
    }

    // Requirement: REQ-TOP-008 (Should)
    // Acceptance: All 7 phase names produce correct English messages
    #[test]
    fn test_phase_message_by_name_all_phases_english() {
        let expectations = vec![
            ("analyst", "Analyzing"),
            ("architect", "architecture"),
            ("test-writer", "test"),
            ("developer", "mplement"),
            ("qa", "alid"),
            ("reviewer", "eview"),
            ("delivery", "eliver"),
        ];
        for (name, expected_substr) in expectations {
            let msg = phase_message_by_name("English", name);
            assert!(
                msg.to_lowercase().contains(&expected_substr.to_lowercase()),
                "phase_message_by_name('{name}') must contain '{expected_substr}': got '{msg}'"
            );
        }
    }

    // Requirement: REQ-TOP-008 (Should)
    // Acceptance: All 8 languages produce non-empty messages for all 7 phases
    #[test]
    fn test_phase_message_by_name_all_languages_all_phases() {
        let languages = [
            "English",
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ];
        let phases = [
            "analyst",
            "architect",
            "test-writer",
            "developer",
            "qa",
            "reviewer",
            "delivery",
        ];
        for lang in &languages {
            for phase_name in &phases {
                let msg = phase_message_by_name(lang, phase_name);
                assert!(
                    !msg.is_empty(),
                    "phase_message_by_name('{lang}', '{phase_name}') must not be empty"
                );
            }
        }
    }

    // Requirement: REQ-TOP-008 (Should)
    // Acceptance: Spanish messages produced correctly by name
    #[test]
    fn test_phase_message_by_name_spanish() {
        let msg = phase_message_by_name("Spanish", "analyst");
        assert!(
            msg.contains("Analizando"),
            "Spanish analyst must contain 'Analizando': got '{msg}'"
        );

        let msg7 = phase_message_by_name("Spanish", "delivery");
        assert!(
            msg7.contains("entrega") || msg7.contains("Entrega") || msg7.contains("Preparando"),
            "Spanish delivery must contain delivery term: got '{msg7}'"
        );
    }

    // Requirement: REQ-TOP-008 (Should)
    // Acceptance: Russian messages produced correctly by name
    #[test]
    fn test_phase_message_by_name_russian() {
        let msg = phase_message_by_name("Russian", "test-writer");
        // Russian phase 3 contains the word for "tests" in Cyrillic
        assert!(
            !msg.is_empty(),
            "Russian test-writer message must not be empty"
        );
    }

    // Requirement: REQ-TOP-008 (Should)
    // Acceptance: Generic fallback for unknown phase names
    #[test]
    fn test_phase_message_by_name_unknown_phase_fallback() {
        let msg = phase_message_by_name("English", "custom-validator");
        assert!(
            !msg.is_empty(),
            "Unknown phase name must produce a non-empty fallback"
        );
        // The fallback should include the phase name (with hyphens replaced by spaces).
        assert!(
            msg.contains("custom validator") || msg.contains("custom-validator"),
            "Fallback must include the phase name: got '{msg}'"
        );
    }

    // Requirement: REQ-TOP-008 (Should)
    // Edge case: Unknown language with known phase name
    #[test]
    fn test_phase_message_by_name_unknown_language() {
        let msg = phase_message_by_name("Klingon", "architect");
        assert!(
            !msg.is_empty(),
            "Unknown language should still produce a message"
        );
    }

    // Requirement: REQ-TOP-008 (Should)
    // Edge case: Empty phase name
    #[test]
    fn test_phase_message_by_name_empty_phase_name() {
        let msg = phase_message_by_name("English", "");
        assert!(
            !msg.is_empty(),
            "Empty phase name should produce a non-empty fallback"
        );
    }

    // Requirement: REQ-TOP-008 (Should)
    // Acceptance: phase_message_by_name produces identical output to phase_message
    //             for all known phase name/number pairs
    #[test]
    fn test_phase_message_by_name_parity_with_phase_message() {
        let mapping = vec![
            ("analyst", 1u8),
            ("architect", 2),
            ("test-writer", 3),
            ("developer", 4),
            ("qa", 5),
            ("reviewer", 6),
            ("delivery", 7),
        ];
        let languages = [
            "English",
            "Spanish",
            "Portuguese",
            "French",
            "German",
            "Italian",
            "Dutch",
            "Russian",
        ];
        for lang in &languages {
            for (name, num) in &mapping {
                let by_name = phase_message_by_name(lang, name);
                let by_num = phase_message(lang, *num, name);
                assert_eq!(
                    by_name, by_num,
                    "Parity: phase_message_by_name('{lang}', '{name}') must equal \
                     phase_message('{lang}', {num}, '{name}')"
                );
            }
        }
    }
}
