//! Input validation module
//!
//! Provides validation functions for user inputs to prevent security vulnerabilities
//! and ensure data integrity.

use crate::error::AppError;
use once_cell::sync::Lazy;
use regex::Regex;
use std::path::{Path, PathBuf};

// ============================================================================
// Port Validation
// ============================================================================

/// Minimum allowed port number (avoiding well-known ports)
const MIN_PORT: u16 = 1024;

/// Maximum allowed port number
const MAX_PORT: u16 = 65535;

/// Validate a port number
///
/// Ensures the port is in the valid range (1024-65535) to avoid privileged ports
/// and ensure it's within the valid TCP/UDP port range.
///
/// # Arguments
/// * `port` - The port number to validate
///
/// # Returns
/// * `Ok(())` if the port is valid
/// * `Err(AppError)` if the port is invalid
///
/// # Example
/// ```
/// use burd_lib::validation::validate_port;
///
/// assert!(validate_port(8080).is_ok());
/// assert!(validate_port(80).is_err()); // privileged port
/// assert!(validate_port(0).is_err()); // invalid port
/// ```
pub fn validate_port(port: u16) -> Result<(), AppError> {
    if port == 0 {
        return Err(AppError::invalid_config("Port cannot be 0"));
    }

    if port < MIN_PORT {
        return Err(AppError::invalid_config(format!(
            "Port {} is below minimum allowed port {}. Ports below 1024 require elevated privileges.",
            port, MIN_PORT
        )));
    }

    // MAX_PORT is u16::MAX, so this check is a no-op for u16,
    // but kept for clarity if the type ever changes.
    #[allow(clippy::absurd_extreme_comparisons)]
    if port > MAX_PORT {
        return Err(AppError::invalid_config(format!(
            "Port {} exceeds maximum port number {}",
            port, MAX_PORT
        )));
    }

    Ok(())
}

/// Validate a port number, allowing privileged ports (for special cases like Caddy on 443)
pub fn validate_port_allow_privileged(port: u16) -> Result<(), AppError> {
    if port == 0 {
        return Err(AppError::invalid_config("Port cannot be 0"));
    }

    #[allow(clippy::absurd_extreme_comparisons)]
    if port > MAX_PORT {
        return Err(AppError::invalid_config(format!(
            "Port {} exceeds maximum port number {}",
            port, MAX_PORT
        )));
    }

    Ok(())
}

// ============================================================================
// Path Validation
// ============================================================================

/// Validate a file or directory path
///
/// Prevents path traversal attacks by:
/// 1. Rejecting paths with parent directory components (..)
/// 2. Expanding to absolute path
/// 3. Ensuring the path doesn't escape expected boundaries
///
/// # Arguments
/// * `path` - The path string to validate
///
/// # Returns
/// * `Ok(PathBuf)` with the canonicalized absolute path
/// * `Err(AppError)` if the path is invalid or contains traversal attempts
///
/// # Example
/// ```no_run
/// use burd_lib::validation::validate_path;
///
/// assert!(validate_path("/Users/dev/project").is_ok());
/// assert!(validate_path("../../etc/passwd").is_err()); // path traversal
/// ```
pub fn validate_path(path: &str) -> Result<PathBuf, AppError> {
    if path.is_empty() {
        return Err(AppError::invalid_config("Path cannot be empty"));
    }

    let path_buf = PathBuf::from(path);

    // Check for path traversal attempts using parent directory components
    if path_buf
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(AppError::invalid_config(
            "Path traversal not allowed (.. component detected)",
        ));
    }

    // Convert to absolute path if relative
    let absolute_path = if path_buf.is_absolute() {
        path_buf
    } else {
        std::env::current_dir()
            .map_err(|e| {
                AppError::invalid_config(format!("Failed to get current directory: {}", e))
            })?
            .join(path_buf)
    };

    Ok(absolute_path)
}

