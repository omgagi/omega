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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
        String::new(),
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
        .build_context(&msg, "Base rules", &needs, None)
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
        .build_context(&msg, "Base rules", &needs, None)
        .await
        .unwrap();
    assert!(
        ctx2.system_prompt.contains("/help"),
        "after learning name, should show stage 1 /help hint"
    );

    // Third message: stage already at 1, no new transition → no hint.
    let ctx3 = store
        .build_context(&msg, "Base rules", &needs, None)
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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
            "",
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

// --- Project-scoped learning tests ---

#[tokio::test]
async fn test_outcomes_project_isolation() {
    let store = test_store().await;

    // Store general outcome.
    store
        .store_outcome(
            "user1",
            "communication",
            1,
            "Be concise",
            "conversation",
            "",
        )
        .await
        .unwrap();
    // Store project-scoped outcome.
    store
        .store_outcome(
            "user1",
            "trading",
            1,
            "Check volume",
            "conversation",
            "omega-trader",
        )
        .await
        .unwrap();

    // No project filter → returns all.
    let all = store.get_recent_outcomes("user1", 10, None).await.unwrap();
    assert_eq!(all.len(), 2);

    // Project filter → returns only that project.
    let trading = store
        .get_recent_outcomes("user1", 10, Some("omega-trader"))
        .await
        .unwrap();
    assert_eq!(trading.len(), 1);
    assert_eq!(trading[0].1, "trading");

    // General filter → returns only general.
    let general = store
        .get_recent_outcomes("user1", 10, Some(""))
        .await
        .unwrap();
    assert_eq!(general.len(), 1);
    assert_eq!(general[0].1, "communication");
}

#[tokio::test]
async fn test_lessons_project_layering() {
    let store = test_store().await;

    // Store general lesson.
    store
        .store_lesson("user1", "communication", "Be concise", "")
        .await
        .unwrap();
    // Store project-scoped lesson.
    store
        .store_lesson("user1", "risk", "Never risk more than 2%", "omega-trader")
        .await
        .unwrap();

    // No project → general only.
    let general = store.get_lessons("user1", None).await.unwrap();
    assert_eq!(general.len(), 1);
    assert_eq!(general[0].0, "communication");
    assert_eq!(general[0].2, ""); // general

    // With project → project-specific FIRST, then general.
    let layered = store
        .get_lessons("user1", Some("omega-trader"))
        .await
        .unwrap();
    assert_eq!(layered.len(), 2);
    assert_eq!(
        layered[0].2, "omega-trader",
        "project lesson should come first"
    );
    assert_eq!(layered[1].2, "", "general lesson should come second");
}

#[tokio::test]
async fn test_lessons_project_separate() {
    let store = test_store().await;

    // Same domain, different projects → separate lessons.
    store
        .store_lesson("user1", "risk", "General risk rule", "")
        .await
        .unwrap();
    store
        .store_lesson("user1", "risk", "Trading risk rule", "omega-trader")
        .await
        .unwrap();

    let all_lessons = store
        .get_lessons("user1", Some("omega-trader"))
        .await
        .unwrap();
    assert_eq!(
        all_lessons.len(),
        2,
        "same domain, different projects = separate"
    );

    // New distinct rule text → creates a new row (multi-lesson).
    store
        .store_lesson("user1", "risk", "Updated trading risk", "omega-trader")
        .await
        .unwrap();
    let updated = store
        .get_lessons("user1", Some("omega-trader"))
        .await
        .unwrap();
    assert_eq!(
        updated.len(),
        3,
        "different rule text creates new row (multi-lesson)"
    );
    let trading_rules: Vec<&str> = updated
        .iter()
        .filter(|l| l.2 == "omega-trader")
        .map(|l| l.1.as_str())
        .collect();
    assert!(trading_rules.contains(&"Trading risk rule"));
    assert!(trading_rules.contains(&"Updated trading risk"));
}

