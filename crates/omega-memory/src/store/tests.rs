use super::context::format_user_profile;
use super::tasks::{descriptions_are_similar, normalize_due_at};
use super::Store;
use omega_core::config::MemoryConfig;
use omega_core::context::ContextNeeds;
use omega_core::message::IncomingMessage;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;

/// Create an in-memory store for testing.
async fn test_store() -> Store {
    let _config = MemoryConfig {
        backend: "sqlite".to_string(),
        db_path: ":memory:".to_string(),
        max_context_messages: 10,
    };
    // For in-memory, we need to bypass shellexpand.
    let opts = SqliteConnectOptions::from_str("sqlite::memory:")
        .unwrap()
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();
    Store::run_migrations(&pool).await.unwrap();
    Store {
        pool,
        max_context_messages: 10,
    }
}

#[tokio::test]
async fn test_create_and_get_tasks() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Call John",
            "2026-12-31T15:00:00",
            None,
            "reminder",
        )
        .await
        .unwrap();
    assert!(!id.is_empty());

    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].1, "Call John");
    assert_eq!(tasks[0].2, "2026-12-31 15:00:00");
    assert!(tasks[0].3.is_none());
    assert_eq!(tasks[0].4, "reminder");
}

#[tokio::test]
async fn test_get_due_tasks() {
    let store = test_store().await;
    // Create a task in the past (due now).
    store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Past task",
            "2020-01-01T00:00:00",
            None,
            "reminder",
        )
        .await
        .unwrap();
    // Create a task in the future.
    store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Future task",
            "2099-12-31T23:59:59",
            None,
            "reminder",
        )
        .await
        .unwrap();

    let due = store.get_due_tasks().await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].4, "Past task");
    assert_eq!(due[0].6, "reminder");
}

#[tokio::test]
async fn test_complete_one_shot() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "One shot",
            "2020-01-01T00:00:00",
            None,
            "reminder",
        )
        .await
        .unwrap();

    store.complete_task(&id, None).await.unwrap();

    // Should no longer appear in pending.
    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert!(tasks.is_empty());

    // Should not appear in due tasks either.
    let due = store.get_due_tasks().await.unwrap();
    assert!(due.is_empty());
}

#[tokio::test]
async fn test_complete_recurring() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Daily standup",
            "2020-01-01T09:00:00",
            Some("daily"),
            "reminder",
        )
        .await
        .unwrap();

    store.complete_task(&id, Some("daily")).await.unwrap();

    // Task should still be pending but with advanced due_at.
    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].2, "2020-01-02 09:00:00"); // Advanced by 1 day
}

#[tokio::test]
async fn test_cancel_task() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Cancel me",
            "2099-12-31T00:00:00",
            None,
            "reminder",
        )
        .await
        .unwrap();

    let prefix = &id[..8];
    let cancelled = store.cancel_task(prefix, "user1").await.unwrap();
    assert!(cancelled);

    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert!(tasks.is_empty());
}

#[tokio::test]
async fn test_cancel_task_wrong_sender() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "My task",
            "2099-12-31T00:00:00",
            None,
            "reminder",
        )
        .await
        .unwrap();

    let prefix = &id[..8];
    let cancelled = store.cancel_task(prefix, "user2").await.unwrap();
    assert!(!cancelled);

    // Task still exists.
    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert_eq!(tasks.len(), 1);
}

#[tokio::test]
async fn test_update_task_description() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Old desc",
            "2099-12-31T00:00:00",
            None,
            "reminder",
        )
        .await
        .unwrap();

    let prefix = &id[..8];
    let updated = store
        .update_task(prefix, "user1", Some("New desc"), None, None)
        .await
        .unwrap();
    assert!(updated);

    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert_eq!(tasks[0].1, "New desc");
}