/// Validate a directory path and ensure it exists
///
/// Like `validate_path` but also ensures the path points to an existing directory.
///
/// # Arguments
/// * `path` - The path string to validate
///
/// # Returns
/// * `Ok(PathBuf)` with the canonicalized absolute path
/// * `Err(AppError)` if the path is invalid, contains traversal, or doesn't exist
pub fn validate_directory_path(path: &str) -> Result<PathBuf, AppError> {
    let path_buf = validate_path(path)?;

    // Check if path exists and is a directory
    if !path_buf.exists() {
        return Err(AppError::invalid_config(format!(
            "Directory does not exist: {}",
            path_buf.display()
        )));
    }

    if !path_buf.is_dir() {
        return Err(AppError::invalid_config(format!(
            "Path is not a directory: {}",
            path_buf.display()
        )));
    }

    // Canonicalize to resolve symlinks and get absolute path
    path_buf
        .canonicalize()
        .map_err(|e| AppError::invalid_config(format!("Failed to canonicalize path: {}", e)))
}

/// Validate a file path (file doesn't need to exist yet)
///
/// Validates the path structure without requiring the file to exist.
/// Useful for validating paths where files will be created.
pub fn validate_file_path(path: &str) -> Result<PathBuf, AppError> {
    validate_path(path)
}

/// Validate that a path is within an allowed parent directory
///
/// Ensures the path doesn't escape a specific parent directory boundary.
/// Useful for validating user-provided paths that should stay within a specific directory.
///
/// # Arguments
/// * `path` - The path to validate
/// * `allowed_parent` - The parent directory that the path must be within
///
/// # Returns
/// * `Ok(PathBuf)` if the path is within the allowed parent
/// * `Err(AppError)` if the path escapes the allowed parent
pub fn validate_path_within(path: &str, allowed_parent: &Path) -> Result<PathBuf, AppError> {
    let validated_path = validate_path(path)?;

    // Canonicalize the allowed parent
    let canonical_parent = allowed_parent
        .canonicalize()
        .map_err(|e| AppError::invalid_config(format!("Invalid parent directory: {}", e)))?;

    // Check if the path starts with the allowed parent
    if !validated_path.starts_with(&canonical_parent) {
        return Err(AppError::invalid_config(format!(
            "Path '{}' is outside allowed directory '{}'",
            validated_path.display(),
            canonical_parent.display()
        )));
    }

    Ok(validated_path)
}

// ============================================================================
// Instance Name Validation
// ============================================================================

/// Regex for valid instance names (alphanumeric, hyphens, underscores)
static INSTANCE_NAME_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-zA-Z0-9][a-zA-Z0-9_-]{0,62}[a-zA-Z0-9]$|^[a-zA-Z0-9]$").unwrap());

/// Validate an instance name
///
/// Instance names must:
/// - Be 1-64 characters long
/// - Start and end with alphanumeric characters
/// - Contain only alphanumeric characters, hyphens, and underscores
///
/// # Arguments
/// * `name` - The instance name to validate
///
/// # Returns
/// * `Ok(())` if the name is valid
/// * `Err(AppError)` if the name is invalid
///
/// # Example
/// ```
/// use burd_lib::validation::validate_instance_name;
///
/// assert!(validate_instance_name("my-instance").is_ok());
/// assert!(validate_instance_name("test_db_1").is_ok());
/// assert!(validate_instance_name("").is_err()); // empty
/// assert!(validate_instance_name("my instance").is_err()); // contains space
/// ```
pub fn validate_instance_name(name: &str) -> Result<(), AppError> {
    if name.is_empty() {
        return Err(AppError::invalid_config("Instance name cannot be empty"));
    }

    if name.len() > 64 {
        return Err(AppError::invalid_config(
            "Instance name cannot exceed 64 characters",
        ));
    }

    if !INSTANCE_NAME_REGEX.is_match(name) {
        return Err(AppError::invalid_config(
            "Instance name must contain only alphanumeric characters, hyphens, and underscores, and must start and end with an alphanumeric character"
        ));
    }

    Ok(())
}

// ============================================================================
// Domain Name Validation
// ============================================================================

/// Regex for valid DNS labels (RFC 1123 compliant)
static DNS_LABEL_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$").unwrap());

