//! Node.js related commands
//!
//! Handles NVM (Node Version Manager) commands.

use crate::nvm::{self, NodeVersion, NvmStatus};

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
