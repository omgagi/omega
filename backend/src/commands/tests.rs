use super::*;
use omega_core::config::MemoryConfig;

use std::sync::atomic::{AtomicU64, Ordering};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Create a temporary on-disk store for testing (unique per call).
async fn test_store() -> Store {
    let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir =
        std::env::temp_dir().join(format!("__omega_cmd_test_{}_{}__", std::process::id(), id));
    let _ = std::fs::create_dir_all(&dir);
    let db_path = dir.join("test.db").to_string_lossy().to_string();
    let _ = std::fs::remove_file(&db_path);
    let config = MemoryConfig {
        backend: "sqlite".to_string(),
        db_path,
        max_context_messages: 10,
    };
    Store::new(&config).await.unwrap()
}

#[test]
fn test_parse_personality_command() {
    assert!(matches!(
        Command::parse("/personality"),
        Some(Command::Personality)
    ));
    assert!(matches!(
        Command::parse("/personality be more casual"),
        Some(Command::Personality)
    ));
    assert!(matches!(
        Command::parse("/personality reset"),
        Some(Command::Personality)
    ));
}

#[test]
fn test_parse_all_commands() {
    assert!(matches!(Command::parse("/status"), Some(Command::Status)));
    assert!(matches!(Command::parse("/memory"), Some(Command::Memory)));
    assert!(matches!(Command::parse("/history"), Some(Command::History)));
    assert!(matches!(Command::parse("/facts"), Some(Command::Facts)));
    assert!(matches!(Command::parse("/forget"), Some(Command::Forget)));
    assert!(matches!(Command::parse("/tasks"), Some(Command::Tasks)));
    assert!(matches!(Command::parse("/cancel x"), Some(Command::Cancel)));
    assert!(matches!(
        Command::parse("/language"),
        Some(Command::Language)
    ));
    assert!(matches!(Command::parse("/lang"), Some(Command::Language)));
    assert!(matches!(
        Command::parse("/personality"),
        Some(Command::Personality)
    ));
    assert!(matches!(Command::parse("/skills"), Some(Command::Skills)));
    assert!(matches!(
        Command::parse("/projects"),
        Some(Command::Projects)
    ));
    assert!(matches!(Command::parse("/project"), Some(Command::Project)));
    assert!(matches!(Command::parse("/purge"), Some(Command::Purge)));
    assert!(matches!(
        Command::parse("/whatsapp"),
        Some(Command::WhatsApp)
    ));
    assert!(matches!(
        Command::parse("/heartbeat"),
        Some(Command::Heartbeat)
    ));
    assert!(matches!(
        Command::parse("/learning"),
        Some(Command::Learning)
    ));
    assert!(matches!(Command::parse("/setup"), Some(Command::Setup)));
    assert!(matches!(Command::parse("/help"), Some(Command::Help)));
}

#[test]
fn test_parse_commands_with_botname_suffix() {
    assert!(matches!(
        Command::parse("/help@omega_bot"),
        Some(Command::Help)
    ));
    assert!(matches!(
        Command::parse("/status@omega_bot"),
        Some(Command::Status)
    ));
    assert!(matches!(
        Command::parse("/cancel@omega_bot task123"),
        Some(Command::Cancel)
    ));
    assert!(matches!(
        Command::parse("/lang@omega_bot"),
        Some(Command::Language)
    ));
    // Unknown command with @botname should still return None.
    assert!(Command::parse("/unknown@omega_bot").is_none());
}

#[test]
fn test_parse_purge_command() {
    assert!(matches!(Command::parse("/purge"), Some(Command::Purge)));
    assert!(matches!(
        Command::parse("/purge@omega_bot"),
        Some(Command::Purge)
    ));
}

#[test]
fn test_parse_unknown_returns_none() {
    assert!(Command::parse("/unknown").is_none());
    assert!(Command::parse("hello").is_none());
    assert!(Command::parse("").is_none());
}