#[tokio::test]
async fn test_lessons_multi_per_domain() {
    let store = test_store().await;

    // Store 3 different rules under the same (sender, domain, project).
    store
        .store_lesson("user1", "trading", "Always set stop-losses", "")
        .await
        .unwrap();
    store
        .store_lesson("user1", "trading", "Never risk more than 2%", "")
        .await
        .unwrap();
    store
        .store_lesson("user1", "trading", "Check volume before entry", "")
        .await
        .unwrap();

    let lessons = store.get_lessons("user1", None).await.unwrap();
    assert_eq!(lessons.len(), 3, "all 3 distinct rules should be stored");
    let rules: Vec<&str> = lessons.iter().map(|l| l.1.as_str()).collect();
    assert!(rules.contains(&"Always set stop-losses"));
    assert!(rules.contains(&"Never risk more than 2%"));
    assert!(rules.contains(&"Check volume before entry"));
}

#[tokio::test]
async fn test_lessons_content_dedup() {
    let store = test_store().await;

    // Store the same rule text twice.
    store
        .store_lesson("user1", "trading", "Always set stop-losses", "")
        .await
        .unwrap();
    store
        .store_lesson("user1", "trading", "Always set stop-losses", "")
        .await
        .unwrap();

    // Should be 1 row with occurrences = 2.
    let lessons = store.get_lessons("user1", None).await.unwrap();
    assert_eq!(
        lessons.len(),
        1,
        "duplicate rule text should not create new row"
    );

    // Verify occurrences was bumped.
    let (occurrences,): (i64,) = sqlx::query_as(
        "SELECT occurrences FROM lessons WHERE sender_id = 'user1' AND domain = 'trading'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(occurrences, 2, "occurrences should be 2 after dedup");
}

#[tokio::test]
async fn test_lessons_cap_enforcement() {
    let store = test_store().await;

    // Store 12 distinct lessons for the same (sender, domain, project).
    for i in 0..12 {
        store
            .store_lesson("user1", "trading", &format!("Rule number {i}"), "")
            .await
            .unwrap();
    }

    let lessons = store.get_lessons("user1", None).await.unwrap();
    assert_eq!(
        lessons.len(),
        10,
        "cap should prune to 10 per (sender, domain, project)"
    );

    // The oldest 2 (Rule number 0 and Rule number 1) should have been pruned.
    let rules: Vec<&str> = lessons.iter().map(|l| l.1.as_str()).collect();
    assert!(
        !rules.contains(&"Rule number 0"),
        "oldest rule should be pruned"
    );
    assert!(
        !rules.contains(&"Rule number 1"),
        "second-oldest rule should be pruned"
    );
    assert!(
        rules.contains(&"Rule number 11"),
        "newest rule should remain"
    );
}

#[tokio::test]
async fn test_lessons_cap_project_isolation() {
    let store = test_store().await;

    // Store 12 lessons in project A.
    for i in 0..12 {
        store
            .store_lesson(
                "user1",
                "trading",
                &format!("Project A rule {i}"),
                "project-a",
            )
            .await
            .unwrap();
    }

    // Store 3 lessons in project B (same domain).
    for i in 0..3 {
        store
            .store_lesson(
                "user1",
                "trading",
                &format!("Project B rule {i}"),
                "project-b",
            )
            .await
            .unwrap();
    }

    // Project A should be capped at 10.
    let a_lessons: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT domain, rule, project FROM lessons \
         WHERE sender_id = 'user1' AND project = 'project-a'",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(a_lessons.len(), 10, "project A capped at 10");

    // Project B should have all 3 — cap is per (sender, domain, project).
    let b_lessons: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT domain, rule, project FROM lessons \
         WHERE sender_id = 'user1' AND project = 'project-b'",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        b_lessons.len(),
        3,
        "project B unaffected by project A's cap"
    );
}

