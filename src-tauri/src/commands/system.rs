//! System related commands
//!
//! Handles settings, CLI, and helper tool management.

use crate::config::DomainTarget;
use crate::constants::CLI_INSTALL_PATH;
use crate::error::LockExt;
use crate::helper_client::HelperClient;
use crate::launchd;
use crate::lock; // Shared macro from error.rs
use crate::validation;
use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use super::AppState;

// ============================================================================
// Settings Commands
// ============================================================================

/// Application settings
#[derive(Debug, Serialize)]
pub struct AppSettings {
    pub tld: String,
    pub dns_port: u16,
    pub proxy_port: u16,
}

/// Get current application settings
#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;

    Ok(AppSettings {
        tld: config.tld,
        dns_port: config.dns_port,
        proxy_port: config.proxy_port,
    })
}

/// Update the TLD setting
/// Note: Requires app restart to take effect for DNS/proxy servers
#[tauri::command]
pub fn update_tld(tld: String, state: State<'_, AppState>) -> Result<(), String> {
    // Validate and normalize TLD
    let new_tld = tld.trim().to_lowercase();
    validation::validate_tld(&new_tld)
        .map_err(|e| format!("Invalid TLD: {}", e))?;

    // Get old TLD to check if it changed
    let old_tld = {
        let config_store = lock!(state.config_store)?;
        config_store.load()?.tld.clone()
    };

    // Update the config
    {
        let config_store = lock!(state.config_store)?;
        config_store.update_tld(new_tld.clone())?;
    }

    // If TLD changed, update resolver and certificates
    if old_tld != new_tld {
        // Update resolver if it was installed for old TLD
        if crate::resolver::is_installed(&old_tld) {

            // Get DNS port from config
            let dns_port = {
                let config_store = lock!(state.config_store)?;
                config_store.load()?.dns_port
            };

            // Uninstall old resolver
            crate::resolver::uninstall(&old_tld)?;

            // Install new resolver
            crate::resolver::install(&new_tld, dns_port)?;

            // Flush DNS cache
            let _ = crate::resolver::flush_dns_cache();
        }

        // Regenerate certificates if daemon is installed
        if launchd::is_installed() {
            // Get all domains with the new TLD
            let routes: Vec<crate::caddy::RouteEntry> = {
                let config_store = lock!(state.config_store)?;
                let config = config_store.load()?;

                let mut routes: Vec<crate::caddy::RouteEntry> = Vec::new();

                // Include all domain routes
                for d in &config.domains {
                    match &d.target {
                        DomainTarget::Instance(instance_id) => {
                            if let Some(instance) = config.instances.iter().find(|i| i.id == *instance_id) {
                                routes.push(crate::caddy::RouteEntry::reverse_proxy(
                                    d.full_domain(&new_tld),
                                    instance.port,
                                    d.id.to_string(),
                                    d.ssl_enabled,
                                ));
                            }
                        }
                        DomainTarget::Port(port) => {
                            routes.push(crate::caddy::RouteEntry::reverse_proxy(
                                d.full_domain(&new_tld),
                                *port,
                                d.id.to_string(),
                                d.ssl_enabled,
                            ));
                        }
                        DomainTarget::StaticFiles { path, browse } => {
                            routes.push(crate::caddy::RouteEntry::file_server(
                                d.full_domain(&new_tld),
                                path.clone(),
                                *browse,
                                d.id.to_string(),
                                d.ssl_enabled,
                            ));
                        }
                    }
                }

                routes
            };

            // Write new Caddyfile (Caddy will auto-reload)
            crate::caddy::write_caddyfile(&new_tld, &routes)?;

            // Restart daemon to ensure it picks up changes
            launchd::restart()?;
        }
    }

    Ok(())
}

// ============================================================================
// CLI Commands
// ============================================================================

/// CLI installation status
#[derive(Debug, Serialize)]
pub struct CliStatus {
    pub installed: bool,
    pub path: Option<String>,
    pub binary_exists: bool,
}

