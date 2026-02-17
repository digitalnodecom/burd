//! Tunnel commands for FRP integration
//!
//! Handles FRP server management, tunnel CRUD, and frpc process control.

use crate::config::{
    FrpServer, ServiceType, SubdomainConfig, Tunnel, TunnelState, TunnelTarget, TunnelWithState,
};
use crate::error::LockExt;
use crate::lock; // Shared macro from error.rs
use crate::tunnel::{get_frpc_config_path, FrpcAdminConfig, FrpcManager};
use serde::Deserialize;
use tauri::State;
use uuid::Uuid;

use super::AppState;

// ==================== FRP Server Commands ====================

/// List all configured frp servers
#[tauri::command]
pub fn list_frp_servers(state: State<'_, AppState>) -> Result<Vec<FrpServer>, String> {
    let config_store = lock!(state.config_store)?;
    config_store.list_frp_servers()
}

/// Create a new frp server configuration
#[tauri::command]
pub fn create_frp_server(
    name: String,
    server_addr: String,
    server_port: u16,
    token: String,
    subdomain_host: String,
    state: State<'_, AppState>,
) -> Result<FrpServer, String> {
    let config_store = lock!(state.config_store)?;
    config_store.create_frp_server(name, server_addr, server_port, token, subdomain_host)
}

/// Update an existing frp server
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn update_frp_server(
    id: String,
    name: Option<String>,
    server_addr: Option<String>,
    server_port: Option<u16>,
    token: Option<String>,
    subdomain_host: Option<String>,
    is_default: Option<bool>,
    state: State<'_, AppState>,
) -> Result<FrpServer, String> {
    let server_id = Uuid::parse_str(&id).map_err(|_| "Invalid server ID")?;
    let config_store = lock!(state.config_store)?;
    config_store.update_frp_server(
        server_id,
        name,
        server_addr,
        server_port,
        token,
        subdomain_host,
        is_default,
    )
}

/// Delete an frp server
#[tauri::command]
pub fn delete_frp_server(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let server_id = Uuid::parse_str(&id).map_err(|_| "Invalid server ID")?;
    let config_store = lock!(state.config_store)?;
    config_store.delete_frp_server(server_id)
}

// ==================== Tunnel Types ====================

/// Tunnel creation request
#[derive(Debug, Deserialize)]
pub struct CreateTunnelRequest {
    pub name: String,
    pub server_id: String,
    pub target_type: String,
    pub target_value: String,
    pub subdomain_type: String,
    pub subdomain_value: Option<String>,
    pub protocol: Option<String>,
    pub auto_start: Option<bool>,
}

