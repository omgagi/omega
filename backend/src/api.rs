//! HTTP API server for SaaS dashboard integration.
//!
//! Provides endpoints for health checks, WhatsApp QR pairing, and inbound webhooks.
//! Spawned as a background task in the gateway, same pattern as scheduler/heartbeat.

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use omega_channels::whatsapp::{self, WhatsAppChannel};
use omega_core::config::{ApiConfig, ChannelConfig};
use omega_core::message::{IncomingMessage, MessageMetadata, OutgoingMessage};
use omega_core::traits::Channel;
use omega_memory::audit::{AuditEntry, AuditLogger, AuditStatus};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Shared state for API handlers.
#[derive(Clone)]
pub struct ApiState {
    channels: HashMap<String, Arc<dyn Channel>>,
    api_key: Option<String>,
    uptime: Instant,
    tx: Option<mpsc::Sender<IncomingMessage>>,
    audit: Option<AuditLogger>,
    channel_config: ChannelConfig,
}

/// Inbound webhook request body.
#[derive(Debug, Deserialize)]
struct WebhookRequest {
    source: String,
    message: String,
    mode: String,
    channel: Option<String>,
    target: Option<String>,
}

/// Constant-time string comparison to prevent timing attacks on API token validation.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.bytes()
        .zip(b.bytes())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

/// Check bearer token auth. Returns `None` if authorized, `Some(response)` if rejected.
fn check_auth(headers: &HeaderMap, api_key: &Option<String>) -> Option<(StatusCode, Json<Value>)> {
    let key = match api_key {
        Some(k) => k,
        None => return None, // No auth configured — allow all.
    };

    let header = match headers.get("authorization") {
        Some(h) => h,
        None => {
            return Some((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "missing Authorization header"})),
            ));
        }
    };

    let value = match header.to_str() {
        Ok(v) => v,
        Err(_) => {
            return Some((
                StatusCode::UNAUTHORIZED,
                Json(json!({"error": "invalid Authorization header"})),
            ));
        }
    };

    match value.strip_prefix("Bearer ") {
        Some(token) if constant_time_eq(token, key) => None, // Authorized.
        _ => Some((
            StatusCode::UNAUTHORIZED,
            Json(json!({"error": "invalid token"})),
        )),
    }
}

/// Downcast the WhatsApp channel from shared state.
fn get_whatsapp(state: &ApiState) -> Result<&WhatsAppChannel, (StatusCode, Json<Value>)> {
    let ch = state.channels.get("whatsapp").ok_or((
        StatusCode::BAD_REQUEST,
        Json(json!({"error": "WhatsApp channel not configured"})),
    ))?;

    ch.as_any().downcast_ref::<WhatsAppChannel>().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": "WhatsApp channel downcast failed"})),
    ))
}

/// `GET /api/health` — Health check with uptime and WhatsApp status.
async fn health(
    headers: HeaderMap,
    State(state): State<ApiState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Some(err) = check_auth(&headers, &state.api_key) {
        return Err(err);
    }

    let uptime_secs = state.uptime.elapsed().as_secs();

    let whatsapp_status = match state.channels.get("whatsapp") {
        Some(ch) => match ch.as_any().downcast_ref::<WhatsAppChannel>() {
            Some(wa) => {
                if wa.is_connected().await {
                    "connected"
                } else {
                    "disconnected"
                }
            }
            None => "error",
        },
        None => "not_configured",
    };

    Ok(Json(json!({
        "status": "ok",
        "uptime_secs": uptime_secs,
        "whatsapp": whatsapp_status,
    })))
}

/// `POST /api/pair` — Trigger WhatsApp pairing, return QR as base64 PNG.
async fn pair(
    headers: HeaderMap,
    State(state): State<ApiState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Some(err) = check_auth(&headers, &state.api_key) {
        return Err(err);
    }

    let wa = get_whatsapp(&state)?;

    // Already paired — no need to generate QR.
    if wa.is_connected().await {
        return Ok(Json(json!({
            "status": "already_paired",
            "message": "WhatsApp is already connected",
        })));
    }

    // Restart bot for fresh QR codes.
    wa.restart_for_pairing().await.map_err(|e| {
        error!("WhatsApp restart_for_pairing failed: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("pairing restart failed: {e}")})),
        )
    })?;

    // Get receivers from the restarted bot.
    let (mut qr_rx, _done_rx) = wa.pairing_channels().await;

    // Wait up to 30s for the first QR code.
    let qr_data = tokio::time::timeout(std::time::Duration::from_secs(30), qr_rx.recv())
        .await
        .map_err(|_| {
            (
                StatusCode::GATEWAY_TIMEOUT,
                Json(json!({"error": "timed out waiting for QR code"})),
            )
        })?
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "QR channel closed unexpectedly"})),
        ))?;

    // Generate PNG and encode as base64.
    let png_bytes = whatsapp::generate_qr_image(&qr_data).map_err(|e| {
        error!("QR image generation failed: {e}");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": format!("QR generation failed: {e}")})),
        )
    })?;

    let qr_base64 = BASE64.encode(&png_bytes);

    Ok(Json(json!({
        "status": "qr_ready",
        "qr_png_base64": qr_base64,
    })))
}

