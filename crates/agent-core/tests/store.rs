use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Duration, Utc};
use sentinel_agent_core::{Admission, Store, FACT_RETENTION_DAYS};
use sentinel_protocol::parse_event_xml;

struct TempDb {
    path: PathBuf,
}

impl TempDb {
    fn new() -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Self {
            path: std::env::temp_dir()
                .join(format!("sentinel-store-{}-{nanos}.db", std::process::id())),
        }
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let mut path = self.path.clone().into_os_string();
            path.push(suffix);
            let _ = std::fs::remove_file(PathBuf::from(path));
        }
    }
}

fn fixture() -> String {
    std::fs::read_to_string(format!(
        "{}/../protocol/tests/fixtures/4624.xml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

fn event_at(
    received_at: DateTime<Utc>,
    record_id: &str,
) -> (sentinel_protocol::NormalizedEvent, String) {
    let xml = fixture().replace(
        "<EventRecordID>1729</EventRecordID>",
        &format!("<EventRecordID>{record_id}</EventRecordID>"),
    );
    let mut event = parse_event_xml(&xml, "lab-1").unwrap();
    event.timestamp = received_at;
    (event, xml)
}

#[test]
fn admit_commits_fact_raw_and_bookmark_together() {
    let db = TempDb::new();
    let mut store = Store::open(&db.path).unwrap();
    let received = "2026-01-10T15:04:06Z".parse::<DateTime<Utc>>().unwrap();
    let (event, xml) = event_at(received, "1729");

    assert_eq!(
        store.admit(&event, &xml, received).unwrap(),
        Admission::Inserted
    );

    let state = store.source_state().unwrap().unwrap();
    assert_eq!(state.source, "windows.security");
    assert_eq!(state.generation, "lab-1");
    assert_eq!(state.last_record_id, Some(1729));
    assert_eq!(
        store.raw_event("lab-1", "1729").unwrap().as_deref(),
        Some(xml.as_str())
    );

    let health = store.health().unwrap();
    assert_eq!(health.fact_count, 1);
    assert!(health.raw_bytes > 0);
    assert_eq!(health.open_gaps, 0);
    assert_eq!(health.degraded_reason, None);
}

#[test]
fn retries_are_idempotent_and_conflicts_do_not_overwrite() {
    let db = TempDb::new();
    let mut store = Store::open(&db.path).unwrap();
    let received = "2026-01-10T15:04:06Z".parse::<DateTime<Utc>>().unwrap();
    let (event, xml) = event_at(received, "1729");

    assert_eq!(
        store.admit(&event, &xml, received).unwrap(),
        Admission::Inserted
    );
    assert_eq!(
        store.admit(&event, &xml, received).unwrap(),
        Admission::Duplicate
    );

    let mut changed = event.clone();
    changed.user.name = Some("different-account".to_string());
    assert_eq!(
        store.admit(&changed, &xml, received).unwrap(),
        Admission::Conflict
    );
    assert_eq!(
        store.admit(&event, &xml, received).unwrap(),
        Admission::Duplicate
    );
    assert_eq!(store.health().unwrap().fact_count, 1);
}

#[test]
fn bookmark_never_moves_backwards_within_a_generation() {
    let db = TempDb::new();
    let mut store = Store::open(&db.path).unwrap();
    let at = "2026-01-10T15:04:06Z".parse::<DateTime<Utc>>().unwrap();
    let (first, first_xml) = event_at(at, "1729");
    let (older, older_xml) = event_at(at, "100");

    store.admit(&first, &first_xml, at).unwrap();
    store.admit(&older, &older_xml, at).unwrap();
    assert_eq!(
        store.source_state().unwrap().unwrap().last_record_id,
        Some(1729)
    );

    let later = at + Duration::seconds(1);
    store.begin_generation("lab-2", later).unwrap();
    assert_eq!(store.source_state().unwrap().unwrap().last_record_id, None);
    let mut newer_generation = older.clone();
    newer_generation.sentinel.source_generation = "lab-2".to_string();
    store.admit(&newer_generation, &older_xml, later).unwrap();
    assert_eq!(
        store.source_state().unwrap().unwrap().last_record_id,
        Some(100)
    );
    assert_eq!(store.source_state().unwrap().unwrap().generation, "lab-2");
}

#[test]
fn gaps_record_a_lower_bound_and_a_recovery_time() {
    let db = TempDb::new();
    let mut store = Store::open(&db.path).unwrap();
    let at = "2026-01-10T15:04:06Z".parse::<DateTime<Utc>>().unwrap();
    let (event, xml) = event_at(at, "1729");
    store.admit(&event, &xml, at).unwrap();

    let id = store
        .record_gap("stale_bookmark", Some(1729), Some(5), at)
        .unwrap();
    let open = store.gaps(true).unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, id);
    assert_eq!(open[0].lower_bound, Some(5));
    assert_eq!(store.health().unwrap().open_gaps, 1);
    assert_eq!(
        store.health().unwrap().degraded_reason.as_deref(),
        Some("stale_bookmark")
    );

    store.resolve_gap(id, at + Duration::minutes(2)).unwrap();
    assert_eq!(store.gaps(true).unwrap().len(), 0);
    assert_eq!(
        store.gaps(false).unwrap()[0].recovered_at,
        Some(at + Duration::minutes(2))
    );
    assert_eq!(store.health().unwrap().degraded_reason, None);
    assert_eq!(store.audit_count().unwrap(), 2);
}

#[test]
fn retention_keeps_daily_counts_and_audits_deletion() {
    let db = TempDb::new();
    let mut store = Store::open(&db.path).unwrap();
    let now = "2026-01-10T15:04:06Z".parse::<DateTime<Utc>>().unwrap();
    let old = now - Duration::days(FACT_RETENTION_DAYS + 1);
    let (old_event, old_xml) = event_at(old, "100");
    let (recent_event, recent_xml) = event_at(now, "1729");

    store.admit(&old_event, &old_xml, old).unwrap();
    store.admit(&recent_event, &recent_xml, now).unwrap();
    assert!(store.health().unwrap().raw_bytes > 0);

    let report = store.apply_retention(now).unwrap();
    assert_eq!(report.raw_deleted, 1);
    assert_eq!(report.facts_rolled_up, 1);
    assert_eq!(
        store
            .daily_count(&old.format("%Y-%m-%d").to_string(), "4624", "success")
            .unwrap(),
        1
    );
    assert_eq!(store.raw_event("lab-1", "100").unwrap(), None);
    assert!(store.raw_event("lab-1", "1729").unwrap().is_some());
    assert_eq!(store.health().unwrap().fact_count, 1);
    assert_eq!(store.audit_count().unwrap(), 1);
}
