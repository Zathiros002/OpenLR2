use std::path::PathBuf;
use thiserror::Error;

/// Unified error type for OpenLR2 Rust runtime.
#[derive(Debug, Error)]
pub enum OpenLr2Error {
    /// BMS/PMS parse error.
    #[error("bms parse error: {0}")]
    BmsParse(String),

    /// Resource not found at the given path.
    #[error("resource not found: {path}")]
    ResourceNotFound { path: PathBuf },

    /// Database error (SQLite wrapper).
    #[error("database error: {0}")]
    Database(String),

    /// I/O error from the platform.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Encoding error (e.g. invalid CP932 sequence).
    #[error("encoding error: {0}")]
    Encoding(String),

    /// Configuration parse/validation error.
    #[error("config error: {0}")]
    Config(String),

    /// Skin script parse error.
    #[error("skin parse error: {0}")]
    SkinParse(String),

    /// Replay format error.
    #[error("replay error: {0}")]
    Replay(String),

    /// Internal invariant violation — should never happen.
    #[error("internal error: {0}")]
    Internal(String),
}
