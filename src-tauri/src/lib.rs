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
mod pm2;
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

use commands::{
    check_instance_health, create_domain, create_instance, rename_instance, delete_binary_version, delete_domain,
    delete_instance, reorder_instances, disable_proxy, download_binary, get_all_binary_statuses,
    get_available_services, get_available_versions, get_binary_status, get_cli_status,
    get_helper_status, get_installed_versions, get_instance_config, get_instance_env, get_instance_info, get_instance_logs,
    get_network_status, get_nvm_status, get_proxy_status, get_proxy_config, get_resolver_status, get_settings,
    get_ca_trust_status, trust_caddy_ca, untrust_caddy_ca,
    install_cli, install_helper, install_node_version, install_resolver, is_nvm_installed,
    list_domains, list_installed_node_versions, list_instances, list_remote_node_versions,
    restart_proxy_for_certs, reinit_domain_ssl, restart_dns_server, restart_proxy_daemon, set_default_node_version,
    set_instance_domain, setup_proxy, start_dns_server, start_instance, start_proxy_daemon,
    stop_dns_server, stop_instance, restart_instance, uninstall_cli, uninstall_helper, open_keychain_access,
    uninstall_node_version, uninstall_resolver, update_domain, update_domain_ssl, get_domain_config,
    update_domain_config, update_instance_config, change_instance_version, update_tld, AppState,
    // PVM commands
    get_pvm_status, get_current_php, list_installed_php_versions, list_remote_php_versions,
    download_php_version, delete_php_version, set_default_php_version,
    get_php_shell_integration_status, configure_php_shell_integration, remove_php_shell_integration,
    fix_php_shell_integration,
    // Tunnel commands
    list_frp_servers, create_frp_server, update_frp_server, delete_frp_server,
    list_tunnels, create_tunnel, update_tunnel, delete_tunnel, start_tunnels, stop_tunnels,
    get_tunnel_status, get_frpc_logs, generate_server_token, check_frpc_installed,
    get_frpc_connection_status, get_frpc_config,
    // PM2 commands
    get_pm2_status, is_pm2_installed, install_pm2, pm2_list, pm2_start, pm2_stop,
    pm2_restart, pm2_delete, pm2_logs, pm2_save, pm2_stop_all, pm2_delete_all,
    // Node-RED commands
    init_nodered_instance, is_nodered_initialized,
    // Mail commands (Mailpit)
    get_mailpit_config, list_emails, get_email, delete_emails, delete_all_emails,
    mark_emails_read, get_unread_count,
    // Tinker commands (PHP Console)
    list_tinker_projects, execute_tinker, get_tinker_history,
    clear_tinker_history, delete_tinker_history_item, get_tinker_php_info,
    // Park commands
    is_park_enabled, list_parked_directories, park_directory, unpark_directory,
    refresh_parked_directory, refresh_all_parked_directories, get_parked_projects,
    update_parked_directory_ssl,
    // Stack commands
    list_stacks, get_stack, create_stack, update_stack, delete_stack,
    add_instances_to_stack, remove_instances_from_stack, move_instance_to_stack,
    export_stack, preview_stack_import, import_stack,
    // Log commands
    get_available_log_sources, get_recent_logs, stream_logs, clear_logs,
};
use tauri::Manager;
use config::ConfigStore;
use dns::DnsServer;
use mail_notifier::MailNotifierState;
use park_watcher::ParkWatcherState;
use process::ProcessManager;
use proxy::ProxyServer;
use binary::BinaryManager;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
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
            // Initialize park directory watchers
            if !parked_dirs_for_watcher.is_empty() {
                let watcher_state = app.state::<ParkWatcherState>();
                park_watcher::init_watchers(&watcher_state, parked_dirs_for_watcher, app.handle().clone());
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
                                if let Some(instance) = config.instances.iter().find(|i| &i.id == instance_id) {
                                    let _ = proxy.register_route(&full_domain, instance.port, &domain.id.to_string(), domain.ssl_enabled);
                                }
                            }
                            config::DomainTarget::Port(port) => {
                                let _ = proxy.register_route(&full_domain, *port, &domain.id.to_string(), domain.ssl_enabled);
                            }
                            config::DomainTarget::StaticFiles { path, browse } => {
                                let _ = proxy.register_static_route(&full_domain, path, *browse, &domain.id.to_string(), domain.ssl_enabled);
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
            // Domain commands
            list_domains,
            create_domain,
            update_domain,
            delete_domain,
            reinit_domain_ssl,
            update_domain_ssl,
            get_domain_config,
            update_domain_config,
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
            // PM2 commands
            get_pm2_status,
            is_pm2_installed,
            install_pm2,
            pm2_list,
            pm2_start,
            pm2_stop,
            pm2_restart,
            pm2_delete,
            pm2_logs,
            pm2_save,
            pm2_stop_all,
            pm2_delete_all,
            // Node-RED commands
            init_nodered_instance,
            is_nodered_initialized,
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