#[tokio::test]
async fn test_update_task_repeat() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "My task",
            "2099-12-31T00:00:00",
            Some("once"),
            "reminder",
        )
        .await
        .unwrap();

    let prefix = &id[..8];
    let updated = store
        .update_task(prefix, "user1", None, None, Some("daily"))
        .await
        .unwrap();
    assert!(updated);

    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert_eq!(tasks[0].3, Some("daily".to_string()));
}

#[tokio::test]
async fn test_update_task_wrong_sender() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "My task",
            "2099-12-31T00:00:00",
            None,
            "reminder",
        )
        .await
        .unwrap();

    let prefix = &id[..8];
    let updated = store
        .update_task(prefix, "user2", Some("Hacked"), None, None)
        .await
        .unwrap();
    assert!(!updated);

    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert_eq!(tasks[0].1, "My task");
}

#[tokio::test]
async fn test_update_task_no_fields() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "My task",
            "2099-12-31T00:00:00",
            None,
            "reminder",
        )
        .await
        .unwrap();

    let prefix = &id[..8];
    let updated = store
        .update_task(prefix, "user1", None, None, None)
        .await
        .unwrap();
    assert!(!updated);
}

#[tokio::test]
async fn test_create_task_with_action_type() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Check BTC price",
            "2026-12-31T14:00:00",
            Some("daily"),
            "action",
        )
        .await
        .unwrap();
    assert!(!id.is_empty());

    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].1, "Check BTC price");
    assert_eq!(tasks[0].4, "action");
}

#[tokio::test]
async fn test_get_due_tasks_returns_task_type() {
    let store = test_store().await;
    store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Reminder task",
            "2020-01-01T00:00:00",
            None,
            "reminder",
        )
        .await
        .unwrap();
    store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Action task",
            "2020-01-01T00:00:00",
            None,
            "action",
        )
        .await
        .unwrap();

    let due = store.get_due_tasks().await.unwrap();
    assert_eq!(due.len(), 2);
    let reminder = due.iter().find(|t| t.4 == "Reminder task").unwrap();
    let action = due.iter().find(|t| t.4 == "Action task").unwrap();
    assert_eq!(reminder.6, "reminder");
    assert_eq!(action.6, "action");
}

#[tokio::test]
async fn test_create_task_dedup() {
    let store = test_store().await;
    let id1 = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Close all positions",
            "2026-02-20T14:30:00",
            None,
            "action",
        )
        .await
        .unwrap();

    // Same sender + description + due_at → returns existing ID.
    let id2 = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Close all positions",
            "2026-02-20T14:30:00",
            None,
            "action",
        )
        .await
        .unwrap();
    assert_eq!(id1, id2);

    // Only one task exists.
    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert_eq!(tasks.len(), 1);
}

#[tokio::test]
async fn test_get_fact() {
    let store = test_store().await;
    // Missing fact returns None.
    assert!(store.get_fact("user1", "color").await.unwrap().is_none());

    store.store_fact("user1", "color", "blue").await.unwrap();
    assert_eq!(
        store.get_fact("user1", "color").await.unwrap(),
        Some("blue".to_string())
    );
}

#[tokio::test]
async fn test_delete_fact() {
    let store = test_store().await;
    // Delete non-existent returns false.
    assert!(!store.delete_fact("user1", "color").await.unwrap());

    store.store_fact("user1", "color", "blue").await.unwrap();
    assert!(store.delete_fact("user1", "color").await.unwrap());
    assert!(store.get_fact("user1", "color").await.unwrap().is_none());
}

#[tokio::test]
async fn test_is_new_user() {
    let store = test_store().await;

    // New user — no welcomed fact yet.
    assert!(store.is_new_user("fresh_user").await.unwrap());

    // Store the welcomed fact.
    store
        .store_fact("fresh_user", "welcomed", "true")
        .await
        .unwrap();

    // No longer new.
    assert!(!store.is_new_user("fresh_user").await.unwrap());
}

