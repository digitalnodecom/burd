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
    list_frp_servers, create_frp_server, update_frp_server, delete_frp_server,
    list_tunnels, create_tunnel, update_tunnel, delete_tunnel,
    start_tunnels, stop_tunnels, get_tunnel_status, get_frpc_logs,
    generate_server_token, check_frpc_installed, get_frpc_connection_status, get_frpc_config,
};

// Re-export node commands (NVM, PM2, Node-RED)
pub use node::{
    get_nvm_status, is_nvm_installed, list_installed_node_versions, list_remote_node_versions,
    install_node_version, uninstall_node_version, set_default_node_version,
    get_pm2_status, is_pm2_installed, install_pm2, pm2_list, pm2_start, pm2_stop,
    pm2_restart, pm2_delete, pm2_logs, pm2_save, pm2_stop_all, pm2_delete_all,
    init_nodered_instance, is_nodered_initialized,
};

// Re-export PHP/PVM commands
pub use php::{
    get_pvm_status, get_current_php, list_installed_php_versions, list_remote_php_versions,
    download_php_version, delete_php_version, set_default_php_version,
    get_php_shell_integration_status, configure_php_shell_integration, remove_php_shell_integration,
    fix_php_shell_integration,
};

// Re-export instance commands
pub use instances::{
    list_instances, create_instance, rename_instance, start_instance, stop_instance, restart_instance, delete_instance,
    check_instance_health, get_instance_logs, get_instance_config, update_instance_config, change_instance_version,
    get_instance_env, get_instance_info, generate_env_for_service, reorder_instances,
};

// Re-export domain commands
pub use domains::{
    list_domains, create_domain, update_domain, delete_domain, set_instance_domain, reinit_domain_ssl,
    update_domain_ssl, get_domain_config, update_domain_config,
};

// Re-export service commands
pub use services::{
    get_binary_status, get_all_binary_statuses, get_available_versions, get_installed_versions,
    delete_binary_version, download_binary, get_available_services, parse_service_type,
};

// Re-export DNS/network commands
pub use dns::{
    get_network_status, get_resolver_status, install_resolver, uninstall_resolver,
    start_dns_server, stop_dns_server, restart_dns_server,
};

// Re-export proxy commands
pub use proxy::{
    get_proxy_status, setup_proxy, disable_proxy, start_proxy_daemon, restart_proxy_daemon,
    restart_proxy_for_certs, get_proxy_config, get_ca_trust_status, trust_caddy_ca, untrust_caddy_ca,
    auto_trust_ca_if_needed,
};

// Re-export system commands (settings, CLI, helper)
pub use system::{
    get_settings, update_tld, get_cli_status, install_cli, uninstall_cli,
    get_helper_status, install_helper, uninstall_helper, open_keychain_access,
};

// Re-export mail commands (Mailpit)
pub use mail::{
    get_mailpit_config, list_emails, get_email, delete_emails, delete_all_emails,
    mark_emails_read, get_unread_count,
};

// Re-export tinker commands (PHP Console)
pub use tinker::{
    list_tinker_projects, execute_tinker, get_tinker_history,
    clear_tinker_history, delete_tinker_history_item, get_tinker_php_info,
};

// Re-export log commands
pub use logs::{
    get_available_log_sources, get_recent_logs, stream_logs, clear_logs,
};

// Re-export park commands
pub use park::{
    is_park_enabled, list_parked_directories, park_directory, unpark_directory,
    refresh_parked_directory, refresh_all_parked_directories, get_parked_projects,
    update_parked_directory_ssl,
};

// Re-export stack commands
pub use stacks::{
    list_stacks, get_stack, create_stack, update_stack, delete_stack,
    add_instances_to_stack, remove_instances_from_stack, move_instance_to_stack,
    export_stack, preview_stack_import, import_stack,
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