#[tokio::test]
async fn test_personality_show_default() {
    let store = test_store().await;
    let result = settings::handle_personality(&store, "user1", "/personality", "English").await;
    assert!(
        result.contains("default personality"),
        "should show default when no preference set"
    );
}

#[tokio::test]
async fn test_personality_set_and_show() {
    let store = test_store().await;
    let result = settings::handle_personality(
        &store,
        "user1",
        "/personality be more casual and funny",
        "English",
    )
    .await;
    assert!(
        result.contains("be more casual and funny"),
        "should confirm the personality was set"
    );

    let result = settings::handle_personality(&store, "user1", "/personality", "English").await;
    assert!(
        result.contains("be more casual and funny"),
        "should show the stored personality"
    );
}

#[tokio::test]
async fn test_personality_reset() {
    let store = test_store().await;
    let _ =
        settings::handle_personality(&store, "user1", "/personality be formal", "English").await;
    let result =
        settings::handle_personality(&store, "user1", "/personality reset", "English").await;
    assert!(result.contains("reset to defaults"), "should confirm reset");

    let result = settings::handle_personality(&store, "user1", "/personality", "English").await;
    assert!(
        result.contains("default personality"),
        "should show default after reset"
    );
}

#[tokio::test]
async fn test_personality_reset_when_already_default() {
    let store = test_store().await;
    let result =
        settings::handle_personality(&store, "user1", "/personality reset", "English").await;
    assert!(
        result.contains("Already using default"),
        "should indicate already default"
    );
}

#[tokio::test]
async fn test_purge_preserves_system_facts() {
    let store = test_store().await;
    // Store system facts.
    store.store_fact("user1", "welcomed", "true").await.unwrap();
    store
        .store_fact("user1", "preferred_language", "Spanish")
        .await
        .unwrap();
    store
        .store_fact("user1", "personality", "casual")
        .await
        .unwrap();
    // Store junk facts.
    store
        .store_fact("user1", "btc_price", "45000")
        .await
        .unwrap();
    store
        .store_fact("user1", "target", "0.5 BTC")
        .await
        .unwrap();
    store.store_fact("user1", "name", "Juan").await.unwrap();

    let result = tasks::handle_purge(&store, "user1", "English").await;
    assert!(
        result.contains("Purged 3 facts"),
        "should report 3 purged: {result}"
    );

    // System facts preserved.
    let facts = store.get_facts("user1").await.unwrap();
    let keys: Vec<&str> = facts.iter().map(|(k, _)| k.as_str()).collect();
    assert!(keys.contains(&"welcomed"));
    assert!(keys.contains(&"preferred_language"));
    assert!(keys.contains(&"personality"));
    // Junk facts removed.
    assert!(!keys.contains(&"btc_price"));
    assert!(!keys.contains(&"target"));
    // Non-system personal facts also removed.
    assert!(!keys.contains(&"name"));
}

#[tokio::test]
async fn test_help_spanish() {
    let result = status::handle_help("Spanish");
    assert!(
        result.contains("Comandos de *OMEGA Ω*"),
        "should have Spanish header: {result}"
    );
    assert!(
        result.contains("/status"),
        "should still contain command names"
    );
}

#[tokio::test]
async fn test_forget_localized() {
    let store = test_store().await;
    let result = tasks::handle_forget(&store, "telegram", "user1", "Spanish").await;
    assert!(
        result.contains("No hay conversación activa"),
        "should show Spanish empty state: {result}"
    );
}

#[test]
fn test_parse_heartbeat_command() {
    assert!(matches!(
        Command::parse("/heartbeat"),
        Some(Command::Heartbeat)
    ));
    assert!(matches!(
        Command::parse("/heartbeat@omega_bot"),
        Some(Command::Heartbeat)
    ));
}

#[test]
fn test_heartbeat_enabled() {
    let result = settings::handle_heartbeat(true, 30, None, "English");
    assert!(result.contains("Heartbeat"), "should have header: {result}");
    assert!(result.contains("active"), "should show active: {result}");
    assert!(result.contains("30"), "should show interval: {result}");
    assert!(
        result.contains("minutes"),
        "should show minutes label: {result}"
    );
}

