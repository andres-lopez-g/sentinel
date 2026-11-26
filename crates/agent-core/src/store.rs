//! The local SQLite spool and source state.

use std::fs;
use std::io::Write;
use std::path::Path;

use chrono::{DateTime, Duration, SecondsFormat, Utc};
use flate2::write::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use rusqlite::{params, Connection, OptionalExtension};
use sentinel_protocol::NormalizedEvent;
use sha2::Digest;

use crate::error::StoreError;

pub const RAW_RETENTION_DAYS: i64 = 30;
pub const FACT_RETENTION_DAYS: i64 = 365;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS source_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    source TEXT NOT NULL,
    generation TEXT NOT NULL,
    last_record_id INTEGER,
    degraded_reason TEXT,
    bookmark_xml TEXT,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS fact (
    generation TEXT NOT NULL,
    record_id TEXT NOT NULL,
    event_id TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    received_at TEXT NOT NULL,
    code TEXT NOT NULL,
    outcome TEXT NOT NULL,
    account TEXT,
    account_id TEXT,
    logon_type INTEGER,
    source_ip TEXT,
    digest TEXT NOT NULL,
    PRIMARY KEY (generation, record_id)
);
CREATE INDEX IF NOT EXISTS fact_received_idx ON fact (received_at, generation, record_id);
CREATE INDEX IF NOT EXISTS fact_code_idx ON fact (code, outcome, occurred_at);

CREATE TABLE IF NOT EXISTS raw_event (
    generation TEXT NOT NULL,
    record_id TEXT NOT NULL,
    day TEXT NOT NULL,
    bytes INTEGER NOT NULL,
    payload BLOB NOT NULL,
    PRIMARY KEY (generation, record_id),
    FOREIGN KEY (generation, record_id)
        REFERENCES fact (generation, record_id) ON DELETE CASCADE
);
CREATE INDEX IF NOT EXISTS raw_day_idx ON raw_event (day);

CREATE TABLE IF NOT EXISTS daily_count (
    day TEXT NOT NULL,
    code TEXT NOT NULL,
    outcome TEXT NOT NULL,
    count INTEGER NOT NULL,
    PRIMARY KEY (day, code, outcome)
);

CREATE TABLE IF NOT EXISTS gap (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    observed_at TEXT NOT NULL,
    generation TEXT NOT NULL,
    reason TEXT NOT NULL,
    last_good_record_id INTEGER,
    lower_bound INTEGER,
    recovered_at TEXT
);

CREATE TABLE IF NOT EXISTS audit (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    at TEXT NOT NULL,
    action TEXT NOT NULL,
    detail TEXT NOT NULL
);
";

