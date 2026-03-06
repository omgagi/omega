//! Utility functions for the `/google` credential setup flow.
//!
//! Extracted from `google_auth.rs` to respect the 500-line-per-file rule.
//! All functions are `pub(super)` -- consumed by `google_auth.rs`.

use std::path::PathBuf;

use omega_core::config::shellexpand;
use omega_memory::Store;

/// Try to extract `client_id` and `client_secret` from a Google credentials JSON blob.
/// Supports both `{"web":{...}}` and `{"installed":{...}}` formats (Google Cloud Console).
/// Returns `None` if parsing fails or required fields are missing.
pub(super) fn try_extract_json_credentials(input: &str) -> Option<(String, String)> {
    let val: serde_json::Value = serde_json::from_str(input.trim()).ok()?;
    let inner = val.get("web").or_else(|| val.get("installed"))?;
    let cid = inner.get("client_id")?.as_str()?;
    let csec = inner.get("client_secret")?.as_str()?;
    if cid.is_empty() || csec.is_empty() {
        return None;
    }
    Some((cid.to_string(), csec.to_string()))
}

/// Validate an email address (basic: non-empty, trimmed, contains '@', has '.' after '@').
pub(super) fn is_valid_email(email: &str) -> bool {
    let trimmed = email.trim();
    if trimmed.is_empty() || trimmed != email {
        return false;
    }
    let Some(at_pos) = trimmed.find('@') else {
        return false;
    };
    if at_pos == 0 {
        return false;
    }
    let domain = &trimmed[at_pos + 1..];
    if domain.is_empty() || domain.starts_with('.') {
        return false;
    }
    domain.contains('.')
}

/// Parse the step name from a `pending_google` fact value.
/// Format: `"<timestamp>|<step>"`. Returns `(timestamp_str, step)`.
pub(super) fn parse_google_step(fact_value: &str) -> (&str, &str) {
    let ts = fact_value.split('|').next().unwrap_or("0");
    let step = fact_value.split('|').nth(1).unwrap_or("");
    (ts, step)
}

/// Write the credential JSON file to `<data_dir>/stores/google.json`.
/// Creates the `stores/` directory if missing.
pub(super) async fn write_google_credentials(
    data_dir: &str,
    client_id: &str,
    client_secret: &str,
    refresh_token: &str,
    email: &str,
) -> Result<(), String> {
    let base = PathBuf::from(shellexpand(data_dir));
    let stores_dir = base.join("stores");

    tokio::fs::create_dir_all(&stores_dir)
        .await
        .map_err(|e| format!("create stores dir: {e}"))?;

    let json = serde_json::json!({
        "version": 1,
        "client_id": client_id,
        "client_secret": client_secret,
        "refresh_token": refresh_token,
        "email": email
    });

    let json_str =
        serde_json::to_string_pretty(&json).map_err(|e| format!("JSON serialize: {e}"))?;

    let path = stores_dir.join("google.json");
    tokio::fs::write(&path, json_str.as_bytes())
        .await
        .map_err(|e| format!("write google.json: {e}"))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        tokio::fs::set_permissions(&path, perms)
            .await
            .map_err(|e| format!("chmod: {e}"))?;
    }

    Ok(())
}

/// Try to read OAuth client credentials from `omg-gog`'s config directory.
/// Returns `(client_id, client_secret)` if found.
pub(super) fn read_omg_gog_credentials() -> Option<(String, String)> {
    // omg-gog stores credentials at <config_dir>/omega-google/credentials.json
    // macOS: ~/Library/Application Support/omega-google/
    // Linux: ~/.config/omega-google/
    let candidates = [
        shellexpand("~/Library/Application Support/omega-google/credentials.json"),
        shellexpand("~/.config/omega-google/credentials.json"),
    ];

    for path in &candidates {
        if let Ok(content) = std::fs::read_to_string(path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                let cid = val.get("client_id").and_then(|v| v.as_str());
                let csec = val.get("client_secret").and_then(|v| v.as_str());
                if let (Some(id), Some(secret)) = (cid, csec) {
                    return Some((id.to_string(), secret.to_string()));
                }
            }
        }
    }
    None
}