#[tokio::test]
async fn test_tasks_project_tag() {
    let store = test_store().await;

    // Create a general task.
    store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "General reminder",
            "2099-12-31T00:00:00",
            None,
            "reminder",
            "",
        )
        .await
        .unwrap();

    // Create a project-scoped task.
    store
        .create_task(
            "telegram",
            "user1",
            "chat1",
            "Check BTC",
            "2020-01-01T00:00:00",
            None,
            "action",
            "omega-trader",
        )
        .await
        .unwrap();

    // get_tasks_for_sender returns project field.
    let tasks = store.get_tasks_for_sender("user1").await.unwrap();
    assert_eq!(tasks.len(), 2);
    let general = tasks.iter().find(|t| t.1 == "General reminder").unwrap();
    assert_eq!(general.5, "");
    let project = tasks.iter().find(|t| t.1 == "Check BTC").unwrap();
    assert_eq!(project.5, "omega-trader");

    // get_due_tasks returns project field.
    let due = store.get_due_tasks().await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].4, "Check BTC");
    assert_eq!(due[0].7, "omega-trader");
}

#[tokio::test]
async fn test_get_all_lessons_project_filter() {
    let store = test_store().await;

    store
        .store_lesson("user1", "comms", "Be clear", "")
        .await
        .unwrap();
    store
        .store_lesson("user2", "trading", "Check volume", "omega-trader")
        .await
        .unwrap();

    // No filter → all lessons.
    let all = store.get_all_lessons(None).await.unwrap();
    assert_eq!(all.len(), 2);

    // Project filter → project + general.
    let filtered = store.get_all_lessons(Some("omega-trader")).await.unwrap();
    assert_eq!(filtered.len(), 2); // 1 project + 1 general
    assert_eq!(filtered[0].2, "omega-trader", "project first");
    assert_eq!(filtered[1].2, "", "general second");
}

#[tokio::test]
async fn test_get_all_facts_by_key() {
    let store = test_store().await;

    store
        .store_fact("user1", "active_project", "omega-trader")
        .await
        .unwrap();
    store
        .store_fact("user2", "active_project", "omega-trader")
        .await
        .unwrap();
    store.store_fact("user3", "name", "Charlie").await.unwrap();

    let active = store.get_all_facts_by_key("active_project").await.unwrap();
    assert_eq!(active.len(), 2);
    assert!(active.iter().all(|(_, v)| v == "omega-trader"));
}

#[tokio::test]
async fn test_migration_existing_data_gets_empty_project() {
    let store = test_store().await;

    // Data created via migration gets project = '' by default.
    store
        .store_outcome("user1", "test", 1, "lesson", "conversation", "")
        .await
        .unwrap();
    store
        .store_lesson("user1", "test", "rule", "")
        .await
        .unwrap();

    let outcomes = store.get_recent_outcomes("user1", 10, None).await.unwrap();
    assert_eq!(outcomes.len(), 1);

    let lessons = store.get_lessons("user1", None).await.unwrap();
    assert_eq!(lessons.len(), 1);
    assert_eq!(lessons[0].2, "", "default project should be empty string");
}

// --- Project session tests ---

#[tokio::test]
async fn test_store_and_get_session() {
    let store = test_store().await;

    // No session initially.
    let sid = store.get_session("telegram", "user1", "").await.unwrap();
    assert!(sid.is_none());

    // Store a session.
    store
        .store_session("telegram", "user1", "", "session-abc")
        .await
        .unwrap();
    let sid = store.get_session("telegram", "user1", "").await.unwrap();
    assert_eq!(sid, Some("session-abc".to_string()));
}

#[tokio::test]
async fn test_session_upsert() {
    let store = test_store().await;

    store
        .store_session("telegram", "user1", "", "session-1")
        .await
        .unwrap();
    store
        .store_session("telegram", "user1", "", "session-2")
        .await
        .unwrap();

    let sid = store.get_session("telegram", "user1", "").await.unwrap();
    assert_eq!(sid, Some("session-2".to_string()), "upsert should update");
}

