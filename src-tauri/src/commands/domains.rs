//! Domain related commands
//!
//! Handles domain routing configuration for instances, ports, and static files.

use crate::caddy;
use crate::commands::auto_trust_ca_if_needed;
use crate::config::{DomainSource, DomainTarget};
use crate::error::LockExt;
use crate::launchd;
use crate::lock; // Shared macro from error.rs
use crate::park;
use crate::validation;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::State;
use uuid::Uuid;

use super::AppState;

// ============================================================================
// Types
// ============================================================================

/// Domain info for frontend (with resolved target details)
#[derive(Debug, Serialize)]
pub struct DomainInfo {
    pub id: String,
    pub subdomain: String,
    pub full_domain: String,
    pub target_type: String,           // "instance", "port", or "static"
    pub target_value: String,          // instance ID, port number, or path
    pub target_name: Option<String>,   // instance name if targeting instance
    pub target_port: Option<u16>,      // resolved port (None for static files)
    pub static_path: Option<String>,   // path for static file server
    pub static_browse: Option<bool>,   // directory listing enabled
    pub ssl_enabled: bool,             // whether SSL/HTTPS is enabled
    pub created_at: String,
    pub source: String,                // "manual", "parked", or "isolated"
    pub project_type: Option<String>,  // For parked: "Laravel", "WordPress", etc.
}

/// Create domain target - instance, port, or static files
#[derive(Debug, Deserialize)]
#[serde(tag = "target_type")]
pub enum CreateDomainTarget {
    #[serde(rename = "instance")]
    Instance { target_value: String }, // UUID string
    #[serde(rename = "port")]
    Port { target_value: u16 }, // Native port number
    #[serde(rename = "static")]
    StaticFiles { path: String, browse: bool }, // Static file server
}

/// Create domain request payload
#[derive(Debug, Deserialize)]
pub struct CreateDomainRequest {
    pub subdomain: String,
    #[serde(flatten)]
    pub target: CreateDomainTarget,
    #[serde(default)]
    pub ssl_enabled: bool,
}

/// Update domain request payload
#[derive(Debug, Deserialize)]
pub struct UpdateDomainRequest {
    pub subdomain: Option<String>,
    pub target_type: Option<String>,
    pub target_value: Option<String>,
    pub static_path: Option<String>,   // For static file server
    pub static_browse: Option<bool>,   // Directory listing for static
}

// ============================================================================
// Legacy Instance Domain Command
// ============================================================================

/// Set instance domain settings
#[tauri::command]
pub async fn set_instance_domain(
    id: String,
    domain: Option<String>,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    // Get old instance info
    let old_instance = {
        let config_store = lock!(state.config_store)?;
        config_store.get_instance(uuid)?
    };

    // Check if process is running
    let is_running = {
        let process_manager = lock!(state.process_manager)?;
        process_manager.is_running(&uuid)
    };

    // Unregister old route if running
    if is_running && old_instance.domain_enabled {
        let proxy = state.proxy_server.lock().await;
        let tld = proxy.tld();
        let _ = proxy.unregister_route(&old_instance.full_domain(tld));
    }

    // Update config
    let new_instance = {
        let config_store = lock!(state.config_store)?;
        config_store.update_instance_domain(uuid, domain, enabled)?
    };

    // Register new route if running and enabled
    // Note: Instance domain routing uses SSL enabled by default
    if is_running && new_instance.domain_enabled {
        let proxy = state.proxy_server.lock().await;
        let tld = proxy.tld();
        proxy.register_route(
            &new_instance.full_domain(tld),
            new_instance.port,
            &new_instance.id.to_string(),
            true, // SSL enabled by default for legacy instance domain routing
        )?;
        let _ = auto_trust_ca_if_needed();
    }

    Ok(())
}

// ============================================================================
// Domain CRUD Commands
// ============================================================================

