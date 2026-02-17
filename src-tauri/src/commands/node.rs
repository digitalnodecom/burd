//! Node.js related commands
//!
//! Handles NVM (Node Version Manager), PM2 (Process Manager), and Node-RED commands.

use crate::nvm::{self, NodeVersion, NvmStatus};
use crate::pm2::{self, Pm2Process, Pm2Status};

// ============================================================================
// NVM Commands (Node Version Manager)
// ============================================================================

/// Get NVM status (installed, current version, default version)
#[tauri::command]
pub fn get_nvm_status() -> NvmStatus {
    nvm::get_nvm_status()
}

/// Check if NVM is installed
#[tauri::command]
pub fn is_nvm_installed() -> bool {
    nvm::is_nvm_installed()
}

/// List installed Node versions
#[tauri::command]
pub async fn list_installed_node_versions() -> Result<Vec<NodeVersion>, String> {
    tokio::task::spawn_blocking(|| nvm::list_installed_versions().map_err(|e| e.to_string()))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// List available remote LTS versions
#[tauri::command]
pub async fn list_remote_node_versions() -> Result<Vec<NodeVersion>, String> {
    tokio::task::spawn_blocking(|| nvm::list_remote_lts_versions().map_err(|e| e.to_string()))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Install a Node version
#[tauri::command]
pub async fn install_node_version(version: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || nvm::install_version(&version).map_err(|e| e.to_string()))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Uninstall a Node version
#[tauri::command]
pub async fn uninstall_node_version(version: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || nvm::uninstall_version(&version).map_err(|e| e.to_string()))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Set the default Node version
#[tauri::command]
pub async fn set_default_node_version(version: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        nvm::set_default_version(&version).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

// ============================================================================
// PM2 Commands (Node Process Manager)
// ============================================================================

/// Get PM2 status (installed, version, running processes)
#[tauri::command]
pub fn get_pm2_status() -> Pm2Status {
    pm2::get_pm2_status()
}

/// Check if PM2 is installed
#[tauri::command]
pub fn is_pm2_installed() -> bool {
    pm2::is_pm2_installed()
}

/// Install PM2 globally via npm
#[tauri::command]
pub async fn install_pm2() -> Result<String, String> {
    tokio::task::spawn_blocking(pm2::install_pm2)
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// List all PM2 processes
#[tauri::command]
pub async fn pm2_list() -> Result<Vec<Pm2Process>, String> {
    tokio::task::spawn_blocking(pm2::list_processes)
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Start an application with PM2
#[tauri::command]
pub async fn pm2_start(
    name: String,
    script: String,
    args: Option<String>,
    cwd: Option<String>,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        pm2::start_app(&name, &script, args.as_deref(), cwd.as_deref())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

/// Stop a PM2 process
#[tauri::command]
pub async fn pm2_stop(name_or_id: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || pm2::stop_app(&name_or_id))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Restart a PM2 process
#[tauri::command]
pub async fn pm2_restart(name_or_id: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || pm2::restart_app(&name_or_id))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Delete a PM2 process
#[tauri::command]
pub async fn pm2_delete(name_or_id: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || pm2::delete_app(&name_or_id))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Get logs for a PM2 process
#[tauri::command]
pub async fn pm2_logs(name_or_id: String, lines: Option<u32>) -> Result<String, String> {
    tokio::task::spawn_blocking(move || pm2::get_logs(&name_or_id, lines.unwrap_or(50)))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Save PM2 process list
#[tauri::command]
pub async fn pm2_save() -> Result<String, String> {
    tokio::task::spawn_blocking(pm2::save)
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Stop all PM2 processes
#[tauri::command]
pub async fn pm2_stop_all() -> Result<String, String> {
    tokio::task::spawn_blocking(pm2::stop_all)
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Delete all PM2 processes
#[tauri::command]
pub async fn pm2_delete_all() -> Result<String, String> {
    tokio::task::spawn_blocking(pm2::delete_all)
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

// ============================================================================
// Node-RED Commands
// ============================================================================

/// Initialize a Node-RED instance (runs npm install)
#[tauri::command]
pub async fn init_nodered_instance(id: String) -> Result<(), String> {
    use crate::config::get_instance_dir;
    use crate::services::nodered::NodeRedService;
    use uuid::Uuid;

    let instance_id = Uuid::parse_str(&id).map_err(|e| format!("Invalid instance ID: {}", e))?;
    let data_dir = get_instance_dir(&instance_id)?;

    // Get the instance to find the version
    let config_store = crate::config::ConfigStore::new()?;
    let config = config_store.load()?;
    let instance = config
        .instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| format!("Instance not found: {}", id))?
        .clone();

    // Run npm install in a blocking thread
    tokio::task::spawn_blocking(move || NodeRedService::init_instance(&data_dir, &instance.version))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Check if a Node-RED instance is initialized
#[tauri::command]
pub async fn is_nodered_initialized(id: String) -> Result<bool, String> {
    use crate::config::get_instance_dir;
    use crate::services::nodered::NodeRedService;
    use uuid::Uuid;

    let instance_id = Uuid::parse_str(&id).map_err(|e| format!("Invalid instance ID: {}", e))?;
    let data_dir = get_instance_dir(&instance_id)?;

    Ok(NodeRedService::is_initialized(&data_dir))
}
