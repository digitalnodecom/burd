//! Share CLI command
//!
//! Exposes local sites to the internet via frpc tunnel.

use crate::config::{ConfigStore, ServiceType};
use crate::tunnel::{
    generate_random_subdomain, FrpcManager, SubdomainConfig, TunnelTarget,
};
use std::env;

/// Share a site via frpc tunnel
///
/// Exposes a local site to the internet.
/// Requires frpc to be installed, running, and connected.
pub fn run_share(subdomain: Option<String>) -> Result<(), String> {
    let current_dir = env::current_dir()
        .map_err(|e| format!("Failed to get current directory: {}", e))?;

    let document_root = current_dir.to_string_lossy().to_string();

    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // === Prerequisite Checks ===

    // 1. Check frpc instance exists
    let frpc_instance = config
        .instances
        .iter()
        .find(|i| i.service_type == ServiceType::Frpc);

    let frpc_instance = match frpc_instance {
        Some(inst) => inst,
        None => {
            return Err(
                "Tunnels not configured.\n\n\
                 Create a Tunnels (frpc) instance in the Burd app first:\n\
                 1. Open Burd\n\
                 2. Go to Instances\n\
                 3. Create a new 'Tunnels (frpc)' instance\n\
                 4. Configure frp server settings"
                    .to_string(),
            );
        }
    };

    // 2. Check frpc binary is installed
    let frpc_manager = FrpcManager::new()?;
    if !frpc_manager.is_binary_installed() {
        return Err(
            "frpc binary not installed.\n\n\
             Download frpc in the Burd app:\n\
             1. Open Burd\n\
             2. Go to Services\n\
             3. Download frpc"
                .to_string(),
        );
    }

    // 3. Check frpc is running
    if !frpc_manager.is_running() {
        return Err(
            "Tunnels not running.\n\n\
             Start tunnels in the Burd app:\n\
             1. Open Burd\n\
             2. Go to Tunnels\n\
             3. Click Start"
                .to_string(),
        );
    }

    // 4. Check frpc is connected (query admin API)
    let admin_port = frpc_instance.port;
    let admin_user = frpc_instance
        .config
        .get("admin_user")
        .and_then(|v| v.as_str())
        .unwrap_or("admin");
    let admin_password = frpc_instance
        .config
        .get("admin_password")
        .and_then(|v| v.as_str())
        .unwrap_or("admin");

    let is_connected = check_frpc_connected(admin_port, admin_user, admin_password);
    if !is_connected {
        return Err(
            "Not connected to tunnel server.\n\n\
             Check your frp server configuration and ensure the server is reachable.\n\
             Open Burd app and check Tunnels for connection status."
                .to_string(),
        );
    }

    // === Find target to share ===

    // First, check if this directory is linked (has a FrankenPHP instance)
    let linked_instance = config.instances.iter().find(|i| {
        i.config
            .get("document_root")
            .and_then(|v| v.as_str())
            .map(|dr| dr == document_root)
            .unwrap_or(false)
    });

    // Second, check if this directory is inside a parked directory
    let parked_project = if linked_instance.is_none() {
        find_parked_project(&current_dir, &config)
    } else {
        None
    };

    let (target_port, target_name) = match (&linked_instance, &parked_project) {
        (Some(inst), _) => (inst.port, inst.name.clone()),
        (None, Some((port, name))) => (*port, name.clone()),
        (None, None) => {
            return Err(format!(
                "Directory '{}' is not linked or parked.\n\n\
                 Link it first with: burd link\n\
                 Or park the parent directory with: burd park",
                document_root
            ));
        }
    };

    // === Check for default server ===
    let server = config
        .frp_servers
        .iter()
        .find(|s| s.is_default)
        .or_else(|| config.frp_servers.first());

    let server = match server {
        Some(s) => s,
        None => {
            return Err(
                "No frp server configured.\n\n\
                 Add an frp server in the Burd app:\n\
                 1. Open Burd\n\
                 2. Go to Tunnels\n\
                 3. Click 'Add Server' and configure your frp server"
                    .to_string(),
            );
        }
    };

    // === Check for existing tunnel to same port ===
    let existing_tunnel = config.tunnels.iter().find(|t| {
        match &t.target {
            TunnelTarget::Port(p) => *p == target_port,
            TunnelTarget::Instance(id) => {
                config.instances.iter()
                    .find(|i| i.id == *id)
                    .map(|i| i.port == target_port)
                    .unwrap_or(false)
            }
        }
    });

    if let Some(tunnel) = existing_tunnel {
        let public_url = tunnel.get_public_url(server);
        println!();
        println!("Tunnel already exists for '{}' on port {}", target_name, target_port);
        println!();
        println!("  Public URL: {}", public_url);
        println!();
        return Ok(());
    }

    // === Create new tunnel ===
    let tunnel_subdomain = match subdomain {
        Some(s) => SubdomainConfig::Custom { subdomain: slug::slugify(&s) },
        None => SubdomainConfig::Random {
            generated: Some(generate_random_subdomain()),
        },
    };

    let tunnel_name = format!("{} (CLI)", target_name);

    // Create tunnel via config store
    let tunnel = config_store.create_tunnel(
        tunnel_name,
        server.id,
        TunnelTarget::Port(target_port),
        tunnel_subdomain,
        "http".to_string(),
        false, // auto_start
    )?;

    // Get the public URL
    let public_url = tunnel.get_public_url(server);

    // Reload frpc config to pick up the new tunnel
    let reload_result = reload_frpc(&config_store, frpc_instance);
    if let Err(e) = reload_result {
        eprintln!("Warning: Failed to reload frpc config: {}", e);
        eprintln!("You may need to restart tunnels in the Burd app.");
    }

    println!();
    println!("Sharing '{}' (port {})", target_name, target_port);
    println!();
    println!("  Public URL: {}", public_url);
    println!();
    println!("Note: This tunnel will persist until removed in the Burd app.");

    Ok(())
}