#[tokio::test]
async fn test_session_project_isolation() {
    let store = test_store().await;

    // Store sessions for different projects.
    store
        .store_session("telegram", "user1", "", "personal-session")
        .await
        .unwrap();
    store
        .store_session("telegram", "user1", "trader", "trader-session")
        .await
        .unwrap();

    // Each project has its own session.
    let personal = store.get_session("telegram", "user1", "").await.unwrap();
    assert_eq!(personal, Some("personal-session".to_string()));

    let trader = store
        .get_session("telegram", "user1", "trader")
        .await
        .unwrap();
    assert_eq!(trader, Some("trader-session".to_string()));
}

#[tokio::test]
async fn test_clear_session() {
    let store = test_store().await;

    store
        .store_session("telegram", "user1", "trader", "session-x")
        .await
        .unwrap();
    store
        .clear_session("telegram", "user1", "trader")
        .await
        .unwrap();

    let sid = store
        .get_session("telegram", "user1", "trader")
        .await
        .unwrap();
    assert!(sid.is_none(), "session should be cleared");
}

#[tokio::test]
async fn test_clear_all_sessions_for_sender() {
    let store = test_store().await;

    store
        .store_session("telegram", "user1", "", "s1")
        .await
        .unwrap();
    store
        .store_session("telegram", "user1", "trader", "s2")
        .await
        .unwrap();
    store
        .store_session("whatsapp", "user1", "", "s3")
        .await
        .unwrap();

    store.clear_all_sessions_for_sender("user1").await.unwrap();

    assert!(store
        .get_session("telegram", "user1", "")
        .await
        .unwrap()
        .is_none());
    assert!(store
        .get_session("telegram", "user1", "trader")
        .await
        .unwrap()
        .is_none());
    assert!(store
        .get_session("whatsapp", "user1", "")
        .await
        .unwrap()
        .is_none());
}

// --- Project-scoped conversation tests ---

#[tokio::test]
async fn test_conversation_project_isolation() {
    let store = test_store().await;

    // Create conversations for different projects.
    let personal = store
        .get_or_create_conversation("telegram", "user1", "")
        .await
        .unwrap();
    let trader = store
        .get_or_create_conversation("telegram", "user1", "trader")
        .await
        .unwrap();

    assert_ne!(
        personal, trader,
        "different projects should get different conversations"
    );

    // Same project returns same conversation.
    let personal2 = store
        .get_or_create_conversation("telegram", "user1", "")
        .await
        .unwrap();
    assert_eq!(
        personal, personal2,
        "same project should return same conversation"
    );
}

#[tokio::test]
async fn test_close_current_conversation_project_scoped() {
    let store = test_store().await;

    // Create conversations for two projects.
    let _personal = store
        .get_or_create_conversation("telegram", "user1", "")
        .await
        .unwrap();
    let _trader = store
        .get_or_create_conversation("telegram", "user1", "trader")
        .await
        .unwrap();

    // Close only the trader project conversation.
    let closed = store
        .close_current_conversation("telegram", "user1", "trader")
        .await
        .unwrap();
    assert!(closed, "should close trader conversation");

    // Personal conversation should still be active.
    let personal_again = store
        .get_or_create_conversation("telegram", "user1", "")
        .await
        .unwrap();
    assert_eq!(
        personal_again, _personal,
        "personal conversation should still be active"
    );

    // Trader should get a new conversation.
    let trader_new = store
        .get_or_create_conversation("telegram", "user1", "trader")
        .await
        .unwrap();
    assert_ne!(
        trader_new, _trader,
        "closed trader should create new conversation"
    );
}

