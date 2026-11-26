//! Shared Sentinel event contract.
//!
//! This crate owns the normalized event model, its validation rules and the
//! parsers that turn source records into that model. It has no I/O or runtime
//! dependencies so the same types can be used by a collector, a server and a
//! test harness.

pub mod error;
pub mod model;
pub mod validation;
pub mod windows;

pub use error::{ParseError, ValidationError};
pub use model::{
    EventCategory, EventFields, EventKind, HostFields, LogonType, NormalizedEvent, OsType, Outcome,
    SentinelFields, SourceFields, UserFields, PARSER_VERSION, SCHEMA_VERSION,
};
pub use validation::{validate, Limit};
pub use windows::{parse_event_xml, SOURCE_NAMESPACE, WINDOWS_SECURITY_SOURCE};
