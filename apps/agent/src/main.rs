//! Local collector loop for the single-host profile.
//!
//! Reads the Windows Security channel, normalizes eligible logon events and
//! commits each record with its bookmark in one local transaction.

#[cfg(windows)]
mod windows_runner {
    use std::env;
    use std::path::PathBuf;
    use std::process::ExitCode;

    use chrono::Utc;
    use sentinel_agent_core::{Admission, Store, StoreError};
    use sentinel_protocol::{parse_event_xml, ParseError};
    use sentinel_windows_source::{EventLogSource, SourceError, SECURITY_CHANNEL};
    use thiserror::Error;

    #[derive(Debug, Error)]
    enum AgentError {
        #[error("store: {0}")]
        Store(#[from] StoreError),
        #[error("source: {0}")]
        Source(#[from] SourceError),
        #[error("parse: {0}")]
        Parse(#[from] ParseError),
        #[error("configuration: {0}")]
        Configuration(String),
    }

    #[derive(Debug)]
    struct Options {
        database: PathBuf,
        generation: String,
        channel: String,
        timeout_ms: u32,
    }

    impl Default for Options {
        fn default() -> Self {
            Self {
                database: PathBuf::from("sentinel.db"),
                generation: "local-1".to_string(),
                channel: SECURITY_CHANNEL.to_string(),
                timeout_ms: 1000,
            }
        }
    }

    fn usage() {
        eprintln!(
            "usage: sentinel-agent [--database FILE] [--generation NAME] [--channel NAME] [--timeout-ms N]"
        );
    }

    fn options() -> Result<Options, AgentError> {
        let mut options = Options::default();
        let mut args = env::args().skip(1);
        while let Some(flag) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| AgentError::Configuration(format!("{flag} needs a value")))?;
            match flag.as_str() {
                "--database" => options.database = PathBuf::from(value),
                "--generation" => options.generation = value,
                "--channel" => options.channel = value,
                "--timeout-ms" => {
                    options.timeout_ms = value.parse::<u32>().map_err(|_| {
                        AgentError::Configuration("invalid --timeout-ms".to_string())
                    })?;
                }
                other => {
                    usage();
                    return Err(AgentError::Configuration(format!("unknown option {other}")));
                }
            }
        }
        if options.timeout_ms == 0 {
            return Err(AgentError::Configuration(
                "--timeout-ms must be positive".to_string(),
            ));
        }
        Ok(options)
    }

    pub fn main() -> ExitCode {
        match run() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("sentinel-agent stopped: {error}");
                ExitCode::FAILURE
            }
        }
    }

    fn run() -> Result<(), AgentError> {
        let options = options()?;
        let mut store = Store::open(&options.database)?;
        let state = store.source_state()?;
        let generation = state
            .map(|value| value.generation)
            .unwrap_or(options.generation);
        let bookmark = store.bookmark()?;
        let last_record_id = store.source_state()?.and_then(|value| value.last_record_id);

        let mut source = match EventLogSource::open(&options.channel, bookmark.as_deref()) {
            Ok(source) => source,
            Err(error) if bookmark.is_some() => {
                // A stale bookmark cannot be rewound. Start a new source
                // generation at the current tail and label the gap as unknown.
                eprintln!("bookmark is unusable, starting a new generation: {error}");
                let next = format!("{}-{}", generation, Utc::now().timestamp_millis());
                store.begin_generation(&next, Utc::now())?;
                store.record_gap("stale_bookmark", last_record_id, None, Utc::now())?;
                EventLogSource::open(&options.channel, None)?
            }
            Err(error) => return Err(error.into()),
        };

        let mut last_reason: Option<String> = None;
        let mut open_gap: Option<i64> = None;

        loop {
            match source.health() {
                Ok(health) if health.degraded => {
                    let reason = health.reasons.join(",");
                    if last_reason.as_deref() != Some(reason.as_str()) {
                        if let Some(id) = open_gap.take() {
                            store.resolve_gap(id, Utc::now())?;
                        }
                        let last_good =
                            store.source_state()?.and_then(|value| value.last_record_id);
                        open_gap = Some(store.record_gap(&reason, last_good, None, Utc::now())?);
                        last_reason = Some(reason);
                    }
                }
                Ok(_) => {
                    if let Some(id) = open_gap.take() {
                        store.resolve_gap(id, Utc::now())?;
                    }
                    last_reason = None;
                }
                Err(error) => eprintln!("channel health unavailable: {error}"),
            }

            let batch = source.next_batch(64, options.timeout_ms)?;
            for rendered in batch {
                let event = match parse_event_xml(&rendered.xml, &generation) {
                    Ok(event) => event,
                    Err(error) => {
                        eprintln!("cannot parse record {}: {error}", rendered.record_id);
                        continue;
                    }
                };
                let bookmark_xml = source.bookmark_xml()?;
                match store.admit_with_bookmark(
                    &event,
                    &rendered.xml,
                    Some(&bookmark_xml),
                    Utc::now(),
                ) {
                    Ok(Admission::Inserted) => {
                        println!(
                            "admitted {} (record {})",
                            event.event.id, rendered.record_id
                        );
                    }
                    Ok(_) => {}
                    Err(error) => eprintln!("cannot admit record {}: {error}", rendered.record_id),
                }
            }
        }
    }
}

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    windows_runner::main()
}

#[cfg(not(windows))]
fn main() -> std::process::ExitCode {
    eprintln!("sentinel-agent requires Windows Event Log and is not available on this platform");
    std::process::ExitCode::FAILURE
}
