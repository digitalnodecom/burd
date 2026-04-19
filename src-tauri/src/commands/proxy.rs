//! Proxy related commands
//!
//! Handles Caddy proxy setup and daemon management.

use crate::binary::BinaryManager;
use crate::caddy;
use crate::config::ServiceType;
use crate::constants::PROXY_PLIST_PATH;
use crate::error::LockExt;
use crate::helper_client::{HelperClient, HelperRequest};
use crate::launchd;
use crate::lock; // Shared macro from error.rs
use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, State};

use super::AppState;

// ============================================================================
// Proxy Commands (Caddy-based)
// ============================================================================

/// Combined proxy status
#[derive(Debug, Serialize, Clone)]
pub struct ProxyStatus {
    pub daemon_installed: bool,
    pub daemon_running: bool,
    pub daemon_pid: Option<u32>,
    pub caddy_installed: bool,
    /// None if daemon not installed/running, Some(true) if Burd's Caddy responds, Some(false) if port hijacked
    pub proxy_healthy: Option<bool>,
}

/// Check if Burd's Caddy is the one actually listening on port 80.
/// Returns None if daemon not installed/running, Some(true) if healthy, Some(false) if port hijacked.
pub fn check_health_sync() -> Option<bool> {
    let daemon_status = launchd::get_status();
    if !daemon_status.installed || !daemon_status.running {
        return None;
    }

    // Make a quick HTTP request to the health endpoint on 127.0.0.1:80
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .ok()?;

    match client
        .get("http://127.0.0.1/_burd/health")
        .header("Host", "health-check.burd")
        .send()
    {
        Ok(resp) => {
            let body = resp.text().unwrap_or_default();
            Some(body.trim() == "burd-ok")
        }
        Err(_) => Some(false),
    }
}

/// Get the status of the privileged proxy daemon (Caddy)
#[tauri::command]
pub fn get_proxy_status(state: State<'_, AppState>) -> Result<ProxyStatus, String> {
    let daemon_status = launchd::get_status();

    // Read cached health from AppState if available
    let proxy_healthy = if daemon_status.installed && daemon_status.running {
        let cached = state.proxy_healthy.load(std::sync::atomic::Ordering::Relaxed);
        // 0 = unknown/not-checked, 1 = healthy, 2 = unhealthy
        match cached {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    } else {
        None
    };

    Ok(ProxyStatus {
        daemon_installed: daemon_status.installed,
        daemon_running: daemon_status.running,
        daemon_pid: daemon_status.pid,
        caddy_installed: caddy::is_caddy_installed(),
        proxy_healthy,
    })
}

/// Manually check proxy health (for frontend polling)
#[tauri::command]
pub async fn check_proxy_health() -> Result<Option<bool>, String> {
    let result =
        tokio::task::spawn_blocking(check_health_sync)
            .await
            .map_err(|e| format!("Task error: {}", e))?;
    Ok(result)
}

/// A process holding a port that Burd's reverse proxy needs
#[derive(Debug, Serialize, Clone)]
pub struct PortConflict {
    pub port: u16,
    pub pid: u32,
    pub command: String,
    pub user: Option<String>,
}

/// Query lsof for processes listening on a given TCP port.
/// Uses `-F pcLn` machine-readable output so we can parse reliably without root.
fn list_port_listeners(port: u16) -> Vec<(u32, String, Option<String>)> {
    let output = std::process::Command::new("lsof")
        .args([
            &format!("-iTCP:{}", port),
            "-sTCP:LISTEN",
            "-n",
            "-P",
            "-F",
            "pcLn",
        ])
        .output();

    let stdout = match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).into_owned(),
        Err(_) => return Vec::new(),
    };

    // lsof -F emits one record per process set:
    //   p<pid>
    //   c<command>
    //   L<user>
    //   f<fd>
    //   n<name>
    // Multiple file sets may follow a single process, so dedupe by pid.
    let mut results: std::collections::BTreeMap<u32, (String, Option<String>)> =
        std::collections::BTreeMap::new();
    let mut cur_pid: Option<u32> = None;
    let mut cur_cmd: Option<String> = None;
    let mut cur_user: Option<String> = None;

    for line in stdout.lines() {
        if line.is_empty() {
            continue;
        }
        let (tag, rest) = line.split_at(1);
        match tag {
            "p" => {
                cur_pid = rest.parse::<u32>().ok();
                cur_cmd = None;
                cur_user = None;
            }
            "c" => cur_cmd = Some(rest.to_string()),
            "L" => cur_user = Some(rest.to_string()),
            "n" => {
                if let Some(pid) = cur_pid {
                    let cmd = cur_cmd.clone().unwrap_or_else(|| "unknown".to_string());
                    results.entry(pid).or_insert((cmd, cur_user.clone()));
                }
            }
            _ => {}
        }
    }

    results
        .into_iter()
        .map(|(pid, (cmd, user))| (pid, cmd, user))
        .collect()
}