/// `GET /api/pair/status` — Long-poll (60s) for pairing completion.
async fn pair_status(
    headers: HeaderMap,
    State(state): State<ApiState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Some(err) = check_auth(&headers, &state.api_key) {
        return Err(err);
    }

    let wa = get_whatsapp(&state)?;

    // Already connected — immediate success.
    if wa.is_connected().await {
        return Ok(Json(json!({
            "status": "paired",
            "message": "WhatsApp is connected",
        })));
    }

    // Get done receiver and long-poll.
    let (_qr_rx, mut done_rx) = wa.pairing_channels().await;

    let paired = tokio::time::timeout(std::time::Duration::from_secs(60), done_rx.recv())
        .await
        .unwrap_or(Some(false))
        .unwrap_or(false);

    if paired {
        Ok(Json(json!({
            "status": "paired",
            "message": "WhatsApp pairing completed",
        })))
    } else {
        Ok(Json(json!({
            "status": "pending",
            "message": "Pairing not yet completed",
        })))
    }
}

/// Build the axum router with shared state.
fn build_router(state: ApiState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/pair", post(pair))
        .route("/api/pair/status", get(pair_status))
        .route("/api/webhook", post(webhook))
        .layer(axum::extract::DefaultBodyLimit::max(1024 * 1024)) // 1 MB max request body
        .with_state(state)
}

/// Start the API server. Called from `Gateway::run()`.
pub async fn serve(
    config: ApiConfig,
    channels: HashMap<String, Arc<dyn Channel>>,
    uptime: Instant,
    tx: mpsc::Sender<IncomingMessage>,
    audit: AuditLogger,
    channel_config: ChannelConfig,
) {
    let api_key = if config.api_key.is_empty() {
        None
    } else {
        Some(config.api_key.clone())
    };

    let state = ApiState {
        channels,
        api_key,
        uptime,
        tx: Some(tx),
        audit: Some(audit),
        channel_config,
    };

    let app = build_router(state);
    let addr = format!("{}:{}", config.host, config.port);

    let listener = match tokio::net::TcpListener::bind(&addr).await {
        Ok(l) => l,
        Err(e) => {
            error!("API server failed to bind to {addr}: {e}");
            return;
        }
    };

    info!("API server listening on {addr}");

    if let Err(e) = axum::serve(listener, app).await {
        error!("API server error: {e}");
    }
}

/// Resolve the default channel when none is explicitly specified.
/// Priority: telegram > whatsapp.
fn resolve_default_channel(channels: &HashMap<String, Arc<dyn Channel>>) -> Option<String> {
    if channels.contains_key("telegram") {
        return Some("telegram".to_string());
    }
    if channels.contains_key("whatsapp") {
        return Some("whatsapp".to_string());
    }
    None
}

/// Resolve the default target (first allowed_user) for a given channel.
fn resolve_default_target(channel_name: &str, channel_config: &ChannelConfig) -> Option<String> {
    match channel_name {
        "telegram" => channel_config
            .telegram
            .as_ref()
            .and_then(|tg| tg.allowed_users.first())
            .map(|id| id.to_string()),
        "whatsapp" => channel_config
            .whatsapp
            .as_ref()
            .and_then(|wa| wa.allowed_users.first())
            .cloned(),
        _ => None,
    }
}

