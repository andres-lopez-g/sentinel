//! Local, single-host persistence for the Sentinel collector.
//!
//! One SQLite file in WAL mode holds facts, compressed raw records, the
//! channel bookmark and health history. A fact and its bookmark advance in
//! one transaction so a crash cannot create an unacknowledged gap.

mod error;
mod store;

pub use error::StoreError;
pub use store::{
    Admission, Gap, Health, RetentionReport, SourceState, Store, FACT_RETENTION_DAYS,
    RAW_RETENTION_DAYS,
};