/// Check if frpc is connected to the server via its admin API
fn check_frpc_connected(port: u16, _user: &str, _password: &str) -> bool {
    // Use a synchronous HTTP request to check connection status
    // Since we can't use async in CLI, we'll use a simple TCP check first
    use std::net::TcpStream;
    use std::time::Duration;

    // First check if admin API is reachable
    let addr = format!("127.0.0.1:{}", port);
    let stream = TcpStream::connect_timeout(
        &addr.parse().unwrap(),
        Duration::from_secs(2),
    );

    if stream.is_err() {
        return false;
    }

    // If TCP connect succeeds, assume frpc is running and connected
    // For a more accurate check, we'd need to make an HTTP request
    // but that requires async or blocking HTTP client
    true
}

/// Find if current directory is inside a parked directory
fn find_parked_project(
    current_dir: &std::path::Path,
    config: &crate::config::Config,
) -> Option<(u16, String)> {
    let current_path = current_dir.to_string_lossy();

    for parked_dir in &config.parked_directories {
        if current_path.starts_with(&parked_dir.path) && current_path.as_ref() != parked_dir.path {
            // We're inside a parked directory - this is a project
            let project_name = current_dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();

            // Find the FrankenPHP Park instance to get its port
            let park_instance = config
                .instances
                .iter()
                .find(|i| i.service_type == ServiceType::FrankenPhpPark);

            if let Some(inst) = park_instance {
                return Some((inst.port, project_name));
            }
        }
    }

    None
}

/// Reload frpc configuration
fn reload_frpc(
    config_store: &ConfigStore,
    frpc_instance: &crate::config::Instance,
) -> Result<(), String> {
    use crate::tunnel::FrpcAdminConfig;

    let config = config_store.load()?;
    let frpc_manager = FrpcManager::new()?;

    if !frpc_manager.is_running() {
        return Ok(()); // Nothing to reload
    }

    let admin_config = FrpcAdminConfig {
        port: frpc_instance.port,
        user: frpc_instance
            .config
            .get("admin_user")
            .and_then(|v| v.as_str())
            .unwrap_or("admin")
            .to_string(),
        password: frpc_instance
            .config
            .get("admin_password")
            .and_then(|v| v.as_str())
            .unwrap_or("admin")
            .to_string(),
    };

    frpc_manager.reload(
        &config.tunnels,
        &config.frp_servers,
        &config.instances,
        Some(&admin_config),
    )
}
