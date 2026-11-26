//! Admit one XML fixture into a local database and print the resulting health.
//!
//! Usage: cargo run -p sentinel-agent-core --example replay_fixture --
//!        <file.xml> <generation> <database.db>

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use chrono::Utc;
use sentinel_agent_core::Store;
use sentinel_protocol::parse_event_xml;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(xml_path), Some(generation), Some(db_path)) = (args.next(), args.next(), args.next())
    else {
        eprintln!("usage: replay_fixture <file.xml> <source_generation> <database.db>");
        return ExitCode::FAILURE;
    };

    let xml = match fs::read_to_string(&xml_path) {
        Ok(xml) => xml,
        Err(error) => {
            eprintln!("cannot read {xml_path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let event = match parse_event_xml(&xml, &generation) {
        Ok(event) => event,
        Err(error) => {
            eprintln!("cannot parse {xml_path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut store = match Store::open(&PathBuf::from(&db_path)) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("cannot open {db_path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    match store.admit(&event, &xml, Utc::now()) {
        Ok(admission) => println!("admission: {admission:?}"),
        Err(error) => {
            eprintln!("cannot admit event: {error}");
            return ExitCode::FAILURE;
        }
    }
    match store.health() {
        Ok(health) => println!("health: {health:?}"),
        Err(error) => {
            eprintln!("cannot read health: {error}");
            return ExitCode::FAILURE;
        }
    }
    ExitCode::SUCCESS
}