/// frpc connection status response
#[derive(Debug, Clone, serde::Serialize)]
pub struct FrpcConnectionStatus {
    pub running: bool,
    pub connected: bool,
    pub server_addr: Option<String>,
    pub error: Option<String>,
    pub proxies: Vec<FrpcProxyStatus>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct FrpcProxyStatus {
    pub name: String,
    pub status: String,
    pub local_addr: String,
    pub remote_addr: Option<String>,
    pub error: Option<String>,
}

// ==================== Tunnel Commands ====================

/// List all tunnels with their current state
#[tauri::command]
pub async fn list_tunnels(state: State<'_, AppState>) -> Result<Vec<TunnelWithState>, String> {
    let (tunnels, servers, instances) = {
        let config_store = lock!(state.config_store)?;
        let mut config = config_store.load()?;

        // Migrate: Generate random subdomains for any tunnels missing them
        let mut needs_save = false;
        for tunnel in &mut config.tunnels {
            if let SubdomainConfig::Random { generated: None } = &tunnel.subdomain {
                tunnel.subdomain = SubdomainConfig::Random {
                    generated: Some(crate::tunnel::generate_random_subdomain()),
                };
                needs_save = true;
            }
        }
        if needs_save {
            let _ = config_store.save(&config);
        }

        (
            config.tunnels.clone(),
            config.frp_servers.clone(),
            config.instances.clone(),
        )
    };

    let frpc_manager = FrpcManager::new()?;
    let running_tunnels = frpc_manager.get_running_tunnel_ids();

    let result: Vec<TunnelWithState> = tunnels
        .iter()
        .map(|t| {
            let server = servers.iter().find(|s| s.id == t.server_id);
            let is_running = running_tunnels.contains(&t.id);

            // Build the public URL
            let public_url = if is_running {
                server.map(|s| {
                    let subdomain = match &t.subdomain {
                        SubdomainConfig::Random { generated } => {
                            generated.clone().unwrap_or_default()
                        }
                        SubdomainConfig::Custom { subdomain } => subdomain.clone(),
                    };
                    format!("https://{}.{}", subdomain, s.subdomain_host)
                })
            } else {
                None
            };

            // Get target name if it's an instance
            let target_name = match &t.target {
                TunnelTarget::Instance(id) => instances
                    .iter()
                    .find(|i| i.id == *id)
                    .map(|i| i.name.clone()),
                TunnelTarget::Port(_) => None,
            };

            // Get target port
            let target_port = match &t.target {
                TunnelTarget::Instance(id) => {
                    instances.iter().find(|i| i.id == *id).map(|i| i.port)
                }
                TunnelTarget::Port(p) => Some(*p),
            };

            TunnelWithState {
                tunnel: t.clone(),
                state: TunnelState {
                    running: is_running,
                    public_url,
                    error: None,
                },
                server_name: server.map(|s| s.name.clone()),
                target_name,
                target_port,
            }
        })
        .collect();

    Ok(result)
}

/// Reload frpc config if it's running (after tunnel changes)
/// This sends SIGHUP to the frpc process to hot-reload the config
fn reload_frpc_if_running(state: &State<'_, AppState>) -> Result<(), String> {
    let frpc_manager = FrpcManager::new()?;
    if !frpc_manager.is_running() {
        return Ok(()); // Not running, nothing to reload
    }

    let (tunnels, servers, instances, admin_config) = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;

        // Look for an frpc instance to get admin config
        let admin_config = config
            .instances
            .iter()
            .find(|i| i.service_type == ServiceType::Frpc)
            .map(|instance| {
                let user = instance
                    .config
                    .get("admin_user")
                    .and_then(|v| v.as_str())
                    .unwrap_or("admin")
                    .to_string();
                let password = instance
                    .config
                    .get("admin_password")
                    .and_then(|v| v.as_str())
                    .unwrap_or("admin")
                    .to_string();
                FrpcAdminConfig {
                    port: instance.port,
                    user,
                    password,
                }
            });

        (
            config.tunnels.clone(),
            config.frp_servers.clone(),
            config.instances.clone(),
            admin_config,
        )
    };

    frpc_manager.reload(&tunnels, &servers, &instances, admin_config.as_ref())
}

/// Create a new tunnel
#[tauri::command]
pub fn create_tunnel(
    request: CreateTunnelRequest,
    state: State<'_, AppState>,
) -> Result<Tunnel, String> {
    let server_id = Uuid::parse_str(&request.server_id).map_err(|_| "Invalid server ID")?;

    // Parse target
    let target = match request.target_type.as_str() {
        "instance" => {
            let instance_id =
                Uuid::parse_str(&request.target_value).map_err(|_| "Invalid instance ID")?;
            TunnelTarget::Instance(instance_id)
        }
        "port" => {
            let port: u16 = request
                .target_value
                .parse()
                .map_err(|_| "Invalid port number")?;
            TunnelTarget::Port(port)
        }
        _ => return Err("Invalid target type. Use 'instance' or 'port'".to_string()),
    };

    // Parse subdomain config
    let subdomain = match request.subdomain_type.as_str() {
        "random" => SubdomainConfig::Random { generated: None },
        "custom" => {
            let subdomain_value = request
                .subdomain_value
                .ok_or("Custom subdomain requires a value")?;
            SubdomainConfig::Custom {
                subdomain: subdomain_value,
            }
        }
        _ => return Err("Invalid subdomain type. Use 'random' or 'custom'".to_string()),
    };

    let protocol = request.protocol.unwrap_or_else(|| "http".to_string());
    let auto_start = request.auto_start.unwrap_or(false);

    let tunnel = {
        let config_store = lock!(state.config_store)?;
        config_store.create_tunnel(
            request.name,
            server_id,
            target,
            subdomain,
            protocol,
            auto_start,
        )?
    };

    // Reload frpc if running to pick up the new tunnel
    let _ = reload_frpc_if_running(&state);

    Ok(tunnel)
}

/// Delete a tunnel
#[tauri::command]
pub fn delete_tunnel(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let tunnel_id = Uuid::parse_str(&id).map_err(|_| "Invalid tunnel ID")?;

    {
        let config_store = lock!(state.config_store)?;
        config_store.delete_tunnel(tunnel_id)?;
    }

    // Reload frpc if running to remove the deleted tunnel
    let _ = reload_frpc_if_running(&state);

    Ok(())
}