#[tokio::test]
async fn test_get_all_facts() {
    let store = test_store().await;

    // Empty store returns empty vec.
    let facts = store.get_all_facts().await.unwrap();
    assert!(facts.is_empty());

    // Store some facts across different users.
    store.store_fact("user1", "name", "Alice").await.unwrap();
    store.store_fact("user2", "name", "Bob").await.unwrap();
    store.store_fact("user1", "timezone", "EST").await.unwrap();
    // Store a welcomed fact — should be excluded.
    store.store_fact("user1", "welcomed", "true").await.unwrap();

    let facts = store.get_all_facts().await.unwrap();
    assert_eq!(facts.len(), 3, "should exclude 'welcomed' facts");
    assert!(facts.iter().any(|(k, v)| k == "name" && v == "Alice"));
    assert!(facts.iter().any(|(k, v)| k == "name" && v == "Bob"));
    assert!(facts.iter().any(|(k, v)| k == "timezone" && v == "EST"));
}

#[tokio::test]
async fn test_get_all_recent_summaries() {
    let store = test_store().await;

    // Empty store returns empty vec.
    let summaries = store.get_all_recent_summaries(3).await.unwrap();
    assert!(summaries.is_empty());

    // Create a conversation, close it with a summary.
    sqlx::query(
        "INSERT INTO conversations (id, channel, sender_id, status, summary, last_activity, updated_at) \
         VALUES ('c1', 'telegram', 'user1', 'closed', 'Discussed project planning', datetime('now'), datetime('now'))",
    )
    .execute(store.pool())
    .await
    .unwrap();

    let summaries = store.get_all_recent_summaries(3).await.unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].0, "Discussed project planning");
}

// --- Limitation tests ---

#[tokio::test]
async fn test_store_limitation_new() {
    let store = test_store().await;
    let is_new = store
        .store_limitation("No email", "Cannot send emails", "Add SMTP")
        .await
        .unwrap();
    assert!(is_new, "first insert should return true");
}

#[tokio::test]
async fn test_store_limitation_duplicate() {
    let store = test_store().await;
    store
        .store_limitation("No email", "Cannot send emails", "Add SMTP")
        .await
        .unwrap();
    let is_new = store
        .store_limitation("No email", "Different desc", "Different plan")
        .await
        .unwrap();
    assert!(!is_new, "duplicate title should return false");
}

#[tokio::test]
async fn test_store_limitation_case_insensitive() {
    let store = test_store().await;
    store
        .store_limitation("No Email", "Cannot send emails", "Add SMTP")
        .await
        .unwrap();
    let is_new = store
        .store_limitation("no email", "Different desc", "Different plan")
        .await
        .unwrap();
    assert!(
        !is_new,
        "case-insensitive duplicate title should return false"
    );
}

#[tokio::test]
async fn test_get_open_limitations() {
    let store = test_store().await;
    store
        .store_limitation("No email", "Cannot send emails", "Add SMTP")
        .await
        .unwrap();
    store
        .store_limitation("No calendar", "Cannot access calendar", "Add Google Cal")
        .await
        .unwrap();

    let limitations = store.get_open_limitations().await.unwrap();
    assert_eq!(limitations.len(), 2);
    assert_eq!(limitations[0].0, "No email");
    assert_eq!(limitations[1].0, "No calendar");
}

// --- User profile tests ---

#[test]
fn test_user_profile_filters_system_facts() {
    let facts = vec![
        ("welcomed".to_string(), "true".to_string()),
        ("preferred_language".to_string(), "English".to_string()),
        ("active_project".to_string(), "omega".to_string()),
        ("name".to_string(), "Alice".to_string()),
    ];
    let profile = format_user_profile(&facts);
    assert!(profile.contains("name: Alice"));
    assert!(!profile.contains("welcomed"));
    assert!(!profile.contains("preferred_language"));
    assert!(!profile.contains("active_project"));
}

