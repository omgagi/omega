//! Audit log — records every interaction through Omega.

use omega_core::error::OmegaError;
use sqlx::SqlitePool;
use tracing::debug;
use uuid::Uuid;

/// An entry to write to the audit log.
pub struct AuditEntry {
    pub channel: String,
    pub sender_id: String,
    pub sender_name: Option<String>,
    pub input_text: String,
    pub output_text: Option<String>,
    pub provider_used: Option<String>,
    pub model: Option<String>,
    pub processing_ms: Option<i64>,
    pub status: AuditStatus,
    pub denial_reason: Option<String>,
}

/// Status of an audited interaction.
pub enum AuditStatus {
    Ok,
    Error,
    Denied,
}

impl AuditStatus {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Error => "error",
            Self::Denied => "denied",
        }
    }
}

/// Audit logger backed by SQLite.
pub struct AuditLogger {
    pool: SqlitePool,
}

impl AuditLogger {
    /// Create a new audit logger sharing the given pool.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Write an entry to the audit log.
    pub async fn log(&self, entry: &AuditEntry) -> Result<(), OmegaError> {
        let id = Uuid::new_v4().to_string();

        sqlx::query(
            "INSERT INTO audit_log \
             (id, channel, sender_id, sender_name, input_text, output_text, \
              provider_used, model, processing_ms, status, denial_reason) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&entry.channel)
        .bind(&entry.sender_id)
        .bind(&entry.sender_name)
        .bind(&entry.input_text)
        .bind(&entry.output_text)
        .bind(&entry.provider_used)
        .bind(&entry.model)
        .bind(entry.processing_ms)
        .bind(entry.status.as_str())
        .bind(&entry.denial_reason)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("audit log write failed: {e}")))?;

        debug!(
            "audit: {} {} [{}] {}",
            entry.channel,
            entry.sender_id,
            entry.status.as_str(),
            truncate(&entry.input_text, 80)
        );

        Ok(())
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        &s[..max]
    }
}
