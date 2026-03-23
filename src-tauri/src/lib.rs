//! Burd - Local Development Environment Manager
//!
//! Burd is a macOS application for managing local development services including:
//! - FrankenPHP/PHP development servers
//! - Databases (MariaDB, PostgreSQL, MongoDB)
//! - Cache services (Redis, Valkey, Memcached)
//! - Mail testing (Mailpit)
//! - Full-text search (Meilisearch, Typesense)
//! - And more
//!
//! This crate provides the core functionality for both the GUI application
//! and the CLI tool.

pub mod analyzer;
pub mod api;
mod binary;
mod caddy;
pub mod cli;
mod commands;
pub mod config;
pub mod constants;
pub mod db_manager;
mod dns;
pub mod domain;
mod drivers;
pub mod error;
mod helper_client;
mod launchd;
pub mod lock_utils;
mod logs;
mod mail_notifier;
pub mod mcp;
mod nvm;
pub mod park;
mod park_watcher;
mod process;
mod proxy;
mod pvm;
mod resolver;
pub mod service_config;
mod services;
mod tinker;
mod tunnel;
pub mod validation;

// Test utilities module (only available in test builds)
#[cfg(test)]
pub mod test_utils;

use binary::BinaryManager;
use commands::{
    add_instances_to_stack,
    change_instance_version,
    check_frpc_installed,
    check_instance_health,
    check_port_status,
    check_proxy_health,
    clear_logs,
    clear_tinker_history,
    configure_php_shell_integration,
    create_domain,
    create_frp_server,
    create_instance,
    create_stack,
    create_tunnel,
    delete_all_emails,
    delete_binary_version,
    delete_domain,
    delete_emails,
    delete_frp_server,
    delete_instance,
    delete_php_version,
    delete_stack,
    delete_tinker_history_item,
    delete_tunnel,
    disable_proxy,
    download_binary,
    download_php_version,
    execute_tinker,
    export_stack,
    fix_php_shell_integration,
    generate_server_token,
    get_all_binary_statuses,
    // Log commands
    get_available_log_sources,
    get_available_services,
    get_available_versions,
    get_binary_status,
    get_ca_trust_status,
    get_cli_status,
    get_current_php,
    get_domain_config,
    get_email,
    get_frpc_config,
    get_frpc_connection_status,
    get_frpc_logs,
    get_helper_status,
    get_installed_versions,
    get_instance_config,
    get_instance_env,
    get_instance_info,
    get_instance_logs,
    // Mail commands (Mailpit)
    get_mailpit_config,
    get_network_status,
    get_nvm_status,
    get_parked_projects,
    get_php_shell_integration_status,
    get_proxy_config,
    get_proxy_status,
    // PVM commands
    get_pvm_status,
    get_recent_logs,
    get_resolver_status,
    get_settings,
    get_stack,
    get_tinker_history,
    get_tinker_php_info,
    get_tunnel_status,
    get_unread_count,
    import_stack,
    install_cli,
    install_helper,
    install_node_version,
    install_resolver,
    is_nvm_installed,
    // Park commands
    is_park_enabled,
    list_domains,
    list_emails,
    // Tunnel commands
    list_frp_servers,
    list_installed_node_versions,
    list_installed_php_versions,
    list_instances,
    list_parked_directories,
    list_remote_node_versions,
    list_remote_php_versions,
    // Stack commands
    list_stacks,
    // Tinker commands (PHP Console)
    list_tinker_projects,
    list_tunnels,
    mark_emails_read,
    move_instance_to_stack,
    open_keychain_access,
    park_directory,
    preview_stack_import,
    refresh_all_parked_directories,
    refresh_parked_directory,
    reinit_domain_ssl,
    remove_instances_from_stack,
    remove_php_shell_integration,
    rename_instance,
    reorder_domains,
    reorder_instances,
    restart_dns_server,
    restart_instance,
    restart_proxy_daemon,
    restart_proxy_for_certs,
    set_default_node_version,
    set_default_php_version,
    set_instance_domain,
    setup_proxy,
    start_dns_server,
    start_instance,
    start_proxy_daemon,
    start_tunnels,
    stop_dns_server,
    stop_instance,
    stop_tunnels,
    stream_logs,
    trust_caddy_ca,
    uninstall_cli,
    uninstall_helper,
    uninstall_node_version,
    uninstall_resolver,
    unpark_directory,
    untrust_caddy_ca,
    update_domain,
    update_domain_config,
    update_domain_ssl,
    update_frp_server,
    update_instance_config,
    update_parked_directory_ssl,
    update_stack,
    update_tld,
    update_tunnel,
    AppState,
};
use config::ConfigStore;
use dns::DnsServer;
use mail_notifier::MailNotifierState;
use park_watcher::ParkWatcherState;
use process::ProcessManager;
use proxy::ProxyServer;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{Emitter, Manager};
use tokio::sync::Mutex as AsyncMutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_store = ConfigStore::new().expect("Failed to initialize config store");

    // Domain migration disabled - domains are now created manually via UI
    // This prevents auto-recreation of domains on restart
    // let _ = config_store.migrate_instance_domains();

    let config = config_store.load().expect("Failed to load config");

    let process_manager = ProcessManager::new();
    let binary_manager = BinaryManager::new();

    // Initialize DNS server with TLD
    let mut dns_server = DnsServer::new(config.dns_port, config.tld.clone());
    let _ = dns_server.start();

    // Initialize proxy server
    let proxy_server = ProxyServer::new(config.proxy_port, config.tld.clone());

    let app_state = AppState {
        config_store: Arc::new(Mutex::new(config_store)),
        process_manager: Arc::new(Mutex::new(process_manager)),
        binary_manager: Arc::new(Mutex::new(binary_manager)),
        dns_server: Arc::new(Mutex::new(dns_server)),
        proxy_server: Arc::new(AsyncMutex::new(proxy_server)),
        proxy_healthy: Arc::new(std::sync::atomic::AtomicU8::new(0)),
    };

    // Check if privileged daemon is installed - if so, skip port 8080 proxy
    let daemon_installed = launchd::is_installed();

    // Collect parked directories for watcher initialization
    let parked_dirs_for_watcher: Vec<(uuid::Uuid, PathBuf)> = config
        .parked_directories
        .iter()
        .map(|pd| (pd.id, PathBuf::from(&pd.path)))
        .collect();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .manage(app_state)
        .manage(MailNotifierState::default())
        .manage(ParkWatcherState::new())
        .setup(move |app| {
            // Check if Laravel Herd is running (conflicts with DNS, proxy, PHP)
            check_herd_conflict(app.handle());

            // Initialize park directory watchers
            if !parked_dirs_for_watcher.is_empty() {
                let watcher_state = app.state::<ParkWatcherState>();
                park_watcher::init_watchers(
                    &watcher_state,
                    parked_dirs_for_watcher,
                    app.handle().clone(),
                );
            }
            // Only start the fallback port 8080 proxy if the daemon is NOT installed
            // When daemon is installed, it handles ports 80 and 443
            if !daemon_installed {
                let state = app.state::<AppState>();
                let proxy_server = state.proxy_server.clone();
                tauri::async_runtime::spawn(async move {
                    let mut proxy = proxy_server.lock().await;
                    let _ = proxy.start().await;
                });
            } else {
                // Daemon is installed - sync all domains to Caddyfile on startup
                let state = app.state::<AppState>();
                let proxy_server = state.proxy_server.clone();

                // Load config synchronously before spawning async task
                let config = match state.config_store.lock() {
                    Ok(store) => match store.load() {
                        Ok(c) => c,
                        Err(_) => return Ok(()),
                    },
                    Err(_) => return Ok(()),
                };

                tauri::async_runtime::spawn(async move {
                    // Register all domains with the proxy server
                    let proxy = proxy_server.lock().await;
                    let tld = config.tld.clone();

                    for domain in &config.domains {
                        let full_domain = domain.full_domain(&tld);
                        match &domain.target {
                            config::DomainTarget::Instance(instance_id) => {
                                // Find the instance to get its port
                                if let Some(instance) =
                                    config.instances.iter().find(|i| &i.id == instance_id)
                                {
                                    let _ = proxy.register_route(
                                        &full_domain,
                                        instance.port,
                                        &domain.id.to_string(),
                                        domain.ssl_enabled,
                                    );
                                }
                            }
                            config::DomainTarget::Port(port) => {
                                let _ = proxy.register_route(
                                    &full_domain,
                                    *port,
                                    &domain.id.to_string(),
                                    domain.ssl_enabled,
                                );
                            }
                            config::DomainTarget::StaticFiles { path, browse } => {
                                let _ = proxy.register_static_route(
                                    &full_domain,
                                    path,
                                    *browse,
                                    &domain.id.to_string(),
                                    domain.ssl_enabled,
                                );
                            }
                        }
                    }
                });
            }

            // Start mail notifier for Mailpit WebSocket events
            mail_notifier::start_mail_notifier(app.handle().clone());

            // Start MCP API server for external control
            let api_state = app.state::<AppState>().inner().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = api::start_server(std::sync::Arc::new(api_state)).await {
                    eprintln!("Failed to start MCP API server: {}", e);
                }
            });

            // Start background proxy health poller
            {
                let proxy_healthy = app.state::<AppState>().proxy_healthy.clone();
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use std::sync::atomic::Ordering;
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(10)).await;

                        let health = tokio::task::spawn_blocking({
                            let _ph = proxy_healthy.clone();
                            commands::check_health_sync
                        })
                        .await
                        .ok()
                        .flatten();

                        let new_val: u8 = match health {
                            Some(true) => 1,
                            Some(false) => 2,
                            None => 0,
                        };
                        let old_val = proxy_healthy.swap(new_val, Ordering::Relaxed);

                        // Emit event when health status changes
                        if old_val != new_val {
                            let _ = app_handle.emit("proxy-health-changed", health);
                        }
                    }
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_instances,
            create_instance,
            rename_instance,
            start_instance,
            stop_instance,
            restart_instance,
            delete_instance,
            reorder_instances,
            get_binary_status,
            get_all_binary_statuses,
            get_available_versions,
            get_installed_versions,
            get_available_services,
            download_binary,
            delete_binary_version,
            check_instance_health,
            check_port_status,
            get_instance_logs,
            get_network_status,
            set_instance_domain,
            install_resolver,
            uninstall_resolver,
            get_resolver_status,
            // Instance config commands
            get_instance_config,
            update_instance_config,
            change_instance_version,
            get_instance_env,
            get_instance_info,
            // DNS Server commands
            start_dns_server,
            stop_dns_server,
            restart_dns_server,
            get_settings,
            update_tld,
            // Proxy commands (Caddy-based)
            get_proxy_status,
            setup_proxy,
            disable_proxy,
            start_proxy_daemon,
            restart_proxy_daemon,
            restart_proxy_for_certs,
            // CA trust commands
            get_ca_trust_status,
            trust_caddy_ca,
            untrust_caddy_ca,
            // Proxy health check
            check_proxy_health,
            // Domain commands
            list_domains,
            create_domain,
            update_domain,
            delete_domain,
            reinit_domain_ssl,
            update_domain_ssl,
            get_domain_config,
            update_domain_config,
            reorder_domains,
            get_proxy_config,
            // NVM commands
            get_nvm_status,
            is_nvm_installed,
            list_installed_node_versions,
            list_remote_node_versions,
            install_node_version,
            uninstall_node_version,
            set_default_node_version,
            // CLI commands
            get_cli_status,
            install_cli,
            uninstall_cli,
            // Helper commands
            get_helper_status,
            install_helper,
            uninstall_helper,
            // Utility commands
            open_keychain_access,
            // PVM (PHP Version Manager) commands
            get_pvm_status,
            get_current_php,
            list_installed_php_versions,
            list_remote_php_versions,
            download_php_version,
            delete_php_version,
            set_default_php_version,
            get_php_shell_integration_status,
            configure_php_shell_integration,
            remove_php_shell_integration,
            fix_php_shell_integration,
            // Tunnel commands
            list_frp_servers,
            create_frp_server,
            update_frp_server,
            delete_frp_server,
            list_tunnels,
            create_tunnel,
            update_tunnel,
            delete_tunnel,
            start_tunnels,
            stop_tunnels,
            get_tunnel_status,
            get_frpc_logs,
            generate_server_token,
            check_frpc_installed,
            get_frpc_connection_status,
            get_frpc_config,
            // Mail commands (Mailpit)
            get_mailpit_config,
            list_emails,
            get_email,
            delete_emails,
            delete_all_emails,
            mark_emails_read,
            get_unread_count,
            // Tinker commands (PHP Console)
            list_tinker_projects,
            execute_tinker,
            get_tinker_history,
            clear_tinker_history,
            delete_tinker_history_item,
            get_tinker_php_info,
            // Park commands
            is_park_enabled,
            list_parked_directories,
            park_directory,
            unpark_directory,
            refresh_parked_directory,
            refresh_all_parked_directories,
            get_parked_projects,
            update_parked_directory_ssl,
            // Stack commands
            list_stacks,
            get_stack,
            create_stack,
            update_stack,
            delete_stack,
            add_instances_to_stack,
            remove_instances_from_stack,
            move_instance_to_stack,
            export_stack,
            preview_stack_import,
            import_stack,
            // Log commands
            get_available_log_sources,
            get_recent_logs,
            stream_logs,
            clear_logs,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Check if Laravel Herd is running and emit a warning event to the frontend
fn check_herd_conflict(handle: &tauri::AppHandle) {
    use std::process::Command;

    // Check for Herd app process or its privileged helper
    let herd_app_running = Command::new("pgrep")
        .args(["-x", "Herd"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let herd_helper_running = Command::new("pgrep")
        .args(["-f", "de.beyondco.herd.helper"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if herd_app_running || herd_helper_running {
        let _ = handle.emit(
            "herd-conflict",
            "Laravel Herd is running and may conflict with Burd's DNS, proxy, and PHP services. Please quit Herd before using Burd.",
        );
    }
}
