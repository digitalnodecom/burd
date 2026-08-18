//! Path utilities for the application
//!
//! Handles paths for app data, binaries, instances, and PIDs.

use std::path::{Path, PathBuf};
use uuid::Uuid;

use super::ServiceType;

/// Recursively sum the byte size of every file under `path` (an instance's data
/// directory). Returns 0 if the path is missing or unreadable. Symlinks are not
/// followed, so cycles can't loop and linked targets aren't double-counted.
pub fn directory_size(path: &Path) -> u64 {
    let mut total: u64 = 0;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                if let Ok(meta) = entry.metadata() {
                    total = total.saturating_add(meta.len());
                }
            }
        }
    }
    total
}

pub fn get_app_dir() -> Result<PathBuf, String> {
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