/// List all domains with resolved target info
#[tauri::command]
pub fn list_domains(state: State<'_, AppState>) -> Result<Vec<DomainInfo>, String> {
    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;
    let tld = config.tld.clone();

    let domains: Vec<DomainInfo> = config.domains.iter().map(|d| {
        let (target_type, target_value, target_name, target_port, static_path, static_browse) = match &d.target {
            DomainTarget::Instance(id) => {
                let instance = config.instances.iter().find(|i| i.id == *id);
                (
                    "instance".to_string(),
                    id.to_string(),
                    instance.map(|i| i.name.clone()),
                    instance.map(|i| i.port),
                    None,
                    None,
                )
            }
            DomainTarget::Port(port) => (
                "port".to_string(),
                port.to_string(),
                None,
                Some(*port),
                None,
                None,
            ),
            DomainTarget::StaticFiles { path, browse } => (
                "static".to_string(),
                path.clone(),
                None,
                None,
                Some(path.clone()),
                Some(*browse),
            ),
        };

        // Determine source and project type
        let (source, project_type) = match &d.source {
            DomainSource::Manual => ("manual".to_string(), None),
            DomainSource::Parked { parked_dir_id } => {
                // Find the parked directory and scan for project type
                let ptype = config
                    .parked_directories
                    .iter()
                    .find(|pd| pd.id == *parked_dir_id)
                    .and_then(|pd| {
                        // Scan directory to find the project matching this subdomain
                        park::scan_directory(Path::new(&pd.path))
                            .ok()
                            .and_then(|projects| {
                                projects.into_iter().find(|p| {
                                    park::generate_subdomain(&p.name) == d.subdomain
                                })
                            })
                            .map(|p| p.project_type.as_str().to_string())
                    });
                ("parked".to_string(), ptype)
            }
            DomainSource::Isolated { .. } => ("isolated".to_string(), None),
        };

        DomainInfo {
            id: d.id.to_string(),
            subdomain: d.subdomain.clone(),
            full_domain: d.full_domain(&tld),
            target_type,
            target_value,
            target_name,
            target_port,
            static_path,
            static_browse,
            ssl_enabled: d.ssl_enabled,
            created_at: d.created_at.to_rfc3339(),
            source,
            project_type,
        }
    }).collect();

    Ok(domains)
}

/// Create a new domain
#[tauri::command]
pub async fn create_domain(
    request: CreateDomainRequest,
    state: State<'_, AppState>,
) -> Result<DomainInfo, String> {
    // Validate inputs before processing
    validation::validate_domain_name(&request.subdomain)
        .map_err(|e| format!("Invalid subdomain: {}", e))?;

    // Validate static file path if applicable
    if let CreateDomainTarget::StaticFiles { path, .. } = &request.target {
        validation::validate_directory_path(path)
            .map_err(|e| format!("Invalid static file path: {}", e))?;
    }

    // Validate port if applicable
    if let CreateDomainTarget::Port { target_value: port } = &request.target {
        validation::validate_port_allow_privileged(*port)
            .map_err(|e| format!("Invalid port: {}", e))?;
    }

    // Create domain and get TLD + instances in one lock acquisition
    let (domain, tld, instances) = {
        let config_store = lock!(state.config_store)?;

        let domain = match &request.target {
            CreateDomainTarget::Instance { target_value: id } => {
                let instance_id = Uuid::parse_str(id)
                    .map_err(|_| "Invalid instance ID")?;
                config_store.create_domain_for_instance(
                    request.subdomain.clone(),
                    instance_id,
                    request.ssl_enabled,
                )?
            }
            CreateDomainTarget::Port { target_value: port } => {
                config_store.create_domain_for_port(
                    request.subdomain.clone(),
                    *port,
                    request.ssl_enabled,
                )?
            }
            CreateDomainTarget::StaticFiles { path, browse } => {
                // Path already validated above
                config_store.create_domain_for_static_files(
                    request.subdomain.clone(),
                    path.clone(),
                    *browse,
                    request.ssl_enabled,
                )?
            }
        };

        let config = config_store.load()?;
        (domain, config.tld.clone(), config.instances.clone())
    };

    // Get target port using cached instances
    let target_port = domain.get_target_port(&instances);

    // Register proxy route based on target type
    match &domain.target {
        DomainTarget::Instance(_) | DomainTarget::Port(_) => {
            if let Some(port) = target_port {
                let proxy = state.proxy_server.lock().await;
                proxy.register_route(&domain.full_domain(&tld), port, &domain.id.to_string(), domain.ssl_enabled)?;
            }
        }
        DomainTarget::StaticFiles { path, browse } => {
            let proxy = state.proxy_server.lock().await;
            proxy.register_static_route(&domain.full_domain(&tld), path, *browse, &domain.id.to_string(), domain.ssl_enabled)?;
        }
    }

    // Build response using cached instances
    let (target_type, target_value, target_name, resolved_port, static_path, static_browse) = match &domain.target {
        DomainTarget::Instance(id) => {
            let instance = instances.iter().find(|i| i.id == *id);
            (
                "instance".to_string(),
                id.to_string(),
                instance.map(|i| i.name.clone()),
                instance.map(|i| i.port),
                None,
                None,
            )
        }
        DomainTarget::Port(p) => (
            "port".to_string(),
            p.to_string(),
            None,
            Some(*p),
            None,
            None,
        ),
        DomainTarget::StaticFiles { path, browse } => (
            "static".to_string(),
            path.clone(),
            None,
            None,
            Some(path.clone()),
            Some(*browse),
        ),
    };

    Ok(DomainInfo {
        id: domain.id.to_string(),
        subdomain: domain.subdomain.clone(),
        full_domain: domain.full_domain(&tld),
        target_type,
        target_value,
        target_name,
        target_port: resolved_port,
        static_path,
        static_browse,
        ssl_enabled: domain.ssl_enabled,
        created_at: domain.created_at.to_rfc3339(),
        source: "manual".to_string(),  // Newly created domains are always manual
        project_type: None,
    })
}

