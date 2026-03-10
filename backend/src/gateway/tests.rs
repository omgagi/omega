use super::keywords::*;
use super::*;

#[test]
fn test_prompts_default_welcome_all_languages() {
    let prompts = Prompts::default();
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
        let msg = prompts.welcome.get(*lang);
        assert!(msg.is_some(), "welcome for {lang} should exist");
        assert!(
            msg.unwrap().contains("*OMEGA \u{03a9}*"),
            "welcome for {lang} should mention *OMEGA \u{03a9}*"
        );
    }
}

#[test]
fn test_prompts_default_welcome_fallback() {
    let prompts = Prompts::default();
    let default = prompts.welcome.get("English").cloned().unwrap_or_default();
    let msg = prompts.welcome.get("Klingon").unwrap_or(&default);
    assert!(msg.contains("*OMEGA \u{03a9}*"));
    assert!(msg.contains("honor"));
}

#[test]
fn test_bundled_system_prompt_contains_identity_soul_system() {
    let content = include_str!("../../../prompts/SYSTEM_PROMPT.md");
    assert!(
        content.contains("## Identity"),
        "bundled system prompt should contain Identity section"
    );
    assert!(
        content.contains("## Soul"),
        "bundled system prompt should contain Soul section"
    );
    assert!(
        content.contains("## System"),
        "bundled system prompt should contain System section"
    );
    assert!(
        content.contains("quietly confident"),
        "bundled system prompt should contain personality principles"
    );
}

#[test]
fn test_bundled_facts_prompt_guided_schema() {
    let content = include_str!("../../../prompts/SYSTEM_PROMPT.md");
    assert!(
        content.contains("preferred_name"),
        "bundled facts section should list preferred_name"
    );
    assert!(
        content.contains("timezone"),
        "bundled facts section should list timezone"
    );
    assert!(
        content.contains("pronouns"),
        "bundled facts section should list pronouns"
    );
    assert!(
        content.contains("PERSON"),
        "bundled facts section should emphasize personal facts"
    );
}

// --- Always-inject prompt tests ---
// All context sections are always injected — no keyword gating.

/// Build a test prompt the same way the pipeline does (all sections always injected).
fn assemble_test_prompt(prompts: &Prompts) -> String {
    let mut prompt = format!(
        "{}\n\n{}\n\n{}",
        prompts.identity, prompts.soul, prompts.system
    );

    prompt.push_str("\n\n");
    prompt.push_str(&prompts.scheduling);
    prompt.push_str("\n\n");
    prompt.push_str(&prompts.projects_rules);
    prompt.push_str("\n\n");
    prompt.push_str(&prompts.builds);
    prompt.push_str("\n\n");
    prompt.push_str(&prompts.meta);

    prompt
}

#[test]
fn test_prompt_always_includes_all_sections() {
    let prompts = Prompts::default();
    let prompt = assemble_test_prompt(&prompts);

    // Core sections
    assert!(prompt.contains("OMEGA"));
    assert!(prompt.contains("precise, warm"));

    // All conditional sections always injected
    assert!(
        prompt.contains("scheduler"),
        "scheduling section must always be injected"
    );
    assert!(
        prompt.contains("Projects path"),
        "projects section must always be injected"
    );
    assert!(
        prompt.contains("SKILL_IMPROVE"),
        "meta section must always be injected"
    );
}

#[test]
fn test_prompt_includes_scheduling_for_any_message() {
    let prompts = Prompts::default();
    let prompt = assemble_test_prompt(&prompts);

    // Even a simple greeting gets all sections
    assert!(
        prompt.contains("scheduler"),
        "scheduling should be present regardless of message content"
    );
}

#[test]
fn test_prompt_includes_projects_for_any_message() {
    let prompts = Prompts::default();
    let prompt = assemble_test_prompt(&prompts);

    assert!(
        prompt.contains("Projects path"),
        "projects section should be present regardless of message content"
    );
}

#[test]
fn test_prompt_includes_meta_for_any_message() {
    let prompts = Prompts::default();
    let prompt = assemble_test_prompt(&prompts);

    assert!(
        prompt.contains("SKILL_IMPROVE"),
        "meta section should be present regardless of message content"
    );
}

