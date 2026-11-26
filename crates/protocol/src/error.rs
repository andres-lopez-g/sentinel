use thiserror::Error;

/// Failure to turn a source record into a normalized event.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ParseError {
    #[error("malformed XML: {0}")]
    Xml(String),
    #[error("missing required source field: {0}")]
    MissingField(&'static str),
    #[error("unsupported event code: {0}")]
    UnsupportedCode(String),
    #[error("invalid event time: {0}")]
    InvalidTimestamp(String),
    #[error("invalid logon type: {0}")]
    InvalidLogonType(String),
    #[error("event failed validation: {0}")]
    Validation(#[from] ValidationError),
}

/// Failure to validate a normalized event before admitting it.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ValidationError {
    #[error("unsupported schema version {0}; expected 1")]
    SchemaVersion(u16),
    #[error("missing required field: {0}")]
    MissingField(&'static str),
    #[error("field {field} exceeds the {limit}-byte limit")]
    TooLong { field: &'static str, limit: usize },
    #[error("field {field} must not be a placeholder value")]
    Placeholder { field: &'static str },
    #[error("event id must not be nil")]
    NilEventId,
    #[error("event code must be numeric")]
    NonNumericCode,
    #[error("event category must include authentication")]
    MissingCategory,
    #[error("too many categories")]
    TooManyCategories,
}