/// Update an existing domain
#[tauri::command]
pub async fn update_domain(
    id: String,
    request: UpdateDomainRequest,
    state: State<'_, AppState>,
) -> Result<DomainInfo, String> {
    let domain_id = Uuid::parse_str(&id).map_err(|_| "Invalid domain ID")?;

    // Validate subdomain if provided
    if let Some(ref subdomain) = request.subdomain {
        validation::validate_domain_name(subdomain)
            .map_err(|e| format!("Invalid subdomain: {}", e))?;
    }

    // Validate static file path if provided
    if let Some(ref path) = request.static_path {
        validation::validate_directory_path(path)
            .map_err(|e| format!("Invalid static file path: {}", e))?;
    }

    // Get old domain info for unregistering route
    let old_domain = {
        let config_store = lock!(state.config_store)?;
        config_store.get_domain(domain_id)?
    };

    // Unregister old route
    {
        let proxy = state.proxy_server.lock().await;
        let tld = proxy.tld();
        let _ = proxy.unregister_route(&old_domain.full_domain(tld));
    }

    // Build new target if provided
    let new_target = if let Some(target_type) = &request.target_type {
        Some(match target_type.as_str() {
            "instance" => {
                let target_value = request.target_value.as_ref()
                    .ok_or("target_value required for instance target")?;
                let instance_id = Uuid::parse_str(target_value)
                    .map_err(|_| "Invalid instance ID")?;
                DomainTarget::Instance(instance_id)
            }
            "port" => {
                let target_value = request.target_value.as_ref()
                    .ok_or("target_value required for port target")?;
                let port: u16 = target_value.parse()
                    .map_err(|_| "Invalid port number")?;
                // Validate port
                validation::validate_port_allow_privileged(port)
                    .map_err(|e| format!("Invalid port: {}", e))?;
                DomainTarget::Port(port)
            }
            "static" => {
                let path = request.static_path.clone()
                    .ok_or("static_path required for static target")?;
                // Path already validated above
                let browse = request.static_browse.unwrap_or(false);
                DomainTarget::StaticFiles { path, browse }
            }
            _ => return Err("Invalid target type. Use 'instance', 'port', or 'static'".to_string()),
        })
    } else {
        None
    };

    // Update domain and get TLD + instances in one lock acquisition
    let (domain, tld, instances) = {
        let config_store = lock!(state.config_store)?;
        let domain = config_store.update_domain(
            domain_id,
            request.subdomain,
            new_target,
        )?;
        let config = config_store.load()?;
        (domain, config.tld.clone(), config.instances.clone())
    };

    // Get target port using cached instances
    let target_port = domain.get_target_port(&instances);

    // Register new route based on target type
    match &domain.target {
        DomainTarget::Instance(_) | DomainTarget::Port(_) => {
            if let Some(port) = target_port {
                let proxy = state.proxy_server.lock().await;
                proxy.register_route(&domain.full_domain(&tld), port, &domain.id.to_string(), domain.ssl_enabled)?;
            }
        }
        DomainTarget::StaticFiles { path, browse } => {
            let proxy = state.proxy_server.lock().await;
            proxy.register_static_route(&domain.full_domain(&tld), path, *browse, &domain.id.to_string(), domain.ssl_enabled)?;
        }
    }

    // Build response using cached instances
    let (target_type, target_value, target_name, resolved_port, static_path, static_browse) = match &domain.target {
        DomainTarget::Instance(inst_id) => {
            let instance = instances.iter().find(|i| i.id == *inst_id);
            (
                "instance".to_string(),
                inst_id.to_string(),
                instance.map(|i| i.name.clone()),
                instance.map(|i| i.port),
                None,
                None,
            )
        }
        DomainTarget::Port(p) => (
            "port".to_string(),
            p.to_string(),
            None,
            Some(*p),
            None,
            None,
        ),
        DomainTarget::StaticFiles { path, browse } => (
            "static".to_string(),
            path.clone(),
            None,
            None,
            Some(path.clone()),
            Some(*browse),
        ),
    };

    let full_domain = domain.full_domain(&tld);

    Ok(DomainInfo {
        id: domain.id.to_string(),
        subdomain: domain.subdomain,
        full_domain,
        target_type,
        target_value,
        target_name,
        target_port: resolved_port,
        static_path,
        static_browse,
        ssl_enabled: domain.ssl_enabled,
        created_at: domain.created_at.to_rfc3339(),
        source: "manual".to_string(),  // Updated domains are treated as manual
        project_type: None,
    })
}

