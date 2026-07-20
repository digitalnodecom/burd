//! Tauri commands for managing PostgreSQL extensions on a database instance.
//!
//! Backs the extensions manager in the PostgreSQL instance settings UI.

use tauri::State;
use uuid::Uuid;

use super::AppState;
use crate::db_manager::{create_manager_for_instance, ExtensionInfo};
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
