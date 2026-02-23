use super::*;

#[test]
fn test_all_keys_have_english_fallback() {
    let keys = [
        "status_header",
        "your_memory",
        "recent_conversations",
        "known_facts",
        "scheduled_tasks",
        "installed_skills",
        "projects_header",
        "commands_header",
        "uptime",
        "provider",
        "database",
        "conversations",
        "messages",
        "facts_label",
        "due",
        "language_label",
        "no_pending_tasks",
        "no_facts",
        "no_history",
        "no_skills",
        "no_projects",
        "conversation_cleared",
        "no_active_conversation",
        "task_cancelled",
        "no_matching_task",
        "task_updated",
        "cancel_usage",
        "personality_reset",
        "personality_already_default",
        "personality_default_prompt",
        "project_deactivated",
        "no_active_project",
        "available",
        "missing_deps",
        "not_set",
        "projects_footer",
        "personality_reset_hint",
        "project_deactivate_hint",
        "no_active_project_hint",
        "once",
        "task_confirmed",
        "task_similar_warning",
        "task_cancelled_confirmed",
        "task_updated_confirmed",
        "task_cancel_failed",
        "task_update_failed",
        "skill_improved",
        "skill_improve_failed",
        "bug_reported",
        "bug_report_failed",
        "heartbeat_header",
        "heartbeat_status",
        "heartbeat_interval",
        "heartbeat_minutes",
        "heartbeat_enabled",
        "heartbeat_disabled",
        "heartbeat_watchlist",
        "heartbeat_no_watchlist",
        "help_heartbeat",
    ];
    for key in keys {
        let val = t(key, "English");
        assert_ne!(val, "???", "key '{key}' should have English fallback");
    }
}

#[test]
fn test_all_languages_have_sample_translations() {
    let langs = [
        "Spanish",
        "Portuguese",
        "French",
        "German",
        "Italian",
        "Dutch",
        "Russian",
    ];
    // Use keys that genuinely differ across all 8 languages.
    let sample_keys = [
        "conversation_cleared",
        "no_pending_tasks",
        "your_memory",
        "known_facts",
    ];
    for lang in langs {
        for key in sample_keys {
            let val = t(key, lang);
            assert_ne!(
                val,
                t(key, "English"),
                "key '{key}' in {lang} should differ from English"
            );
        }
    }
}

#[test]
fn test_unknown_language_falls_back_to_english() {
    assert_eq!(t("status_header", "Klingon"), t("status_header", "English"));
}

#[test]
fn test_unknown_key_returns_placeholder() {
    assert_eq!(t("nonexistent_key", "English"), "???");
}

#[test]
fn test_format_helpers() {
    // language_set
    assert!(language_set("Spanish", "French").contains("French"));
    assert!(language_set("English", "German").contains("German"));

    // personality_updated
    assert!(personality_updated("English", "be casual").contains("be casual"));

    // purge_result
    assert!(purge_result("English", 5, "a, b").contains("5"));

    // project_activated
    assert!(project_activated("Spanish", "test").contains("test"));

    // project_not_found
    assert!(project_not_found("English", "xyz").contains("xyz"));

    // active_project
    assert!(active_project("English", "omega").contains("omega"));

    // tasks_confirmed
    assert!(tasks_confirmed("English", 3).contains("3 tasks"));
    assert!(tasks_confirmed("Spanish", 2).contains("2 tareas"));

    // task_save_failed
    assert!(task_save_failed("English", 1).contains("1 task"));
    assert!(task_save_failed("Spanish", 2).contains("2 tarea"));

    // tasks_cancelled_confirmed
    assert!(tasks_cancelled_confirmed("English", 3).contains("3 tasks"));
    assert!(tasks_cancelled_confirmed("Spanish", 2).contains("2 tareas"));

    // tasks_updated_confirmed
    assert!(tasks_updated_confirmed("English", 3).contains("3 tasks"));
    assert!(tasks_updated_confirmed("Spanish", 2).contains("2 tareas"));
}

#[test]
fn test_help_commands_all_languages() {
    let help_keys = [
        "help_status",
        "help_memory",
        "help_history",
        "help_facts",
        "help_forget",
        "help_tasks",
        "help_cancel",
        "help_language",
        "help_personality",
        "help_purge",
        "help_skills",
        "help_projects",
        "help_project",
        "help_whatsapp",
        "help_heartbeat",
        "help_help",
    ];
    // All help keys should contain the command name (slash prefix)
    for key in help_keys {
        let val = t(key, "English");
        let cmd = key.strip_prefix("help_").unwrap();
        assert!(
            val.contains(&format!("/{cmd}")),
            "help key '{key}' should contain '/{cmd}'"
        );
    }
}
