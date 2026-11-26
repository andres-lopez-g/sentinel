//! Render one Windows Event Log XML fixture as a normalized event.
//!
//! Usage: cargo run -p sentinel-protocol --example inspect_xml -- <file.xml> <generation>

use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let (Some(path), Some(generation)) = (args.next(), args.next()) else {
        eprintln!("usage: inspect_xml <file.xml> <source_generation>");
        return ExitCode::FAILURE;
    };

    let xml = match fs::read_to_string(&path) {
        Ok(xml) => xml,
        Err(error) => {
            eprintln!("cannot read {path}: {error}");
            return ExitCode::FAILURE;
        }
    };

    match sentinel_protocol::parse_event_xml(&xml, &generation) {
        Ok(event) => match serde_json::to_string_pretty(&event) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("cannot render event: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("cannot parse {path}: {error}");
            ExitCode::FAILURE
        }
    }
}