#[test]
fn test_heartbeat_disabled() {
    let result = settings::handle_heartbeat(false, 15, None, "English");
    assert!(
        result.contains("disabled"),
        "should show disabled: {result}"
    );
}

#[test]
fn test_heartbeat_localized() {
    let result = settings::handle_heartbeat(true, 60, None, "Spanish");
    assert!(
        result.contains("activo"),
        "should show Spanish status: {result}"
    );
    assert!(
        result.contains("Intervalo:"),
        "should show Spanish interval label: {result}"
    );
}

#[test]
fn test_heartbeat_with_nonexistent_project_falls_back_to_global() {
    // A project that doesn't exist should fall back to global heartbeat
    // (same behavior as None since the project file won't exist)
    let with_project =
        settings::handle_heartbeat(true, 30, Some("nonexistent-project-xyz"), "English");
    let without_project = settings::handle_heartbeat(true, 30, None, "English");
    assert_eq!(
        with_project, without_project,
        "nonexistent project should fall back to global"
    );
}

#[test]
fn test_help_includes_heartbeat() {
    let result = status::handle_help("English");
    assert!(
        result.contains("/heartbeat"),
        "help should list /heartbeat: {result}"
    );
}

#[test]
fn test_parse_learning_command() {
    assert!(matches!(
        Command::parse("/learning"),
        Some(Command::Learning)
    ));
    assert!(matches!(
        Command::parse("/learning@omega_bot"),
        Some(Command::Learning)
    ));
}

#[tokio::test]
async fn test_learning_empty() {
    let store = test_store().await;
    let result = learning::handle_learning(&store, "user1", "English").await;
    assert!(
        result.contains("No learning data yet"),
        "should show empty state: {result}"
    );
}

#[tokio::test]
async fn test_learning_with_data() {
    let store = test_store().await;
    store
        .store_outcome(
            "user1",
            "training",
            1,
            "User prefers morning workouts",
            "conversation",
            "",
        )
        .await
        .unwrap();
    store
        .store_lesson(
            "user1",
            "scheduling",
            "Remind 1h before, not at exact time",
            "",
        )
        .await
        .unwrap();

    let result = learning::handle_learning(&store, "user1", "English").await;
    assert!(result.contains("Learning"), "should have header: {result}");
    assert!(
        result.contains("Behavioral rules"),
        "should show rules section: {result}"
    );
    assert!(
        result.contains("scheduling"),
        "should show lesson domain: {result}"
    );
    assert!(
        result.contains("Remind 1h before"),
        "should show lesson rule: {result}"
    );
    assert!(
        result.contains("Recent outcomes"),
        "should show outcomes section: {result}"
    );
    assert!(
        result.contains("training"),
        "should show outcome domain: {result}"
    );
}

#[tokio::test]
async fn test_learning_localized() {
    let store = test_store().await;
    store
        .store_lesson("user1", "crypto", "Track BTC daily", "")
        .await
        .unwrap();
    let result = learning::handle_learning(&store, "user1", "Spanish").await;
    assert!(
        result.contains("Aprendizaje"),
        "should have Spanish header: {result}"
    );
    assert!(
        result.contains("Reglas de comportamiento"),
        "should have Spanish rules label: {result}"
    );
}

#[test]
fn test_help_includes_learning() {
    let result = status::handle_help("English");
    assert!(
        result.contains("/learning"),
        "help should list /learning: {result}"
    );
}

// ===================================================================
// REQ-BRAIN-001 (Must): /setup command registration in Command::parse()
// ===================================================================

// Requirement: REQ-BRAIN-001 (Must)
// Acceptance: /setup returns Some(Command::Setup)
#[test]
fn test_parse_setup_command() {
    assert!(
        matches!(Command::parse("/setup"), Some(Command::Setup)),
        "/setup must parse to Command::Setup"
    );
}

