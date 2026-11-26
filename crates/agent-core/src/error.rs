use thiserror::Error;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("time: {0}")]
    Time(#[from] chrono::ParseError),
    #[error("utf8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
    #[error("journal mode is {0}, expected wal")]
    JournalMode(String),
}