/// Update an existing tunnel
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub fn update_tunnel(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    server_id: Option<String>,
    target_type: Option<String>,
    target_value: Option<String>,
    subdomain_type: Option<String>,
    subdomain_value: Option<String>,
    protocol: Option<String>,
) -> Result<Tunnel, String> {
    let tunnel_id = Uuid::parse_str(&id).map_err(|_| "Invalid tunnel ID")?;

    // Parse optional server_id
    let server_id_parsed = match server_id {
        Some(s) => Some(Uuid::parse_str(&s).map_err(|_| "Invalid server ID")?),
        None => None,
    };

    // Parse optional target
    let target = match (target_type.as_deref(), target_value) {
        (Some("instance"), Some(val)) => {
            let instance_id = Uuid::parse_str(&val).map_err(|_| "Invalid instance ID")?;
            Some(TunnelTarget::Instance(instance_id))
        }
        (Some("port"), Some(val)) => {
            let port: u16 = val.parse().map_err(|_| "Invalid port number")?;
            Some(TunnelTarget::Port(port))
        }
        (None, None) => None,
        _ => {
            return Err(
                "Invalid target: both target_type and target_value must be provided together"
                    .to_string(),
            )
        }
    };

    // Parse optional subdomain config
    let subdomain = match (subdomain_type.as_deref(), subdomain_value) {
        (Some("random"), _) => Some(SubdomainConfig::Random { generated: None }),
        (Some("custom"), Some(val)) => Some(SubdomainConfig::Custom { subdomain: val }),
        (Some("custom"), None) => return Err("Custom subdomain requires a value".to_string()),
        (None, _) => None,
        _ => return Err("Invalid subdomain type".to_string()),
    };

    let tunnel = {
        let config_store = lock!(state.config_store)?;
        config_store.update_tunnel(
            tunnel_id,
            name,
            server_id_parsed,
            target,
            subdomain,
            protocol,
            None,
        )?
    };

    // Reload frpc if running to apply changes
    let _ = reload_frpc_if_running(&state);

    Ok(tunnel)
}

/// Start tunnels (launches frpc with all active tunnels)
#[tauri::command]
pub async fn start_tunnels(state: State<'_, AppState>) -> Result<(), String> {
    let (tunnels, servers, instances, admin_config) = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;

        // Look for an frpc instance to get admin config
        let admin_config = config
            .instances
            .iter()
            .find(|i| i.service_type == ServiceType::Frpc)
            .map(|instance| {
                let user = instance
                    .config
                    .get("admin_user")
                    .and_then(|v| v.as_str())
                    .unwrap_or("admin")
                    .to_string();
                let password = instance
                    .config
                    .get("admin_password")
                    .and_then(|v| v.as_str())
                    .unwrap_or("admin")
                    .to_string();
                FrpcAdminConfig {
                    port: instance.port,
                    user,
                    password,
                }
            });

        (
            config.tunnels.clone(),
            config.frp_servers.clone(),
            config.instances.clone(),
            admin_config,
        )
    };

    // Generate random subdomains for any tunnels that need them
    let mut updated_tunnels = tunnels.clone();
    let mut needs_save = false;

    for tunnel in &mut updated_tunnels {
        if let SubdomainConfig::Random { generated: None } = &tunnel.subdomain {
            let random_subdomain = crate::tunnel::generate_random_subdomain();
            tunnel.subdomain = SubdomainConfig::Random {
                generated: Some(random_subdomain),
            };
            needs_save = true;
        }
    }

    // Save updated tunnels with generated subdomains
    if needs_save {
        let config_store = lock!(state.config_store)?;
        for tunnel in &updated_tunnels {
            if let SubdomainConfig::Random { generated: Some(_) } = &tunnel.subdomain {
                config_store.update_tunnel(
                    tunnel.id,
                    None,
                    None,
                    None,
                    Some(tunnel.subdomain.clone()),
                    None,
                    None,
                )?;
            }
        }
    }

    let mut frpc_manager = FrpcManager::new()?;
    frpc_manager
        .start(
            &updated_tunnels,
            &servers,
            &instances,
            admin_config.as_ref(),
        )
        .await
}

/// Stop all tunnels
#[tauri::command]
pub async fn stop_tunnels() -> Result<(), String> {
    let mut frpc_manager = FrpcManager::new()?;
    frpc_manager.stop()
}