// Requirement: REQ-BRAIN-001 (Must)
// Acceptance: /setup with description returns Some(Command::Setup)
#[test]
fn test_parse_setup_command_with_description() {
    assert!(
        matches!(Command::parse("/setup I'm a realtor"), Some(Command::Setup)),
        "/setup with description must parse to Command::Setup"
    );
}

// Requirement: REQ-BRAIN-001 (Must)
// Acceptance: /setup with long description returns Some(Command::Setup)
#[test]
fn test_parse_setup_command_with_long_description() {
    assert!(
        matches!(
            Command::parse("/setup I'm a realtor in Lisbon, residential properties"),
            Some(Command::Setup)
        ),
        "/setup with long description must parse to Command::Setup"
    );
}

// Requirement: REQ-BRAIN-001 (Must)
// Acceptance: /setup@botname suffix stripped, returns Some(Command::Setup)
#[test]
fn test_parse_setup_command_with_botname_suffix() {
    assert!(
        matches!(
            Command::parse("/setup@omega_bot I'm a realtor"),
            Some(Command::Setup)
        ),
        "/setup@omega_bot must parse to Command::Setup (botname stripped)"
    );
}

// Requirement: REQ-BRAIN-001 (Must)
// Acceptance: /setup@botname with no text returns Some(Command::Setup)
#[test]
fn test_parse_setup_command_with_botname_no_text() {
    assert!(
        matches!(Command::parse("/setup@omega_bot"), Some(Command::Setup)),
        "/setup@omega_bot with no text must still parse to Command::Setup"
    );
}

// Requirement: REQ-BRAIN-001 (Must)
// Acceptance: /settings does NOT match /setup (no false match on prefix collision)
#[test]
fn test_parse_settings_does_not_match_setup() {
    assert!(
        Command::parse("/settings").is_none(),
        "/settings must NOT match /setup -- exact match required"
    );
}

// Requirement: REQ-BRAIN-001 (Must)
// Acceptance: /setUp (wrong case) does NOT match
#[test]
fn test_parse_setup_case_sensitive() {
    assert!(
        Command::parse("/Setup").is_none(),
        "/Setup (wrong case) must NOT match -- commands are case-sensitive"
    );
}

// Requirement: REQ-BRAIN-001 (Must)
// Edge case: /setup with unicode description
#[test]
fn test_parse_setup_command_unicode_description() {
    assert!(
        matches!(
            Command::parse("/setup Soy agente inmobiliario en Lisboa"),
            Some(Command::Setup)
        ),
        "/setup with Spanish text must parse to Command::Setup"
    );
}

// Requirement: REQ-BRAIN-001 (Must)
// Edge case: /setup with emoji description
#[test]
fn test_parse_setup_command_emoji_description() {
    assert!(
        matches!(
            Command::parse("/setup I sell houses \u{1f3e0}\u{1f3e1}"),
            Some(Command::Setup)
        ),
        "/setup with emoji text must parse to Command::Setup"
    );
}

// Requirement: REQ-BRAIN-001 (Must)
// Acceptance: /setup in test_parse_all_commands (comprehensive registry check)
// This test will need the existing test_parse_all_commands to include Setup.
// The developer must add it to the existing test. This test is standalone validation.
#[test]
fn test_parse_setup_registered_in_command_enum() {
    // Verify /setup is registered and does not break other commands.
    assert!(matches!(Command::parse("/setup"), Some(Command::Setup)));
    assert!(matches!(Command::parse("/status"), Some(Command::Status)));
    assert!(matches!(Command::parse("/help"), Some(Command::Help)));
    assert!(matches!(Command::parse("/forget"), Some(Command::Forget)));
    assert!(Command::parse("/unknown").is_none());
}

// Requirement: REQ-BRAIN-001 (Must)
// Acceptance: help text includes /setup
#[test]
fn test_help_includes_setup() {
    let result = status::handle_help("English");
    assert!(
        result.contains("/setup"),
        "help must list /setup command: {result}"
    );
}
