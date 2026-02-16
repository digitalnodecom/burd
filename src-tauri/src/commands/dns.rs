//! DNS and Network related commands
//!
//! Handles DNS server, resolver, and network status commands.

use crate::error::LockExt;
use crate::lock; // Shared macro from error.rs
use crate::resolver;
use serde::Serialize;
use tauri::State;

use super::AppState;

// ============================================================================
// Network Commands (DNS, Proxy, Resolver)
// ============================================================================

/// Network status information
#[derive(Debug, Serialize)]
pub struct NetworkStatus {
    pub dns_running: bool,
    pub dns_port: u16,
    pub proxy_running: bool,
    pub proxy_port: u16,
    pub resolver_installed: bool,
    pub active_routes: Vec<RouteInfo>,
    pub tld: String,
}

#[derive(Debug, Serialize)]
pub struct RouteInfo {
    pub domain: String,
    pub port: Option<u16>,   // None for static file routes
    pub route_type: String,  // "reverse_proxy" or "file_server"
    pub instance_id: String,
}

#[tauri::command]
pub async fn get_network_status(state: State<'_, AppState>) -> Result<NetworkStatus, String> {
    let (dns_running, dns_port) = {
        let dns = lock!(state.dns_server)?;
        (dns.is_running(), dns.port())
    };

    // Read TLD from config so it reflects recent changes (dns_server stores TLD from startup)
    let tld = {
        let config_store = lock!(state.config_store)?;
        config_store.load()?.tld.clone()
    };

    let (proxy_running, proxy_port, active_routes) = {
        let proxy = state.proxy_server.lock().await;
        let routes = proxy.list_routes();
        let route_infos: Vec<RouteInfo> = routes
            .into_iter()
            .map(|r| {
                let port = r.port();
                let route_type = match &r.route_type {
                    crate::proxy::ProxyRouteType::ReverseProxy { .. } => "reverse_proxy".to_string(),
                    crate::proxy::ProxyRouteType::FileServer { .. } => "file_server".to_string(),
                };
                RouteInfo {
                    domain: r.domain,
                    port,
                    route_type,
                    instance_id: r.instance_id,
                }
            })
            .collect();
        (proxy.is_running(), proxy.port(), route_infos)
    };

    let resolver_installed = resolver::is_installed(&tld);

    Ok(NetworkStatus {
        dns_running,
        dns_port,
        proxy_running,
        proxy_port,
        resolver_installed,
        active_routes,
        tld,
    })
}

/// Resolver status information
#[derive(Debug, Serialize)]
pub struct ResolverStatus {
    pub installed: bool,
    pub port: Option<u16>,
    pub tld: String,
}

#[tauri::command]
pub fn get_resolver_status(state: State<'_, AppState>) -> Result<ResolverStatus, String> {
    // Read TLD from config so it reflects recent changes
    let tld = {
        let config_store = lock!(state.config_store)?;
        config_store.load()?.tld.clone()
    };

    let installed = resolver::is_installed(&tld);
    let port = resolver::get_current_config(&tld).map(|c| c.port);

    Ok(ResolverStatus { installed, port, tld })
}

/// Install the macOS resolver file (requires admin privileges)
#[tauri::command]
pub fn install_resolver(state: State<'_, AppState>) -> Result<(), String> {
    // Read from config so it reflects recent changes
    let (dns_port, tld) = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;
        (config.dns_port, config.tld.clone())
    };

    resolver::install(&tld, dns_port)?;
    resolver::flush_dns_cache()?;

    Ok(())
}

/// Uninstall the macOS resolver file (requires admin privileges)
#[tauri::command]
pub fn uninstall_resolver(state: State<'_, AppState>) -> Result<(), String> {
    // Read TLD from config so it reflects recent changes
    let tld = {
        let config_store = lock!(state.config_store)?;
        config_store.load()?.tld.clone()
    };

    resolver::uninstall(&tld)?;
    resolver::flush_dns_cache()?;

    Ok(())
}

// ============================================================================
// DNS Server Commands
// ============================================================================

/// Start the DNS server
#[tauri::command]
pub fn start_dns_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut dns = lock!(state.dns_server)?;
    dns.start()
}

/// Stop the DNS server
#[tauri::command]
pub fn stop_dns_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut dns = lock!(state.dns_server)?;
    dns.stop();
    Ok(())
}

/// Restart the DNS server
#[tauri::command]
pub fn restart_dns_server(state: State<'_, AppState>) -> Result<(), String> {
    let mut dns = lock!(state.dns_server)?;
    dns.stop();
    dns.start()
}
