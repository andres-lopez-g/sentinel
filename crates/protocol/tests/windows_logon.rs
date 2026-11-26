use std::net::IpAddr;

use chrono::{DateTime, Utc};
use sentinel_protocol::model::LogonType;
use sentinel_protocol::{
    parse_event_xml, Outcome, ParseError, PARSER_VERSION, SCHEMA_VERSION, WINDOWS_SECURITY_SOURCE,
};

fn fixture(name: &str) -> String {
    std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .expect("fixture must be readable")
}

#[test]
fn parses_success_logon_by_field_name() {
    let event = parse_event_xml(&fixture("4624.xml"), "lab-1").expect("valid fixture");

    assert_eq!(event.schema_version, SCHEMA_VERSION);
    assert_eq!(event.event.code, "4624");
    assert_eq!(event.event.outcome, Outcome::Success);
    assert_eq!(event.host.name, "lab-win-01");
    assert_eq!(event.user.name.as_deref(), Some("sample-user"));
    assert_eq!(event.user.id.as_deref(), Some("S-1-5-21-100-200-300-1001"));
    assert_eq!(
        event.source.ip,
        Some("192.0.2.14".parse::<IpAddr>().unwrap())
    );
    assert_eq!(event.source.logon_type, Some(LogonType::INTERACTIVE));
    assert_eq!(event.source.logon_type.unwrap().name(), "interactive");
    assert_eq!(event.sentinel.source, WINDOWS_SECURITY_SOURCE);
    assert_eq!(
        event.sentinel.provider,
        "Microsoft-Windows-Security-Auditing"
    );
    assert_eq!(event.sentinel.record_id, "1729");
    assert_eq!(event.sentinel.source_generation, "lab-1");
    assert_eq!(event.sentinel.parser_version, PARSER_VERSION);
    assert_eq!(
        event.timestamp,
        "2026-01-10T15:04:05.1234567Z"
            .parse::<DateTime<Utc>>()
            .unwrap()
    );
    assert_eq!(event.local_key(), ("lab-1".to_string(), "1729".to_string()));
}

#[test]
fn id_is_stable_for_a_source_record_and_changes_with_generation() {
    let xml = fixture("4624.xml");
    let first = parse_event_xml(&xml, "lab-1").unwrap();
    let retry = parse_event_xml(&xml, "lab-1").unwrap();
    let new_generation = parse_event_xml(&xml, "lab-2").unwrap();

    assert_eq!(first.event.id, retry.event.id);
    assert_ne!(first.event.id, new_generation.event.id);
}

#[test]
fn missing_optional_ip_and_sid_stay_absent() {
    let event = parse_event_xml(&fixture("4625.xml"), "lab-1").unwrap();

    assert_eq!(event.event.code, "4625");
    assert_eq!(event.event.outcome, Outcome::Failure);
    assert_eq!(event.user.name.as_deref(), Some("bad-user"));
    assert_eq!(event.user.id, None);
    assert_eq!(event.source.ip, None);
    assert_eq!(event.source.logon_type, Some(LogonType::NETWORK));
    assert_eq!(
        event.timestamp,
        "2026-01-10T15:06:11.5Z".parse::<DateTime<Utc>>().unwrap()
    );
}

#[test]
fn tolerates_field_reordering_unknown_fields_and_localized_messages() {
    let event = parse_event_xml(&fixture("variant_locale.xml"), "gen-7").unwrap();

    assert_eq!(event.event.code, "4624");
    assert_eq!(event.host.name, "lab-win-02.lab.example");
    assert_eq!(event.user.name.as_deref(), Some("usuario"));
    assert_eq!(event.user.id.as_deref(), Some("S-1-5-21-100-200-300-1002"));
    assert_eq!(
        event.source.ip,
        Some("198.51.100.7".parse::<IpAddr>().unwrap())
    );
    assert_eq!(event.source.logon_type, Some(LogonType::CACHED_INTERACTIVE));
    assert_eq!(event.local_key(), ("gen-7".to_string(), "1731".to_string()));
}

#[test]
fn rejects_unsupported_codes_and_malformed_records() {
    assert_eq!(
        parse_event_xml(&fixture("unsupported_code.xml"), "lab-1").unwrap_err(),
        ParseError::UnsupportedCode("4634".to_string())
    );
    assert!(matches!(
        parse_event_xml("<Event><System>", "lab-1").unwrap_err(),
        ParseError::Xml(_)
    ));
    assert_eq!(
        parse_event_xml(&fixture("4624.xml"), "  ").unwrap_err(),
        ParseError::MissingField("source_generation")
    );
}

#[test]
fn rejects_records_without_a_provider_or_with_a_bad_logon_type() {
    let no_provider = fixture("4624.xml").replace(
        "<Provider Name=\"Microsoft-Windows-Security-Auditing\" Guid=\"{54849625-5478-4994-A5BA-3E3B0328C30D}\" />",
        "",
    );
    assert_eq!(
        parse_event_xml(&no_provider, "lab-1").unwrap_err(),
        ParseError::MissingField("Provider Name")
    );

    let bad_type = fixture("4624.xml").replace(
        "<Data Name=\"LogonType\">2</Data>",
        "<Data Name=\"LogonType\">two</Data>",
    );
    assert_eq!(
        parse_event_xml(&bad_type, "lab-1").unwrap_err(),
        ParseError::InvalidLogonType("two".to_string())
    );
}
