//! PHP related commands
//!
//! Handles PVM (PHP Version Manager) commands for managing PHP versions.

use crate::pvm::{self, CurrentPHP, PHPVersion, PvmStatus, RemotePHPVersion, ShellIntegrationStatus};

// ============================================================================
// PVM Commands (PHP Version Manager)
// ============================================================================

/// Get PVM status (installed, current version, default version)
#[tauri::command]
pub fn get_pvm_status() -> PvmStatus {
    pvm::get_pvm_status()
}

/// Get currently active PHP version
#[tauri::command]
pub fn get_current_php() -> Option<CurrentPHP> {
    pvm::get_current_php()
}

/// List installed PHP versions
#[tauri::command]
pub async fn list_installed_php_versions() -> Result<Vec<PHPVersion>, String> {
    tokio::task::spawn_blocking(pvm::list_installed_versions)
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// List available remote PHP versions
#[tauri::command]
pub async fn list_remote_php_versions() -> Result<Vec<RemotePHPVersion>, String> {
    pvm::list_remote_versions().await
}

/// Download and install a PHP version
#[tauri::command]
pub async fn download_php_version(version: String, app: tauri::AppHandle) -> Result<(), String> {
    pvm::download_version(&version, &app).await
}

/// Delete a PHP version
#[tauri::command]
pub async fn delete_php_version(version: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || pvm::delete_version(&version))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Set the default PHP version
#[tauri::command]
pub async fn set_default_php_version(version: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || pvm::set_default_version(&version))
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Get PHP shell integration status
#[tauri::command]
pub fn get_php_shell_integration_status() -> ShellIntegrationStatus {
    pvm::get_shell_integration_status()
}

/// Configure PHP shell integration
#[tauri::command]
pub async fn configure_php_shell_integration() -> Result<(), String> {
    tokio::task::spawn_blocking(pvm::configure_shell_integration)
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Remove PHP shell integration
#[tauri::command]
pub async fn remove_php_shell_integration() -> Result<(), String> {
    tokio::task::spawn_blocking(pvm::remove_shell_integration)
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Fix PHP shell integration by reasserting Burd PATH at end of profile
#[tauri::command]
pub async fn fix_php_shell_integration() -> Result<(), String> {
    tokio::task::spawn_blocking(pvm::reassert_shell_integration)
        .await
        .map_err(|e| format!("Task error: {}", e))?
}