#[test]
fn test_prompt_full_size_includes_all_conditional_sections() {
    let prompts = Prompts::default();
    let prompt = assemble_test_prompt(&prompts);

    // The prompt should include all conditional sections' content
    let conditional_size =
        prompts.scheduling.len() + prompts.projects_rules.len() + prompts.meta.len();
    assert!(
        prompt.len() > conditional_size,
        "prompt ({}) must include all conditional sections ({})",
        prompt.len(),
        conditional_size
    );
}

#[test]
fn test_bundled_prompt_has_conditional_sections() {
    let content = include_str!("../../../prompts/SYSTEM_PROMPT.md");
    assert!(
        content.contains("## Scheduling"),
        "bundled prompt should have ## Scheduling section"
    );
    assert!(
        content.contains("## Projects"),
        "bundled prompt should have ## Projects section"
    );
    assert!(
        content.contains("## Meta"),
        "bundled prompt should have ## Meta section"
    );
}

// --- Build confirm/cancel tests ---

#[test]
fn test_is_build_confirmed_multilingual() {
    assert!(is_build_confirmed("yes"));
    assert!(is_build_confirmed("Yes"));
    assert!(is_build_confirmed("go ahead"));
    assert!(is_build_confirmed("sí"));
    assert!(is_build_confirmed("dale"));
    assert!(is_build_confirmed("sim"));
    assert!(is_build_confirmed("oui"));
    assert!(is_build_confirmed("ja"));
    assert!(!is_build_confirmed("maybe later"));
    assert!(!is_build_confirmed("tell me more"));
}

#[test]
fn test_is_build_cancelled_multilingual() {
    assert!(is_build_cancelled("no"));
    assert!(is_build_cancelled("cancel"));
    assert!(is_build_cancelled("cancelar"));
    assert!(is_build_cancelled("annuler"));
    assert!(is_build_cancelled("nein"));
    assert!(!is_build_cancelled("yes please"));
}

// --- Fact validation tests ---

#[test]
fn test_is_valid_fact_rejects_system_keys() {
    assert!(!is_valid_fact("welcomed", "true"));
    assert!(!is_valid_fact("preferred_language", "English"));
    assert!(!is_valid_fact("active_project", "trader"));
    assert!(!is_valid_fact("onboarding_stage", "3"));
}

#[test]
fn test_is_valid_fact_accepts_user_facts() {
    assert!(is_valid_fact("name", "Alice"));
    assert!(is_valid_fact("occupation", "Software Engineer"));
    assert!(is_valid_fact("hobby", "Painting"));
}

#[test]
fn test_is_valid_fact_rejects_too_long() {
    let long_key = "k".repeat(51);
    assert!(!is_valid_fact(&long_key, "value"));
    let long_val = "v".repeat(201);
    assert!(!is_valid_fact("key", &long_val));
}

#[test]
fn test_is_valid_fact_rejects_numeric_key() {
    assert!(!is_valid_fact("123", "value"));
    assert!(!is_valid_fact("1key", "value"));
}

#[test]
fn test_is_valid_fact_rejects_dollar_value() {
    assert!(!is_valid_fact("price", "$100"));
}

// --- Localized message tests ---

#[test]
fn test_setup_help_message_all_languages() {
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
        let msg = setup_help_message(lang);
        assert!(
            msg.contains("/setup"),
            "setup help for {lang} should mention /setup"
        );
        assert!(
            msg.contains("OMEGA"),
            "setup help for {lang} should mention OMEGA"
        );
    }
}

#[test]
fn test_build_cancelled_message_all_languages() {
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
        let msg = build_cancelled_message(lang);
        assert!(
            !msg.is_empty(),
            "cancel message for {lang} should not be empty"
        );
    }
}

// --- WhatsApp help intercept ---

#[test]
fn test_kw_match_help_keywords_whatsapp() {
    assert!(kw_match("what can you do", HELP_KW));
    assert!(kw_match("qué puedes hacer", HELP_KW));
    assert!(kw_match("was kannst du", HELP_KW));
    assert!(!kw_match("hello there", HELP_KW));
}