/// Delete a domain
#[tauri::command]
pub async fn delete_domain(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let domain_id = Uuid::parse_str(&id).map_err(|_| "Invalid domain ID")?;

    // Get domain info for unregistering route
    let domain = {
        let config_store = lock!(state.config_store)?;
        config_store.get_domain(domain_id)?
    };

    // Unregister route
    {
        let proxy = state.proxy_server.lock().await;
        let tld = proxy.tld();
        let _ = proxy.unregister_route(&domain.full_domain(tld));
    }

    // Delete domain
    let config_store = lock!(state.config_store)?;
    config_store.delete_domain(domain_id)
}

/// Reinitialize SSL certificate for a specific domain
/// With Caddy, certificates are managed automatically - this just restarts the daemon
#[tauri::command]
pub async fn reinit_domain_ssl(id: String, _state: State<'_, AppState>) -> Result<(), String> {
    // Validate domain ID format
    let _domain_id = Uuid::parse_str(&id).map_err(|_| "Invalid domain ID")?;

    // With Caddy, certificates are auto-managed
    // Just restart the daemon to force a config reload
    if launchd::is_installed() {
        tokio::task::spawn_blocking(|| {
            launchd::restart()
        })
        .await
        .map_err(|e| format!("Task error: {}", e))??;
    }

    Ok(())
}

