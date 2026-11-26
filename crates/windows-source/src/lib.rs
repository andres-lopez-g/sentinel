//! Windows Event Log source adapter.
//!
//! The rules that decide what to collect and when to declare a gap are kept
//! pure and testable on every platform. The native FFI layer is Windows-only.

mod health;

#[cfg(windows)]
mod native;

pub use health::{
    assess_channel, is_eligible_code, stale_bookmark_lower_bound, ChannelHealth, ChannelInfo,
};
#[cfg(windows)]
pub use native::{EventLogSource, RenderedEvent, SourceError};

/// Channels the collector reads in the local MVP.
pub const SECURITY_CHANNEL: &str = "Security";
/// Provider required for the authentication events in scope.
pub const SECURITY_PROVIDER: &str = "Microsoft-Windows-Security-Auditing";
/// Event IDs admitted by the collector.
pub const ELIGIBLE_CODES: [&str; 2] = ["4624", "4625"];
/// Source-side filter: the same provider/channel/code triple used by tests.
pub const EVENT_QUERY: &str =
    "*[System[Provider[@Name='Microsoft-Windows-Security-Auditing'] and (EventID=4624 or EventID=4625)]]";