#[test]
fn test_user_profile_groups_identity_first() {
    let facts = vec![
        ("timezone".to_string(), "EST".to_string()),
        ("interests".to_string(), "chess".to_string()),
        ("name".to_string(), "Alice".to_string()),
        ("pronouns".to_string(), "she/her".to_string()),
        ("occupation".to_string(), "engineer".to_string()),
    ];
    let profile = format_user_profile(&facts);
    let lines: Vec<&str> = profile.lines().collect();
    assert_eq!(lines[0], "User profile:");
    // Identity keys (name, pronouns) should come before context keys (timezone, occupation).
    let name_pos = lines.iter().position(|l| l.contains("name:")).unwrap();
    let pronouns_pos = lines.iter().position(|l| l.contains("pronouns:")).unwrap();
    let timezone_pos = lines.iter().position(|l| l.contains("timezone:")).unwrap();
    let occupation_pos = lines
        .iter()
        .position(|l| l.contains("occupation:"))
        .unwrap();
    let interests_pos = lines.iter().position(|l| l.contains("interests:")).unwrap();
    assert!(name_pos < timezone_pos);
    assert!(pronouns_pos < timezone_pos);
    assert!(timezone_pos < interests_pos);
    assert!(occupation_pos < interests_pos);
}

#[test]
fn test_user_profile_empty_for_system_only() {
    let facts = vec![
        ("welcomed".to_string(), "true".to_string()),
        ("preferred_language".to_string(), "English".to_string()),
    ];
    let profile = format_user_profile(&facts);
    assert!(profile.is_empty());
}

// --- Onboarding hint tests ---

#[test]
fn test_build_system_prompt_shows_action_badge() {
    use super::context::build_system_prompt;
    let facts = vec![
        ("welcomed".to_string(), "true".to_string()),
        ("preferred_language".to_string(), "English".to_string()),
        ("name".to_string(), "Alice".to_string()),
        ("occupation".to_string(), "engineer".to_string()),
        ("timezone".to_string(), "EST".to_string()),
    ];
    let tasks = vec![(
        "abcd1234-0000".to_string(),
        "Check BTC price".to_string(),
        "2026-02-18T14:00:00".to_string(),
        Some("daily".to_string()),
        "action".to_string(),
    )];
    let prompt = build_system_prompt("Rules", &facts, &[], &[], &tasks, &[], &[], "English", None);
    assert!(
        prompt.contains("[action]"),
        "should show [action] badge for action tasks"
    );
}

#[test]
fn test_onboarding_stage0_first_conversation() {
    use super::context::build_system_prompt;
    let facts = vec![
        ("welcomed".to_string(), "true".to_string()),
        ("preferred_language".to_string(), "Spanish".to_string()),
    ];
    let prompt = build_system_prompt("Rules", &facts, &[], &[], &[], &[], &[], "Spanish", Some(0));
    assert!(
        prompt.contains("first conversation"),
        "stage 0 should include first-conversation intro"
    );
}

#[test]
fn test_onboarding_stage1_help_hint() {
    use super::context::build_system_prompt;
    let facts = vec![
        ("welcomed".to_string(), "true".to_string()),
        ("preferred_language".to_string(), "English".to_string()),
        ("name".to_string(), "Alice".to_string()),
    ];
    let prompt = build_system_prompt("Rules", &facts, &[], &[], &[], &[], &[], "English", Some(1));
    assert!(
        prompt.contains("/help"),
        "stage 1 should mention /help command"
    );
}

#[test]
fn test_onboarding_no_hint_when_none() {
    use super::context::build_system_prompt;
    let facts = vec![
        ("welcomed".to_string(), "true".to_string()),
        ("preferred_language".to_string(), "English".to_string()),
        ("name".to_string(), "Alice".to_string()),
        ("occupation".to_string(), "engineer".to_string()),
        ("timezone".to_string(), "EST".to_string()),
    ];
    let prompt = build_system_prompt("Rules", &facts, &[], &[], &[], &[], &[], "English", None);
    assert!(
        !prompt.contains("Onboarding hint"),
        "should NOT include onboarding hint when None"
    );
    assert!(
        !prompt.contains("first conversation"),
        "should NOT include first-conversation intro when None"
    );
}