/// Get processes holding ports 80/443, excluding Burd's own Caddy daemon.
/// Used to show the user which services are preventing the reverse proxy from binding.
#[tauri::command]
pub async fn get_proxy_port_conflicts() -> Result<Vec<PortConflict>, String> {
    tokio::task::spawn_blocking(|| {
        let burd_pid = launchd::get_status().pid;

        let mut conflicts = Vec::new();
        for port in [80u16, 443u16] {
            for (pid, command, user) in list_port_listeners(port) {
                if Some(pid) == burd_pid {
                    continue;
                }
                conflicts.push(PortConflict {
                    port,
                    pid,
                    command,
                    user,
                });
            }
        }
        conflicts
    })
    .await
    .map_err(|e| format!("Task error: {}", e))
}

/// Setup the privileged proxy (download Caddy, install launchd daemon)
/// This requires admin privileges and will prompt the user.
#[tauri::command]
pub async fn setup_proxy(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let tld = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;
        config.tld.clone()
    };

    // Download Caddy if not installed
    if !caddy::is_caddy_installed() {
        // Get the latest Caddy version
        let versions = BinaryManager::new()
            .get_available_versions(ServiceType::Caddy)
            .await?;

        let version = versions
            .first()
            .ok_or("No Caddy versions available")?
            .version
            .clone();

        // Download Caddy
        let binary_manager = {
            state
                .binary_manager
                .lock()
                .map_err(|_| "Lock error")?
                .clone()
        };

        let binary_info = binary_manager
            .download(ServiceType::Caddy, &version, app)
            .await?;

        // Update config with the binary info
        let config_store = lock!(state.config_store)?;
        config_store.update_binary_info(ServiceType::Caddy, binary_info)?;
    }

    // Install Caddy binary to system location for daemon use
    tokio::task::spawn_blocking(caddy::install_caddy_for_daemon)
        .await
        .map_err(|e| format!("Task error: {}", e))??;

    // Write initial Caddyfile with current routes
    let routes = {
        let proxy = state.proxy_server.lock().await;
        proxy
            .list_routes()
            .into_iter()
            .map(|r| match r.route_type {
                crate::proxy::ProxyRouteType::ReverseProxy { port } => {
                    caddy::RouteEntry::reverse_proxy(r.domain, port, r.instance_id, r.ssl_enabled)
                }
                crate::proxy::ProxyRouteType::FileServer { path, browse } => {
                    caddy::RouteEntry::file_server(
                        r.domain,
                        path,
                        browse,
                        r.instance_id,
                        r.ssl_enabled,
                    )
                }
            })
            .collect::<Vec<_>>()
    };

    let tld_for_caddyfile = tld.clone();
    tokio::task::spawn_blocking(move || caddy::write_caddyfile(&tld_for_caddyfile, &routes))
        .await
        .map_err(|e| format!("Task error: {}", e))??;

    // Install and start launchd daemon (this will start Caddy)
    tokio::task::spawn_blocking(launchd::install)
        .await
        .map_err(|e| format!("Task error: {}", e))??;

    // Wait a moment for Caddy to initialize and create directories
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fix permissions on Caddy data directory so user can read CA certs
    tokio::task::spawn_blocking(|| {
        let _ = fix_caddy_data_permissions();
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?;

    // Auto-trust CA if it exists but isn't trusted yet
    // Note: CA may not exist yet if no HTTPS domain has been accessed
    tokio::task::spawn_blocking(|| {
        let _ = auto_trust_ca_if_needed();
    })
    .await
    .map_err(|e| format!("Task error: {}", e))?;

    // Update config to reflect installation
    {
        let config_store = lock!(state.config_store)?;
        config_store.set_proxy_installed(true)?;
    }

    Ok(())
}

/// Disable the privileged proxy (uninstall launchd daemon)
/// This requires admin privileges and will prompt the user.
#[tauri::command]
pub async fn disable_proxy(state: State<'_, AppState>) -> Result<(), String> {
    // Uninstall launchd plist (run in blocking task for osascript)
    tokio::task::spawn_blocking(launchd::uninstall)
        .await
        .map_err(|e| format!("Task error: {}", e))??;

    // Update config to reflect uninstallation
    let config_store = lock!(state.config_store)?;
    config_store.set_proxy_installed(false)?;

    Ok(())
}

/// Start the privileged proxy daemon
#[tauri::command]
pub fn start_proxy_daemon() -> Result<(), String> {
    launchd::start()?;
    // Give Caddy time to start and fix permissions
    std::thread::sleep(std::time::Duration::from_millis(300));
    let _ = fix_caddy_data_permissions();
    Ok(())
}

/// Restart the privileged proxy daemon
#[tauri::command]
pub fn restart_proxy_daemon() -> Result<(), String> {
    launchd::restart()?;
    // Give Caddy time to restart and fix permissions
    std::thread::sleep(std::time::Duration::from_millis(300));
    let _ = fix_caddy_data_permissions();
    Ok(())
}

/// Restart proxy daemon
#[tauri::command]
pub fn restart_proxy_for_certs() -> Result<(), String> {
    // With Caddy, we don't need to regenerate certificates
    // Just restart the daemon if it's installed
    if launchd::is_installed() {
        launchd::restart()?;
        // Give Caddy time to restart and fix permissions
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = fix_caddy_data_permissions();
    }
    Ok(())
}

/// Proxy configuration info for debugging
#[derive(Debug, Serialize)]
pub struct ProxyConfigInfo {
    pub caddyfile_path: String,
    pub caddyfile_content: Option<String>,
    pub plist_file: String,
    pub plist_content: Option<String>,
    pub daemon_installed: bool,
    pub daemon_running: bool,
    pub daemon_pid: Option<u32>,
    pub tld: String,
    pub caddy_version: Option<String>,
}

/// Get the current proxy configuration for debugging
#[tauri::command]
pub fn get_proxy_config(state: State<'_, AppState>) -> Result<ProxyConfigInfo, String> {
    let tld = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;
        config.tld.clone()
    };

    // Read Caddyfile content
    let caddyfile_content = caddy::read_caddyfile().ok();

    // Read plist file content
    let plist_content = std::fs::read_to_string(PROXY_PLIST_PATH).ok();

    // Get daemon status
    let daemon_status = launchd::get_status();

    // Get Caddy version
    let caddy_version = caddy::get_caddy_version().ok();

    Ok(ProxyConfigInfo {
        caddyfile_path: caddy::get_caddyfile_path().display().to_string(),
        caddyfile_content,
        plist_file: PROXY_PLIST_PATH.to_string(),
        plist_content,
        daemon_installed: daemon_status.installed,
        daemon_running: daemon_status.running,
        daemon_pid: daemon_status.pid,
        tld,
        caddy_version,
    })
}

