//! Tauri commands for a database instance's data views: PostgreSQL extensions,
//! the per-database size breakdown, and the instance's on-disk usage.

use tauri::State;
use uuid::Uuid;

use super::AppState;
use crate::config::{directory_size, get_instance_dir};
use crate::db_manager::{create_manager_for_instance, DatabaseInfo, ExtensionInfo};
use crate::error::LockExt;
use crate::lock;

/// Resolve an instance by id from the current config.
fn find_instance(state: &AppState, instance_id: &str) -> Result<crate::config::Instance, String> {
    let uuid = Uuid::parse_str(instance_id).map_err(|_| "Invalid instance ID".to_string())?;
    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;
    config
        .instances
        .iter()
        .find(|i| i.id == uuid)
        .cloned()
        .ok_or_else(|| "Instance not found".to_string())
}

/// List databases in a PostgreSQL instance (for the extensions picker).
#[tauri::command]
pub async fn list_instance_databases(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<String>, String> {
    let instance = find_instance(&state, &instance_id)?;
    let manager = create_manager_for_instance(&instance)?;
    Ok(manager
        .list_databases()?
        .into_iter()
        .map(|d| d.name)
        .collect())
}

/// On-disk size (bytes) of an instance's data directory. Works for any service
/// type — stopped or running — since the data directory persists either way.
#[tauri::command]
pub async fn get_instance_disk_usage(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<u64, String> {
    let instance = find_instance(&state, &instance_id)?;
    let dir = get_instance_dir(&instance.id)?;
    // The directory walk hits the filesystem, so keep it off the async runtime.
    tokio::task::spawn_blocking(move || directory_size(&dir))
        .await
        .map_err(|e| format!("Failed to compute disk usage: {e}"))
}

/// List the databases in a DB instance with each database's size — backs the
/// "Databases" view shown on PostgreSQL/MariaDB instances.
#[tauri::command]
pub async fn list_instance_database_details(
    state: State<'_, AppState>,
    instance_id: String,
) -> Result<Vec<DatabaseInfo>, String> {
    let instance = find_instance(&state, &instance_id)?;
    let manager = create_manager_for_instance(&instance)?;
    manager.list_databases()
}

/// List extensions available in a database and whether each is enabled.
#[tauri::command]
pub async fn list_database_extensions(
    state: State<'_, AppState>,
    instance_id: String,
    database: String,
) -> Result<Vec<ExtensionInfo>, String> {
    let instance = find_instance(&state, &instance_id)?;
    let manager = create_manager_for_instance(&instance)?;
    manager.list_extensions(&database)
}

/// Enable or disable a PostgreSQL extension on a database.
#[tauri::command]
pub async fn set_database_extension(
    state: State<'_, AppState>,
    instance_id: String,
    database: String,
    extension: String,
    enabled: bool,
) -> Result<(), String> {
    let instance = find_instance(&state, &instance_id)?;
    let manager = create_manager_for_instance(&instance)?;
    if enabled {
        manager.enable_extension(&database, &extension)
    } else {
        manager.disable_extension(&database, &extension)
    }
}