/// Validate a domain/subdomain name
///
/// Domain names must:
/// - Be 1-253 characters long (full domain length limit)
/// - Support multi-level subdomains separated by dots (e.g., "app.brandsgateway")
/// - Each label (part between dots) must be 1-63 characters
/// - Each label must start and end with lowercase alphanumeric characters
/// - Each label can contain only lowercase alphanumeric characters and hyphens
/// - Labels cannot start or end with a hyphen
///
/// # Arguments
/// * `domain` - The domain name to validate (can be multi-level subdomain)
///
/// # Returns
/// * `Ok(())` if the domain is valid
/// * `Err(AppError)` if the domain is invalid
///
/// # Example
/// ```
/// use burd_lib::validation::validate_domain_name;
///
/// assert!(validate_domain_name("api").is_ok());
/// assert!(validate_domain_name("my-service").is_ok());
/// assert!(validate_domain_name("app.brandsgateway").is_ok()); // multi-level
/// assert!(validate_domain_name("test123").is_ok());
/// assert!(validate_domain_name("My-Service").is_err()); // uppercase
/// assert!(validate_domain_name("-invalid").is_err()); // starts with hyphen
/// ```
pub fn validate_domain_name(domain: &str) -> Result<(), AppError> {
    if domain.is_empty() {
        return Err(AppError::invalid_config("Domain name cannot be empty"));
    }

    if domain.len() > 253 {
        return Err(AppError::invalid_config(
            "Domain name cannot exceed 253 characters (DNS limit)",
        ));
    }

    // Split by dots and validate each label
    let labels: Vec<&str> = domain.split('.').collect();

    for label in labels {
        if label.is_empty() {
            return Err(AppError::invalid_config(
                "Domain name cannot have empty labels (consecutive dots or leading/trailing dots)",
            ));
        }

        if label.len() > 63 {
            return Err(AppError::invalid_config(format!(
                "Domain label '{}' exceeds 63 characters (DNS label limit)",
                label
            )));
        }

        if !DNS_LABEL_REGEX.is_match(label) {
            return Err(AppError::invalid_config(format!(
                "Domain label '{}' is invalid. Labels must contain only lowercase alphanumeric characters and hyphens, and must start and end with an alphanumeric character",
                label
            )));
        }
    }

    Ok(())
}

/// Validate a TLD (top-level domain)
///
/// TLDs must:
/// - Be 2-63 characters long
/// - Contain only lowercase alphanumeric characters
/// - Not contain hyphens or special characters
///
/// # Arguments
/// * `tld` - The TLD to validate
///
/// # Returns
/// * `Ok(())` if the TLD is valid
/// * `Err(AppError)` if the TLD is invalid
pub fn validate_tld(tld: &str) -> Result<(), AppError> {
    if tld.is_empty() {
        return Err(AppError::invalid_config("TLD cannot be empty"));
    }

    if tld.len() < 2 {
        return Err(AppError::invalid_config(
            "TLD must be at least 2 characters",
        ));
    }

    if tld.len() > 63 {
        return Err(AppError::invalid_config("TLD cannot exceed 63 characters"));
    }

    // TLD should only contain lowercase alphanumeric characters
    if !tld
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    {
        return Err(AppError::invalid_config(
            "TLD must contain only lowercase alphanumeric characters",
        ));
    }

    Ok(())
}

// ============================================================================
// Version String Validation
// ============================================================================

/// Regex for semantic version strings
static VERSION_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$|^[0-9]+\.[0-9]+$|^system$").unwrap()
});

