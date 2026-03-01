//! Build-related pipeline stages — discovery continuation, build confirmation, and build keyword handling.

use std::path::PathBuf;

use tracing::{info, warn};

use omega_core::config::shellexpand;
use omega_core::message::IncomingMessage;

use super::builds_agents::AgentFilesGuard;
use super::builds_parse::{
    discovery_file_path, parse_discovery_output, parse_discovery_round, truncate_brief_preview,
    DiscoveryOutput,
};
use super::builds_topology;
use super::keywords::*;
use super::Gateway;

impl Gateway {
    /// Handle an active discovery session (pending_discovery fact exists).
    ///
    /// Returns `true` if the caller should early-return from `handle_message`,
    /// `false` if processing should continue (e.g. session expired and fell through).
    pub(super) async fn handle_pending_discovery(
        &self,
        incoming: &IncomingMessage,
        clean_text: &str,
        typing_handle: &mut Option<tokio::task::JoinHandle<()>>,
    ) -> bool {
        let pending_discovery: Option<String> = self
            .memory
            .get_fact(&incoming.sender_id, "pending_discovery")
            .await
            .ok()
            .flatten();

        let discovery_value = match pending_discovery {
            Some(v) => v,
            None => return false,
        };

        // Parse timestamp from "timestamp|sender_id" format.
        let (stored_ts, _) = discovery_value
            .split_once('|')
            .unwrap_or(("0", &discovery_value));
        let created_at: i64 = stored_ts.parse().unwrap_or(0);
        let now = chrono::Utc::now().timestamp();
        let expired = (now - created_at) > DISCOVERY_TTL_SECS;

        let user_lang = self
            .memory
            .get_fact(&incoming.sender_id, "preferred_language")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "English".to_string());

        if expired {
            // Clean up expired session.
            let _ = self
                .memory
                .delete_fact(&incoming.sender_id, "pending_discovery")
                .await;
            let disc_file = discovery_file_path(&self.data_dir, &incoming.sender_id);
            let _ = tokio::fs::remove_file(&disc_file).await;
            info!("[{}] discovery session expired", incoming.channel);
            self.send_text(incoming, discovery_expired_message(&user_lang))
                .await;
            // Fall through — the current message might be a new build request or normal chat.
            return false;
        } else if is_build_cancelled(clean_text) {
            // User cancelled discovery.
            let _ = self
                .memory
                .delete_fact(&incoming.sender_id, "pending_discovery")
                .await;
            let disc_file = discovery_file_path(&self.data_dir, &incoming.sender_id);
            let _ = tokio::fs::remove_file(&disc_file).await;
            if let Some(h) = typing_handle.take() {
                h.abort();
            }
            self.send_text(incoming, discovery_cancelled_message(&user_lang))
                .await;
            return true;
        }

        // Active discovery session — process the user's answer.
        let disc_file = discovery_file_path(&self.data_dir, &incoming.sender_id);
        let mut discovery_context = tokio::fs::read_to_string(&disc_file)
            .await
            .unwrap_or_default();

        // Parse current round from file header.
        let current_round = parse_discovery_round(&discovery_context);

        // Append user's answer to the file.
        discovery_context.push_str(&format!("\n### User Response\n{}\n", clean_text));

        let is_final_round = current_round >= 3;
        let next_round = current_round + 1;

        // Build prompt for discovery agent.
        let agent_prompt = if is_final_round {
            format!(
                "This is the FINAL round. You MUST output DISCOVERY_COMPLETE with an Idea Brief.\n\
                 Synthesize everything below into a brief.\n\n{discovery_context}"
            )
        } else {
            format!(
                "Discovery round {next_round}/3. Read the accumulated context and either:\n\
                 - Output DISCOVERY_QUESTIONS if you need more info\n\
                 - Output DISCOVERY_COMPLETE if you have enough\n\n{discovery_context}"
            )
        };

