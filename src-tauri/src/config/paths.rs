//! Path utilities for the application
//!
//! Handles paths for app data, binaries, instances, and PIDs.

use std::path::PathBuf;
use uuid::Uuid;

use super::ServiceType;

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
        ServiceType::NodeRed => "node-red",
        ServiceType::Caddy => "caddy",
        ServiceType::Centrifugo => "centrifugo",
        ServiceType::Gitea => "gitea",
    }
}

/// Get the old flat binary path (for backward compatibility/migration)
pub fn get_binary_path(service_type: ServiceType) -> Result<PathBuf, String> {
    let binary_name = get_binary_name(service_type);
    get_bin_dir().map(|p| p.join(binary_name))
}

/// Get the versioned binary path: bin/{service_type}/{version}/{binary_name}
pub fn get_versioned_binary_path(service_type: ServiceType, version: &str) -> Result<PathBuf, String> {
    let binary_name = get_binary_name(service_type);
    get_bin_dir().map(|p| p.join(service_type.as_str()).join(version).join(binary_name))
}

/// Get the versioned directory: bin/{service_type}/{version}/
pub fn get_versioned_binary_dir(service_type: ServiceType, version: &str) -> Result<PathBuf, String> {
    get_bin_dir().map(|p| p.join(service_type.as_str()).join(version))
}

/// Get the service directory: bin/{service_type}/
pub fn get_service_bin_dir(service_type: ServiceType) -> Result<PathBuf, String> {
    get_bin_dir().map(|p| p.join(service_type.as_str()))
}