#[tokio::test]
async fn test_find_idle_conversations_includes_project() {
    let store = test_store().await;

    // Manually insert an old conversation with a project.
    sqlx::query(
        "INSERT INTO conversations (id, channel, sender_id, project, status, last_activity) \
         VALUES ('old1', 'telegram', 'user1', 'trader', 'active', datetime('now', '-3 hours'))",
    )
    .execute(store.pool())
    .await
    .unwrap();

    let idle = store.find_idle_conversations().await.unwrap();
    assert_eq!(idle.len(), 1);
    assert_eq!(idle[0].0, "old1");
    assert_eq!(idle[0].3, "trader", "should include project field");
}

// --- Multi-lesson edge case tests (migration 013) ---

#[tokio::test]
async fn test_lessons_dedup_reorders_by_updated_at() {
    let store = test_store().await;

    // Store 3 lessons, then set explicit timestamps so ordering is deterministic.
    store
        .store_lesson("user1", "cooking", "Rule A", "")
        .await
        .unwrap();
    store
        .store_lesson("user1", "cooking", "Rule B", "")
        .await
        .unwrap();
    store
        .store_lesson("user1", "cooking", "Rule C", "")
        .await
        .unwrap();

    // Manually set distinct timestamps: A oldest, C newest.
    sqlx::query("UPDATE lessons SET updated_at = '2026-01-01 00:00:00' WHERE rule = 'Rule A'")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE lessons SET updated_at = '2026-01-01 00:01:00' WHERE rule = 'Rule B'")
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE lessons SET updated_at = '2026-01-01 00:02:00' WHERE rule = 'Rule C'")
        .execute(store.pool())
        .await
        .unwrap();

    // Before reinforce: Rule C is newest (first in results).
    let before = store.get_lessons("user1", None).await.unwrap();
    assert_eq!(before[0].1, "Rule C", "newest should be first");
    assert_eq!(before[2].1, "Rule A", "oldest should be last");

    // Reinforce "Rule A" — dedup bumps updated_at to now (well past 2026-01-01).
    store
        .store_lesson("user1", "cooking", "Rule A", "")
        .await
        .unwrap();

    // After reinforce: Rule A should float to the top.
    let after = store.get_lessons("user1", None).await.unwrap();
    assert_eq!(after.len(), 3, "dedup should not create a 4th row");
    assert_eq!(
        after[0].1, "Rule A",
        "reinforced lesson should be first (most recent updated_at)"
    );
}

#[tokio::test]
async fn test_lessons_reinforced_survives_cap() {
    let store = test_store().await;

    // Store 10 lessons with explicit timestamps so ordering is deterministic.
    for i in 0..10 {
        store
            .store_lesson("user1", "trading", &format!("Rule {i}"), "")
            .await
            .unwrap();
        sqlx::query(&format!(
            "UPDATE lessons SET updated_at = '2026-01-01 00:{:02}:00' WHERE rule = 'Rule {}'",
            i, i
        ))
        .execute(store.pool())
        .await
        .unwrap();
    }

    // Reinforce the oldest ("Rule 0") — bumps its updated_at to now (past all others).
    store
        .store_lesson("user1", "trading", "Rule 0", "")
        .await
        .unwrap();

    // Now add an 11th distinct rule — cap prunes the oldest (which is no longer "Rule 0").
    store
        .store_lesson("user1", "trading", "Rule 10", "")
        .await
        .unwrap();

    let lessons = store.get_lessons("user1", None).await.unwrap();
    assert_eq!(lessons.len(), 10, "cap should keep 10");
    let rules: Vec<&str> = lessons.iter().map(|l| l.1.as_str()).collect();
    assert!(
        rules.contains(&"Rule 0"),
        "reinforced Rule 0 should survive cap (its updated_at was bumped)"
    );
    assert!(
        !rules.contains(&"Rule 1"),
        "Rule 1 (now oldest) should be pruned"
    );
    assert!(
        rules.contains(&"Rule 10"),
        "newest Rule 10 should be present"
    );
}

