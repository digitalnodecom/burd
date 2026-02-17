//! Typed error system for the Burd application
//!
//! Provides structured errors with codes, messages, and optional context
//! for better error handling and frontend display.

use serde::Serialize;
use std::fmt;
use std::sync::{Arc, PoisonError};

/// Error codes for categorizing errors
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Resource not found (instance, domain, service, etc.)
    NotFound,
    /// Invalid configuration or parameters
    InvalidConfig,
    /// Permission denied or authorization failed
    PermissionDenied,
    /// File system or I/O error
    IoError,
    /// Network or connection error
    NetworkError,
    /// Process management error (start, stop, etc.)
    ProcessError,
    /// Operation timed out
    Timeout,
    /// Internal lock/mutex error
    LockError,
    /// Service-specific error
    ServiceError,
    /// Parse error (UUID, config, etc.)
    ParseError,
    /// Operation already in progress or resource busy
    Busy,
    /// General/unknown error
    Internal,
}

/// Structured application error
#[derive(Debug, Clone, Serialize)]
pub struct AppError {
    /// Error category
    pub code: ErrorCode,
    /// Human-readable error message
    pub message: String,
    /// Optional additional context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

impl AppError {
    /// Create a new error with code and message
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            context: None,
        }
    }

    /// Create a new error with context
    pub fn with_context(
        code: ErrorCode,
        message: impl Into<String>,
        context: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            context: Some(context.into()),
        }
    }

    /// Create a not found error
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// Create an invalid config error
    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidConfig, message)
    }

    /// Create a permission denied error
    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::PermissionDenied, message)
    }

    /// Create an I/O error
    pub fn io_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::IoError, message)
    }

    /// Create a network error
    pub fn network_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NetworkError, message)
    }

    /// Create a process error
    pub fn process_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ProcessError, message)
    }

    /// Create a timeout error
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Timeout, message)
    }

    /// Create a lock error
    pub fn lock_error() -> Self {
        Self::new(ErrorCode::LockError, "Failed to acquire lock")
    }

    /// Create a parse error
    pub fn parse_error(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ParseError, message)
    }

    /// Create an internal error
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref ctx) = self.context {
            write!(f, "{}: {}", self.message, ctx)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for AppError {}

// Conversion from String for backwards compatibility
impl From<String> for AppError {
    fn from(s: String) -> Self {
        AppError::internal(s)
    }
}

impl From<&str> for AppError {
    fn from(s: &str) -> Self {
        AppError::internal(s)
    }
}

// Conversion from std::io::Error
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::io_error(e.to_string())
    }
}

// Conversion from PoisonError (mutex lock failures)
impl<T> From<PoisonError<T>> for AppError {
    fn from(_: PoisonError<T>) -> Self {
        AppError::lock_error()
    }
}

/// Result type alias using AppError
pub type AppResult<T> = Result<T, AppError>;

/// Helper macro for locking a mutex and converting errors to String
/// Use this in Tauri command handlers where Result<T, String> is required
#[macro_export]
macro_rules! lock {
    ($mutex:expr) => {
        $mutex.lock_or_err().map_err(|e| e.to_string())
    };
}

/// Acquire a read lock on an RwLock in Tauri command handlers
#[macro_export]
macro_rules! read_lock {
    ($rwlock:expr) => {
        $rwlock.read_or_err().map_err(|e| e.to_string())
    };
}

/// Acquire a write lock on an RwLock in Tauri command handlers
#[macro_export]
macro_rules! write_lock {
    ($rwlock:expr) => {
        $rwlock.write_or_err().map_err(|e| e.to_string())
    };
}

/// Extension trait for easily converting mutex locks to AppResult
pub trait LockExt<T> {
    /// Lock the mutex, converting any poison error to AppError
    fn lock_or_err(&self) -> AppResult<std::sync::MutexGuard<'_, T>>;
}

impl<T> LockExt<T> for std::sync::Mutex<T> {
    fn lock_or_err(&self) -> AppResult<std::sync::MutexGuard<'_, T>> {
        self.lock().map_err(|_| AppError::lock_error())
    }
}

impl<T> LockExt<T> for Arc<std::sync::Mutex<T>> {
    fn lock_or_err(&self) -> AppResult<std::sync::MutexGuard<'_, T>> {
        self.lock().map_err(|_| AppError::lock_error())
    }
}

/// Extension trait for RwLock
pub trait RwLockExt<T> {
    /// Read lock, converting any poison error to AppError
    fn read_or_err(&self) -> AppResult<std::sync::RwLockReadGuard<'_, T>>;
    /// Write lock, converting any poison error to AppError
    fn write_or_err(&self) -> AppResult<std::sync::RwLockWriteGuard<'_, T>>;
}

impl<T> RwLockExt<T> for std::sync::RwLock<T> {
    fn read_or_err(&self) -> AppResult<std::sync::RwLockReadGuard<'_, T>> {
        self.read().map_err(|_| AppError::lock_error())
    }

    fn write_or_err(&self) -> AppResult<std::sync::RwLockWriteGuard<'_, T>> {
        self.write().map_err(|_| AppError::lock_error())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_creation() {
        let err = AppError::not_found("Instance not found");
        assert_eq!(err.code, ErrorCode::NotFound);
        assert_eq!(err.message, "Instance not found");
        assert!(err.context.is_none());
    }

    #[test]
    fn test_error_with_context() {
        let err =
            AppError::with_context(ErrorCode::IoError, "Failed to read file", "/path/to/file");
        assert_eq!(err.code, ErrorCode::IoError);
        assert_eq!(err.context, Some("/path/to/file".to_string()));
    }

    #[test]
    fn test_error_display() {
        let err = AppError::not_found("Instance not found");
        assert_eq!(err.to_string(), "Instance not found");

        let err_with_ctx =
            AppError::with_context(ErrorCode::IoError, "Failed to read", "/path/to/file");
        assert_eq!(err_with_ctx.to_string(), "Failed to read: /path/to/file");
    }

    #[test]
    fn test_from_string() {
        let err: AppError = "Some error".into();
        assert_eq!(err.code, ErrorCode::Internal);
    }
}