/// Get the current tunnel status
#[tauri::command]
pub fn get_tunnel_status() -> Result<TunnelState, String> {
    let frpc_manager = FrpcManager::new()?;
    Ok(frpc_manager.get_status())
}

/// Get frpc logs
#[tauri::command]
pub fn get_frpc_logs() -> Result<String, String> {
    FrpcManager::read_logs()
}

/// Generate a secure random token for frp server authentication
#[tauri::command]
pub fn generate_server_token() -> String {
    use rand::distr::Alphanumeric;
    use rand::Rng;

    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}

/// Check if frpc binary is installed
#[tauri::command]
pub fn check_frpc_installed() -> bool {
    match FrpcManager::new() {
        Ok(manager) => manager.is_binary_installed(),
        Err(_) => false,
    }
}

/// Get frpc connection status by querying its admin API
#[tauri::command]
pub async fn get_frpc_connection_status(
    state: State<'_, AppState>,
) -> Result<FrpcConnectionStatus, String> {
    // Find the frpc instance to get its port and credentials
    let (port, user, password) = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;

        let frpc_instance = config
            .instances
            .iter()
            .find(|i| i.service_type == ServiceType::Frpc);

        match frpc_instance {
            Some(inst) => {
                let user = inst
                    .config
                    .get("admin_user")
                    .and_then(|v| v.as_str())
                    .unwrap_or("admin")
                    .to_string();
                let password = inst
                    .config
                    .get("admin_password")
                    .and_then(|v| v.as_str())
                    .unwrap_or("admin")
                    .to_string();
                (inst.port, user, password)
            }
            None => {
                return Ok(FrpcConnectionStatus {
                    running: false,
                    connected: false,
                    server_addr: None,
                    error: Some("No frpc instance configured".to_string()),
                    proxies: vec![],
                });
            }
        }
    };

    // Check if frpc is running by trying to connect to its admin API
    let url = format!("http://127.0.0.1:{}/api/status", port);

    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .basic_auth(&user, Some(&password))
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await;

    match response {
        Ok(resp) => {
            if resp.status().is_success() {
                // Parse the response to get proxy statuses
                let body = resp.text().await.unwrap_or_default();
                parse_frpc_status_response(&body)
            } else {
                Ok(FrpcConnectionStatus {
                    running: true,
                    connected: false,
                    server_addr: None,
                    error: Some(format!("Admin API returned status: {}", resp.status())),
                    proxies: vec![],
                })
            }
        }
        Err(e) => {
            // Could not connect to admin API - frpc is not running
            Ok(FrpcConnectionStatus {
                running: false,
                connected: false,
                server_addr: None,
                error: Some(format!("frpc not running: {}", e)),
                proxies: vec![],
            })
        }
    }
}

fn parse_frpc_status_response(body: &str) -> Result<FrpcConnectionStatus, String> {
    // frpc /api/status returns JSON like:
    // {"tcp":[],"udp":[],"http":[{"name":"tunnel-xxx","status":"running","local_addr":"127.0.0.1:8080",...}],...}

    let json: serde_json::Value = serde_json::from_str(body).unwrap_or(serde_json::Value::Null);

    let mut proxies = Vec::new();
    let mut any_connected = false;

    // Check all proxy types
    for proxy_type in ["tcp", "udp", "http", "https", "stcp", "sudp", "xtcp"] {
        if let Some(arr) = json.get(proxy_type).and_then(|v| v.as_array()) {
            for proxy in arr {
                let name = proxy
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let status = proxy
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let local_addr = proxy
                    .get("local_addr")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let remote_addr = proxy
                    .get("remote_addr")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                let err = proxy
                    .get("err")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                if status == "running" {
                    any_connected = true;
                }

                proxies.push(FrpcProxyStatus {
                    name,
                    status,
                    local_addr,
                    remote_addr,
                    error: err,
                });
            }
        }
    }

    Ok(FrpcConnectionStatus {
        running: true,
        connected: any_connected,
        server_addr: None, // Could parse from config if needed
        error: None,
        proxies,
    })
}

/// Get the current frpc configuration file content
#[tauri::command]
pub fn get_frpc_config() -> Result<String, String> {
    let config_path = get_frpc_config_path()?;

    if !config_path.exists() {
        return Err(
            "frpc configuration file does not exist. Configure a server and tunnels first."
                .to_string(),
        );
    }

    std::fs::read_to_string(&config_path).map_err(|e| format!("Failed to read frpc config: {}", e))
}