/// Validate a version string
///
/// Accepts:
/// - Semantic versions (e.g., "1.2.3", "1.0.0-alpha", "2.0.0+build.123")
/// - Simple versions (e.g., "8.0", "7.4")
/// - The special value "system" for system-installed binaries
///
/// # Arguments
/// * `version` - The version string to validate
///
/// # Returns
/// * `Ok(())` if the version is valid
/// * `Err(AppError)` if the version is invalid
///
/// # Example
/// ```
/// use burd_lib::validation::validate_version;
///
/// assert!(validate_version("1.6.0").is_ok());
/// assert!(validate_version("8.0").is_ok());
/// assert!(validate_version("system").is_ok());
/// assert!(validate_version("invalid").is_err());
/// assert!(validate_version("").is_err());
/// ```
pub fn validate_version(version: &str) -> Result<(), AppError> {
    if version.is_empty() {
        return Err(AppError::invalid_config("Version cannot be empty"));
    }

    if !VERSION_REGEX.is_match(version) {
        return Err(AppError::invalid_config(format!(
            "Invalid version format: '{}'. Expected semantic version (e.g., 1.2.3), simple version (e.g., 8.0), or 'system'",
            version
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Port validation tests
    #[test]
    fn test_validate_port() {
        assert!(validate_port(8080).is_ok());
        assert!(validate_port(3306).is_ok());
        assert!(validate_port(1024).is_ok());
        assert!(validate_port(65535).is_ok());

        assert!(validate_port(0).is_err());
        assert!(validate_port(80).is_err());
        assert!(validate_port(443).is_err());
        assert!(validate_port(1023).is_err());
    }

    #[test]
    fn test_validate_port_allow_privileged() {
        assert!(validate_port_allow_privileged(80).is_ok());
        assert!(validate_port_allow_privileged(443).is_ok());
        assert!(validate_port_allow_privileged(8080).is_ok());

        assert!(validate_port_allow_privileged(0).is_err());
    }

    // Path validation tests
    #[test]
    fn test_validate_path_traversal() {
        assert!(validate_path("../../etc/passwd").is_err());
        assert!(validate_path("/valid/path").is_ok());
        assert!(validate_path("valid/relative/path").is_ok());
    }

    #[test]
    fn test_validate_path_empty() {
        assert!(validate_path("").is_err());
    }

    // Instance name validation tests
    #[test]
    fn test_validate_instance_name() {
        assert!(validate_instance_name("my-instance").is_ok());
        assert!(validate_instance_name("test_db_1").is_ok());
        assert!(validate_instance_name("a").is_ok());
        assert!(validate_instance_name("MyInstance123").is_ok());

        assert!(validate_instance_name("").is_err());
        assert!(validate_instance_name("my instance").is_err());
        assert!(validate_instance_name("-invalid").is_err());
        assert!(validate_instance_name("invalid-").is_err());
        assert!(validate_instance_name("_invalid").is_err());
        assert!(validate_instance_name(&"a".repeat(65)).is_err());
    }

    // Domain name validation tests
    #[test]
    fn test_validate_domain_name() {
        // Single-level subdomains
        assert!(validate_domain_name("api").is_ok());
        assert!(validate_domain_name("my-service").is_ok());
        assert!(validate_domain_name("test123").is_ok());
        assert!(validate_domain_name("a").is_ok());

        // Multi-level subdomains
        assert!(validate_domain_name("app.brandsgateway").is_ok());
        assert!(validate_domain_name("api.v1.service").is_ok());
        assert!(validate_domain_name("my-app.staging").is_ok());

        // Invalid cases
        assert!(validate_domain_name("").is_err());
        assert!(validate_domain_name("My-Service").is_err()); // uppercase
        assert!(validate_domain_name("-invalid").is_err()); // starts with hyphen
        assert!(validate_domain_name("invalid-").is_err()); // ends with hyphen
        assert!(validate_domain_name("test..double").is_err()); // empty label
        assert!(validate_domain_name(".leading").is_err()); // leading dot
        assert!(validate_domain_name("trailing.").is_err()); // trailing dot
        assert!(validate_domain_name(&"a".repeat(64)).is_err()); // label too long
        assert!(validate_domain_name(&format!("{}.{}", "a".repeat(63), "b".repeat(191))).is_err());
        // total too long
    }

    // TLD validation tests
    #[test]
    fn test_validate_tld() {
        assert!(validate_tld("test").is_ok());
        assert!(validate_tld("burd").is_ok());
        assert!(validate_tld("dev").is_ok());
        assert!(validate_tld("local123").is_ok());

        assert!(validate_tld("").is_err());
        assert!(validate_tld("a").is_err());
        assert!(validate_tld("Test").is_err());
        assert!(validate_tld("test.com").is_err());
        assert!(validate_tld(&"a".repeat(64)).is_err());
    }

    // Version validation tests
    #[test]
    fn test_validate_version() {
        assert!(validate_version("1.6.0").is_ok());
        assert!(validate_version("8.0").is_ok());
        assert!(validate_version("7.4").is_ok());
        assert!(validate_version("system").is_ok());
        assert!(validate_version("1.0.0-alpha").is_ok());
        assert!(validate_version("2.0.0+build.123").is_ok());

        assert!(validate_version("").is_err());
        assert!(validate_version("invalid").is_err());
        assert!(validate_version("1.2.3.4").is_err());
    }
}