// ============================================================================
// CA Trust Commands
// ============================================================================

/// Status of Caddy's root CA trust
#[derive(Debug, Serialize)]
pub struct CATrustStatus {
    /// Whether the CA certificate file exists (generated by Caddy on first HTTPS access)
    pub ca_exists: bool,
    /// Whether the CA is trusted in the system keychain
    pub is_trusted: bool,
    /// Path to the CA certificate file
    pub ca_path: String,
    /// Certificate common name (e.g., "Caddy Local Authority - 2026 ECC Root")
    pub cert_name: Option<String>,
    /// Certificate expiration date (e.g., "Nov 11 08:46:28 2035 GMT")
    pub cert_expiry: Option<String>,
}

/// Get the path to Caddy's root CA certificate
fn get_caddy_ca_path() -> PathBuf {
    // When running as daemon with XDG_DATA_HOME set to user space, Caddy stores PKI here
    launchd::get_caddy_data_dir().join("caddy/pki/authorities/local/root.crt")
}

/// Parse certificate metadata using openssl (for user-accessible paths only)
fn get_cert_metadata_local(cert_path: &std::path::Path) -> (Option<String>, Option<String>) {
    use std::process::Command;

    let cert_path_str = cert_path.to_string_lossy();

    // Get subject (certificate name)
    let name = Command::new("openssl")
        .args(["x509", "-in", cert_path_str.as_ref(), "-noout", "-subject"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let output = String::from_utf8_lossy(&o.stdout);
                output
                    .split("CN = ")
                    .nth(1)
                    .or_else(|| output.split("CN=").nth(1))
                    .map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    // Get expiry date
    let expiry = Command::new("openssl")
        .args(["x509", "-in", cert_path_str.as_ref(), "-noout", "-enddate"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let output = String::from_utf8_lossy(&o.stdout);
                output.split('=').nth(1).map(|s| s.trim().to_string())
            } else {
                None
            }
        });

    (name, expiry)
}

/// Internal function to get CA trust status (callable from other modules)
pub fn get_ca_trust_status_internal() -> Result<CATrustStatus, String> {
    let ca_path = get_caddy_ca_path();
    let ca_path_str = ca_path.to_string_lossy().to_string();

    // Use helper to check CA (root-owned directory requires elevated access)
    let (ca_exists, cert_name, cert_expiry) = if HelperClient::is_running() {
        match HelperClient::send_request(HelperRequest::GetCertInfo {
            cert_path: ca_path_str.clone(),
        }) {
            Ok(response) if response.success => {
                if response.message == "not_found" {
                    (false, None, None)
                } else if response.message.starts_with("exists|") {
                    // Parse "exists|name|expiry" format
                    let parts: Vec<&str> = response.message.splitn(3, '|').collect();
                    let name = parts
                        .get(1)
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());
                    let expiry = parts
                        .get(2)
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string());
                    (true, name, expiry)
                } else {
                    (false, None, None)
                }
            }
            _ => (false, None, None),
        }
    } else {
        // Fallback to direct check if helper not running
        let exists = ca_path.exists();
        let (name, expiry) = if exists {
            get_cert_metadata_local(&ca_path)
        } else {
            (None, None)
        };
        (exists, name, expiry)
    };

    // Check if trusted by running security verify-cert locally
    // This works now that we've fixed permissions on the CA directory
    let is_trusted = if ca_exists {
        std::process::Command::new("security")
            .args([
                "verify-cert",
                "-c",
                &ca_path_str,
                "-p",
                "ssl",
                "-l", // use only local keychains
            ])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    } else {
        false
    };

    Ok(CATrustStatus {
        ca_exists,
        is_trusted,
        ca_path: ca_path_str,
        cert_name,
        cert_expiry,
    })
}

