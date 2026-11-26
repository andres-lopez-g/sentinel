//! ECS-aligned subset with a versioned Sentinel envelope.
//!
//! The JSON shape here is the published v1 contract. Optional values are
//! omitted rather than sent as placeholders; a missing IP stays missing.

use std::net::IpAddr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Version of the normalized event envelope, independent of the URL version.
pub const SCHEMA_VERSION: u16 = 1;
/// Version of the source-specific field mapping.
pub const PARSER_VERSION: &str = "1.0.0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NormalizedEvent {
    pub schema_version: u16,
    #[serde(rename = "@timestamp")]
    pub timestamp: DateTime<Utc>,
    pub event: EventFields,
    pub host: HostFields,
    pub user: UserFields,
    #[serde(default)]
    pub source: SourceFields,
    pub sentinel: SentinelFields,
}

impl NormalizedEvent {
    /// Stable local identity for the single-host profile.
    ///
    /// The fleet design uses a server-generated UUID as the primary key. The
    /// local profile instead identifies a record by its channel-local record
    /// ID within one source generation, so no central identity service is
    /// needed.
    pub fn local_key(&self) -> (String, String) {
        (
            self.sentinel.source_generation.clone(),
            self.sentinel.record_id.clone(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventFields {
    pub id: Uuid,
    pub kind: EventKind,
    pub category: Vec<EventCategory>,
    pub code: String,
    pub outcome: Outcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Event,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    Authentication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Success,
    Failure,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostFields {
    pub name: String,
    pub os: HostOs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostOs {
    #[serde(rename = "type")]
    pub os_type: OsType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OsType {
    Windows,
    Linux,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<IpAddr>,
    /// Typed source extension for the Windows `LogonType` field.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logon_type: Option<LogonType>,
}

/// A typed, non-lossy representation of the Windows logon type.
///
/// Serializes as its integer code so unknown future codes can still round
/// trip while keeping named constants for the codes the MVP understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct LogonType(i32);

impl LogonType {
    pub const SYSTEM: Self = Self(0);
    pub const INTERACTIVE: Self = Self(2);
    pub const NETWORK: Self = Self(3);
    pub const BATCH: Self = Self(4);
    pub const SERVICE: Self = Self(5);
    pub const UNLOCK: Self = Self(6);
    pub const NETWORK_CLEARTEXT: Self = Self(7);
    pub const NEW_CREDENTIALS: Self = Self(8);
    pub const REMOTE_INTERACTIVE: Self = Self(9);
    pub const CACHED_INTERACTIVE: Self = Self(10);
    pub const CACHED_REMOTE_INTERACTIVE: Self = Self(11);
    pub const CACHED_UNLOCK: Self = Self(12);

    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    pub const fn code(self) -> i32 {
        self.0
    }

    pub const fn name(self) -> &'static str {
        match self.0 {
            0 => "system",
            2 => "interactive",
            3 => "network",
            4 => "batch",
            5 => "service",
            6 => "unlock",
            7 => "network_cleartext",
            8 => "new_credentials",
            9 => "remote_interactive",
            10 => "cached_interactive",
            11 => "cached_remote_interactive",
            12 => "cached_unlock",
            _ => "unknown",
        }
    }
}

impl std::fmt::Display for LogonType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SentinelFields {
    /// Source namespace, for example `windows.security`.
    pub source: String,
    /// Provider or source-specific origin name.
    pub provider: String,
    /// Channel-local record ID, never a global event ID.
    pub record_id: String,
    /// Source generation, changed after a channel clear or stale bookmark.
    pub source_generation: String,
    pub parser_version: String,
}