// --- compute_onboarding_stage tests ---

#[test]
fn test_compute_onboarding_stage_sequential() {
    use super::context::compute_onboarding_stage;
    // Stage 0 → 1 when 1+ real facts.
    assert_eq!(compute_onboarding_stage(0, 1, false), 1);
    // Stage 0 stays at 0 with no facts.
    assert_eq!(compute_onboarding_stage(0, 0, false), 0);
    // Stage 1 → 2 when 3+ real facts.
    assert_eq!(compute_onboarding_stage(1, 3, false), 2);
    // Stage 1 stays with only 2.
    assert_eq!(compute_onboarding_stage(1, 2, false), 1);
    // Stage 2 → 3 when has_tasks.
    assert_eq!(compute_onboarding_stage(2, 3, true), 3);
    // Stage 2 stays without tasks.
    assert_eq!(compute_onboarding_stage(2, 3, false), 2);
    // Stage 3 → 4 when 5+ real facts.
    assert_eq!(compute_onboarding_stage(3, 5, true), 4);
    // Stage 3 stays with 4 facts.
    assert_eq!(compute_onboarding_stage(3, 4, true), 3);
    // Stage 4 → 5 always.
    assert_eq!(compute_onboarding_stage(4, 5, true), 5);
    // Stage 5 stays done.
    assert_eq!(compute_onboarding_stage(5, 10, true), 5);
}

#[test]
fn test_compute_onboarding_stage_no_skip() {
    use super::context::compute_onboarding_stage;
    // Even with many facts, can't skip from 0 to 2.
    assert_eq!(compute_onboarding_stage(0, 10, true), 1);
}

#[test]
fn test_onboarding_hint_text_contains_commands() {
    use super::context::onboarding_hint_text;
    // Stage 1 mentions /help.
    let hint1 = onboarding_hint_text(1, "English").unwrap();
    assert!(hint1.contains("/help"));
    // Stage 2 mentions /personality.
    let hint2 = onboarding_hint_text(2, "English").unwrap();
    assert!(hint2.contains("/personality"));
    // Stage 3 mentions /tasks.
    let hint3 = onboarding_hint_text(3, "English").unwrap();
    assert!(hint3.contains("/tasks"));
    // Stage 4 mentions /projects.
    let hint4 = onboarding_hint_text(4, "English").unwrap();
    assert!(hint4.contains("/projects"));
    // Stage 5 returns None.
    assert!(onboarding_hint_text(5, "English").is_none());
}

#[test]
fn test_onboarding_hint_text_includes_language() {
    use super::context::onboarding_hint_text;
    // Stage 0: should contain language name, no hardcoded '¡Hola'.
    let hint0 = onboarding_hint_text(0, "French").unwrap();
    assert!(
        hint0.contains("French"),
        "stage 0 should reference the language"
    );
    assert!(
        !hint0.contains("¡Hola"),
        "stage 0 should not have hardcoded Spanish greeting"
    );

    // Stages 1-4: should contain "Respond in {language}".
    for stage in 1..=4 {
        let hint = onboarding_hint_text(stage, "German").unwrap();
        assert!(
            hint.contains("Respond in German"),
            "stage {stage} should contain 'Respond in German'"
        );
    }
}