/// `POST /api/webhook` — Accept inbound messages from external tools.
///
/// Two modes: "direct" sends text straight to messaging channel (bypasses AI),
/// "ai" injects the message into the full AI pipeline.
async fn webhook(
    headers: HeaderMap,
    State(state): State<ApiState>,
    body: Result<Json<WebhookRequest>, axum::extract::rejection::JsonRejection>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    // 1. Auth check.
    if let Some(err) = check_auth(&headers, &state.api_key) {
        return Err(err);
    }

    // 2. Parse JSON body.
    let Json(request) = body.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("invalid request: {e}")})),
        )
    })?;

    // 3. Validate required fields.
    if request.source.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "source must not be empty"})),
        ));
    }
    if request.message.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "message must not be empty"})),
        ));
    }
    if request.mode != "direct" && request.mode != "ai" {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"error": format!("invalid mode '{}', expected 'direct' or 'ai'", request.mode)}),
            ),
        ));
    }

    // 4. Resolve channel.
    let resolved_channel = match &request.channel {
        Some(ch) => ch.clone(),
        None => resolve_default_channel(&state.channels).ok_or((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "no channels configured"})),
        ))?,
    };

    // Verify channel exists.
    if !state.channels.contains_key(&resolved_channel) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("channel '{}' not configured", resolved_channel)})),
        ));
    }

    // 5. Resolve target.
    let resolved_target = match &request.target {
        Some(t) => t.clone(),
        None => resolve_default_target(&resolved_channel, &state.channel_config).ok_or((
            StatusCode::BAD_REQUEST,
            Json(json!({"error": format!("no default target for channel '{}'", resolved_channel)})),
        ))?,
    };

    // 6. Branch on mode.
    match request.mode.as_str() {
        "direct" => {
            let channel = state.channels.get(&resolved_channel).ok_or((
                StatusCode::BAD_REQUEST,
                Json(json!({"error": format!("channel '{}' not configured", resolved_channel)})),
            ))?;

            let msg = OutgoingMessage {
                text: request.message.clone(),
                metadata: MessageMetadata::default(),
                reply_target: Some(resolved_target.clone()),
            };

            channel.send(msg).await.map_err(|e| {
                error!("webhook direct delivery failed: {e}");
                (
                    StatusCode::BAD_GATEWAY,
                    Json(json!({"error": format!("delivery failed: {e}")})),
                )
            })?;

            // Audit (best-effort, don't block response).
            if let Some(ref audit) = state.audit {
                let entry = AuditEntry {
                    channel: resolved_channel.clone(),
                    sender_id: resolved_target.clone(),
                    sender_name: Some(format!("webhook:{}", request.source)),
                    input_text: request.message.clone(),
                    output_text: None,
                    provider_used: None,
                    model: None,
                    processing_ms: None,
                    status: AuditStatus::Ok,
                    denial_reason: None,
                };
                if let Err(e) = audit.log(&entry).await {
                    warn!("webhook audit log failed: {e}");
                }
            }

            info!(
                "webhook direct delivered to {}:{} from {}",
                resolved_channel, resolved_target, request.source
            );

            Ok((
                StatusCode::OK,
                Json(json!({
                    "status": "delivered",
                    "channel": resolved_channel,
                    "target": resolved_target,
                })),
            ))
        }
        "ai" => {
            let tx = state.tx.as_ref().ok_or((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({"error": "gateway unavailable"})),
            ))?;

            let request_id = Uuid::new_v4();
            let incoming = IncomingMessage {
                id: request_id,
                channel: resolved_channel.clone(),
                sender_id: resolved_target.clone(),
                sender_name: Some(format!("webhook:{}", request.source)),
                text: format!("[webhook:{}] {}", request.source, request.message),
                timestamp: Utc::now(),
                reply_to: None,
                attachments: vec![],
                reply_target: Some(resolved_target),
                is_group: false,
                source: Some(request.source.clone()),
            };

            tx.send(incoming).await.map_err(|_| {
                error!("webhook ai mode: gateway receiver dropped");
                (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(json!({"error": "gateway unavailable"})),
                )
            })?;

            info!("webhook ai queued {} from {}", request_id, request.source);

            Ok((
                StatusCode::ACCEPTED,
                Json(json!({
                    "status": "queued",
                    "request_id": request_id.to_string(),
                })),
            ))
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"error": format!("invalid mode '{}', expected 'direct' or 'ai'", request.mode)}),
            ),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use omega_core::config::{ChannelConfig, TelegramConfig, WhatsAppConfig};
    use omega_core::error::OmegaError;
    use omega_core::message::{IncomingMessage, OutgoingMessage};
    use std::any::Any;
    use std::sync::Mutex;
    use tokio::sync::mpsc;
    use tower::ServiceExt;

    // -----------------------------------------------------------------------
    // Mock Channel for webhook direct-mode tests
    // -----------------------------------------------------------------------

    /// A mock channel that records sent messages for assertion.
    /// Used to verify direct-mode webhook delivery calls `channel.send()`.
    struct MockChannel {
        name: String,
        sent: Arc<Mutex<Vec<OutgoingMessage>>>,
        /// When true, `send()` returns an error (simulates delivery failure).
        fail_send: bool,
    }

    impl MockChannel {
        fn new(name: &str) -> (Self, Arc<Mutex<Vec<OutgoingMessage>>>) {
            let sent = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    name: name.to_string(),
                    sent: Arc::clone(&sent),
                    fail_send: false,
                },
                sent,
            )
        }

        fn new_failing(name: &str) -> Self {
            Self {
                name: name.to_string(),
                sent: Arc::new(Mutex::new(Vec::new())),
                fail_send: true,
            }
        }
    }

    #[async_trait]
    impl Channel for MockChannel {
        fn name(&self) -> &str {
            &self.name
        }

        async fn start(&self) -> Result<tokio::sync::mpsc::Receiver<IncomingMessage>, OmegaError> {
            let (_tx, rx) = tokio::sync::mpsc::channel(1);
            Ok(rx)
        }

        async fn send(&self, message: OutgoingMessage) -> Result<(), OmegaError> {
            if self.fail_send {
                return Err(OmegaError::Channel("connection reset".to_string()));
            }
            self.sent.lock().unwrap().push(message);
            Ok(())
        }

        async fn stop(&self) -> Result<(), OmegaError> {
            Ok(())
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    /// Build a test router with no channels (WhatsApp not configured).
    /// Updated to include new ApiState fields required by webhook feature.
    fn test_router(api_key: Option<String>) -> Router {
        let state = ApiState {
            channels: HashMap::new(),
            api_key,
            uptime: Instant::now(),
            tx: None,
            audit: None,
            channel_config: ChannelConfig::default(),
        };
        build_router(state)
    }

    /// Build a test router with a mock channel and mpsc sender for webhook tests.
    fn webhook_router(
        api_key: Option<String>,
        channels: HashMap<String, Arc<dyn Channel>>,
        tx: Option<mpsc::Sender<IncomingMessage>>,
        channel_config: ChannelConfig,
    ) -> Router {
        let state = ApiState {
            channels,
            api_key,
            uptime: Instant::now(),
            tx,
            audit: None,
            channel_config,
        };
        build_router(state)
    }

    /// Build a ChannelConfig with telegram allowed_users.
    fn telegram_channel_config(users: Vec<i64>) -> ChannelConfig {
        ChannelConfig {
            telegram: Some(TelegramConfig {
                enabled: true,
                bot_token: String::new(),
                allowed_users: users,
                whisper_api_key: None,
            }),
            whatsapp: None,
        }
    }

    /// Build a ChannelConfig with both telegram and whatsapp.
    fn dual_channel_config(tg_users: Vec<i64>, wa_users: Vec<String>) -> ChannelConfig {
        ChannelConfig {
            telegram: Some(TelegramConfig {
                enabled: true,
                bot_token: String::new(),
                allowed_users: tg_users,
                whisper_api_key: None,
            }),
            whatsapp: Some(WhatsAppConfig {
                enabled: true,
                allowed_users: wa_users,
                whisper_api_key: None,
            }),
        }
    }

    /// Helper to POST JSON to /api/webhook.
    fn webhook_request(body: &str) -> Request<Body> {
        Request::post("/api/webhook")
            .header("Content-Type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    /// Helper to POST JSON to /api/webhook with bearer auth.
    fn webhook_request_auth(body: &str, token: &str) -> Request<Body> {
        Request::post("/api/webhook")
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {token}"))
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    /// Parse response body as JSON.
    async fn body_json(resp: axum::http::Response<Body>) -> Value {
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    // -----------------------------------------------------------------------
    // Existing tests (updated for new ApiState fields)
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_health_no_auth() {
        let app = test_router(None);
        let req = Request::get("/api/health").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let json = body_json(resp).await;
        assert_eq!(json["status"], "ok");
        assert_eq!(json["whatsapp"], "not_configured");
    }

    #[tokio::test]
    async fn test_health_valid_auth() {
        let app = test_router(Some("secret".to_string()));
        let req = Request::get("/api/health")
            .header("Authorization", "Bearer secret")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_bad_auth() {
        let app = test_router(Some("secret".to_string()));
        let req = Request::get("/api/health")
            .header("Authorization", "Bearer wrong")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_health_missing_auth() {
        let app = test_router(Some("secret".to_string()));
        let req = Request::get("/api/health").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_pair_no_whatsapp() {
        let app = test_router(None);
        let req = Request::post("/api/pair").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        let json = body_json(resp).await;
        assert!(json["error"].as_str().unwrap().contains("not configured"));
    }

    #[tokio::test]
    async fn test_pair_status_no_whatsapp() {
        let app = test_router(None);
        let req = Request::get("/api/pair/status")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // =======================================================================
    // WEBHOOK TESTS — TDD (Red Phase)
    //
    // These tests define the contract for the inbound webhook feature.
    // They will NOT compile until the developer implements:
    //   1. `WebhookRequest` struct (source, message, mode, channel, target)
    //   2. `webhook` handler function registered at POST /api/webhook
    //   3. Expanded `ApiState` with tx, audit, channel_config fields
    //   4. `source: Option<String>` on `IncomingMessage`
    //   5. `resolve_default_channel()` and `resolve_default_target()`
    // =======================================================================

    // -----------------------------------------------------------------------
    // Must: T-WH-001 — POST /api/webhook returns 200 for valid direct request
    // Requirement: WH-001 (Must), WH-003 (Must)
    // Acceptance: Endpoint accepts POST with JSON body, returns 200 with
    //             status "delivered" for direct mode
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_direct_valid_request_returns_200() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(
            r#"{"source":"todo","message":"Buy milk","mode":"direct","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["status"], "delivered");
        assert_eq!(json["channel"], "telegram");
        assert_eq!(json["target"], "842277204");
    }

    // -----------------------------------------------------------------------
    // Must: T-WH-002 — POST /api/webhook returns 401 for invalid auth
    // Requirement: WH-002 (Must)
    // Acceptance: Invalid/missing bearer token returns 401
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_invalid_auth_returns_401() {
        let app = test_router(Some("secret".to_string()));

        // Wrong token
        let req = webhook_request_auth(
            r#"{"source":"todo","message":"Buy milk","mode":"direct"}"#,
            "wrong-token",
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

        let json = body_json(resp).await;
        assert!(json["error"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_webhook_missing_auth_returns_401() {
        let app = test_router(Some("secret".to_string()));

        // No Authorization header at all
        let req = webhook_request(r#"{"source":"todo","message":"Buy milk","mode":"direct"}"#);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_webhook_no_auth_configured_allows_all() {
        // Requirement: WH-002 — "No auth configured: allow all"
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(
            r#"{"source":"todo","message":"Buy milk","mode":"direct","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        // Should succeed without any auth header when api_key is None
        assert_eq!(resp.status(), StatusCode::OK);
    }

    // -----------------------------------------------------------------------
    // Must: T-WH-003 — POST /api/webhook returns 400 for missing required fields
    // Requirement: WH-004 (Must)
    // Acceptance: Missing source, message, or mode returns 400
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_missing_source_returns_400() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        // Missing source field entirely
        let req = webhook_request(r#"{"message":"Buy milk","mode":"direct"}"#);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_webhook_missing_message_returns_400() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        // Missing message field
        let req = webhook_request(r#"{"source":"todo","mode":"direct"}"#);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_webhook_missing_mode_returns_400() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        // Missing mode field
        let req = webhook_request(r#"{"source":"todo","message":"Buy milk"}"#);
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // -----------------------------------------------------------------------
    // Must: T-WH-004 — POST /api/webhook returns 400 for invalid mode
    // Requirement: WH-004 (Must), WH-010 (Must)
    // Acceptance: Invalid mode value returns 400 with descriptive error
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_invalid_mode_returns_400() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(r#"{"source":"todo","message":"Buy milk","mode":"foo"}"#);
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        let error_msg = json["error"].as_str().unwrap();
        assert!(
            error_msg.contains("invalid mode"),
            "Error should mention 'invalid mode', got: {error_msg}"
        );
        assert!(
            error_msg.contains("direct") && error_msg.contains("ai"),
            "Error should mention valid options 'direct' and 'ai', got: {error_msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Must: T-WH-005 — POST /api/webhook returns 400 for empty message
    // Requirement: WH-004 (Must), WH-010 (Must)
    // Acceptance: Empty message string returns 400 with "message must not be empty"
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_empty_message_returns_400() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(r#"{"source":"todo","message":"","mode":"direct"}"#);
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        let error_msg = json["error"].as_str().unwrap();
        assert!(
            error_msg.contains("message must not be empty"),
            "Error should say 'message must not be empty', got: {error_msg}"
        );
    }

    #[tokio::test]
    async fn test_webhook_whitespace_only_message_returns_400() {
        // Edge case: whitespace-only message should be treated as empty
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(r#"{"source":"todo","message":"   ","mode":"direct"}"#);
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // -----------------------------------------------------------------------
    // Must: T-WH-006 — Direct mode delivers via channel.send()
    // Requirement: WH-003 (Must)
    // Acceptance: Mode "direct" sends text to channel using OutgoingMessage +
    //             channel.send(), message text matches request
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_direct_mode_calls_channel_send() {
        let (mock_ch, sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(
            r#"{"source":"todo","message":"Buy milk","mode":"direct","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // Verify the mock channel received exactly one message
        let sent_msgs = sent.lock().unwrap();
        assert_eq!(sent_msgs.len(), 1, "channel.send() should be called once");
        assert_eq!(sent_msgs[0].text, "Buy milk");
        assert_eq!(
            sent_msgs[0].reply_target.as_deref(),
            Some("842277204"),
            "reply_target should match resolved target"
        );
    }

    #[tokio::test]
    async fn test_webhook_direct_mode_send_failure_returns_502() {
        // Requirement: WH-003, WH-010 — channel.send() fails: HTTP 502
        let mock_ch = MockChannel::new_failing("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(
            r#"{"source":"todo","message":"Buy milk","mode":"direct","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_GATEWAY);
        let json = body_json(resp).await;
        let error_msg = json["error"].as_str().unwrap();
        assert!(
            error_msg.contains("delivery failed"),
            "502 error should mention 'delivery failed', got: {error_msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Must: T-WH-007 — AI mode returns 202 with request_id
    // Requirement: WH-006 (Must), WH-011 (Must)
    // Acceptance: Mode "ai" returns HTTP 202 with status "queued" and a
    //             UUID-format request_id
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_ai_mode_returns_202_with_request_id() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let (tx, _rx) = mpsc::channel::<IncomingMessage>(256);
        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, Some(tx), config);

        let req = webhook_request(
            r#"{"source":"monitor","message":"CPU at 95%","mode":"ai","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let json = body_json(resp).await;
        assert_eq!(json["status"], "queued");
        // request_id should be present and look like a UUID
        let request_id = json["request_id"]
            .as_str()
            .expect("response must include request_id");
        assert!(
            uuid::Uuid::parse_str(request_id).is_ok(),
            "request_id should be valid UUID, got: {request_id}"
        );
    }

    #[tokio::test]
    async fn test_webhook_ai_mode_no_gateway_returns_503() {
        // Requirement: WH-010 — tx is None (gateway not wired): 503
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        // tx = None simulates gateway not wired
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(
            r#"{"source":"monitor","message":"CPU at 95%","mode":"ai","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let json = body_json(resp).await;
        let error_msg = json["error"].as_str().unwrap();
        assert!(
            error_msg.contains("gateway unavailable"),
            "503 should say 'gateway unavailable', got: {error_msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Must: T-WH-008 — AI mode sends IncomingMessage through tx
    // Requirement: WH-006 (Must), WH-007 (Must)
    // Acceptance: Synthetic IncomingMessage sent via tx.send() with correct
    //             channel, sender_id, text prefix, and source field
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_ai_mode_sends_incoming_message_via_tx() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let (tx, mut rx) = mpsc::channel::<IncomingMessage>(256);
        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, Some(tx), config);

        let req = webhook_request(
            r#"{"source":"monitor","message":"CPU at 95%","mode":"ai","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);

        // Verify the IncomingMessage was sent through the mpsc channel
        let incoming = rx
            .try_recv()
            .expect("IncomingMessage should be sent via tx");

        assert_eq!(incoming.channel, "telegram");
        assert_eq!(incoming.sender_id, "842277204");
        assert!(
            incoming.text.contains("[webhook:monitor]"),
            "AI mode text should be prefixed with [webhook:source], got: {}",
            incoming.text
        );
        assert!(
            incoming.text.contains("CPU at 95%"),
            "AI mode text should contain original message, got: {}",
            incoming.text
        );
        assert_eq!(
            incoming.reply_target.as_deref(),
            Some("842277204"),
            "reply_target should match resolved target"
        );
        assert_eq!(incoming.is_group, false);
        // Source field (WH-009 — tested separately but verified here too)
        assert_eq!(incoming.source.as_deref(), Some("monitor"));
    }

    #[tokio::test]
    async fn test_webhook_ai_mode_sender_name_includes_source() {
        // Requirement: WH-006 — sender_name includes webhook source for tracing
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let (tx, mut rx) = mpsc::channel::<IncomingMessage>(256);
        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, Some(tx), config);

        let req = webhook_request(
            r#"{"source":"home-automation","message":"Lights turned off","mode":"ai","channel":"telegram","target":"842277204"}"#,
        );
        let _resp = app.oneshot(req).await.unwrap();

        let incoming = rx.try_recv().unwrap();
        let sender_name = incoming
            .sender_name
            .as_ref()
            .expect("sender_name should be set");
        assert!(
            sender_name.contains("webhook") && sender_name.contains("home-automation"),
            "sender_name should contain 'webhook' and source, got: {sender_name}"
        );
    }

    // -----------------------------------------------------------------------
    // Must: T-WH-009 — Default channel resolution: telegram > whatsapp
    // Requirement: WH-005 (Must)
    // Acceptance: When channel is omitted, telegram is preferred over whatsapp
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_default_channel_prefers_telegram() {
        let (mock_tg, sent_tg) = MockChannel::new("telegram");
        let (mock_wa, sent_wa) = MockChannel::new("whatsapp");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_tg));
        channels.insert("whatsapp".to_string(), Arc::new(mock_wa));

        let config = dual_channel_config(vec![842277204], vec!["5511999887766".to_string()]);
        let app = webhook_router(None, channels, None, config);

        // Omit channel — should default to telegram
        let req = webhook_request(
            r#"{"source":"todo","message":"Buy milk","mode":"direct","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["channel"], "telegram");

        // Telegram should have received the message, not WhatsApp
        assert_eq!(sent_tg.lock().unwrap().len(), 1);
        assert_eq!(sent_wa.lock().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_webhook_default_channel_falls_back_to_whatsapp() {
        // Only WhatsApp configured — should use it as default
        let (mock_wa, sent_wa) = MockChannel::new("whatsapp");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("whatsapp".to_string(), Arc::new(mock_wa));

        let config = ChannelConfig {
            telegram: None,
            whatsapp: Some(WhatsAppConfig {
                enabled: true,
                allowed_users: vec!["5511999887766".to_string()],
                whisper_api_key: None,
            }),
        };
        let app = webhook_router(None, channels, None, config);

        // Omit channel — should fall back to whatsapp
        let req = webhook_request(
            r#"{"source":"todo","message":"Buy milk","mode":"direct","target":"5511999887766"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["channel"], "whatsapp");
        assert_eq!(sent_wa.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_webhook_default_target_uses_first_allowed_user() {
        // Requirement: WH-005 — Omitted target: first allowed_user
        let (mock_ch, sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204, 123456789]);
        let app = webhook_router(None, channels, None, config);

        // Omit target — should use first allowed_user (842277204)
        let req = webhook_request(
            r#"{"source":"todo","message":"Buy milk","mode":"direct","channel":"telegram"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["target"], "842277204");

        let sent_msgs = sent.lock().unwrap();
        assert_eq!(sent_msgs[0].reply_target.as_deref(), Some("842277204"));
    }

    // -----------------------------------------------------------------------
    // Must: T-WH-010 — Returns 400 when no channels configured
    // Requirement: WH-005 (Must), WH-010 (Must)
    // Acceptance: No channels in HashMap → 400 "no channels configured"
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_no_channels_returns_400() {
        // Empty channels HashMap
        let app = webhook_router(None, HashMap::new(), None, ChannelConfig::default());

        let req = webhook_request(r#"{"source":"todo","message":"Buy milk","mode":"direct"}"#);
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        let error_msg = json["error"].as_str().unwrap();
        assert!(
            error_msg.contains("no channels configured"),
            "Should say 'no channels configured', got: {error_msg}"
        );
    }

    #[tokio::test]
    async fn test_webhook_no_default_target_returns_400() {
        // Requirement: WH-005 — allowed_users = [], target omitted: 400
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        // Telegram configured but with empty allowed_users
        let config = telegram_channel_config(vec![]);
        let app = webhook_router(None, channels, None, config);

        // Omit target — no allowed_users to fall back on
        let req = webhook_request(
            r#"{"source":"todo","message":"Buy milk","mode":"direct","channel":"telegram"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        let error_msg = json["error"].as_str().unwrap();
        assert!(
            error_msg.contains("no default target"),
            "Should mention 'no default target', got: {error_msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Must: T-WH-011 — Explicit channel/target in request overrides defaults
    // Requirement: WH-004 (Must), WH-005 (Must)
    // Acceptance: Explicit channel + target are used even when defaults differ
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_explicit_channel_overrides_default() {
        let (mock_tg, sent_tg) = MockChannel::new("telegram");
        let (mock_wa, sent_wa) = MockChannel::new("whatsapp");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_tg));
        channels.insert("whatsapp".to_string(), Arc::new(mock_wa));

        let config = dual_channel_config(vec![842277204], vec!["5511999887766".to_string()]);
        let app = webhook_router(None, channels, None, config);

        // Explicitly request whatsapp even though telegram has higher priority
        let req = webhook_request(
            r#"{"source":"todo","message":"Buy milk","mode":"direct","channel":"whatsapp","target":"5511999887766"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["channel"], "whatsapp");

        // WhatsApp should get the message, not Telegram
        assert_eq!(sent_tg.lock().unwrap().len(), 0);
        assert_eq!(sent_wa.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_webhook_explicit_channel_not_configured_returns_400() {
        // Requirement: WH-010 — channel not found: 400
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(
            r#"{"source":"todo","message":"Buy milk","mode":"direct","channel":"signal","target":"12345"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let json = body_json(resp).await;
        let error_msg = json["error"].as_str().unwrap();
        assert!(
            error_msg.contains("not configured"),
            "Should mention channel not configured, got: {error_msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Should: T-WH-012 — Source field preserved on IncomingMessage in AI mode
    // Requirement: WH-009 (Should)
    // Acceptance: IncomingMessage.source = Some(request.source) for webhook
    //             messages, existing messages get source = None
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_ai_mode_preserves_source_field() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let (tx, mut rx) = mpsc::channel::<IncomingMessage>(256);
        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, Some(tx), config);

        let req = webhook_request(
            r#"{"source":"home-automation","message":"Motion detected","mode":"ai","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);

        let incoming = rx.try_recv().unwrap();
        assert_eq!(
            incoming.source.as_deref(),
            Some("home-automation"),
            "source field should preserve the webhook source identifier"
        );
    }

    // -----------------------------------------------------------------------
    // Should: T-WH-013 — Audit entry created for direct mode delivery
    // Requirement: WH-008 (Should)
    // Acceptance: Direct mode creates AuditEntry with source in sender_name.
    //             Note: This test verifies the audit function is called; full
    //             audit verification requires SQLite integration test.
    //             Here we verify the response shape (audit is best-effort and
    //             doesn't block the response).
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_direct_mode_returns_correct_response_shape() {
        // Requirement: WH-008, WH-011 — verify response contract for direct mode
        // Audit logging is best-effort; we verify the HTTP response shape here.
        // A full audit integration test would require SQLite setup.
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(
            r#"{"source":"daily-report","message":"All systems nominal","mode":"direct","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        // Verify all required fields in direct-mode response (WH-011)
        assert_eq!(json["status"], "delivered");
        assert!(
            json["channel"].as_str().is_some(),
            "response must include channel"
        );
        assert!(
            json["target"].as_str().is_some(),
            "response must include target"
        );
        // Should NOT have request_id (that's AI mode only)
        assert!(
            json.get("request_id").is_none(),
            "direct mode should not have request_id"
        );
    }

    // -----------------------------------------------------------------------
    // Edge case: Invalid JSON body
    // Requirement: WH-001, WH-010 — invalid JSON returns 400
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_invalid_json_returns_400() {
        let app = test_router(None);

        let req = Request::post("/api/webhook")
            .header("Content-Type", "application/json")
            .body(Body::from("not json at all"))
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();

        // axum's Json extractor returns 400 or 422 for parse failure
        let status = resp.status().as_u16();
        assert!(
            status == 400 || status == 422,
            "Invalid JSON should return 400 or 422, got: {status}"
        );
    }

    // -----------------------------------------------------------------------
    // Edge case: Unicode/emoji in message
    // Requirement: WH-004 — message content with special characters
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_unicode_message_accepted() {
        let (mock_ch, sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        // Note: \u{XXXX} is Rust syntax, not valid JSON. JSON uses \uXXXX (4 hex digits)
        // or surrogate pairs for characters outside BMP. Using literal UTF-8 here instead.
        let body = format!(
            r#"{{"source":"todo","message":"Comprar leche {} y pan {}","mode":"direct","channel":"telegram","target":"842277204"}}"#,
            '\u{1f95b}', '\u{1f35e}'
        );
        let req = webhook_request(&body);
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let sent_msgs = sent.lock().unwrap();
        assert!(
            sent_msgs[0].text.contains('\u{1f95b}'),
            "Unicode should be preserved"
        );
    }

    // -----------------------------------------------------------------------
    // Edge case: Very large message
    // Requirement: WH-004 — large input handling
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_large_message_accepted() {
        let (mock_ch, sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        // 10KB message
        let large_msg = "x".repeat(10_000);
        let body = format!(
            r#"{{"source":"bulk","message":"{}","mode":"direct","channel":"telegram","target":"842277204"}}"#,
            large_msg
        );
        let req = webhook_request(&body);
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let sent_msgs = sent.lock().unwrap();
        assert_eq!(sent_msgs[0].text.len(), 10_000);
    }

    // -----------------------------------------------------------------------
    // Edge case: AI mode with dropped rx (gateway shutdown)
    // Requirement: WH-010 — tx.send() fails when receiver dropped
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_ai_mode_dropped_receiver_returns_503() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let (tx, rx) = mpsc::channel::<IncomingMessage>(256);
        // Drop the receiver to simulate gateway shutdown
        drop(rx);

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, Some(tx), config);

        let req = webhook_request(
            r#"{"source":"monitor","message":"Test","mode":"ai","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        let json = body_json(resp).await;
        let error_msg = json["error"].as_str().unwrap();
        assert!(
            error_msg.contains("gateway unavailable"),
            "Should say 'gateway unavailable' when rx dropped, got: {error_msg}"
        );
    }

    // -----------------------------------------------------------------------
    // Edge case: Empty source string
    // Requirement: WH-004 — source must be non-empty
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_empty_source_returns_400() {
        let (mock_ch, _sent) = MockChannel::new("telegram");
        let mut channels: HashMap<String, Arc<dyn Channel>> = HashMap::new();
        channels.insert("telegram".to_string(), Arc::new(mock_ch));

        let config = telegram_channel_config(vec![842277204]);
        let app = webhook_router(None, channels, None, config);

        let req = webhook_request(
            r#"{"source":"","message":"Buy milk","mode":"direct","channel":"telegram","target":"842277204"}"#,
        );
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    }

    // -----------------------------------------------------------------------
    // Edge case: GET method on webhook endpoint (should be rejected)
    // Requirement: WH-001 — endpoint only accepts POST
    // -----------------------------------------------------------------------
    #[tokio::test]
    async fn test_webhook_get_method_returns_405() {
        let app = test_router(None);

        let req = Request::get("/api/webhook").body(Body::empty()).unwrap();
        let resp = app.oneshot(req).await.unwrap();

        assert_eq!(resp.status(), StatusCode::METHOD_NOT_ALLOWED);
    }
}
