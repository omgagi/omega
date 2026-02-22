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
    let result = settings::handle_heartbeat(true, 30, "English");
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
    let result = settings::handle_heartbeat(false, 15, "English");
    assert!(
        result.contains("disabled"),
        "should show disabled: {result}"
    );
}

#[test]
fn test_heartbeat_localized() {
    let result = settings::handle_heartbeat(true, 60, "Spanish");
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
fn test_help_includes_heartbeat() {
    let result = status::handle_help("English");
    assert!(
        result.contains("/heartbeat"),
        "help should list /heartbeat: {result}"
    );
}