#[tokio::test]
async fn test_build_context_advances_onboarding_stage() {
    let store = test_store().await;
    let sender = "onboard_user";

    // First contact: no facts at all → should show stage 0 (first conversation).
    let msg = IncomingMessage {
        id: uuid::Uuid::new_v4(),
        channel: "telegram".to_string(),
        sender_id: sender.to_string(),
        sender_name: None,
        text: "hello".to_string(),
        timestamp: chrono::Utc::now(),
        reply_to: None,
        attachments: vec![],
        reply_target: Some("chat1".to_string()),
        is_group: false,
    };
    let needs = ContextNeeds::default();
    let ctx = store
        .build_context(&msg, "Base rules", &needs)
        .await
        .unwrap();
    assert!(
        ctx.system_prompt.contains("first conversation"),
        "first contact should trigger stage 0 intro"
    );

    // Store a real fact (simulating the AI learned the user's name).
    store.store_fact(sender, "welcomed", "true").await.unwrap();
    store.store_fact(sender, "name", "Alice").await.unwrap();

    // Second message: should advance to stage 1 and show /help hint.
    let ctx2 = store
        .build_context(&msg, "Base rules", &needs)
        .await
        .unwrap();
    assert!(
        ctx2.system_prompt.contains("/help"),
        "after learning name, should show stage 1 /help hint"
    );

    // Third message: stage already at 1, no new transition → no hint.
    let ctx3 = store
        .build_context(&msg, "Base rules", &needs)
        .await
        .unwrap();
    assert!(
        !ctx3.system_prompt.contains("Onboarding hint"),
        "no hint when stage hasn't changed"
    );
}

// --- User alias tests ---

#[tokio::test]
async fn test_resolve_sender_id_no_alias() {
    let store = test_store().await;
    // No alias → returns original.
    let resolved = store.resolve_sender_id("phone123").await.unwrap();
    assert_eq!(resolved, "phone123");
}

#[tokio::test]
async fn test_create_and_resolve_alias() {
    let store = test_store().await;
    store.create_alias("phone123", "telegram456").await.unwrap();
    let resolved = store.resolve_sender_id("phone123").await.unwrap();
    assert_eq!(resolved, "telegram456");
}

#[tokio::test]
async fn test_create_alias_idempotent() {
    let store = test_store().await;
    store.create_alias("phone123", "telegram456").await.unwrap();
    // Second insert is a no-op (INSERT OR IGNORE).
    store.create_alias("phone123", "telegram456").await.unwrap();
    let resolved = store.resolve_sender_id("phone123").await.unwrap();
    assert_eq!(resolved, "telegram456");
}