/// Get the path to the burd CLI binary
fn get_cli_binary_path(app_handle: &AppHandle) -> Option<std::path::PathBuf> {
    // Try to get the path from the resource directory (production)
    if let Ok(resource_dir) = app_handle.path().resource_dir() {
        let prod_path = resource_dir.join("burd");
        if prod_path.exists() {
            return Some(prod_path);
        }
    }

    // Try the app local data directory
    if let Ok(app_dir) = app_handle.path().app_local_data_dir() {
        let local_path = app_dir.join("bin").join("burd");
        if local_path.exists() {
            return Some(local_path);
        }
    }

    // Development: look for the binary relative to the executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(parent) = exe_path.parent() {
            let dev_path = parent.join("burd");
            if dev_path.exists() {
                return Some(dev_path);
            }
        }
    }

    None
}

/// Get CLI installation status
#[tauri::command]
pub fn get_cli_status(app_handle: AppHandle) -> CliStatus {
    let install_path = std::path::Path::new(CLI_INSTALL_PATH);
    let installed = install_path.exists() || install_path.is_symlink();

    let binary_exists = get_cli_binary_path(&app_handle).is_some();

    let path = if installed {
        Some(CLI_INSTALL_PATH.to_string())
    } else {
        None
    };

    CliStatus {
        installed,
        path,
        binary_exists,
    }
}

/// Install the CLI to /usr/local/bin
/// This may require admin privileges on some systems
#[tauri::command]
pub async fn install_cli(app_handle: AppHandle) -> Result<String, String> {
    let binary_path = get_cli_binary_path(&app_handle)
        .ok_or_else(|| "CLI binary not found. Build the project first.".to_string())?;

    let binary_path_str = binary_path.to_string_lossy().to_string();
    let install_path = CLI_INSTALL_PATH.to_string();

    // Use osascript to run with admin privileges on macOS
    tokio::task::spawn_blocking(move || {
        use std::process::Command;

        // First, check if /usr/local/bin exists, create if not
        let mkdir_script = r#"do shell script "mkdir -p /usr/local/bin" with administrator privileges"#;
        let _ = Command::new("osascript")
            .args(["-e", mkdir_script])
            .output();

        // Remove existing symlink/file if present
        let rm_script = format!(
            r#"do shell script "rm -f {}" with administrator privileges"#,
            install_path
        );
        let _ = Command::new("osascript")
            .args(["-e", &rm_script])
            .output();

        // Create the symlink
        let ln_script = format!(
            r#"do shell script "ln -sf '{}' '{}'" with administrator privileges"#,
            binary_path_str,
            install_path
        );

        let output = Command::new("osascript")
            .args(["-e", &ln_script])
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        if output.status.success() {
            Ok(format!("CLI installed to {}", install_path))
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("User canceled") || stderr.contains("canceled") {
                Err("Installation cancelled by user".to_string())
            } else {
                Err(format!("Failed to install CLI: {}", stderr))
            }
        }
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

/// Uninstall the CLI from /usr/local/bin
#[tauri::command]
pub async fn uninstall_cli() -> Result<String, String> {
    let install_path = CLI_INSTALL_PATH.to_string();

    tokio::task::spawn_blocking(move || {
        use std::process::Command;

        let rm_script = format!(
            r#"do shell script "rm -f {}" with administrator privileges"#,
            install_path
        );

        let output = Command::new("osascript")
            .args(["-e", &rm_script])
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        if output.status.success() {
            Ok("CLI uninstalled successfully".to_string())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("User canceled") || stderr.contains("canceled") {
                Err("Uninstallation cancelled by user".to_string())
            } else {
                Err(format!("Failed to uninstall CLI: {}", stderr))
            }
        }
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

// ============================================================================
// Helper Tool Commands
// ============================================================================

#[derive(Debug, Serialize)]
pub struct HelperStatus {
    pub installed: bool,
    pub running: bool,
}

#[tauri::command]
pub fn get_helper_status() -> HelperStatus {
    HelperStatus {
        installed: HelperClient::is_installed(),
        running: HelperClient::is_running(),
    }
}

#[tauri::command]
pub async fn install_helper() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        HelperClient::install()?;
        Ok("Helper installed successfully".to_string())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

#[tauri::command]
pub async fn uninstall_helper() -> Result<String, String> {
    tokio::task::spawn_blocking(|| {
        HelperClient::uninstall()?;
        Ok("Helper uninstalled successfully".to_string())
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?
}

/// Open Keychain Access application
#[tauri::command]
pub fn open_keychain_access() -> Result<(), String> {
    std::process::Command::new("open")
        .args(["-a", "Keychain Access"])
        .spawn()
        .map_err(|e| format!("Failed to open Keychain Access: {}", e))?;
    Ok(())
}
