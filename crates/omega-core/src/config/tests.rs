use super::*;

#[test]
fn test_timeout_config_default() {
    let cc = ClaudeCodeConfig::default();
    assert_eq!(cc.timeout_secs, 3600);
    assert_eq!(cc.max_resume_attempts, 5);
}

#[test]
fn test_timeout_config_from_toml() {
    let toml_str = r#"
        enabled = true
        max_turns = 10
        timeout_secs = 300
    "#;
    let cc: ClaudeCodeConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(cc.timeout_secs, 300);
}

#[test]
fn test_timeout_config_default_when_missing() {
    let toml_str = r#"
        enabled = true
        max_turns = 10
    "#;
    let cc: ClaudeCodeConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(cc.timeout_secs, 3600);
    assert_eq!(cc.max_resume_attempts, 5);
}

#[test]
fn test_install_bundled_prompts_creates_files() {
    let tmp = std::env::temp_dir().join("__omega_test_bundled_prompts__");
    let _ = std::fs::remove_dir_all(&tmp);

    install_bundled_prompts(tmp.to_str().unwrap());

    let prompt_path = tmp.join("prompts/SYSTEM_PROMPT.md");
    let welcome_path = tmp.join("prompts/WELCOME.toml");
    assert!(prompt_path.exists(), "SYSTEM_PROMPT.md should be deployed");
    assert!(welcome_path.exists(), "WELCOME.toml should be deployed");

    let prompt_content = std::fs::read_to_string(&prompt_path).unwrap();
    assert!(
        prompt_content.contains("## System"),
        "should contain System section"
    );
    assert!(
        prompt_content.contains("## Summarize"),
        "should contain Summarize section"
    );

    let welcome_content = std::fs::read_to_string(&welcome_path).unwrap();
    assert!(
        welcome_content.contains("[messages]"),
        "should contain messages table"
    );
    assert!(
        welcome_content.contains("English"),
        "should contain English key"
    );

    // Run again with custom content — should not overwrite.
    std::fs::write(&prompt_path, "custom prompt").unwrap();
    std::fs::write(&welcome_path, "custom welcome").unwrap();
    install_bundled_prompts(tmp.to_str().unwrap());
    assert_eq!(
        std::fs::read_to_string(&prompt_path).unwrap(),
        "custom prompt",
        "should not overwrite user edits to SYSTEM_PROMPT.md"
    );
    assert_eq!(
        std::fs::read_to_string(&welcome_path).unwrap(),
        "custom welcome",
        "should not overwrite user edits to WELCOME.toml"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_parse_identity_soul_system_sections() {
    let content = "## Identity\nI am OMEGA.\n\n## Soul\nBe helpful.\n\n## System\nRules here.";
    let prompts = Prompts::default();
    // Load from temp dir with custom content
    let tmp = std::env::temp_dir().join("__omega_test_parse_sections__");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("prompts")).unwrap();
    std::fs::write(tmp.join("prompts/SYSTEM_PROMPT.md"), content).unwrap();

    let loaded = Prompts::load(tmp.to_str().unwrap());
    assert_eq!(loaded.identity, "I am OMEGA.");
    assert_eq!(loaded.soul, "Be helpful.");
    assert_eq!(loaded.system, "Rules here.");
    // Verify defaults are still reasonable
    assert!(
        prompts.identity.contains("OMEGA"),
        "default identity should mention OMEGA"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_backward_compat_system_only() {
    // When a user's SYSTEM_PROMPT.md only has ## System, identity and soul
    // should keep their compiled defaults.
    let tmp = std::env::temp_dir().join("__omega_test_compat__");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("prompts")).unwrap();
    std::fs::write(
        tmp.join("prompts/SYSTEM_PROMPT.md"),
        "## System\nCustom rules only.",
    )
    .unwrap();

    let prompts = Prompts::load(tmp.to_str().unwrap());
    assert_eq!(prompts.system, "Custom rules only.");
    // Identity and soul should be defaults (not empty).
    assert!(
        prompts.identity.contains("OMEGA"),
        "identity should keep default"
    );
    assert!(
        prompts.soul.contains("quietly confident"),
        "soul should keep default"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_prompts_default_has_identity_soul() {
    let prompts = Prompts::default();
    assert!(
        prompts.identity.contains("OMEGA"),
        "default identity should mention OMEGA"
    );
    assert!(
        prompts.soul.contains("quietly confident"),
        "default soul should contain personality"
    );
    assert!(
        prompts.system.contains("outcome in plain language"),
        "default system should contain rules"
    );
}

#[test]
fn test_telegram_config_with_whisper() {
    let toml_str = r#"
        enabled = true
        bot_token = "tok:EN"
        allowed_users = [42]
        whisper_api_key = "sk-test123"
    "#;
    let cfg: TelegramConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.whisper_api_key.as_deref(), Some("sk-test123"));
}

#[test]
fn test_telegram_config_without_whisper() {
    let toml_str = r#"
        enabled = true
        bot_token = "tok:EN"
        allowed_users = [42]
    "#;
    let cfg: TelegramConfig = toml::from_str(toml_str).unwrap();
    assert!(cfg.whisper_api_key.is_none());
}

#[test]
fn test_gemini_config_defaults() {
    let toml_str = r#"
        enabled = true
        api_key = "AIza-test"
    "#;
    let cfg: GeminiConfig = toml::from_str(toml_str).unwrap();
    assert!(cfg.enabled);
    assert_eq!(cfg.api_key, "AIza-test");
    assert_eq!(cfg.model, "gemini-2.0-flash");
}

#[test]
fn test_whatsapp_config_with_whisper() {
    let toml_str = r#"
        enabled = true
        allowed_users = ["5511999887766"]
        whisper_api_key = "sk-wa-test"
    "#;
    let cfg: WhatsAppConfig = toml::from_str(toml_str).unwrap();
    assert_eq!(cfg.whisper_api_key.as_deref(), Some("sk-wa-test"));
    assert_eq!(cfg.allowed_users, vec!["5511999887766"]);
}

#[test]
fn test_whatsapp_config_without_whisper() {
    let toml_str = r#"
        enabled = true
        allowed_users = []
    "#;
    let cfg: WhatsAppConfig = toml::from_str(toml_str).unwrap();
    assert!(cfg.whisper_api_key.is_none());
}

#[test]
fn test_migrate_layout_moves_files() {
    let tmp = std::env::temp_dir().join("__omega_test_migrate__");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Create old flat files.
    std::fs::write(tmp.join("memory.db"), "db").unwrap();
    std::fs::write(tmp.join("omega.log"), "log").unwrap();
    std::fs::write(tmp.join("SYSTEM_PROMPT.md"), "prompt").unwrap();
    std::fs::write(tmp.join("WELCOME.toml"), "welcome").unwrap();
    std::fs::write(tmp.join("HEARTBEAT.md"), "hb").unwrap();

    migrate_layout(tmp.to_str().unwrap(), "/nonexistent/config.toml");

    // Old files should be gone.
    assert!(!tmp.join("memory.db").exists());
    assert!(!tmp.join("omega.log").exists());
    assert!(!tmp.join("SYSTEM_PROMPT.md").exists());
    assert!(!tmp.join("WELCOME.toml").exists());
    assert!(!tmp.join("HEARTBEAT.md").exists());

    // New files should exist with correct content.
    assert_eq!(
        std::fs::read_to_string(tmp.join("data/memory.db")).unwrap(),
        "db"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.join("logs/omega.log")).unwrap(),
        "log"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.join("prompts/SYSTEM_PROMPT.md")).unwrap(),
        "prompt"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.join("prompts/WELCOME.toml")).unwrap(),
        "welcome"
    );
    assert_eq!(
        std::fs::read_to_string(tmp.join("prompts/HEARTBEAT.md")).unwrap(),
        "hb"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_migrate_layout_idempotent() {
    let tmp = std::env::temp_dir().join("__omega_test_migrate_idem__");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // Create old file.
    std::fs::write(tmp.join("memory.db"), "old").unwrap();

    // First migration.
    migrate_layout(tmp.to_str().unwrap(), "/nonexistent/config.toml");
    assert_eq!(
        std::fs::read_to_string(tmp.join("data/memory.db")).unwrap(),
        "old"
    );

    // Second migration — should be a no-op (no source, dest exists).
    migrate_layout(tmp.to_str().unwrap(), "/nonexistent/config.toml");
    assert_eq!(
        std::fs::read_to_string(tmp.join("data/memory.db")).unwrap(),
        "old",
        "should not overwrite on re-run"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_migrate_layout_no_overwrite() {
    let tmp = std::env::temp_dir().join("__omega_test_migrate_noover__");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(tmp.join("data")).unwrap();

    // Pre-existing new file.
    std::fs::write(tmp.join("data/memory.db"), "new").unwrap();
    // Old file also present.
    std::fs::write(tmp.join("memory.db"), "old").unwrap();

    migrate_layout(tmp.to_str().unwrap(), "/nonexistent/config.toml");

    // New file should NOT be overwritten.
    assert_eq!(
        std::fs::read_to_string(tmp.join("data/memory.db")).unwrap(),
        "new"
    );
    // Old file should still be there (wasn't moved because dest exists).
    assert!(tmp.join("memory.db").exists());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_migrate_layout_patches_config() {
    let tmp = std::env::temp_dir().join("__omega_test_migrate_cfg__");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    let cfg_path = tmp.join("config.toml");
    std::fs::write(
        &cfg_path,
        "db_path = \"~/.omega/memory.db\"\nother = true\n",
    )
    .unwrap();

    migrate_layout(tmp.to_str().unwrap(), cfg_path.to_str().unwrap());

    let content = std::fs::read_to_string(&cfg_path).unwrap();
    assert!(
        content.contains("~/.omega/data/memory.db"),
        "should patch old default"
    );
    assert!(
        !content.contains("\"~/.omega/memory.db\""),
        "old default should be gone"
    );
    assert!(
        content.contains("other = true"),
        "should preserve other config"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn test_migrate_layout_fresh_install() {
    let tmp = std::env::temp_dir().join("__omega_test_migrate_fresh__");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();

    // No old files — just create subdirs.
    migrate_layout(tmp.to_str().unwrap(), "/nonexistent/config.toml");

    assert!(tmp.join("data").is_dir());
    assert!(tmp.join("logs").is_dir());
    assert!(tmp.join("prompts").is_dir());

    let _ = std::fs::remove_dir_all(&tmp);
}
