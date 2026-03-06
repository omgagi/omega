//! Google account credential setup -- `/google` command state machine.
//!
//! Handles a 4-step wizard: project_id, setup_guide (JSON credentials),
//! auth_code, and completion (with email_fallback if needed).
//! OAuth token exchange is handled server-side -- no raw refresh_token needed.
//! Credentials are stored in `~/.omega/stores/google.json` with 0600 permissions.
//! Credential messages are deleted from chat for security (best-effort).
//! Credentials NEVER reach the AI provider pipeline.

use std::path::PathBuf;

use omega_core::config::shellexpand;
use omega_core::message::IncomingMessage;
use omega_memory::audit::{AuditEntry, AuditStatus};
use tracing::warn;

use super::google_auth_i18n::*;
use super::google_auth_oauth;
use super::google_auth_utils::*;
use super::keywords::{is_build_cancelled, GOOGLE_AUTH_TTL_SECS};
use super::Gateway;

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

        // If omg-gog credentials exist, skip manual entry and jump to auth_code.
        if let Some((cid, csec)) = read_omg_gog_credentials() {
            let _ = self
                .memory
                .store_fact(&incoming.sender_id, "_google_client_id", &cid)
                .await;
            let _ = self
                .memory
                .store_fact(&incoming.sender_id, "_google_client_secret", &csec)
                .await;
            let auth_url = google_auth_oauth::build_authorization_url(&cid);
            let fact_value = format!("{now}|auth_code");
            let _ = self
                .memory
                .store_fact(&incoming.sender_id, "pending_google", &fact_value)
                .await;
            let msg = google_step_auth_code_message(&user_lang, &auth_url);
            self.send_text_plain(incoming, &msg).await;
            return;
        }

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
                // User must paste full Google credentials JSON.
                if let Some((cid, csec)) = try_extract_json_credentials(input) {
                    // Delete message containing credentials.
                    self.delete_user_message(incoming).await;

                    if let Err(e) = self
                        .memory
                        .store_fact(&incoming.sender_id, "_google_client_id", &cid)
                        .await
                    {
                        warn!("failed to store _google_client_id: {e}");
                        self.send_text(incoming, google_store_error_message(&user_lang))
                            .await;
                        return;
                    }
                    if let Err(e) = self
                        .memory
                        .store_fact(&incoming.sender_id, "_google_client_secret", &csec)
                        .await
                    {
                        warn!("failed to store _google_client_secret: {e}");
                        self.send_text(incoming, google_store_error_message(&user_lang))
                            .await;
                        return;
                    }

                    let auth_url = google_auth_oauth::build_authorization_url(&cid);
                    let new_value = format!("{ts_str}|auth_code");
                    let _ = self
                        .memory
                        .store_fact(&incoming.sender_id, "pending_google", &new_value)
                        .await;
                    let msg = google_step_auth_code_message(&user_lang, &auth_url);
                    self.send_text_plain(incoming, &msg).await;
                } else {
                    // Not valid JSON credentials — ask again.
                    self.send_text(incoming, google_invalid_json_message(&user_lang))
                        .await;
                }
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
                // Sync credentials to omg-gog's config dir so the CLI can refresh tokens.
                sync_omg_gog_credentials(client_id, client_secret).await;

                cleanup_google_session(&self.memory, &incoming.sender_id).await;
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