        // Write agent files and run discovery agent.
        let workspace_dir = PathBuf::from(shellexpand(&self.data_dir)).join("workspace");
        let _agent_guard = match builds_topology::load_topology(&self.data_dir, "development") {
            Ok(loaded) => {
                match AgentFilesGuard::write_from_topology(&workspace_dir, &loaded).await {
                    Ok(guard) => guard,
                    Err(e) => {
                        warn!("Failed to write agent files for discovery: {e}");
                        let _ = self
                            .memory
                            .delete_fact(&incoming.sender_id, "pending_discovery")
                            .await;
                        let _ = tokio::fs::remove_file(&disc_file).await;
                        if let Some(h) = typing_handle.take() {
                            h.abort();
                        }
                        self.send_text(incoming, "Discovery failed (agent setup error).")
                            .await;
                        return true;
                    }
                }
            }
            Err(e) => {
                warn!("Failed to load topology for discovery: {e}");
                let _ = self
                    .memory
                    .delete_fact(&incoming.sender_id, "pending_discovery")
                    .await;
                let _ = tokio::fs::remove_file(&disc_file).await;
                if let Some(h) = typing_handle.take() {
                    h.abort();
                }
                self.send_text(incoming, "Discovery failed (topology load error).")
                    .await;
                return true;
            }
        };

        let result = self
            .run_build_phase(
                "build-discovery",
                &agent_prompt,
                &self.model_complex,
                Some(15),
            )
            .await;

