//! Error handling for the storage role.

use std::fmt;

/// Errors that can occur during storage operations.
#[derive(Debug)]
pub enum StorageError {
    /// Backend-specific error (database connection, file I/O, etc.)
    BackendError(String),
    /// Serialization/deserialization error
    SerializationError(String),
    /// Data not found in storage
    NotFound(String),
    /// Invalid data format
    InvalidData(String),
    /// Configuration error
    ConfigError(String),
    /// Storage backend not available
    BackendUnavailable,
}

impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::BackendError(msg) => write!(f, "Backend error: {}", msg),
            StorageError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            StorageError::NotFound(msg) => write!(f, "Not found: {}", msg),
            StorageError::InvalidData(msg) => write!(f, "Invalid data: {}", msg),
            StorageError::ConfigError(msg) => write!(f, "Configuration error: {}", msg),
            StorageError::BackendUnavailable => write!(f, "Storage backend unavailable"),
        }
    }
}

impl std::error::Error for StorageError {}

pub type StorageResult<T> = Result<T, StorageError>;