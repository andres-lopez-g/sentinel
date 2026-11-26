//! Rendered Windows Event Log XML to the normalized v1 event.
//!
//! Values are looked up by `EventData/Data@Name`, never by message text or
//! field position, so fixture versions, extra fields and localized messages
//! do not change the mapping.

use std::collections::HashMap;
use std::net::IpAddr;

use chrono::{DateTime, Utc};
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use quick_xml::XmlVersion;
use uuid::Uuid;

use crate::error::ParseError;
use crate::model::{
    EventCategory, EventFields, EventKind, HostFields, HostOs, LogonType, NormalizedEvent, OsType,
    Outcome, SentinelFields, SourceFields, UserFields, PARSER_VERSION, SCHEMA_VERSION,
};
use crate::validation::{clean_optional, validate};

pub const WINDOWS_SECURITY_SOURCE: &str = "windows.security";

/// Namespace for deterministic event IDs derived from source identity.
///
/// The local profile has no server to allocate UUIDs. Deriving them from the
/// source generation and record ID keeps retries idempotent without making
/// the record number globally unique.
pub const SOURCE_NAMESPACE: Uuid = Uuid::from_u128(0x3f2b7c1d9a644e508b6d2f1c0a7e5d93);

pub fn parse_event_xml(xml: &str, source_generation: &str) -> Result<NormalizedEvent, ParseError> {
    if source_generation.trim().is_empty() {
        return Err(ParseError::MissingField("source_generation"));
    }

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut system: HashMap<String, String> = HashMap::new();
    let mut data: HashMap<String, String> = HashMap::new();
    let mut provider: Option<String> = None;
    let mut stack: Vec<String> = Vec::new();
    let mut data_name: Option<String> = None;
    let mut in_system = false;
    let mut in_event_data = false;
    let mut buf = Vec::new();

    loop {
        let event = reader.read_event_into(&mut buf).map_err(xml_error)?;
        match event {
            Event::Start(element) => {
                let name = local_name(element.name().as_ref());
                match name.as_str() {
                    "System" => in_system = true,
                    "EventData" | "UserData" => in_event_data = true,
                    "Provider" if in_system => {
                        provider = read_attribute(&element, "Name")?;
                    }
                    "TimeCreated" if in_system => {
                        if let Some(value) = read_attribute(&element, "SystemTime")? {
                            system.insert("SystemTime".to_string(), value);
                        }
                    }
                    "Data" if in_event_data => {
                        data_name = read_attribute(&element, "Name")?;
                    }
                    _ => {}
                }
                stack.push(name);
            }
            Event::Empty(element) => {
                let name = local_name(element.name().as_ref());
                match name.as_str() {
                    "Provider" if in_system => {
                        provider = read_attribute(&element, "Name")?;
                    }
                    "TimeCreated" if in_system => {
                        if let Some(value) = read_attribute(&element, "SystemTime")? {
                            system.insert("SystemTime".to_string(), value);
                        }
                    }
                    "Data" if in_event_data => {
                        if let Some(field) = read_attribute(&element, "Name")? {
                            data.insert(field, String::new());
                        }
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                let value = text.xml10_content().into_owned();
                record_text(
                    &stack,
                    &value,
                    in_system,
                    in_event_data,
                    &data_name,
                    &mut system,
                    &mut data,
                );
            }
            Event::CData(text) => {
                let value = text.xml10_content().into_owned();
                record_text(
                    &stack,
                    &value,
                    in_system,
                    in_event_data,
                    &data_name,
                    &mut system,
                    &mut data,
                );
            }
            Event::End(element) => {
                let name = local_name(element.name().as_ref());
                match name.as_str() {
                    "Data" => data_name = None,
                    "System" => in_system = false,
                    "EventData" | "UserData" => in_event_data = false,
                    _ => {}
                }
                if stack
                    .last()
                    .map(|current| current == &name)
                    .unwrap_or(false)
                {
                    stack.pop();
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }

    if let Some(unclosed) = stack.last() {
        return Err(ParseError::Xml(format!("unclosed element: {unclosed}")));
    }

    let provider = provider.ok_or(ParseError::MissingField("Provider Name"))?;
    let code = required(&system, "EventID", "EventID")?;
    let record_id = required(&system, "EventRecordID", "EventRecordID")?;
    let computer = required(&system, "Computer", "Computer")?;
    let system_time = required(&system, "SystemTime", "TimeCreated@SystemTime")?;

    let outcome = match code.as_str() {
        "4624" => Outcome::Success,
        "4625" => Outcome::Failure,
        other => return Err(ParseError::UnsupportedCode(other.to_string())),
    };

    let user_name = clean_optional(field(&data, "TargetUserName")).map(str::to_string);
    let user_id =
        clean_optional(field(&data, "TargetUserSid").or_else(|| field(&data, "TargetSid")))
            .map(str::to_string);
    let ip =
        clean_optional(field(&data, "IpAddress")).and_then(|value| value.parse::<IpAddr>().ok());
    let logon_type = match clean_optional(field(&data, "LogonType")) {
        Some(value) => Some(
            value
                .parse::<i32>()
                .map(LogonType::new)
                .map_err(|_| ParseError::InvalidLogonType(value.to_string()))?,
        ),
        None => None,
    };

    let id = Uuid::new_v5(
        &SOURCE_NAMESPACE,
        format!("{source_generation}:{record_id}").as_bytes(),
    );
    let event = NormalizedEvent {
        schema_version: SCHEMA_VERSION,
        timestamp: parse_windows_time(&system_time)?,
        event: EventFields {
            id,
            kind: EventKind::Event,
            category: vec![EventCategory::Authentication],
            code,
            outcome,
        },
        host: HostFields {
            name: computer,
            os: HostOs {
                os_type: OsType::Windows,
            },
        },
        user: UserFields {
            name: user_name,
            id: user_id,
        },
        source: SourceFields { ip, logon_type },
        sentinel: SentinelFields {
            source: WINDOWS_SECURITY_SOURCE.to_string(),
            provider,
            record_id,
            source_generation: source_generation.trim().to_string(),
            parser_version: PARSER_VERSION.to_string(),
        },
    };

    validate(&event)?;
    Ok(event)
}

fn xml_error(error: impl std::fmt::Display) -> ParseError {
    ParseError::Xml(error.to_string())
}

fn local_name(name: &str) -> String {
    name.rsplit(':').next().unwrap_or(name).to_string()
}

fn read_attribute(element: &BytesStart<'_>, key: &str) -> Result<Option<String>, ParseError> {
    match element.try_get_attribute(key).map_err(xml_error)? {
        Some(attribute) => Ok(Some(
            attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map_err(xml_error)?
                .into_owned(),
        )),
        None => Ok(None),
    }
}

#[allow(clippy::too_many_arguments)]
fn record_text(
    stack: &[String],
    value: &str,
    in_system: bool,
    in_event_data: bool,
    data_name: &Option<String>,
    system: &mut HashMap<String, String>,
    data: &mut HashMap<String, String>,
) {
    let Some(current) = stack.last() else {
        return;
    };
    if current == "Data" && in_event_data {
        if let Some(field) = data_name {
            data.insert(field.clone(), value.to_string());
        }
    } else if in_system
        && matches!(
            current.as_str(),
            "EventID" | "EventRecordID" | "Computer" | "Channel" | "Version"
        )
    {
        system.insert(current.clone(), value.to_string());
    }
}

fn required(
    map: &HashMap<String, String>,
    key: &str,
    field: &'static str,
) -> Result<String, ParseError> {
    map.get(key)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or(ParseError::MissingField(field))
}

fn field<'a>(map: &'a HashMap<String, String>, key: &str) -> Option<&'a str> {
    map.get(key).map(String::as_str)
}

fn parse_windows_time(value: &str) -> Result<DateTime<Utc>, ParseError> {
    let value = value.trim();
    DateTime::parse_from_rfc3339(value)
        .or_else(|_| DateTime::parse_from_rfc3339(&truncate_fraction(value)))
        .map(|moment| moment.with_timezone(&Utc))
        .map_err(|_| ParseError::InvalidTimestamp(value.to_string()))
}

/// Windows writes seven fractional digits; RFC 3339 parsers accept at most
/// nine, but malformed or longer fractions need a bounded retry.
fn truncate_fraction(value: &str) -> String {
    let Some(dot) = value.find('.') else {
        return value.to_string();
    };
    let (head, tail) = value.split_at(dot + 1);
    let fraction: String = tail
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .take(9)
        .collect();
    let suffix: String = tail.chars().skip_while(|c| c.is_ascii_digit()).collect();
    format!("{head}{fraction}{suffix}")
}