        match result {
            Ok(output) => {
                let parsed = parse_discovery_output(&output);
                // If final round, force Complete.
                let parsed = if is_final_round {
                    match parsed {
                        DiscoveryOutput::Questions(q) => DiscoveryOutput::Complete(q),
                        other => other,
                    }
                } else {
                    parsed
                };

                match parsed {
                    DiscoveryOutput::Questions(questions) => {
                        // Update discovery file with new round header.
                        let updated = format!(
                            "{discovery_context}\n## Round {next_round}\n### Agent Questions\n{questions}\n"
                        );
                        // Update ROUND: header in file.
                        let updated = if updated.contains("ROUND:") {
                            updated.replacen(
                                &format!("ROUND: {current_round}"),
                                &format!("ROUND: {next_round}"),
                                1,
                            )
                        } else {
                            updated
                        };
                        let _ = tokio::fs::write(&disc_file, &updated).await;

                        // Send follow-up questions (next_round >= 2 in continuation path).
                        let msg = discovery_followup_message(&user_lang, &questions, next_round);
                        if let Some(h) = typing_handle.take() {
                            h.abort();
                        }
                        self.send_text(incoming, &msg).await;
                        true
                    }
                    DiscoveryOutput::Complete(brief) => {
                        // Discovery complete — clean up and hand off to confirmation.
                        let _ = self
                            .memory
                            .delete_fact(&incoming.sender_id, "pending_discovery")
                            .await;
                        let _ = tokio::fs::remove_file(&disc_file).await;

                        // Store enriched brief as pending_build_request.
                        let stamped = format!("{}|{}", chrono::Utc::now().timestamp(), brief);
                        let _ = self
                            .memory
                            .store_fact(&incoming.sender_id, "pending_build_request", &stamped)
                            .await;

                        // Send discovery complete + confirmation message.
                        let preview = truncate_brief_preview(&brief, 300);
                        let msg = discovery_complete_message(&user_lang, &preview);
                        if let Some(h) = typing_handle.take() {
                            h.abort();
                        }
                        self.send_text(incoming, &msg).await;
                        true
                    }
                }
            }
            Err(e) => {
                // Discovery agent failed — clean up, inform user.
                let _ = self
                    .memory
                    .delete_fact(&incoming.sender_id, "pending_discovery")
                    .await;
                let _ = tokio::fs::remove_file(&disc_file).await;
                if let Some(h) = typing_handle.take() {
                    h.abort();
                }
                self.send_text(incoming, &format!("Discovery failed: {e}"))
                    .await;
                true
            }
        }
    }

    /// Handle a pending build confirmation (pending_build_request fact exists).
    ///
    /// Returns `true` if the caller should early-return from `handle_message`,
    /// `false` if processing should continue (expired or not confirmed).
    pub(super) async fn handle_pending_build_confirmation(
        &self,
        incoming: &IncomingMessage,
        clean_text: &str,
        typing_handle: &mut Option<tokio::task::JoinHandle<()>>,
    ) -> bool {
        let pending_build: Option<String> = self
            .memory
            .get_fact(&incoming.sender_id, "pending_build_request")
            .await
            .ok()
            .flatten();

        let stored_value = match pending_build {
            Some(v) => v,
            None => return false,
        };

        // Always clear the pending state — one-shot.
        let _ = self
            .memory
            .delete_fact(&incoming.sender_id, "pending_build_request")
            .await;

        // Parse "timestamp|request_text" and check TTL.
        let (stored_ts, stored_request) =
            stored_value.split_once('|').unwrap_or(("0", &stored_value));
        let created_at: i64 = stored_ts.parse().unwrap_or(0);
        let now = chrono::Utc::now().timestamp();
        let expired = (now - created_at) > BUILD_CONFIRM_TTL_SECS;

        if expired {
            info!(
                "[{}] pending build expired ({}s ago) — ignoring",
                incoming.channel,
                now - created_at
            );
        } else if is_build_confirmed(clean_text) {
            info!(
                "[{}] build CONFIRMED → multi-phase pipeline",
                incoming.channel
            );
            let mut build_incoming = incoming.clone();
            build_incoming.text = stored_request.to_string();
            self.handle_build_request(&build_incoming, typing_handle.take())
                .await;
            return true;
        } else if is_build_cancelled(clean_text) {
            info!("[{}] build explicitly CANCELLED by user", incoming.channel);
            let user_lang = self
                .memory
                .get_fact(&incoming.sender_id, "preferred_language")
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| "English".to_string());
            if let Some(h) = typing_handle.take() {
                h.abort();
            }
            self.send_text(incoming, build_cancelled_message(&user_lang))
                .await;
            return true;
        } else {
            info!(
                "[{}] build NOT confirmed — proceeding with normal pipeline",
                incoming.channel
            );
        }
        // Fall through to normal message processing.
        false
    }

    /// Handle build keyword detection — run discovery agent before confirmation.
    ///
    /// This method always results in an early return from `handle_message`
    /// (sends a response and returns).
    pub(super) async fn handle_build_keyword_discovery(
        &self,
        incoming: &IncomingMessage,
        typing_handle: &mut Option<tokio::task::JoinHandle<()>>,
    ) {
        info!(
            "[{}] build keyword detected \u{2192} starting discovery",
            incoming.channel
        );

        let user_lang = self
            .memory
            .get_fact(&incoming.sender_id, "preferred_language")
            .await
            .ok()
            .flatten()
            .unwrap_or_else(|| "English".to_string());

        // Write agent files for discovery.
        let workspace_dir = PathBuf::from(shellexpand(&self.data_dir)).join("workspace");
        let _agent_guard = match builds_topology::load_topology(&self.data_dir, "development") {
            Ok(loaded) => {
                match AgentFilesGuard::write_from_topology(&workspace_dir, &loaded).await {
                    Ok(guard) => guard,
                    Err(e) => {
                        // Fall back to direct confirmation if agent files fail.
                        warn!("Failed to write agent files for discovery: {e}");
                        let stamped =
                            format!("{}|{}", chrono::Utc::now().timestamp(), incoming.text);
                        let _ = self
                            .memory
                            .store_fact(&incoming.sender_id, "pending_build_request", &stamped)
                            .await;
                        let confirm_msg = build_confirm_message(&user_lang, &incoming.text);
                        if let Some(h) = typing_handle.take() {
                            h.abort();
                        }
                        self.send_text(incoming, &confirm_msg).await;
                        return;
                    }
                }
            }
            Err(e) => {
                // Fall back to direct confirmation if topology fails.
                warn!("Failed to load topology for discovery: {e}");
                let stamped = format!("{}|{}", chrono::Utc::now().timestamp(), incoming.text);
                let _ = self
                    .memory
                    .store_fact(&incoming.sender_id, "pending_build_request", &stamped)
                    .await;
                let confirm_msg = build_confirm_message(&user_lang, &incoming.text);
                if let Some(h) = typing_handle.take() {
                    h.abort();
                }
                self.send_text(incoming, &confirm_msg).await;
                return;
            }
        };

        // Run first discovery round with raw request.
        let agent_prompt = format!(
            "Discovery round 1/3. Analyze this build request and decide:\n\
             - If specific enough, output DISCOVERY_COMPLETE with an Idea Brief\n\
             - If vague, output DISCOVERY_QUESTIONS with 3-5 clarifying questions\n\n\
             User request: {}",
            incoming.text
        );

        let result = self
            .run_build_phase(
                "build-discovery",
                &agent_prompt,
                &self.model_complex,
                Some(15),
            )
            .await;

        match result {
            Ok(output) => {
                let parsed = parse_discovery_output(&output);
                match parsed {
                    DiscoveryOutput::Complete(brief) => {
                        // Request was specific — skip multi-round, go straight to confirmation.
                        let stamped = format!("{}|{}", chrono::Utc::now().timestamp(), brief);
                        let _ = self
                            .memory
                            .store_fact(&incoming.sender_id, "pending_build_request", &stamped)
                            .await;
                        let preview = truncate_brief_preview(&brief, 300);
                        let msg = discovery_complete_message(&user_lang, &preview);
                        if let Some(h) = typing_handle.take() {
                            h.abort();
                        }
                        self.send_text(incoming, &msg).await;
                    }
                    DiscoveryOutput::Questions(questions) => {
                        // Request was vague — start multi-round discovery session.
                        let disc_file = discovery_file_path(&self.data_dir, &incoming.sender_id);
                        let discovery_dir = disc_file
                            .parent()
                            .expect("discovery path always has parent");
                        let _ = tokio::fs::create_dir_all(discovery_dir).await;

                        // Create discovery file with round 1 content.
                        let file_content = format!(
                            "# Discovery Session\n\n\
                             CREATED: {}\n\
                             ROUND: 1\n\
                             ORIGINAL_REQUEST: {}\n\n\
                             ## Round 1\n\
                             ### Agent Questions\n{}\n",
                            chrono::Utc::now().timestamp(),
                            incoming.text,
                            questions
                        );
                        let _ = tokio::fs::write(&disc_file, &file_content).await;

                        // Store pending_discovery fact.
                        let stamped =
                            format!("{}|{}", chrono::Utc::now().timestamp(), incoming.sender_id);
                        let _ = self
                            .memory
                            .store_fact(&incoming.sender_id, "pending_discovery", &stamped)
                            .await;

                        // Send questions to user.
                        let msg = discovery_intro_message(&user_lang, &questions);
                        if let Some(h) = typing_handle.take() {
                            h.abort();
                        }
                        self.send_text(incoming, &msg).await;
                    }
                }
            }
            Err(e) => {
                // Discovery failed — fall back to old behavior (direct confirmation).
                warn!("Discovery agent failed, falling back to direct confirmation: {e}");
                let stamped = format!("{}|{}", chrono::Utc::now().timestamp(), incoming.text);
                let _ = self
                    .memory
                    .store_fact(&incoming.sender_id, "pending_build_request", &stamped)
                    .await;
                let msg = build_confirm_message(&user_lang, &incoming.text);
                if let Some(h) = typing_handle.take() {
                    h.abort();
                }
                self.send_text(incoming, &msg).await;
            }
        }
    }
}