/// Get the trust status of Caddy's root CA
#[tauri::command]
pub fn get_ca_trust_status() -> Result<CATrustStatus, String> {
    get_ca_trust_status_internal()
}

/// Internal function to trust CA (callable from other modules)
/// Adds the certificate to the user's login keychain trust settings
pub fn trust_caddy_ca_internal() -> Result<(), String> {
    let ca_path = get_caddy_ca_path();

    if !ca_path.exists() {
        return Err("CA certificate not found".to_string());
    }

    // Add to user trust settings (doesn't require admin privileges)
    // This trusts the certificate for SSL in the current user's keychain
    let output = std::process::Command::new("security")
        .args([
            "add-trusted-cert",
            "-r",
            "trustRoot",
            "-p",
            "ssl",
            ca_path.to_str().ok_or("Invalid path")?,
        ])
        .output()
        .map_err(|e| format!("Failed to run security command: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!("Failed to trust CA: {}", stderr))
    }
}

/// Auto-trust CA if it exists but is not yet trusted
/// Returns Ok(true) if CA was trusted, Ok(false) if no action needed, Err on failure
pub fn auto_trust_ca_if_needed() -> Result<bool, String> {
    if let Ok(status) = get_ca_trust_status_internal() {
        if status.ca_exists && !status.is_trusted {
            trust_caddy_ca_internal()?;
            return Ok(true);
        }
    }
    Ok(false)
}

/// Fix permissions on Caddy data directory so user can read CA certificates
/// Caddy runs as root and creates files with restrictive permissions
pub fn fix_caddy_data_permissions() -> Result<(), String> {
    if !HelperClient::is_running() {
        return Err("Helper is not running".to_string());
    }

    let caddy_data_path = launchd::get_caddy_data_dir();
    let path_str = caddy_data_path.to_string_lossy().to_string();

    let response =
        HelperClient::send_request(HelperRequest::FixCaddyPermissions { path: path_str })?;

    if response.success {
        Ok(())
    } else {
        Err(response.message)
    }
}

/// Trust Caddy's root CA in the system keychain
/// Uses osascript for admin privileges (shows password prompt)
#[tauri::command]
pub async fn trust_caddy_ca() -> Result<(), String> {
    let ca_path = get_caddy_ca_path();

    if !ca_path.exists() {
        return Err(format!(
            "Caddy root CA not found. It will be generated when you first access a domain via HTTPS. Path: {}",
            ca_path.display()
        ));
    }

    // Run in blocking task since osascript blocks for password prompt
    tokio::task::spawn_blocking(trust_caddy_ca_internal)
        .await
        .map_err(|e| format!("Task error: {}", e))?
}

/// Remove Caddy's root CA from the user's trust settings
#[tauri::command]
pub async fn untrust_caddy_ca() -> Result<(), String> {
    let ca_path = get_caddy_ca_path();

    if !ca_path.exists() {
        return Ok(()); // Nothing to untrust
    }

    // Remove from user trust settings (no admin needed)
    let output = std::process::Command::new("security")
        .args([
            "remove-trusted-cert",
            ca_path.to_str().ok_or("Invalid path")?,
        ])
        .output()
        .map_err(|e| format!("Failed to run security command: {}", e))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // Not an error if cert wasn't trusted
        if stderr.contains("not found") || stderr.is_empty() {
            Ok(())
        } else {
            Err(format!("Failed to untrust CA: {}", stderr))
        }
    }
}