#[tokio::test]
async fn test_lessons_dedup_cross_project_isolation() {
    let store = test_store().await;

    // Same rule text in two different projects — should NOT dedup.
    store
        .store_lesson("user1", "risk", "Never risk more than 2%", "project-a")
        .await
        .unwrap();
    store
        .store_lesson("user1", "risk", "Never risk more than 2%", "project-b")
        .await
        .unwrap();

    // Each project should have its own row.
    let a: Vec<(String,)> = sqlx::query_as(
        "SELECT rule FROM lessons WHERE sender_id = 'user1' AND project = 'project-a'",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    let b: Vec<(String,)> = sqlx::query_as(
        "SELECT rule FROM lessons WHERE sender_id = 'user1' AND project = 'project-b'",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(a.len(), 1, "project-a should have its own row");
    assert_eq!(b.len(), 1, "project-b should have its own row");

    // Both should have occurrences = 1 (no dedup across projects).
    let (occ_a,): (i64,) = sqlx::query_as(
        "SELECT occurrences FROM lessons WHERE sender_id = 'user1' AND project = 'project-a'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    let (occ_b,): (i64,) = sqlx::query_as(
        "SELECT occurrences FROM lessons WHERE sender_id = 'user1' AND project = 'project-b'",
    )
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(occ_a, 1, "project-a occurrences should be 1");
    assert_eq!(occ_b, 1, "project-b occurrences should be 1");
}

#[tokio::test]
async fn test_lessons_dedup_cross_sender_isolation() {
    let store = test_store().await;

    // Same rule text from two different senders — should NOT dedup.
    store
        .store_lesson("user1", "cooking", "Salt the water", "")
        .await
        .unwrap();
    store
        .store_lesson("user2", "cooking", "Salt the water", "")
        .await
        .unwrap();

    let u1: Vec<(String,)> =
        sqlx::query_as("SELECT rule FROM lessons WHERE sender_id = 'user1' AND domain = 'cooking'")
            .fetch_all(store.pool())
            .await
            .unwrap();
    let u2: Vec<(String,)> =
        sqlx::query_as("SELECT rule FROM lessons WHERE sender_id = 'user2' AND domain = 'cooking'")
            .fetch_all(store.pool())
            .await
            .unwrap();
    assert_eq!(u1.len(), 1, "user1 should have its own row");
    assert_eq!(u2.len(), 1, "user2 should have its own row");
}

#[tokio::test]
async fn test_get_lessons_limit_50() {
    let store = test_store().await;

    // Store 55 lessons across multiple domains (cap is per domain, LIMIT 50 is on query).
    for i in 0..11 {
        for domain in &["a", "b", "c", "d", "e"] {
            store
                .store_lesson("user1", domain, &format!("Rule {domain}-{i}"), "")
                .await
                .unwrap();
        }
    }

    // Total in DB: 5 domains * 10 (capped) = 50. Exactly at limit.
    // Add 5 more in a 6th domain to push past 50 total.
    for i in 0..5 {
        store
            .store_lesson("user1", "f", &format!("Rule f-{i}"), "")
            .await
            .unwrap();
    }

    let lessons = store.get_lessons("user1", None).await.unwrap();
    assert!(
        lessons.len() <= 50,
        "get_lessons should return at most 50, got {}",
        lessons.len()
    );
}

#[tokio::test]
async fn test_get_all_lessons_limit_50() {
    let store = test_store().await;

    // Store lessons across multiple users and domains to exceed 50 total.
    for user in &["u1", "u2", "u3", "u4", "u5", "u6"] {
        for i in 0..10 {
            store
                .store_lesson(user, "general", &format!("Rule {user}-{i}"), "")
                .await
                .unwrap();
        }
    }

    // Total in DB: 6 users * 10 = 60. Query should cap at 50.
    let all = store.get_all_lessons(None).await.unwrap();
    assert!(
        all.len() <= 50,
        "get_all_lessons should return at most 50, got {}",
        all.len()
    );
}