/// Update SSL enabled status for a domain
#[tauri::command]
pub async fn update_domain_ssl(
    id: String,
    ssl_enabled: bool,
    state: State<'_, AppState>,
) -> Result<DomainInfo, String> {
    let domain_id = Uuid::parse_str(&id).map_err(|_| "Invalid domain ID")?;

    // Update domain SSL setting
    let domain = {
        let config_store = lock!(state.config_store)?;
        config_store.update_domain_ssl(domain_id, ssl_enabled)?
    };

    // Get TLD for route registration
    let tld = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;
        config.tld.clone()
    };

    // Re-register route with new SSL setting
    match &domain.target {
        DomainTarget::Instance(_) | DomainTarget::Port(_) => {
            let target_port = {
                let config_store = lock!(state.config_store)?;
                let config = config_store.load()?;
                domain.get_target_port(&config.instances)
            };
            if let Some(port) = target_port {
                let proxy = state.proxy_server.lock().await;
                proxy.register_route(&domain.full_domain(&tld), port, &domain.id.to_string(), domain.ssl_enabled)?;
            }
        }
        DomainTarget::StaticFiles { path, browse } => {
            let proxy = state.proxy_server.lock().await;
            proxy.register_static_route(&domain.full_domain(&tld), path, *browse, &domain.id.to_string(), domain.ssl_enabled)?;
        }
    }

    // Auto-trust CA if SSL was just enabled
    if ssl_enabled {
        let _ = auto_trust_ca_if_needed();
    }

    // Build response
    let (target_type, target_value, target_name, resolved_port, static_path, static_browse) = match &domain.target {
        DomainTarget::Instance(inst_id) => {
            let config_store = lock!(state.config_store)?;
            let config = config_store.load()?;
            let instance = config.instances.iter().find(|i| i.id == *inst_id);
            (
                "instance".to_string(),
                inst_id.to_string(),
                instance.map(|i| i.name.clone()),
                instance.map(|i| i.port),
                None,
                None,
            )
        }
        DomainTarget::Port(p) => (
            "port".to_string(),
            p.to_string(),
            None,
            Some(*p),
            None,
            None,
        ),
        DomainTarget::StaticFiles { path, browse } => (
            "static".to_string(),
            path.clone(),
            None,
            None,
            Some(path.clone()),
            Some(*browse),
        ),
    };

    let full_domain = domain.full_domain(&tld);

    Ok(DomainInfo {
        id: domain.id.to_string(),
        subdomain: domain.subdomain,
        full_domain,
        target_type,
        target_value,
        target_name,
        target_port: resolved_port,
        static_path,
        static_browse,
        ssl_enabled: domain.ssl_enabled,
        created_at: domain.created_at.to_rfc3339(),
        source: "manual".to_string(),  // SSL toggle is for manual domains
        project_type: None,
    })
}

/// Update the Caddy configuration for a specific domain with custom content
#[tauri::command]
pub fn update_domain_config(id: String, config: String, state: State<'_, AppState>) -> Result<(), String> {
    let domain_id = Uuid::parse_str(&id).map_err(|_| "Invalid domain ID")?;

    let config_store = lock!(state.config_store)?;
    let domain = config_store.get_domain(domain_id)?;
    let app_config = config_store.load()?;
    let full_domain = domain.full_domain(&app_config.tld);

    // Write the custom config to the domain's .caddy file
    let filepath = caddy::get_domain_filepath(&full_domain);
    caddy::write_domain_config_raw(&filepath, &config)?;

    Ok(())
}

/// Get the Caddy configuration for a specific domain
#[tauri::command]
pub fn get_domain_config(id: String, state: State<'_, AppState>) -> Result<String, String> {
    let domain_id = Uuid::parse_str(&id).map_err(|_| "Invalid domain ID")?;

    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;
    let domain = config_store.get_domain(domain_id)?;
    let full_domain = domain.full_domain(&config.tld);

    // Build the RouteEntry for this domain
    let route = match &domain.target {
        DomainTarget::Instance(instance_id) => {
            let instance = config.instances.iter()
                .find(|i| i.id == *instance_id)
                .ok_or_else(|| "Instance not found for this domain".to_string())?;
            caddy::RouteEntry::reverse_proxy(
                full_domain,
                instance.port,
                domain.id.to_string(),
                domain.ssl_enabled,
            )
        }
        DomainTarget::Port(port) => {
            caddy::RouteEntry::reverse_proxy(
                full_domain,
                *port,
                domain.id.to_string(),
                domain.ssl_enabled,
            )
        }
        DomainTarget::StaticFiles { path, browse } => {
            caddy::RouteEntry::file_server(
                full_domain,
                path.clone(),
                *browse,
                domain.id.to_string(),
                domain.ssl_enabled,
            )
        }
    };

    // Generate and return the Caddy config
    Ok(caddy::generate_domain_config(&route))
}
