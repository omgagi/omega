# HTTP API Server (backend/src/api.rs)

## Overview

Lightweight HTTP API server built on `axum` for SaaS dashboard integration and external tool communication. Spawned as a background task in the gateway (same pattern as scheduler/heartbeat). Provides health checks, WhatsApp QR pairing over HTTP, and an inbound webhook for external tools.

## Configuration

Enable in `config.toml`:

```toml
[api]
enabled = true
host = "127.0.0.1"    # Localhost only -- use reverse proxy for external access
port = 3000
api_key = "your-secret-token"  # Bearer token auth. Empty = no auth
```

## Authentication

All endpoints require Bearer token authentication when `api_key` is configured:

```
Authorization: Bearer your-secret-token
```

When `api_key` is empty (default), all requests are allowed without auth. Token comparison uses constant-time comparison to prevent timing attacks.

## Endpoints

### `GET /api/health`

Returns system health status.

**Response (200):**
```json
{
  "status": "ok",
  "uptime_secs": 3600,
  "whatsapp": "connected"
}
```

WhatsApp status values: `connected`, `disconnected`, `not_configured`, `error`.

### `POST /api/pair`

Trigger WhatsApp pairing and get QR code.

**Response (200) -- already paired:**
```json
{ "status": "already_paired" }
```

**Response (200) -- QR ready:**
```json
{
  "status": "qr_ready",
  "qr_png_base64": "iVBORw0KGgo..."
}
```

The `qr_png_base64` field contains a base64-encoded PNG image of the QR code. Display it in your dashboard for scanning.

### `GET /api/pair/status`

Long-poll for pairing completion (up to 60 seconds).

**Response (200):**
```json
{ "status": "paired" }
```
or
```json
{ "status": "pending" }
```

### `POST /api/webhook`

Inbound webhook for external tool message delivery.

**Request body:**
```json
{
  "source": "todo-app",
  "message": "Task completed: Deploy v2.1",
  "mode": "direct",
  "channel": "telegram",
  "target": "842277204"
}
```

| Field | Required | Description |
|-------|----------|-------------|
| `source` | Yes | Tool identifier (e.g., "todo-app", "monitoring") |
| `message` | Yes | Text to deliver |
| `mode` | Yes | `"direct"` (send as-is) or `"ai"` (process through AI pipeline) |
| `channel` | No | Channel override (defaults to first available: telegram > whatsapp) |
| `target` | No | Target override (defaults to first `allowed_users` entry) |

**Direct mode response (200):**
```json
{
  "status": "delivered",
  "channel": "telegram",
  "target": "842277204"
}
```

**AI mode response (202):**
```json
{
  "status": "queued",
  "request_id": "550e8400-e29b-41d4-a716-446655440000"
}
```

## Error Responses

| Status | When |
|--------|------|
| 400 | Validation error (missing field, unknown channel, no default target) |
| 401 | Invalid or missing Bearer token |
| 405 | Wrong HTTP method |
| 502 | Channel send failure (direct mode) |
| 503 | Gateway unavailable (AI mode, dropped sender) |

## Usage Examples

### Health check
```bash
curl -H "Authorization: Bearer your-token" http://localhost:3000/api/health
```

### Send a direct message
```bash
curl -X POST http://localhost:3000/api/webhook \
  -H "Authorization: Bearer your-token" \
  -H "Content-Type: application/json" \
  -d '{"source":"cron","message":"Backup complete","mode":"direct"}'
```

### Send a message through AI pipeline
```bash
curl -X POST http://localhost:3000/api/webhook \
  -H "Authorization: Bearer your-token" \
  -H "Content-Type: application/json" \
  -d '{"source":"alert-system","message":"Server CPU at 95% for 10 minutes","mode":"ai"}'
```

## Design Notes

- **Localhost by default:** The API binds to `127.0.0.1` only. Use a reverse proxy (nginx, caddy) for external access.
- **Same binary:** No separate service needed -- the API runs as a background task within the Omega process.
- **Channel resolution:** When channel/target are not specified, the API resolves defaults: first enabled channel by priority (telegram > whatsapp), first `allowed_users` entry as target.
- **Audit logging:** Direct mode webhook deliveries are logged in the audit system. AI mode messages enter the normal pipeline and are audited there.
