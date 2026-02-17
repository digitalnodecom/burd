//! Service binary related commands
//!
//! Handles binary downloads, version management, and service registry.

use crate::binary::{BinaryStatus, VersionInfo};
use crate::config::ServiceType;
use crate::error::LockExt;
use crate::lock; // Shared macro from error.rs
use crate::service_config::{ServiceInfo, ServiceRegistry};
use tauri::{AppHandle, State};

use super::AppState;

pub fn parse_service_type(s: &str) -> Result<ServiceType, String> {
    match s.to_lowercase().as_str() {
        "meilisearch" => Ok(ServiceType::Meilisearch),
        "mongodb" => Ok(ServiceType::MongoDB),
        "typesense" => Ok(ServiceType::Typesense),
        "minio" => Ok(ServiceType::MinIO),
        "frankenphp" => Ok(ServiceType::FrankenPHP),
        "frankenphp-park" => Ok(ServiceType::FrankenPhpPark),
        "mariadb" => Ok(ServiceType::MariaDB),
        "mysql" => Ok(ServiceType::MySQL),
        "postgresql" => Ok(ServiceType::PostgreSQL),
        "redis" => Ok(ServiceType::Redis),
        "valkey" => Ok(ServiceType::Valkey),
        "mailpit" => Ok(ServiceType::Mailpit),
        "beanstalkd" => Ok(ServiceType::Beanstalkd),
        "memcached" => Ok(ServiceType::Memcached),
        "frpc" => Ok(ServiceType::Frpc),
        "nodered" => Ok(ServiceType::NodeRed),
        "caddy" => Ok(ServiceType::Caddy),
        "centrifugo" => Ok(ServiceType::Centrifugo),
        "gitea" => Ok(ServiceType::Gitea),
        _ => Err(format!("Unknown service type: {}", s)),
    }
}

// ============================================================================
// Binary/Version Commands
// ============================================================================

#[tauri::command]
pub fn get_binary_status(
    service_type: String,
    state: State<'_, AppState>,
) -> Result<BinaryStatus, String> {
    let svc_type = parse_service_type(&service_type)?;

    let config_store = lock!(state.config_store)?;
    let binary_manager = lock!(state.binary_manager)?;

    binary_manager.get_status_sync(svc_type, &config_store)
}

#[tauri::command]
pub fn get_all_binary_statuses(state: State<'_, AppState>) -> Result<Vec<BinaryStatus>, String> {
    let config_store = lock!(state.config_store)?;
    let binary_manager = lock!(state.binary_manager)?;

    binary_manager.get_all_statuses_sync(&config_store)
}

#[tauri::command]
pub async fn get_available_versions(
    service_type: String,
    state: State<'_, AppState>,
) -> Result<Vec<VersionInfo>, String> {
    let svc_type = parse_service_type(&service_type)?;

    let binary_manager = {
        state
            .binary_manager
            .lock()
            .map_err(|_| "Lock error")?
            .clone()
    };

    binary_manager.get_available_versions(svc_type).await
}

/// Get installed versions for a service type
#[tauri::command]
pub fn get_installed_versions(
    service_type: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let svc_type = parse_service_type(&service_type)?;

    let binary_manager = lock!(state.binary_manager)?;
    binary_manager.get_installed_versions_sync(svc_type)
}

/// Delete a specific version of a binary
#[tauri::command]
pub fn delete_binary_version(
    service_type: String,
    version: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let svc_type = parse_service_type(&service_type)?;

    // Check if any instances are using this version
    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;

    for instance in &config.instances {
        if instance.service_type == svc_type && instance.version == version {
            return Err(format!(
                "Cannot delete version {} - instance '{}' is using it",
                version, instance.name
            ));
        }
    }
    drop(config_store);

    // Delete the binary version
    let binary_manager = lock!(state.binary_manager)?;
    binary_manager.delete_version(svc_type, &version)?;
    drop(binary_manager);

    // Remove from config
    let config_store = lock!(state.config_store)?;
    config_store.remove_binary_version(svc_type, &version)?;

    Ok(())
}

#[tauri::command]
pub async fn download_binary(
    service_type: String,
    version: String,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let svc_type = parse_service_type(&service_type)?;

    // Get binary manager and config store, perform the download
    let binary_manager = {
        state
            .binary_manager
            .lock()
            .map_err(|_| "Lock error")?
            .clone()
    };

    let binary_info = binary_manager.download(svc_type, &version, app).await?;

    // Update config with the binary info
    let config_store = lock!(state.config_store)?;
    config_store.update_binary_info(svc_type, binary_info)?;

    Ok(())
}

// ============================================================================
// Service Registry
// ============================================================================

#[tauri::command]
pub fn get_available_services() -> Vec<ServiceInfo> {
    let registry = ServiceRegistry::load();
    registry.get_service_info_list()
}