#[tokio::test]
async fn test_find_canonical_user() {
    let store = test_store().await;
    // No users → None.
    assert!(store
        .find_canonical_user("new_user")
        .await
        .unwrap()
        .is_none());

    // Add an existing welcomed user.
    store
        .store_fact("telegram456", "welcomed", "true")
        .await
        .unwrap();

    // find_canonical_user from a different sender → returns the existing one.
    let canonical = store
        .find_canonical_user("phone123")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(canonical, "telegram456");

    // Excluding the existing user → None.
    assert!(store
        .find_canonical_user("telegram456")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn test_alias_shares_facts() {
    let store = test_store().await;
    // Store facts under canonical ID.
    store
        .store_fact("telegram456", "name", "Alice")
        .await
        .unwrap();
    store
        .store_fact("telegram456", "welcomed", "true")
        .await
        .unwrap();

    // Create alias.
    store.create_alias("phone123", "telegram456").await.unwrap();

    // Resolve alias and read facts using canonical ID.
    let resolved = store.resolve_sender_id("phone123").await.unwrap();
    let facts = store.get_facts(&resolved).await.unwrap();
    assert!(facts.iter().any(|(k, v)| k == "name" && v == "Alice"));
}

#[test]
fn test_normalize_due_at_strips_z() {
    assert_eq!(
        normalize_due_at("2026-02-22T07:00:00Z"),
        "2026-02-22 07:00:00"
    );
}

#[test]
fn test_normalize_due_at_replaces_t() {
    assert_eq!(
        normalize_due_at("2026-02-22T07:00:00"),
        "2026-02-22 07:00:00"
    );
}

#[test]
fn test_normalize_due_at_already_normalized() {
    assert_eq!(
        normalize_due_at("2026-02-22 07:00:00"),
        "2026-02-22 07:00:00"
    );
}

#[test]
fn test_descriptions_similar_email_variants() {
    assert!(descriptions_are_similar(
        "Enviar email de amor diario a Adriana (adri_navega@hotmail.com)",
        "Enviar email de amor diario a Adriana (adri_navega@hotmail.com) — escribir un mensaje"
    ));
}

#[test]
fn test_descriptions_similar_hostinger() {
    assert!(descriptions_are_similar(
        "Cancel Hostinger plan — expires March 17",
        "Cancel Hostinger VPS — last reminder, expires TOMORROW"
    ));
}

#[test]
fn test_descriptions_different() {
    assert!(!descriptions_are_similar(
        "Send good morning message to the team",
        "Cancel Hostinger plan and subscription"
    ));
}

#[test]
fn test_descriptions_short_skipped() {
    // Short descriptions (< 3 significant words) skip fuzzy matching.
    assert!(!descriptions_are_similar("Reminder task", "Action task"));
}

#[tokio::test]
async fn test_create_task_fuzzy_dedup() {
    let store = test_store().await;

    // Create first task.
    let id1 = store
        .create_task(
            "telegram",
            "user1",
            "reply1",
            "Send daily email to Adriana",
            "2026-02-22 07:00:00",
            Some("daily"),
            "action",
        )
        .await
        .unwrap();

    // Same task with different datetime format — should dedup.
    let id2 = store
        .create_task(
            "telegram",
            "user1",
            "reply1",
            "Send daily email to Adriana",
            "2026-02-22T07:00:00Z",
            Some("daily"),
            "action",
        )
        .await
        .unwrap();
    assert_eq!(id1, id2, "exact dedup with normalized datetime");

    // Similar description, same time window — should fuzzy dedup.
    let id3 = store
        .create_task(
            "telegram",
            "user1",
            "reply1",
            "Send daily love email to Adriana via gmail",
            "2026-02-22 07:05:00",
            Some("daily"),
            "action",
        )
        .await
        .unwrap();
    assert_eq!(id1, id3, "fuzzy dedup: similar description within 30min");

    // Different sender — should NOT dedup.
    let id4 = store
        .create_task(
            "telegram",
            "user2",
            "reply2",
            "Send daily email to Adriana",
            "2026-02-22 07:00:00",
            Some("daily"),
            "action",
        )
        .await
        .unwrap();
    assert_ne!(id1, id4, "different sender should create new task");
}

#[tokio::test]
async fn test_fail_task_retries() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Send email",
            "2020-01-01T00:00:00",
            None,
            "action",
        )
        .await
        .unwrap();

    // First failure: should retry (retry_count 1 < max 3).
    let will_retry = store.fail_task(&id, "SMTP error", 3).await.unwrap();
    assert!(will_retry, "should retry on first failure");

    // Task is still pending (rescheduled).
    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert_eq!(tasks.len(), 1, "task should still be pending");

    // Second failure.
    let will_retry = store.fail_task(&id, "SMTP error again", 3).await.unwrap();
    assert!(will_retry, "should retry on second failure");

    // Third failure: permanently failed (retry_count 3 >= max 3).
    let will_retry = store.fail_task(&id, "SMTP final error", 3).await.unwrap();
    assert!(!will_retry, "should NOT retry after max retries");

    // Task is no longer pending.
    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert!(tasks.is_empty(), "failed task should not appear in pending");
}

#[tokio::test]
async fn test_fail_task_stores_error() {
    let store = test_store().await;
    let id = store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Check price",
            "2020-01-01T00:00:00",
            None,
            "action",
        )
        .await
        .unwrap();

    store.fail_task(&id, "connection refused", 3).await.unwrap();

    // Verify last_error is stored.
    let row: Option<(String, i64)> =
        sqlx::query_as("SELECT last_error, retry_count FROM scheduled_tasks WHERE id = ?")
            .bind(&id)
            .fetch_optional(store.pool())
            .await
            .unwrap();

    let (last_error, retry_count) = row.unwrap();
    assert_eq!(last_error, "connection refused");
    assert_eq!(retry_count, 1);
}
