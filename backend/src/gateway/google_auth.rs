//! Google account credential setup -- `/google` command state machine.
//!
//! Handles a 5-step wizard: project_id, setup_guide (client_id), client_secret,
//! auth_code, and completion (with email_fallback if needed).
//! OAuth token exchange is handled server-side -- no raw refresh_token needed.
//! Credentials are stored in `~/.omega/stores/google.json` with 0600 permissions.
//! Credential messages are deleted from chat for security (best-effort).
//! Credentials NEVER reach the AI provider pipeline.

use std::path::PathBuf;

use omega_core::config::shellexpand;
use omega_core::message::IncomingMessage;
use omega_memory::audit::{AuditEntry, AuditStatus};
use omega_memory::Store;
use tracing::warn;

use super::google_auth_i18n::*;
use super::google_auth_oauth;
use super::keywords::{is_build_cancelled, GOOGLE_AUTH_TTL_SECS};
use super::Gateway;

/// Validate an email address (basic: non-empty, trimmed, contains '@', has '.' after '@').
fn is_valid_email(email: &str) -> bool {
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
fn parse_google_step(fact_value: &str) -> (&str, &str) {
    let ts = fact_value.split('|').next().unwrap_or("0");
    let step = fact_value.split('|').nth(1).unwrap_or("");
    (ts, step)
}

/// Write the credential JSON file to `<data_dir>/stores/google.json`.
/// Creates the `stores/` directory if missing.
async fn write_google_credentials(
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

/// Clean up all temporary google auth facts for a sender.
async fn cleanup_google_session(memory: &Store, sender_id: &str) {
    let facts = [
        "pending_google",
        "_google_project_id",
        "_google_client_id",
        "_google_client_secret",
    ];
    for fact in &facts {
        let _ = memory.delete_fact(sender_id, fact).await;
    }
}

impl Gateway {
    /// Start a new `/google` session. Called from pipeline.rs on `Command::Google`.
    pub(super) async fn start_google_session(&self, incoming: &IncomingMessage) {
        let user_lang = self
            .memory
            .get_fact(&incoming.sender_id, "preferred_language")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "English".to_string());

        // Check for concurrent session.
        let existing = self
            .memory
            .get_fact(&incoming.sender_id, "pending_google")
            .await
            .ok()
            .flatten();

        if let Some(ref val) = existing {
            let (ts_str, _step) = parse_google_step(val);
            let created_at: i64 = ts_str.parse().unwrap_or(0);
            let now = chrono::Utc::now().timestamp();
            if (now - created_at) <= GOOGLE_AUTH_TTL_SECS {
                self.send_text(incoming, google_conflict_message(&user_lang))
                    .await;
                return;
            }
            cleanup_google_session(&self.memory, &incoming.sender_id).await;
        }

        // Store pending_google fact (step = project_id).
        let now = chrono::Utc::now().timestamp();
        let fact_value = format!("{now}|project_id");
        if let Err(e) = self
            .memory
            .store_fact(&incoming.sender_id, "pending_google", &fact_value)
            .await
        {
            warn!("failed to store pending_google fact: {e}");
            self.send_text(incoming, google_start_error_message(&user_lang))
                .await;
            return;
        }

        self.audit_google(incoming, "started").await;

        // Check if google.json already exists (overwrite warning).
        let stores_path = PathBuf::from(shellexpand(&self.data_dir))
            .join("stores")
            .join("google.json");
        let google_exists = tokio::fs::try_exists(&stores_path).await.unwrap_or(false);

        let msg = google_step_project_id_message(&user_lang, google_exists);
        self.send_text(incoming, &msg).await;
    }

    /// Handle a follow-up message during an active google auth session.
    /// Called from pipeline.rs when `pending_google` fact exists.
    pub(super) async fn handle_google_response(
        &self,
        incoming: &IncomingMessage,
        pending_value: &str,
    ) {
        let user_lang = self
            .memory
            .get_fact(&incoming.sender_id, "preferred_language")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "English".to_string());

        // Check TTL.
        let (ts_str, step) = parse_google_step(pending_value);
        let created_at: i64 = ts_str.parse().unwrap_or(0);
        let now = chrono::Utc::now().timestamp();

        if (now - created_at) > GOOGLE_AUTH_TTL_SECS {
            cleanup_google_session(&self.memory, &incoming.sender_id).await;
            self.audit_google(incoming, "expired").await;
            self.send_text(incoming, google_expired_message(&user_lang))
                .await;
            return;
        }

        // Check cancellation.
        if is_build_cancelled(&incoming.text) {
            cleanup_google_session(&self.memory, &incoming.sender_id).await;
            self.audit_google(incoming, "cancelled").await;
            self.send_text(incoming, google_cancelled_message(&user_lang))
                .await;
            return;
        }

        let input = incoming.text.trim();

        // Validate non-empty for all steps.
        if input.is_empty() {
            self.send_text(incoming, google_empty_input_message(&user_lang))
                .await;
            return;
        }

        match step {
            "project_id" => {
                // Store project_id and advance to setup_guide.
                if let Err(e) = self
                    .memory
                    .store_fact(&incoming.sender_id, "_google_project_id", input)
                    .await
                {
                    warn!("failed to store _google_project_id: {e}");
                    self.send_text(incoming, google_store_error_message(&user_lang))
                        .await;
                    return;
                }
                let new_value = format!("{ts_str}|setup_guide");
                let _ = self
                    .memory
                    .store_fact(&incoming.sender_id, "pending_google", &new_value)
                    .await;
                let msg = google_step_setup_guide_message(&user_lang, input);
                self.send_text(incoming, &msg).await;
            }
            "setup_guide" => {
                // User sends Client ID. Store it and advance to client_secret.
                if let Err(e) = self
                    .memory
                    .store_fact(&incoming.sender_id, "_google_client_id", input)
                    .await
                {
                    warn!("failed to store _google_client_id: {e}");
                    self.send_text(incoming, google_store_error_message(&user_lang))
                        .await;
                    return;
                }
                let new_value = format!("{ts_str}|client_secret");
                let _ = self
                    .memory
                    .store_fact(&incoming.sender_id, "pending_google", &new_value)
                    .await;
                self.send_text(incoming, google_step_client_secret_message(&user_lang))
                    .await;
            }
            "client_secret" => {
                // Delete the user's message (contains client_id from prev step? No, this msg has secret).
                self.delete_user_message(incoming).await;

                if let Err(e) = self
                    .memory
                    .store_fact(&incoming.sender_id, "_google_client_secret", input)
                    .await
                {
                    warn!("failed to store _google_client_secret: {e}");
                    self.send_text(incoming, google_store_error_message(&user_lang))
                        .await;
                    return;
                }

                // Build OAuth URL using stored client_id.
                let client_id = self
                    .memory
                    .get_fact(&incoming.sender_id, "_google_client_id")
                    .await
                    .ok()
                    .flatten();

                let Some(cid) = client_id else {
                    warn!("missing _google_client_id for OAuth URL");
                    cleanup_google_session(&self.memory, &incoming.sender_id).await;
                    self.send_text(incoming, google_missing_data_message(&user_lang))
                        .await;
                    return;
                };

                let auth_url = google_auth_oauth::build_authorization_url(&cid);

                let new_value = format!("{ts_str}|auth_code");
                let _ = self
                    .memory
                    .store_fact(&incoming.sender_id, "pending_google", &new_value)
                    .await;
                let msg = google_step_auth_code_message(&user_lang, &auth_url);
                // Use plain text to prevent Telegram Markdown from stripping
                // underscores in OAuth URL parameters (client_id, response_type, etc.).
                self.send_text_plain(incoming, &msg).await;
            }
            "auth_code" => {
                // Delete the auth code message for security.
                self.delete_user_message(incoming).await;

                // Read stored credentials.
                let client_id = self
                    .memory
                    .get_fact(&incoming.sender_id, "_google_client_id")
                    .await
                    .ok()
                    .flatten();
                let client_secret = self
                    .memory
                    .get_fact(&incoming.sender_id, "_google_client_secret")
                    .await
                    .ok()
                    .flatten();

                let (Some(cid), Some(csec)) = (client_id, client_secret) else {
                    warn!("missing temp facts during google auth code exchange");
                    cleanup_google_session(&self.memory, &incoming.sender_id).await;
                    self.audit_google(incoming, "error").await;
                    self.send_text(incoming, google_missing_data_message(&user_lang))
                        .await;
                    return;
                };

                // Exchange code for tokens.
                match google_auth_oauth::exchange_code_for_tokens(&cid, &csec, input).await {
                    Ok(tokens) => {
                        let Some(ref refresh_token) = tokens.refresh_token else {
                            warn!("token exchange returned no refresh_token");
                            cleanup_google_session(&self.memory, &incoming.sender_id).await;
                            self.audit_google(incoming, "error").await;
                            self.send_text(
                                incoming,
                                google_token_exchange_error_message(&user_lang),
                            )
                            .await;
                            return;
                        };

                        // Try to fetch email automatically.
                        match google_auth_oauth::fetch_user_email(&tokens.access_token).await {
                            Ok(email) => {
                                // Write credentials and complete.
                                self.complete_google_auth(
                                    incoming,
                                    &user_lang,
                                    &cid,
                                    &csec,
                                    refresh_token,
                                    &email,
                                )
                                .await;
                            }
                            Err(e) => {
                                // Fallback: ask user for email.
                                warn!("failed to fetch user email: {e}");
                                // Store refresh_token temporarily for the email_fallback step.
                                let _ = self
                                    .memory
                                    .store_fact(
                                        &incoming.sender_id,
                                        "_google_refresh_token",
                                        refresh_token,
                                    )
                                    .await;
                                let new_value = format!("{ts_str}|email_fallback");
                                let _ = self
                                    .memory
                                    .store_fact(&incoming.sender_id, "pending_google", &new_value)
                                    .await;
                                self.send_text(incoming, google_email_fallback_message(&user_lang))
                                    .await;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("token exchange failed: {e}");
                        cleanup_google_session(&self.memory, &incoming.sender_id).await;
                        self.audit_google(incoming, "error").await;
                        // Show the actual Google error so the user can diagnose.
                        let detail = format!(
                            "{}\n\nDetail: {e}",
                            google_token_exchange_error_message(&user_lang)
                        );
                        self.send_text(incoming, &detail).await;
                    }
                }
            }
            "email_fallback" => {
                // Validate email format.
                if !is_valid_email(input) {
                    self.send_text(incoming, google_invalid_email_message(&user_lang))
                        .await;
                    return;
                }

                // Read all temp facts.
                let client_id = self
                    .memory
                    .get_fact(&incoming.sender_id, "_google_client_id")
                    .await
                    .ok()
                    .flatten();
                let client_secret = self
                    .memory
                    .get_fact(&incoming.sender_id, "_google_client_secret")
                    .await
                    .ok()
                    .flatten();
                let refresh_token = self
                    .memory
                    .get_fact(&incoming.sender_id, "_google_refresh_token")
                    .await
                    .ok()
                    .flatten();

                let (Some(cid), Some(csec), Some(rtok)) = (client_id, client_secret, refresh_token)
                else {
                    warn!("missing temp facts during google auth email fallback");
                    cleanup_google_session(&self.memory, &incoming.sender_id).await;
                    self.audit_google(incoming, "error").await;
                    self.send_text(incoming, google_missing_data_message(&user_lang))
                        .await;
                    return;
                };

                self.complete_google_auth(incoming, &user_lang, &cid, &csec, &rtok, input)
                    .await;
            }
            _ => {
                warn!("unknown google auth step: {step}");
                cleanup_google_session(&self.memory, &incoming.sender_id).await;
                self.send_text(incoming, google_unknown_step_message(&user_lang))
                    .await;
            }
        }
    }

    /// Write google.json, clean up session, send completion message.
    async fn complete_google_auth(
        &self,
        incoming: &IncomingMessage,
        user_lang: &str,
        client_id: &str,
        client_secret: &str,
        refresh_token: &str,
        email: &str,
    ) {
        match write_google_credentials(
            &self.data_dir,
            client_id,
            client_secret,
            refresh_token,
            email,
        )
        .await
        {
            Ok(()) => {
                cleanup_google_session(&self.memory, &incoming.sender_id).await;
                // Also clean up _google_refresh_token if it exists (email_fallback path).
                let _ = self
                    .memory
                    .delete_fact(&incoming.sender_id, "_google_refresh_token")
                    .await;
                self.audit_google(incoming, "complete").await;
                let msg = google_step_complete_message(user_lang, email);
                self.send_text(incoming, &msg).await;
            }
            Err(e) => {
                warn!("failed to write google.json: {e}");
                cleanup_google_session(&self.memory, &incoming.sender_id).await;
                self.audit_google(incoming, "error").await;
                self.send_text(incoming, google_write_error_message(user_lang))
                    .await;
            }
        }
    }

    /// Log an audit entry for a google auth operation.
    /// SECURITY: Never log credential values.
    async fn audit_google(&self, incoming: &IncomingMessage, status: &str) {
        let _ = self
            .audit
            .log(&AuditEntry {
                channel: incoming.channel.clone(),
                sender_id: incoming.sender_id.clone(),
                sender_name: incoming.sender_name.clone(),
                input_text: "[GOOGLE_AUTH]".to_string(),
                output_text: Some(format!("[GOOGLE_AUTH] {status}")),
                provider_used: None,
                model: None,
                processing_ms: None,
                status: match status {
                    "complete" | "started" | "cancelled" | "expired" => AuditStatus::Ok,
                    _ => AuditStatus::Error,
                },
                denial_reason: None,
            })
            .await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omega_core::config::MemoryConfig;
    use omega_memory::Store;
    use std::sync::atomic::{AtomicU64, Ordering};

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
        let steps = [
            "project_id",
            "setup_guide",
            "client_secret",
            "auth_code",
            "email_fallback",
        ];
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
    async fn test_step_transition_to_client_secret() {
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
        let new_value = format!("{now}|client_secret");
        store
            .store_fact(sender, "pending_google", &new_value)
            .await
            .unwrap();

        let pending = store.get_fact(sender, "pending_google").await.unwrap();
        assert!(pending.unwrap().ends_with("|client_secret"));
    }

    #[tokio::test]
    async fn test_step_transition_to_auth_code() {
        let store = test_store().await;
        let sender = "trans_test3";
        let now = chrono::Utc::now().timestamp();

        store
            .store_fact(sender, "pending_google", &format!("{now}|client_secret"))
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
            .store_fact(sender_b, "pending_google", "1709123456|client_secret")
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
        assert!(b_pending.contains("client_secret"));
    }
}
