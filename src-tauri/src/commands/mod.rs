// Sub-modules
mod dns;
mod domains;
mod instances;
mod logs;
mod mail;
mod node;
mod park;
mod php;
mod proxy;
mod services;
mod stacks;
mod system;
mod tinker;
mod tunnels;

// Re-export tunnel commands
pub use tunnels::{
    check_frpc_installed, create_frp_server, create_tunnel, delete_frp_server, delete_tunnel,
    generate_server_token, get_frpc_config, get_frpc_connection_status, get_frpc_logs,
    get_tunnel_status, list_frp_servers, list_tunnels, start_tunnels, stop_tunnels,
    update_frp_server, update_tunnel,
};

// Re-export node commands (NVM, PM2, Node-RED)
pub use node::{
    get_nvm_status, get_pm2_status, init_nodered_instance, install_node_version, install_pm2,
    is_nodered_initialized, is_nvm_installed, is_pm2_installed, list_installed_node_versions,
    list_remote_node_versions, pm2_delete, pm2_delete_all, pm2_list, pm2_logs, pm2_restart,
    pm2_save, pm2_start, pm2_stop, pm2_stop_all, set_default_node_version, uninstall_node_version,
};

// Re-export PHP/PVM commands
pub use php::{
    configure_php_shell_integration, delete_php_version, download_php_version,
    fix_php_shell_integration, get_current_php, get_php_shell_integration_status, get_pvm_status,
    list_installed_php_versions, list_remote_php_versions, remove_php_shell_integration,
    set_default_php_version,
};

// Re-export instance commands
pub use instances::{
    change_instance_version, check_instance_health, create_instance, delete_instance,
    generate_env_for_service, get_instance_config, get_instance_env, get_instance_info,
    get_instance_logs, list_instances, rename_instance, reorder_instances, restart_instance,
    start_instance, stop_instance, update_instance_config,
};

// Re-export domain commands
pub use domains::{
    create_domain, delete_domain, get_domain_config, list_domains, reinit_domain_ssl,
    set_instance_domain, update_domain, update_domain_config, update_domain_ssl,
};

// Re-export service commands
pub use services::{
    delete_binary_version, download_binary, get_all_binary_statuses, get_available_services,
    get_available_versions, get_binary_status, get_installed_versions, parse_service_type,
};

// Re-export DNS/network commands
pub use dns::{
    get_network_status, get_resolver_status, install_resolver, restart_dns_server,
    start_dns_server, stop_dns_server, uninstall_resolver,
};

// Re-export proxy commands
pub use proxy::{
    auto_trust_ca_if_needed, disable_proxy, get_ca_trust_status, get_proxy_config,
    get_proxy_status, restart_proxy_daemon, restart_proxy_for_certs, setup_proxy,
    start_proxy_daemon, trust_caddy_ca, untrust_caddy_ca,
};

// Re-export system commands (settings, CLI, helper)
pub use system::{
    get_cli_status, get_helper_status, get_settings, install_cli, install_helper,
    open_keychain_access, uninstall_cli, uninstall_helper, update_tld,
};

// Re-export mail commands (Mailpit)
pub use mail::{
    delete_all_emails, delete_emails, get_email, get_mailpit_config, get_unread_count, list_emails,
    mark_emails_read,
};

// Re-export tinker commands (PHP Console)
pub use tinker::{
    clear_tinker_history, delete_tinker_history_item, execute_tinker, get_tinker_history,
    get_tinker_php_info, list_tinker_projects,
};

// Re-export log commands
pub use logs::{clear_logs, get_available_log_sources, get_recent_logs, stream_logs};

// Re-export park commands
pub use park::{
    get_parked_projects, is_park_enabled, list_parked_directories, park_directory,
    refresh_all_parked_directories, refresh_parked_directory, unpark_directory,
    update_parked_directory_ssl,
};

// Re-export stack commands
pub use stacks::{
    add_instances_to_stack, create_stack, delete_stack, export_stack, get_stack, import_stack,
    list_stacks, move_instance_to_stack, preview_stack_import, remove_instances_from_stack,
    update_stack,
};

use crate::binary::BinaryManager;
use crate::config::ConfigStore;
use crate::dns::DnsServer;
use crate::process::ProcessManager;
use crate::proxy::ProxyServer;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

/// Application state shared between Tauri commands and the HTTP API.
///
/// All fields are wrapped in Arc to allow cloning and sharing with the API server.
#[derive(Clone)]
pub struct AppState {
    pub config_store: Arc<Mutex<ConfigStore>>,
    pub process_manager: Arc<Mutex<ProcessManager>>,
    pub binary_manager: Arc<Mutex<BinaryManager>>,
    pub dns_server: Arc<Mutex<DnsServer>>,
    pub proxy_server: Arc<AsyncMutex<ProxyServer>>,
}