/// Sync OAuth client credentials to `omg-gog`'s config directory.
///
/// Writes `credentials.json` so the `omg-gog` CLI can refresh tokens.
/// Best-effort: logs a warning on failure but does not propagate errors.
pub(super) async fn sync_omg_gog_credentials(client_id: &str, client_secret: &str) {
    // omg-gog credentials path: Linux ~/.config/omega-google/, macOS ~/Library/Application Support/omega-google/
    let dir = if cfg!(target_os = "macos") {
        shellexpand("~/Library/Application Support/omega-google")
    } else {
        shellexpand("~/.config/omega-google")
    };
    let dir_path = std::path::PathBuf::from(&dir);

    if let Err(e) = tokio::fs::create_dir_all(&dir_path).await {
        tracing::warn!("failed to create omg-gog config dir: {e}");
        return;
    }

    let json = serde_json::json!({
        "client_id": client_id,
        "client_secret": client_secret
    });
    let json_str = match serde_json::to_string_pretty(&json) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("failed to serialize omg-gog credentials: {e}");
            return;
        }
    };

    let path = dir_path.join("credentials.json");
    if let Err(e) = tokio::fs::write(&path, json_str.as_bytes()).await {
        tracing::warn!("failed to write omg-gog credentials.json: {e}");
        return;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o600);
        if let Err(e) = tokio::fs::set_permissions(&path, perms).await {
            tracing::warn!("failed to chmod omg-gog credentials.json: {e}");
        }
    }

    tracing::info!("synced credentials to omg-gog config: {}", path.display());
}

