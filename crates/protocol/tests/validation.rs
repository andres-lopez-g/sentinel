use sentinel_protocol::validation::{clean_optional, validate, Limit};
use sentinel_protocol::{parse_event_xml, ParseError, ValidationError};

fn event() -> sentinel_protocol::NormalizedEvent {
    let xml = std::fs::read_to_string(format!(
        "{}/tests/fixtures/4624.xml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    parse_event_xml(&xml, "lab-1").unwrap()
}

#[test]
fn admits_a_valid_normalized_event() {
    assert_eq!(validate(&event()), Ok(()));
}

#[test]
fn rejects_invalid_envelopes() {
    let mut bad = event();
    bad.schema_version = 2;
    assert_eq!(validate(&bad), Err(ValidationError::SchemaVersion(2)));

    let mut bad = event();
    bad.event.id = uuid::Uuid::nil();
    assert_eq!(validate(&bad), Err(ValidationError::NilEventId));

    let mut bad = event();
    bad.event.code.clear();
    assert_eq!(
        validate(&bad),
        Err(ValidationError::MissingField("event.code"))
    );

    let mut bad = event();
    bad.event.code = "46x4".to_string();
    assert_eq!(validate(&bad), Err(ValidationError::NonNumericCode));

    let mut bad = event();
    bad.event.category.clear();
    assert_eq!(validate(&bad), Err(ValidationError::MissingCategory));

    let mut bad = event();
    bad.user.name = Some("x".repeat(Limit::MaxStringBytes as usize + 1));
    assert_eq!(
        validate(&bad),
        Err(ValidationError::TooLong {
            field: "user.name",
            limit: Limit::MaxStringBytes as usize
        })
    );

    let mut bad = event();
    bad.sentinel.source_generation.clear();
    assert_eq!(
        validate(&bad),
        Err(ValidationError::MissingField("sentinel.source_generation"))
    );
}

#[test]
fn placeholders_are_not_values() {
    assert_eq!(clean_optional(Some(" - ")), None);
    assert_eq!(clean_optional(Some("")), None);
    assert_eq!(clean_optional(Some("N/A")), None);
    assert_eq!(clean_optional(None), None);
    assert_eq!(clean_optional(Some(" sample ")), Some("sample"));
}

#[test]
fn validation_errors_reach_the_parser_as_a_wrapped_error() {
    let xml = std::fs::read_to_string(format!(
        "{}/tests/fixtures/4624.xml",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let long_name = "a".repeat(Limit::MaxStringBytes as usize + 1);
    let changed = xml.replace(">sample-user<", format!(">{long_name}<").as_str());
    assert!(matches!(
        parse_event_xml(&changed, "lab-1").unwrap_err(),
        ParseError::Validation(ValidationError::TooLong { .. })
    ));
}
