//! Admission rules for normalized events.
//!
//! These rules are deliberately independent of any parser. A record created
//! by a fixture, a Windows source or a future Linux source must satisfy the
//! same contract before it can be stored.

use crate::error::ValidationError;
use crate::model::{EventCategory, NormalizedEvent, SCHEMA_VERSION};

/// Contract limits shared by validation and the HTTP batch rules.
pub enum Limit {
    MaxStringBytes = 2048,
    MaxCategories = 8,
}

pub fn validate(event: &NormalizedEvent) -> Result<(), ValidationError> {
    if event.schema_version != SCHEMA_VERSION {
        return Err(ValidationError::SchemaVersion(event.schema_version));
    }
    if event.event.id.is_nil() {
        return Err(ValidationError::NilEventId);
    }
    required("event.code", &event.event.code)?;
    bounded("event.code", &event.event.code)?;
    if !event.event.code.chars().all(|c| c.is_ascii_digit()) {
        return Err(ValidationError::NonNumericCode);
    }
    if event.event.category.is_empty()
        || !event
            .event
            .category
            .contains(&EventCategory::Authentication)
    {
        return Err(ValidationError::MissingCategory);
    }
    if event.event.category.len() > Limit::MaxCategories as usize {
        return Err(ValidationError::TooManyCategories);
    }

    bounded("host.name", &event.host.name)?;
    if let Some(name) = &event.user.name {
        bounded("user.name", name)?;
        if clean_optional(Some(name)).is_none() {
            return Err(ValidationError::Placeholder { field: "user.name" });
        }
    }
    if let Some(id) = &event.user.id {
        bounded("user.id", id)?;
    }

    required("sentinel.source", &event.sentinel.source)?;
    required("sentinel.provider", &event.sentinel.provider)?;
    required("sentinel.record_id", &event.sentinel.record_id)?;
    required(
        "sentinel.source_generation",
        &event.sentinel.source_generation,
    )?;
    required("sentinel.parser_version", &event.sentinel.parser_version)?;
    bounded("sentinel.source", &event.sentinel.source)?;
    bounded("sentinel.provider", &event.sentinel.provider)?;
    bounded("sentinel.record_id", &event.sentinel.record_id)?;
    bounded(
        "sentinel.source_generation",
        &event.sentinel.source_generation,
    )?;
    bounded("sentinel.parser_version", &event.sentinel.parser_version)?;

    Ok(())
}

fn required(field: &'static str, value: &str) -> Result<(), ValidationError> {
    if value.trim().is_empty() {
        Err(ValidationError::MissingField(field))
    } else {
        Ok(())
    }
}

fn bounded(field: &'static str, value: &str) -> Result<(), ValidationError> {
    let limit = Limit::MaxStringBytes as usize;
    if value.len() > limit {
        Err(ValidationError::TooLong { field, limit })
    } else {
        Ok(())
    }
}

/// Windows uses `-` for absent optional fields; these must stay absent rather
/// than becoming string values in the normalized record.
pub fn clean_optional(value: Option<&str>) -> Option<&str> {
    value
        .map(str::trim)
        .filter(|v| !v.is_empty() && !matches!(*v, "-" | "N/A" | "(null)" | "null"))
}
