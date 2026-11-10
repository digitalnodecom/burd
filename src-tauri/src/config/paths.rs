//! Path utilities for the application
//!
//! Handles paths for app data, binaries, instances, and PIDs.

use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::ServiceType;

/// Sum regular files without following symlinks. A failed walk is an error,
/// rather than a misleading zero or an incomplete estimate.
pub fn directory_size(path: &Path) -> Result<u64, String> {
    fn measure(path: &Path) -> std::io::Result<u64> {
        let mut total = 0u64;
        let mut stack = vec![path.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir)? {
                let entry = entry?;
                let file_type = entry.file_type()?;
                if file_type.is_dir() {
                    stack.push(entry.path());
                } else if file_type.is_file() {
                    total = total.saturating_add(entry.metadata()?.len());
                }
            }
        }
        Ok(total)
    }
    measure(path).map_err(|error| format!("Cannot measure {}: {error}", path.display()))
}

pub fn get_app_dir() -> Result<PathBuf, String> {
    // Explicit profiles allow automation and integration tests to isolate all
    // configuration, binaries, PID files, logs, and instance data.
    if let Some(directory) = std::env::var_os("BURD_DATA_DIR") {
        let directory = PathBuf::from(directory);
        if !directory.is_absolute() {
            return Err("BURD_DATA_DIR must be an absolute path".into());
        }
        return Ok(directory);
    }
    dirs::data_dir()
        .map(|p| p.join("Burd"))
        .ok_or_else(|| "Could not determine application data directory".to_string())
}

pub fn get_bin_dir() -> Result<PathBuf, String> {
    get_app_dir().map(|p| p.join("bin"))
}

pub fn get_pids_dir() -> Result<PathBuf, String> {
    get_app_dir().map(|p| p.join("pids"))
}

pub fn get_instances_dir() -> Result<PathBuf, String> {
    get_app_dir().map(|p| p.join("instances"))
}

pub fn get_instance_dir(id: &Uuid) -> Result<PathBuf, String> {
    get_instances_dir().map(|p| p.join(id.to_string()))
}

/// Get the binary name for a service type
pub fn get_binary_name(service_type: ServiceType) -> &'static str {
    match service_type {
        ServiceType::Meilisearch => "meilisearch",
        ServiceType::MongoDB => "mongod",
        ServiceType::Typesense => "typesense-server",
        ServiceType::MinIO => "minio",
        ServiceType::FrankenPHP => "frankenphp",
        ServiceType::FrankenPhpPark => "frankenphp", // Uses same binary as FrankenPHP
        ServiceType::MariaDB => "mariadbd",
        ServiceType::MySQL => "mysqld",
        ServiceType::PostgreSQL => "postgres",
        ServiceType::Redis => "redis-server",
        ServiceType::Valkey => "valkey-server",
        ServiceType::Mailpit => "mailpit",
        ServiceType::Beanstalkd => "beanstalkd",
        ServiceType::Memcached => "memcached",
        ServiceType::Frpc => "frpc",
        ServiceType::Caddy => "caddy",
        ServiceType::Centrifugo => "centrifugo",
        ServiceType::Gitea => "gitea",
        ServiceType::Bun => "bun",
    }
}

/// Get the old flat binary path (for backward compatibility/migration)
pub fn get_binary_path(service_type: ServiceType) -> Result<PathBuf, String> {
    let binary_name = get_binary_name(service_type);
    get_bin_dir().map(|p| p.join(binary_name))
}

/// Get the versioned binary path: bin/{service_type}/{version}/{binary_name}
pub fn get_versioned_binary_path(
    service_type: ServiceType,
    version: &str,
) -> Result<PathBuf, String> {
    let binary_name = get_binary_name(service_type);
    get_bin_dir().map(|p| {
        p.join(service_type.as_str())
            .join(version)
            .join(binary_name)
    })
}

/// Get the versioned directory: bin/{service_type}/{version}/
pub fn get_versioned_binary_dir(
    service_type: ServiceType,
    version: &str,
) -> Result<PathBuf, String> {
    get_bin_dir().map(|p| p.join(service_type.as_str()).join(version))
}

/// Get the service directory: bin/{service_type}/
pub fn get_service_bin_dir(service_type: ServiceType) -> Result<PathBuf, String> {
    get_bin_dir().map(|p| p.join(service_type.as_str()))
}

#[cfg(test)]
mod disk_usage_tests {
    use super::*;

    #[test]
    fn measures_files_and_ignores_symlink_cycles() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("data"), b"12345").unwrap();
        std::os::unix::fs::symlink(dir.path(), dir.path().join("loop")).unwrap();
        assert_eq!(directory_size(dir.path()).unwrap(), 5);
    }

    #[test]
    fn failed_walk_is_not_reported_as_zero() {
        let dir = tempfile::tempdir().unwrap();
        assert!(directory_size(&dir.path().join("missing")).is_err());
        assert_eq!(directory_size(dir.path()).unwrap(), 0);
    }
}
