//! The published example in docs/DATA_MODEL.md is part of the contract.

use sentinel_protocol::{NormalizedEvent, Outcome, PARSER_VERSION};

const CANONICAL_EXAMPLE: &str = r#"
{
  "schema_version": 1,
  "@timestamp": "2026-01-10T15:04:05.123Z",
  "event": {
    "id": "41e446fb-1f33-46f0-b972-716905054bd9",
    "kind": "event",
    "category": ["authentication"],
    "code": "4625",
    "outcome": "failure"
  },
  "host": {"name": "lab-win-01", "os": {"type": "windows"}},
  "user": {"name": "sample-user"},
  "source": {"ip": "192.0.2.14"},
  "sentinel": {
    "source": "windows.security",
    "provider": "Microsoft-Windows-Security-Auditing",
    "record_id": "1729",
    "source_generation": "lab-1",
    "parser_version": "1.0.0"
  }
}
"#;

#[test]
fn documented_example_deserializes_and_round_trips() {
    let event: NormalizedEvent = serde_json::from_str(CANONICAL_EXAMPLE).unwrap();

    assert_eq!(event.event.code, "4625");
    assert_eq!(event.event.outcome, Outcome::Failure);
    assert_eq!(event.sentinel.parser_version, PARSER_VERSION);
    assert_eq!(event.user.id, None);
    assert_eq!(event.source.logon_type, None);

    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["@timestamp"], "2026-01-10T15:04:05.123Z");
    assert_eq!(json["event"]["outcome"], "failure");
    assert_eq!(json["host"]["os"]["type"], "windows");
    assert!(json["user"].get("id").is_none());
    assert!(json["source"].get("logon_type").is_none());

    let reparsed: NormalizedEvent = serde_json::from_value(json).unwrap();
    assert_eq!(reparsed, event);
}

#[test]
fn unknown_source_fields_do_not_break_the_contract() {
    let mut json: serde_json::Value = serde_json::from_str(CANONICAL_EXAMPLE).unwrap();
    json["event"]["future_field"] = serde_json::json!(7);
    let event: NormalizedEvent = serde_json::from_value(json).unwrap();
    assert_eq!(event.event.code, "4625");
}
