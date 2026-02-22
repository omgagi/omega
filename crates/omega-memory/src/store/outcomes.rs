//! Reward-based learning: raw outcomes (working memory) and distilled lessons (long-term memory).

use super::Store;
use omega_core::error::OmegaError;

impl Store {
    /// Store a raw outcome from a REWARD marker.
    pub async fn store_outcome(
        &self,
        sender_id: &str,
        domain: &str,
        score: i32,
        lesson: &str,
        source: &str,
    ) -> Result<(), OmegaError> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO outcomes (id, sender_id, domain, score, lesson, source) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(sender_id)
        .bind(domain)
        .bind(score)
        .bind(lesson)
        .bind(source)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("store outcome: {e}")))?;
        Ok(())
    }

    /// Get recent outcomes for a sender (for regular conversation prompt injection).
    ///
    /// Returns `(score, domain, lesson, timestamp)` ordered newest first.
    pub async fn get_recent_outcomes(
        &self,
        sender_id: &str,
        limit: i64,
    ) -> Result<Vec<(i32, String, String, String)>, OmegaError> {
        let rows: Vec<(i32, String, String, String)> = sqlx::query_as(
            "SELECT score, domain, lesson, timestamp FROM outcomes \
             WHERE sender_id = ? ORDER BY timestamp DESC LIMIT ?",
        )
        .bind(sender_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("get recent outcomes: {e}")))?;
        Ok(rows)
    }

    /// Get recent outcomes across all users (for heartbeat enrichment).
    ///
    /// Returns `(score, domain, lesson, timestamp)` within the last N hours.
    pub async fn get_all_recent_outcomes(
        &self,
        hours: i64,
        limit: i64,
    ) -> Result<Vec<(i32, String, String, String)>, OmegaError> {
        let rows: Vec<(i32, String, String, String)> = sqlx::query_as(
            "SELECT score, domain, lesson, timestamp FROM outcomes \
             WHERE datetime(timestamp) >= datetime('now', ? || ' hours') \
             ORDER BY timestamp DESC LIMIT ?",
        )
        .bind(-hours)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("get all recent outcomes: {e}")))?;
        Ok(rows)
    }

    /// Store or update a distilled lesson (upsert by sender_id + domain).
    ///
    /// If a lesson already exists for this domain, the rule is replaced and
    /// occurrences is incremented. Otherwise a new lesson is created.
    pub async fn store_lesson(
        &self,
        sender_id: &str,
        domain: &str,
        rule: &str,
    ) -> Result<(), OmegaError> {
        let id = uuid::Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO lessons (id, sender_id, domain, rule) VALUES (?, ?, ?, ?) \
             ON CONFLICT(sender_id, domain) DO UPDATE SET \
             rule = excluded.rule, \
             occurrences = occurrences + 1, \
             updated_at = datetime('now')",
        )
        .bind(&id)
        .bind(sender_id)
        .bind(domain)
        .bind(rule)
        .execute(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("store lesson: {e}")))?;
        Ok(())
    }

    /// Get all lessons for a sender.
    ///
    /// Returns `(domain, rule)` ordered by most-updated first.
    pub async fn get_lessons(&self, sender_id: &str) -> Result<Vec<(String, String)>, OmegaError> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT domain, rule FROM lessons \
             WHERE sender_id = ? ORDER BY updated_at DESC",
        )
        .bind(sender_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| OmegaError::Memory(format!("get lessons: {e}")))?;
        Ok(rows)
    }

    /// Get all lessons across all users (for heartbeat enrichment).
    ///
    /// Returns `(domain, rule)` ordered by most-updated first.
    pub async fn get_all_lessons(&self) -> Result<Vec<(String, String)>, OmegaError> {
        let rows: Vec<(String, String)> =
            sqlx::query_as("SELECT domain, rule FROM lessons ORDER BY updated_at DESC")
                .fetch_all(&self.pool)
                .await
                .map_err(|e| OmegaError::Memory(format!("get all lessons: {e}")))?;
        Ok(rows)
    }
}
