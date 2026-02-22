# Specification: omega-memory/migrations/010_outcomes.sql

## Path

`crates/omega-memory/migrations/010_outcomes.sql`

## Purpose

Creates the `outcomes` and `lessons` tables for a two-tier reward-based learning system. Outcomes are short-term working memory (24-48h window) — raw reward signals from every interaction. Lessons are long-term behavioral rules — distilled patterns that persist permanently and shape future behavior.

Before this migration, OMEGA had no structured way to learn from interaction outcomes. Facts stored user preferences, but there was no mechanism to track what worked, what was redundant, and what patterns emerged across interactions.

## Prerequisites

- Migration `001_init.sql` must have been applied (creates the database structure).

---

## Schema Changes

### New Table: `outcomes`

```sql
CREATE TABLE IF NOT EXISTS outcomes (
    id        TEXT PRIMARY KEY,
    timestamp TEXT NOT NULL DEFAULT (datetime('now')),
    sender_id TEXT NOT NULL,
    domain    TEXT NOT NULL,
    score     INTEGER NOT NULL CHECK (score IN (-1, 0, 1)),
    lesson    TEXT NOT NULL,
    source    TEXT NOT NULL DEFAULT 'conversation'
);

CREATE INDEX IF NOT EXISTS idx_outcomes_sender_time ON outcomes (sender_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_outcomes_time ON outcomes (timestamp);
```

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | `TEXT` | `PRIMARY KEY` | UUID v4 string. |
| `timestamp` | `TEXT` | `NOT NULL`, default `datetime('now')` | When the outcome was recorded. |
| `sender_id` | `TEXT` | `NOT NULL` | The user this outcome belongs to. |
| `domain` | `TEXT` | `NOT NULL` | Domain/category (e.g., `"training"`, `"crypto"`, `"scheduling"`). |
| `score` | `INTEGER` | `NOT NULL`, CHECK `IN (-1, 0, 1)` | Reward signal: `+1` (helpful), `0` (neutral), `-1` (redundant/annoying). |
| `lesson` | `TEXT` | `NOT NULL` | What was learned from this interaction. |
| `source` | `TEXT` | `NOT NULL`, default `'conversation'` | Origin: `'conversation'` or `'heartbeat'`. |

**Indexes:**
- `idx_outcomes_sender_time` on `(sender_id, timestamp)` -- per-sender recent outcome queries.
- `idx_outcomes_time` on `(timestamp)` -- cross-user time-windowed queries for heartbeat enrichment.

### New Table: `lessons`

```sql
CREATE TABLE IF NOT EXISTS lessons (
    id          TEXT PRIMARY KEY,
    sender_id   TEXT NOT NULL,
    domain      TEXT NOT NULL,
    rule        TEXT NOT NULL,
    occurrences INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(sender_id, domain)
);

CREATE INDEX IF NOT EXISTS idx_lessons_sender ON lessons (sender_id);
```

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | `TEXT` | `PRIMARY KEY` | UUID v4 string. |
| `sender_id` | `TEXT` | `NOT NULL` | The user this lesson belongs to. |
| `domain` | `TEXT` | `NOT NULL` | Domain/category. |
| `rule` | `TEXT` | `NOT NULL` | The distilled behavioral rule. |
| `occurrences` | `INTEGER` | `NOT NULL`, default `1` | Times this domain's lesson was updated (confidence signal). |
| `created_at` | `TEXT` | `NOT NULL`, default `datetime('now')` | First created. |
| `updated_at` | `TEXT` | `NOT NULL`, default `datetime('now')` | Last updated. |

**Unique constraint:** `(sender_id, domain)` -- one lesson per domain per user. Upserts replace the rule and increment occurrences.

**Index:**
- `idx_lessons_sender` on `(sender_id)` -- per-sender lesson queries.

---

## Backward Compatibility

Both tables are new (`CREATE TABLE IF NOT EXISTS`), so existing databases are unaffected. No ALTER TABLE statements. No existing data is modified.

---

## Migration Tracking

This migration is registered with name `"010_outcomes"` in the `_migrations` table.

**Migration definitions (compile-time embedded):**
```rust
("010_outcomes", include_str!("../migrations/010_outcomes.sql"))
```

---

## Application-Level Usage

### Store Methods

| Method | Purpose |
|--------|---------|
| `store_outcome(sender_id, domain, score, lesson, source)` | Insert a raw outcome from a REWARD marker. |
| `get_recent_outcomes(sender_id, limit)` | Get the N most recent outcomes for a sender (regular context injection, limit=15). |
| `get_all_recent_outcomes(hours, limit)` | Get outcomes within a time window across all users (heartbeat enrichment, hours=24, limit=20). |
| `store_lesson(sender_id, domain, rule)` | Upsert a distilled lesson by (sender_id, domain). Replaces rule, increments occurrences. |
| `get_lessons(sender_id)` | Get all lessons for a sender (regular context injection). |
| `get_all_lessons()` | Get all lessons across all users (heartbeat enrichment). |

### Marker Protocol

| Marker | Format | Storage |
|--------|--------|---------|
| `REWARD:` | `REWARD: +1\|domain\|lesson` | `outcomes` table via `store_outcome()` |
| `LESSON:` | `LESSON: domain\|rule` | `lessons` table via `store_lesson()` |

### Context Injection

- **Regular messages:** `build_context()` always loads outcomes (limit 15) and lessons (all) for the sender. Injected into the system prompt as "Learned behavioral rules:" and "Recent outcomes:" sections.
- **Heartbeat:** `build_enrichment()` loads all lessons and last-24h outcomes across all users. Injected into the heartbeat context.

---

## Relationship to Other Migrations

| Migration | Name | What It Creates |
|-----------|------|----------------|
| `001_init.sql` | `001_init` | `conversations`, `messages`, `facts` tables |
| `009_task_retry.sql` | `009_task_retry` | ALTER `scheduled_tasks` (+2 cols) |
| **`010_outcomes.sql`** | **`010_outcomes`** | **`outcomes` + `lessons` tables, 3 indexes** |