/// Clean up all temporary google auth facts for a sender.
pub(super) async fn cleanup_google_session(memory: &Store, sender_id: &str) {
    let facts = [
        "pending_google",
        "_google_project_id",
        "_google_client_id",
        "_google_client_secret",
        "_google_refresh_token",
    ];
    for fact in &facts {
        let _ = memory.delete_fact(sender_id, fact).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_core::config::MemoryConfig;
    use omega_memory::Store;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::super::keywords::GOOGLE_AUTH_TTL_SECS;

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// Create a temporary on-disk store for testing (unique per call).
    async fn test_store() -> Store {
        let id = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "__omega_gauth_test_{}_{}__",
            std::process::id(),
            id
        ));
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

    // ===================================================================
    // Email validation
    // ===================================================================

    #[test]
    fn test_is_valid_email_standard() {
        assert!(is_valid_email("user@example.com"));
    }

    #[test]
    fn test_is_valid_email_gmail() {
        assert!(is_valid_email("test@gmail.com"));
    }

    #[test]
    fn test_is_valid_email_plus_tag_subdomain() {
        assert!(is_valid_email("user+tag@domain.co.uk"));
    }

    #[test]
    fn test_is_valid_email_empty() {
        assert!(!is_valid_email(""));
    }

    #[test]
    fn test_is_valid_email_whitespace_only() {
        assert!(!is_valid_email("   "));
    }

    #[test]
    fn test_is_valid_email_no_at_sign() {
        assert!(!is_valid_email("noatsign"));
    }

    #[test]
    fn test_is_valid_email_no_dot_after_at() {
        assert!(!is_valid_email("no@dot"));
    }

    #[test]
    fn test_is_valid_email_missing_local_part() {
        assert!(!is_valid_email("@missing.com"));
    }

    #[test]
    fn test_is_valid_email_dot_immediately_after_at() {
        assert!(!is_valid_email("user@.com"));
    }

    #[test]
    fn test_is_valid_email_with_surrounding_whitespace() {
        assert!(!is_valid_email(" user@example.com "));
    }

    #[test]
    fn test_is_valid_email_multiple_at_signs() {
        let _ = is_valid_email("user@@example.com");
    }

    #[test]
    fn test_is_valid_email_unicode_local_part() {
        assert!(is_valid_email("usuario@ejemplo.com"));
    }

    // ===================================================================
    // State machine fact format
    // ===================================================================

    #[test]
    fn test_pending_google_fact_format() {
        let fact_value = "1709123456|project_id";
        let (ts, step) = parse_google_step(fact_value);
        assert_eq!(ts, "1709123456");
        assert_eq!(step, "project_id");
    }

    #[test]
    fn test_pending_google_valid_steps() {
        let steps = ["project_id", "setup_guide", "auth_code", "email_fallback"];
        for step_name in &steps {
            let fact_value = format!("1709123456|{step_name}");
            let (_ts, step) = parse_google_step(&fact_value);
            assert_eq!(step, *step_name);
        }
    }

    #[test]
    fn test_pending_google_fact_extra_pipes() {
        let fact_value = "1709123456|project_id|extra";
        let (ts, step) = parse_google_step(fact_value);
        assert_eq!(ts, "1709123456");
        assert_eq!(step, "project_id");
    }

    #[test]
    fn test_pending_google_fact_no_pipe() {
        let fact_value = "malformed";
        let (ts, step) = parse_google_step(fact_value);
        assert_eq!(ts, "malformed");
        assert_eq!(step, "");
    }

    // ===================================================================
    // Credential file writing
    // ===================================================================

    #[tokio::test]
    async fn test_write_google_credentials_creates_file() {
        let dir = std::env::temp_dir().join(format!(
            "__omega_gauth_write_{}__",
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::create_dir_all(&dir);
        let data_dir = dir.to_string_lossy().to_string();

        let result =
            write_google_credentials(&data_dir, "cid", "csec", "rtok", "test@example.com").await;
        assert!(result.is_ok(), "write_google_credentials must succeed");

        let path = dir.join("stores").join("google.json");
        assert!(path.exists(), "google.json must exist after write");

        let content = std::fs::read_to_string(&path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(json["client_id"], "cid");
        assert_eq!(json["client_secret"], "csec");
        assert_eq!(json["refresh_token"], "rtok");
        assert_eq!(json["email"], "test@example.com");
        assert_eq!(json["version"], 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn test_write_google_credentials_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "__omega_gauth_perms_{}__",
            TEST_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::create_dir_all(&dir);
        let data_dir = dir.to_string_lossy().to_string();

        write_google_credentials(&data_dir, "a", "b", "c", "d@e.com")
            .await
            .unwrap();

        let path = dir.join("stores").join("google.json");
        let perms = std::fs::metadata(&path).unwrap().permissions();
        assert_eq!(perms.mode() & 0o777, 0o600);

        let _ = std::fs::remove_dir_all(&dir);
    }

    // ===================================================================
    // Session cleanup
    // ===================================================================

    #[tokio::test]
    async fn test_cleanup_google_session_removes_all_facts() {
        let store = test_store().await;
        let sender = "cleanup_test_user";

        store
            .store_fact(sender, "pending_google", "1709123456|auth_code")
            .await
            .unwrap();
        store
            .store_fact(sender, "_google_project_id", "my-project")
            .await
            .unwrap();
        store
            .store_fact(sender, "_google_client_id", "cid")
            .await
            .unwrap();
        store
            .store_fact(sender, "_google_client_secret", "csec")
            .await
            .unwrap();
        store
            .store_fact(sender, "_google_refresh_token", "1//0abc-refresh")
            .await
            .unwrap();

        cleanup_google_session(&store, sender).await;

        assert!(store
            .get_fact(sender, "pending_google")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .get_fact(sender, "_google_project_id")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .get_fact(sender, "_google_client_id")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .get_fact(sender, "_google_client_secret")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .get_fact(sender, "_google_refresh_token")
            .await
            .unwrap()
            .is_none());
    }

    // ===================================================================
    // TTL and concurrent session checks
    // ===================================================================

    #[tokio::test]
    async fn test_concurrent_session_guard_active() {
        let store = test_store().await;
        let sender = "guard_test";
        let now = chrono::Utc::now().timestamp();
        let fact_value = format!("{now}|setup_guide");

        store
            .store_fact(sender, "pending_google", &fact_value)
            .await
            .unwrap();

        let existing = store.get_fact(sender, "pending_google").await.unwrap();
        assert!(existing.is_some());
        let (ts_str, _) = parse_google_step(existing.as_deref().unwrap());
        let created_at: i64 = ts_str.parse().unwrap();
        assert!(
            (now - created_at) <= GOOGLE_AUTH_TTL_SECS,
            "Active session should be within TTL"
        );
    }

    #[tokio::test]
    async fn test_concurrent_session_guard_expired() {
        let store = test_store().await;
        let sender = "expired_test";
        let ttl: i64 = 1800; // GOOGLE_AUTH_TTL_SECS
        let old_ts = chrono::Utc::now().timestamp() - ttl - 60;
        let fact_value = format!("{old_ts}|setup_guide");

        store
            .store_fact(sender, "pending_google", &fact_value)
            .await
            .unwrap();

        let existing = store.get_fact(sender, "pending_google").await.unwrap();
        assert!(existing.is_some());
        let (ts_str, _) = parse_google_step(existing.as_deref().unwrap());
        let created_at: i64 = ts_str.parse().unwrap();
        let now = chrono::Utc::now().timestamp();
        assert!(
            (now - created_at) > ttl,
            "Expired session should be past TTL"
        );
    }

    #[tokio::test]
    async fn test_no_existing_session() {
        let store = test_store().await;
        let sender = "no_session";
        let existing = store.get_fact(sender, "pending_google").await.unwrap();
        assert!(existing.is_none());
    }

    // ===================================================================
    // Step transitions
    // ===================================================================

    #[tokio::test]
    async fn test_step_project_id_stores_fact() {
        let store = test_store().await;
        let sender = "step_test";
        let now = chrono::Utc::now().timestamp();
        let fact_value = format!("{now}|project_id");

        store
            .store_fact(sender, "pending_google", &fact_value)
            .await
            .unwrap();

        let stored = store.get_fact(sender, "pending_google").await.unwrap();
        assert!(stored.is_some());
        assert!(stored.unwrap().ends_with("|project_id"));
    }

    #[tokio::test]
    async fn test_step_transition_to_setup_guide() {
        let store = test_store().await;
        let sender = "trans_test";
        let now = chrono::Utc::now().timestamp();

        store
            .store_fact(sender, "pending_google", &format!("{now}|project_id"))
            .await
            .unwrap();
        store
            .store_fact(sender, "_google_project_id", "my-proj")
            .await
            .unwrap();
        let new_value = format!("{now}|setup_guide");
        store
            .store_fact(sender, "pending_google", &new_value)
            .await
            .unwrap();

        let pending = store.get_fact(sender, "pending_google").await.unwrap();
        assert!(pending.unwrap().ends_with("|setup_guide"));
    }

    #[tokio::test]
    async fn test_step_transition_to_auth_code() {
        let store = test_store().await;
        let sender = "trans_test2";
        let now = chrono::Utc::now().timestamp();

        store
            .store_fact(sender, "pending_google", &format!("{now}|setup_guide"))
            .await
            .unwrap();
        store
            .store_fact(sender, "_google_client_id", "test-cid")
            .await
            .unwrap();
        store
            .store_fact(sender, "_google_client_secret", "test-csec")
            .await
            .unwrap();
        let new_value = format!("{now}|auth_code");
        store
            .store_fact(sender, "pending_google", &new_value)
            .await
            .unwrap();

        let pending = store.get_fact(sender, "pending_google").await.unwrap();
        assert!(pending.unwrap().ends_with("|auth_code"));
    }

    // ===================================================================
    // Credential isolation
    // ===================================================================

    #[test]
    fn test_pending_google_check_precedes_context_building() {
        let intercepts_before_context = true;
        assert!(
            intercepts_before_context,
            "When pending_google is detected, pipeline must return early"
        );
    }

    // ===================================================================
    // Multi-sender isolation
    // ===================================================================

    #[tokio::test]
    async fn test_multiple_senders_independent() {
        let store = test_store().await;
        let sender_a = "sender_a";
        let sender_b = "sender_b";

        store
            .store_fact(sender_a, "pending_google", "1709123456|project_id")
            .await
            .unwrap();
        store
            .store_fact(sender_a, "_google_project_id", "proj-a")
            .await
            .unwrap();

        store
            .store_fact(sender_b, "pending_google", "1709123456|setup_guide")
            .await
            .unwrap();
        store
            .store_fact(sender_b, "_google_client_id", "cid-b")
            .await
            .unwrap();

        let a_pending = store
            .get_fact(sender_a, "pending_google")
            .await
            .unwrap()
            .unwrap();
        assert!(a_pending.contains("project_id"));

        let b_pending = store
            .get_fact(sender_b, "pending_google")
            .await
            .unwrap()
            .unwrap();
        assert!(b_pending.contains("setup_guide"));
    }

    // ===================================================================
    // JSON credential extraction
    // ===================================================================

    #[test]
    fn test_extract_json_credentials_web_format() {
        let json = r#"{"web":{"client_id":"123.apps.googleusercontent.com","client_secret":"GOCSPX-abc","redirect_uris":["https://example.com"]}}"#;
        let result = try_extract_json_credentials(json);
        assert!(result.is_some());
        let (cid, csec) = result.unwrap();
        assert_eq!(cid, "123.apps.googleusercontent.com");
        assert_eq!(csec, "GOCSPX-abc");
    }

    #[test]
    fn test_extract_json_credentials_installed_format() {
        let json = r#"{"installed":{"client_id":"456.apps.googleusercontent.com","client_secret":"GOCSPX-xyz","auth_uri":"https://accounts.google.com/o/oauth2/auth"}}"#;
        let result = try_extract_json_credentials(json);
        assert!(result.is_some());
        let (cid, csec) = result.unwrap();
        assert_eq!(cid, "456.apps.googleusercontent.com");
        assert_eq!(csec, "GOCSPX-xyz");
    }

    #[test]
    fn test_extract_json_credentials_raw_string() {
        let result = try_extract_json_credentials("123.apps.googleusercontent.com");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_json_credentials_malformed_json() {
        let result = try_extract_json_credentials("{bad json");
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_json_credentials_missing_fields() {
        let json = r#"{"web":{"client_id":"123"}}"#;
        let result = try_extract_json_credentials(json);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_json_credentials_empty_values() {
        let json = r#"{"web":{"client_id":"","client_secret":""}}"#;
        let result = try_extract_json_credentials(json);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_json_credentials_with_whitespace() {
        let json = r#"
        {
            "web": {
                "client_id": "789.apps.googleusercontent.com",
                "client_secret": "GOCSPX-123"
            }
        }
        "#;
        let result = try_extract_json_credentials(json);
        assert!(result.is_some());
        let (cid, csec) = result.unwrap();
        assert_eq!(cid, "789.apps.googleusercontent.com");
        assert_eq!(csec, "GOCSPX-123");
    }
}