/// Outcome of trying to admit one source record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Admission {
    /// New fact, raw record and bookmark committed together.
    Inserted,
    /// Same identity and payload already stored; nothing changed.
    Duplicate,
    /// Same identity but a different payload; the caller must quarantine it.
    Conflict,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceState {
    pub source: String,
    pub generation: String,
    pub last_record_id: Option<u64>,
    pub degraded_reason: Option<String>,
    pub bookmark_xml: Option<String>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Gap {
    pub id: i64,
    pub observed_at: DateTime<Utc>,
    pub generation: String,
    pub reason: String,
    pub last_good_record_id: Option<u64>,
    /// Lower bound on lost channel records; eligible losses may be unknown.
    pub lower_bound: Option<u64>,
    pub recovered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Health {
    pub source: String,
    pub generation: String,
    pub last_record_id: Option<u64>,
    pub degraded_reason: Option<String>,
    pub open_gaps: i64,
    pub raw_bytes: i64,
    pub fact_count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetentionReport {
    pub raw_deleted: usize,
    pub facts_rolled_up: usize,
    pub audit_id: i64,
}

pub struct Store {
    connection: Connection,
}

impl Store {
    /// Open or create a WAL-mode database, applying the incremental vacuum
    /// setting before the first table exists.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch("PRAGMA foreign_keys = ON;")?;
        let journal: String =
            connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
        if journal != "wal" {
            return Err(StoreError::JournalMode(journal));
        }
        let auto_vacuum: i64 = connection.query_row("PRAGMA auto_vacuum", [], |row| row.get(0))?;
        if auto_vacuum == 0 {
            connection.execute_batch("PRAGMA auto_vacuum = INCREMENTAL; VACUUM;")?;
        }
        connection.execute_batch(SCHEMA)?;
        Ok(Self { connection })
    }

    /// Store a normalized record and advance the channel bookmark atomically.
    pub fn admit(
        &mut self,
        event: &NormalizedEvent,
        raw_xml: &str,
        received_at: DateTime<Utc>,
    ) -> Result<Admission, StoreError> {
        self.admit_with_bookmark(event, raw_xml, None, received_at)
    }

    /// Store a record and the source bookmark that produced it in one
    /// transaction. A crash cannot leave the channel ahead of the database.
    pub fn admit_with_bookmark(
        &mut self,
        event: &NormalizedEvent,
        raw_xml: &str,
        bookmark_xml: Option<&str>,
        received_at: DateTime<Utc>,
    ) -> Result<Admission, StoreError> {
        let transaction = self.connection.transaction()?;
        let (generation, record_id) = event.local_key();
        let digest = digest(event)?;

        let existing: Option<String> = transaction
            .query_row(
                "SELECT digest FROM fact WHERE generation = ?1 AND record_id = ?2",
                params![generation, record_id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing) = existing {
            return Ok(if existing == digest {
                Admission::Duplicate
            } else {
                Admission::Conflict
            });
        }

        let received = format_time(received_at);
        transaction.execute(
            "INSERT INTO fact (
                generation, record_id, event_id, occurred_at, received_at,
                code, outcome, account, account_id, logon_type, source_ip, digest
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
            params![
                generation,
                record_id,
                event.event.id.to_string(),
                format_time(event.timestamp),
                received,
                event.event.code,
                outcome_name(event),
                event.user.name,
                event.user.id,
                event.source.logon_type.map(|value| value.code()),
                event.source.ip.map(|value| value.to_string()),
                digest,
            ],
        )?;
        transaction.execute(
            "INSERT INTO raw_event (generation, record_id, day, bytes, payload)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                generation,
                record_id,
                day(received_at),
                raw_xml.len() as i64,
                compress(raw_xml)?,
            ],
        )?;
        transaction.execute(
            "INSERT INTO source_state (id, source, generation, last_record_id, bookmark_xml, updated_at)
             VALUES (1, ?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET
                generation = excluded.generation,
                last_record_id = CASE
                    WHEN excluded.last_record_id IS NULL THEN source_state.last_record_id
                    WHEN source_state.last_record_id IS NULL THEN excluded.last_record_id
                    ELSE MAX(source_state.last_record_id, excluded.last_record_id)
                END,
                bookmark_xml = COALESCE(excluded.bookmark_xml, source_state.bookmark_xml),
                updated_at = excluded.updated_at",
            params![
                event.sentinel.source,
                generation,
                record_id.parse::<u64>().ok().map(|value| value as i64),
                bookmark_xml,
                received,
            ],
        )?;
        transaction.commit()?;
        Ok(Admission::Inserted)
    }

    pub fn source_state(&self) -> Result<Option<SourceState>, StoreError> {
        self.connection
            .query_row(
                "SELECT source, generation, last_record_id, degraded_reason, bookmark_xml, updated_at
                 FROM source_state WHERE id = 1",
                [],
                |row| {
                    Ok(SourceState {
                        source: row.get(0)?,
                        generation: row.get(1)?,
                        last_record_id: row.get::<_, Option<i64>>(2)?.map(|value| value as u64),
                        degraded_reason: row.get(3)?,
                        bookmark_xml: row.get(4)?,
                        updated_at: parse_row_time(&row.get::<_, String>(5)?)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    /// Persisted channel bookmark, used to resume after a service restart.
    pub fn bookmark(&self) -> Result<Option<String>, StoreError> {
        Ok(self.source_state()?.and_then(|state| state.bookmark_xml))
    }

    /// Start a new source generation after a stale bookmark or channel clear.
    pub fn begin_generation(
        &mut self,
        generation: &str,
        at: DateTime<Utc>,
    ) -> Result<(), StoreError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE source_state SET generation = ?1, last_record_id = NULL,
                degraded_reason = NULL, bookmark_xml = NULL, updated_at = ?2 WHERE id = 1",
            params![generation, format_time(at)],
        )?;
        audit(&transaction, at, "generation_started", generation)?;
        transaction.commit()?;
        Ok(())
    }

    /// Record a source health event. `lower_bound` is a known lower bound on
    /// lost channel records; eligible losses remain unknown when it is absent.
    pub fn record_gap(
        &mut self,
        reason: &str,
        last_good_record_id: Option<u64>,
        lower_bound: Option<u64>,
        observed_at: DateTime<Utc>,
    ) -> Result<i64, StoreError> {
        let transaction = self.connection.transaction()?;
        let generation: String = transaction.query_row(
            "SELECT generation FROM source_state WHERE id = 1",
            [],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO gap (observed_at, generation, reason, last_good_record_id, lower_bound)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                format_time(observed_at),
                generation,
                reason,
                last_good_record_id.map(|value| value as i64),
                lower_bound.map(|value| value as i64),
            ],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.execute(
            "UPDATE source_state SET degraded_reason = ?1, updated_at = ?2 WHERE id = 1",
            params![reason, format_time(observed_at)],
        )?;
        audit(&transaction, observed_at, "gap_recorded", reason)?;
        transaction.commit()?;
        Ok(id)
    }

    pub fn resolve_gap(&mut self, id: i64, at: DateTime<Utc>) -> Result<(), StoreError> {
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "UPDATE gap SET recovered_at = ?1 WHERE id = ?2",
            params![format_time(at), id],
        )?;
        transaction.execute(
            "UPDATE source_state SET degraded_reason = NULL, updated_at = ?1
             WHERE id = 1 AND NOT EXISTS (
                SELECT 1 FROM gap WHERE recovered_at IS NULL
             )",
            params![format_time(at)],
        )?;
        audit(&transaction, at, "gap_resolved", &id.to_string())?;
        transaction.commit()?;
        Ok(())
    }

    pub fn gaps(&self, only_open: bool) -> Result<Vec<Gap>, StoreError> {
        let sql = if only_open {
            "SELECT id, observed_at, generation, reason, last_good_record_id, lower_bound,
                    recovered_at FROM gap WHERE recovered_at IS NULL ORDER BY id"
        } else {
            "SELECT id, observed_at, generation, reason, last_good_record_id, lower_bound,
                    recovered_at FROM gap ORDER BY id"
        };
        let mut statement = self.connection.prepare(sql)?;
        let rows = statement.query_map([], |row| {
            Ok(Gap {
                id: row.get(0)?,
                observed_at: parse_row_time(&row.get::<_, String>(1)?)?,
                generation: row.get(2)?,
                reason: row.get(3)?,
                last_good_record_id: row.get::<_, Option<i64>>(4)?.map(|value| value as u64),
                lower_bound: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
                recovered_at: row
                    .get::<_, Option<String>>(6)?
                    .map(|value| parse_row_time(&value))
                    .transpose()?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Return the rendered XML for one admitted record, decompressed.
    pub fn raw_event(
        &self,
        generation: &str,
        record_id: &str,
    ) -> Result<Option<String>, StoreError> {
        let payload: Option<Vec<u8>> = self
            .connection
            .query_row(
                "SELECT payload FROM raw_event WHERE generation = ?1 AND record_id = ?2",
                params![generation, record_id],
                |row| row.get(0),
            )
            .optional()?;
        payload.map(decompress).transpose()
    }

    pub fn health(&self) -> Result<Health, StoreError> {
        let state = self.source_state()?;
        let raw_bytes: i64 = self.connection.query_row(
            "SELECT COALESCE(SUM(bytes), 0) FROM raw_event",
            [],
            |row| row.get(0),
        )?;
        let fact_count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM fact", [], |row| row.get(0))?;
        let open_gaps: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM gap WHERE recovered_at IS NULL",
            [],
            |row| row.get(0),
        )?;
        Ok(Health {
            source: state
                .as_ref()
                .map(|value| value.source.clone())
                .unwrap_or_else(|| "unknown".to_string()),
            generation: state
                .as_ref()
                .map(|value| value.generation.clone())
                .unwrap_or_default(),
            last_record_id: state.as_ref().and_then(|value| value.last_record_id),
            degraded_reason: state.and_then(|value| value.degraded_reason),
            open_gaps,
            raw_bytes,
            fact_count,
        })
    }

    /// Delete expired raw records and roll expired facts into daily counts.
    pub fn apply_retention(&mut self, now: DateTime<Utc>) -> Result<RetentionReport, StoreError> {
        let raw_cutoff = day(now - Duration::days(RAW_RETENTION_DAYS));
        let fact_cutoff = format_time(now - Duration::days(FACT_RETENTION_DAYS));
        let transaction = self.connection.transaction()?;
        let raw_deleted =
            transaction.execute("DELETE FROM raw_event WHERE day < ?1", params![raw_cutoff])?;
        transaction.execute(
            "INSERT INTO daily_count (day, code, outcome, count)
             SELECT substr(occurred_at, 1, 10), code, outcome, COUNT(*)
             FROM fact WHERE occurred_at < ?1
             GROUP BY substr(occurred_at, 1, 10), code, outcome
             ON CONFLICT(day, code, outcome) DO UPDATE
             SET count = daily_count.count + excluded.count",
            params![fact_cutoff],
        )?;
        let facts_rolled_up = transaction.execute(
            "DELETE FROM fact WHERE occurred_at < ?1",
            params![fact_cutoff],
        )?;
        audit(
            &transaction,
            now,
            "retention",
            &format!("raw={raw_deleted} facts={facts_rolled_up}"),
        )?;
        let audit_id = transaction.last_insert_rowid();
        transaction.commit()?;
        self.connection
            .execute_batch("PRAGMA incremental_vacuum;")?;
        Ok(RetentionReport {
            raw_deleted,
            facts_rolled_up,
            audit_id,
        })
    }

    pub fn audit_count(&self) -> Result<i64, StoreError> {
        self.connection
            .query_row("SELECT COUNT(*) FROM audit", [], |row| row.get(0))
            .map_err(Into::into)
    }

    pub fn daily_count(&self, day: &str, code: &str, outcome: &str) -> Result<i64, StoreError> {
        self.connection
            .query_row(
                "SELECT count FROM daily_count WHERE day = ?1 AND code = ?2 AND outcome = ?3",
                params![day, code, outcome],
                |row| row.get(0),
            )
            .optional()
            .map(|value| value.unwrap_or(0))
            .map_err(Into::into)
    }
}

fn outcome_name(event: &NormalizedEvent) -> &'static str {
    match event.event.outcome {
        sentinel_protocol::Outcome::Success => "success",
        sentinel_protocol::Outcome::Failure => "failure",
        sentinel_protocol::Outcome::Unknown => "unknown",
    }
}

fn digest(event: &NormalizedEvent) -> Result<String, StoreError> {
    let canonical = serde_json::to_vec(event)?;
    Ok(format!("{:x}", sha2::Sha256::digest(&canonical)))
}

fn format_time(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}

/// Row mappers must return `rusqlite::Error`, so map parse failures into the
/// conversion failure variant instead of the crate-level error.
fn parse_row_time(value: &str) -> Result<DateTime<Utc>, rusqlite::Error> {
    DateTime::parse_from_rfc3339(value)
        .map(|moment| moment.with_timezone(&Utc))
        .map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                0,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })
}

fn day(value: DateTime<Utc>) -> String {
    value.format("%Y-%m-%d").to_string()
}

fn compress(xml: &str) -> Result<Vec<u8>, StoreError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(xml.as_bytes())?;
    Ok(encoder.finish()?)
}

fn decompress(payload: Vec<u8>) -> Result<String, StoreError> {
    let mut decoder = GzDecoder::new(Vec::new());
    decoder.write_all(&payload)?;
    Ok(String::from_utf8(decoder.finish()?)?)
}

fn audit(
    transaction: &rusqlite::Transaction<'_>,
    at: DateTime<Utc>,
    action: &str,
    detail: &str,
) -> Result<(), StoreError> {
    transaction.execute(
        "INSERT INTO audit (at, action, detail) VALUES (?1, ?2, ?3)",
        params![format_time(at), action, detail],
    )?;
    Ok(())
}
