//! Instance related commands
//!
//! Handles CRUD operations for service instances, lifecycle management,
//! health checks, logs, configuration, and environment variables.

use crate::config::{Domain, Instance, ServiceType};
use crate::error::LockExt;
use crate::lock; // Shared macro from error.rs
use crate::process::ProcessManager;
use crate::service_config::ServiceRegistry;
use crate::services::{get_service, HealthCheck};
use crate::validation;
use futures_util::future;
use serde::Serialize;
use tauri::State;
use uuid::Uuid;

use super::AppState;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct InstanceWithHealth {
    pub id: String,
    pub name: String,
    pub port: u16,
    pub service_type: String,
    pub version: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub healthy: Option<bool>,
    pub has_config: bool,
    pub domain: String,
    pub domain_enabled: bool,
    pub process_manager: String,
    pub stack_id: Option<String>,
    pub mapped_domains: Vec<String>,
}

/// Instance configuration response
#[derive(Debug, Serialize)]
pub struct InstanceConfig {
    pub id: String,
    pub name: String,
    pub service_type: String,
    pub config: serde_json::Value,
}

/// Instance information response
#[derive(Debug, Serialize)]
pub struct InstanceInfo {
    pub id: String,
    pub name: String,
    pub service_type: String,
    pub version: String,
    pub port: u16,
    pub running: bool,
    pub pid: Option<u32>,
    pub categories: Vec<InfoCategory>,
}

#[derive(Debug, Serialize)]
pub struct InfoCategory {
    pub title: String,
    pub items: Vec<InfoItem>,
}

#[derive(Debug, Serialize)]
pub struct InfoItem {
    pub label: String,
    pub value: String,
    pub copyable: bool,
}

// ============================================================================
// Helper Functions
// ============================================================================

async fn check_health_for_service(port: u16, service_type: ServiceType) -> bool {
    let service = get_service(service_type);
    match service.health_check() {
        HealthCheck::Http { path } => {
            let url = format!("http://127.0.0.1:{}{}", port, path);
            reqwest::get(&url)
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
        }
        HealthCheck::Tcp => {
            use std::net::TcpStream;
            use std::time::Duration;
            // Parse the socket address safely
            let addr = match format!("127.0.0.1:{}", port).parse() {
                Ok(addr) => addr,
                Err(_) => return false, // Invalid port number
            };
            TcpStream::connect_timeout(&addr, Duration::from_secs(2)).is_ok()
        }
    }
}

// ============================================================================
// Instance CRUD Commands
// ============================================================================

#[tauri::command]
pub async fn list_instances(state: State<'_, AppState>) -> Result<Vec<InstanceWithHealth>, String> {
    // Collect instance data while holding lock
    let (instances_data, tld, domains): (Vec<(Instance, bool, Option<u32>)>, String, Vec<Domain>) = {
        let config_store = lock!(state.config_store)?;
        let process_manager = lock!(state.process_manager)?;

        let config = config_store.load()?;
        let tld = config.tld.clone();
        let domains = config.domains.clone();
        let instances = config
            .instances
            .into_iter()
            .map(|instance| {
                let status = process_manager.get_status(&instance);
                (instance, status.running, status.pid)
            })
            .collect();
        (instances, tld, domains)
    };
    // Lock is released here

    // Parallelize health checks for better performance (5-10x faster for 10+ instances)
    let health_check_futures: Vec<_> = instances_data
        .into_iter()
        .map(|(instance, running, pid)| {
            let tld_clone = tld.clone();
            let domains_clone = domains.clone();
            async move {
                // Perform health check asynchronously
                let healthy = if running {
                    Some(check_health_for_service(instance.port, instance.service_type).await)
                } else {
                    None
                };

                let service = get_service(instance.service_type);
                let has_config = !instance.config.is_null() && instance.config != serde_json::json!({});

                // Find all domains that map to this instance
                let mapped_domains: Vec<String> = domains_clone
                    .iter()
                    .filter(|d| d.routes_to_instance(&instance.id))
                    .map(|d| d.full_domain(&tld_clone))
                    .collect();

                // Only show domain if instance has explicit custom domain AND it exists in mapped_domains
                // Otherwise return empty string to indicate no domain
                let domain = if let Some(ref custom_domain) = instance.domain {
                    let full = format!("{}.{}", custom_domain, tld_clone);
                    if mapped_domains.contains(&full) {
                        full
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                };
                let domain_enabled = instance.domain_enabled;

                let process_manager = match service.process_manager() {
                    crate::services::ProcessManager::Binary => "binary".to_string(),
                    crate::services::ProcessManager::Pm2 => "pm2".to_string(),
                };

                InstanceWithHealth {
                    id: instance.id.to_string(),
                    name: instance.name,
                    port: instance.port,
                    service_type: instance.service_type.as_str().to_string(),
                    version: instance.version.clone(),
                    running,
                    pid,
                    healthy,
                    has_config,
                    domain,
                    domain_enabled,
                    process_manager,
                    stack_id: instance.stack_id.map(|id| id.to_string()),
                    mapped_domains,
                }
            }
        })
        .collect();

    // Execute all health checks concurrently
    let results = future::join_all(health_check_futures).await;

    Ok(results)
}

#[tauri::command]
pub fn create_instance(
    name: String,
    port: u16,
    service_type: String,
    version: String,
    config: Option<serde_json::Value>,
    custom_domain: Option<String>,
    state: State<'_, AppState>,
) -> Result<InstanceWithHealth, String> {
    // Validate instance name
    validation::validate_instance_name(&name)
        .map_err(|e| format!("Invalid instance name: {}", e))?;

    // Validate port
    validation::validate_port(port)
        .map_err(|e| format!("Invalid port: {}", e))?;

    // Validate version
    validation::validate_version(&version)
        .map_err(|e| format!("Invalid version: {}", e))?;

    // Parse service type
    let svc_type = super::parse_service_type(&service_type)?;

    let binary_manager = lock!(state.binary_manager)?;
    let installed_versions = binary_manager.get_installed_versions_sync(svc_type)?;
    if !installed_versions.contains(&version) {
        return Err(format!("Version {} is not installed for {}", version, service_type));
    }
    drop(binary_manager);

    // Check if this service type has auto_create_domain enabled
    let registry = ServiceRegistry::load();
    let service_def = registry.get_service(&service_type.to_lowercase());
    let auto_create_domain = service_def
        .map(|s| s.auto_create_domain)
        .unwrap_or(false);

    let service_config = config.unwrap_or_else(|| serde_json::json!({}));
    let config_store = lock!(state.config_store)?;
    let app_config = config_store.load()?;

    // Check max_instances limit
    if let Some(service_def) = service_def {
        if let Some(max) = service_def.max_instances {
            let existing_count = app_config.instances.iter()
                .filter(|i| i.service_type == svc_type)
                .count();
            if existing_count >= max {
                return Err(format!(
                    "{} is limited to {} instance(s)",
                    service_def.display_name,
                    max
                ));
            }
        }
    }
    let tld = app_config.tld.clone();
    let instance = config_store.create_instance(name, port, svc_type, version.clone(), service_config, custom_domain.clone())?;

    // Create domain if custom_domain is provided
    if let Some(ref domain_name) = custom_domain {
        config_store.create_domain_for_instance(
            domain_name.clone(),
            instance.id,
            true, // ssl_enabled
        )?;
    }

    let service = get_service(instance.service_type);
    let has_config = !instance.config.is_null() && instance.config != serde_json::json!({});

    // Get mapped domains for this new instance
    let config = config_store.load()?;
    let mapped_domains: Vec<String> = config
        .domains
        .iter()
        .filter(|d| d.routes_to_instance(&instance.id))
        .map(|d| d.full_domain(&tld))
        .collect();

    // Only show domain if instance has explicit custom domain AND it exists in mapped_domains
    // Otherwise return empty string to indicate no domain
    let domain = if let Some(ref custom_domain) = instance.domain {
        let full = format!("{}.{}", custom_domain, tld);
        if mapped_domains.contains(&full) {
            full
        } else {
            String::new()
        }
    } else {
        String::new()
    };
    let domain_enabled = instance.domain_enabled;
    let process_manager = match service.process_manager() {
        crate::services::ProcessManager::Binary => "binary".to_string(),
        crate::services::ProcessManager::Pm2 => "pm2".to_string(),
    };

    Ok(InstanceWithHealth {
        id: instance.id.to_string(),
        name: instance.name.clone(),
        port: instance.port,
        service_type: instance.service_type.as_str().to_string(),
        version: instance.version.clone(),
        running: false,
        pid: None,
        healthy: None,
        has_config,
        domain,
        domain_enabled,
        process_manager,
        stack_id: instance.stack_id.map(|id| id.to_string()),
        mapped_domains,
    })
}

#[tauri::command]
pub fn rename_instance(
    id: String,
    new_name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    // Validate new instance name
    validation::validate_instance_name(&new_name)
        .map_err(|e| format!("Invalid instance name: {}", e))?;

    let config_store = lock!(state.config_store)?;
    let mut config = config_store.load()?;

    let instance = config
        .instances
        .iter_mut()
        .find(|i| i.id == uuid)
        .ok_or_else(|| format!("Instance {} not found", id))?;

    instance.name = new_name.trim().to_string();
    config_store.save(&config)?;

    Ok(())
}

// ============================================================================
// Instance Lifecycle Commands
// ============================================================================

#[tauri::command]
pub async fn start_instance(id: String, state: State<'_, AppState>) -> Result<u32, String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    let (instance, pid, tld, domains) = {
        let config_store = lock!(state.config_store)?;
        let process_manager = lock!(state.process_manager)?;

        let config = config_store.load()?;
        let tld = config.tld.clone();
        let instance = config_store.get_instance(uuid)?;

        // Validate that the version is installed
        if instance.version.is_empty() {
            return Err(format!(
                "Instance '{}' has no version set. Please set a version in the settings.",
                instance.name
            ));
        }

        let installed = config.binaries.get(&instance.service_type);
        let version_exists = installed
            .map(|versions| versions.contains_key(&instance.version))
            .unwrap_or(false);

        if !version_exists {
            return Err(format!(
                "Version {} is not installed for {}. Please download it first.",
                instance.version,
                instance.service_type.display_name()
            ));
        }

        // Get domains that route to this instance
        let domains: Vec<Domain> = config.domains.iter()
            .filter(|d| d.routes_to_instance(&uuid))
            .cloned()
            .collect();

        // Check if any domain has SSL enabled
        let ssl_enabled = domains.iter().any(|d| d.ssl_enabled);

        let pid = process_manager.start(&instance, Some(&tld), ssl_enabled)?;

        (instance, pid, tld, domains)
    };

    // Register proxy routes for all domains targeting this instance
    {
        let proxy = state.proxy_server.lock().await;
        for domain in &domains {
            let _ = proxy.register_route(&domain.full_domain(&tld), instance.port, &domain.id.to_string(), domain.ssl_enabled);
        }
    }

    Ok(pid)
}

#[tauri::command]
pub async fn stop_instance(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    // Get domains before stopping
    let domains = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;

        // Get domains that route to this instance
        let domains: Vec<Domain> = config.domains.iter()
            .filter(|d| d.routes_to_instance(&uuid))
            .cloned()
            .collect();

        domains
    };

    // Stop the process
    {
        let process_manager = lock!(state.process_manager)?;
        process_manager.stop(&uuid)?;
    }

    // Unregister proxy routes for all domains targeting this instance
    {
        let proxy = state.proxy_server.lock().await;
        let tld = proxy.tld();
        for domain in &domains {
            let _ = proxy.unregister_route(&domain.full_domain(tld));
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn restart_instance(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    let instance = {
        let config_store = lock!(state.config_store)?;
        config_store.get_instance(uuid)?
    };

    // Check if this is a PM2-managed service
    let service = crate::services::get_service(instance.service_type);
    if service.process_manager() == crate::services::ProcessManager::Pm2 {
        let process_manager = lock!(state.process_manager)?;
        process_manager.restart_pm2(&instance)?;
    } else {
        // For binary services, stop and start
        {
            let process_manager = lock!(state.process_manager)?;
            process_manager.stop(&uuid)?;
        }
        // Small delay between stop and start
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        // Restart by calling start_instance logic
        start_instance(id.clone(), state.clone()).await?;
    }

    Ok(())
}

#[tauri::command]
pub async fn delete_instance(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    // Get instance info and stop if running
    let instance = {
        let config_store = lock!(state.config_store)?;
        let process_manager = lock!(state.process_manager)?;

        let instance = config_store.get_instance(uuid)?;

        // Stop if running
        if process_manager.is_running(&uuid) {
            process_manager.stop(&uuid)?;
        }

        instance
    };

    // Get domains that route to this instance and unregister their routes
    let domains_to_remove = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;
        config.domains.iter()
            .filter(|d| d.routes_to_instance(&uuid))
            .cloned()
            .collect::<Vec<_>>()
    };

    // Unregister proxy routes for all domains pointing to this instance
    {
        let proxy = state.proxy_server.lock().await;
        let tld = proxy.tld();
        for domain in &domains_to_remove {
            let _ = proxy.unregister_route(&domain.full_domain(tld));
        }
        // Also unregister the legacy instance domain route if enabled
        if instance.domain_enabled {
            let _ = proxy.unregister_route(&instance.full_domain(tld));
        }
    }

    // Delete instance and associated domains from config
    let config_store = lock!(state.config_store)?;
    config_store.delete_domains_for_instance(uuid)?;
    config_store.delete_instance(uuid)
}

// ============================================================================
// Instance Health & Logs Commands
// ============================================================================

#[tauri::command]
pub async fn check_instance_health(port: u16, service_type: String) -> Result<bool, String> {
    let svc_type = super::parse_service_type(&service_type)?;
    Ok(check_health_for_service(port, svc_type).await)
}

#[tauri::command]
pub fn get_instance_logs(id: String) -> Result<String, String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    // Check if this is a PM2-managed instance
    if ProcessManager::is_pm2_managed(&uuid) {
        ProcessManager::read_logs_pm2(&uuid, 100)
    } else {
        ProcessManager::read_logs(&uuid)
    }
}

// ============================================================================
// Instance Config Commands
// ============================================================================

/// Get instance configuration
#[tauri::command]
pub fn get_instance_config(id: String, state: State<'_, AppState>) -> Result<InstanceConfig, String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    let config_store = lock!(state.config_store)?;
    let instance = config_store.get_instance(uuid)?;

    Ok(InstanceConfig {
        id: instance.id.to_string(),
        name: instance.name,
        service_type: instance.service_type.as_str().to_string(),
        config: instance.config,
    })
}

/// Update instance configuration
#[tauri::command]
pub fn update_instance_config(
    id: String,
    config: serde_json::Value,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    let config_store = lock!(state.config_store)?;
    config_store.update_instance_config(uuid, config)?;

    Ok(())
}

/// Change instance version
#[tauri::command]
pub fn change_instance_version(
    id: String,
    new_version: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    // Get current instance to check service type
    let config_store = lock!(state.config_store)?;
    let instance = config_store.get_instance(uuid)?;

    // Validate new version is installed for this service type
    let binary_manager = lock!(state.binary_manager)?;
    let installed = binary_manager.get_installed_versions_sync(instance.service_type)?;
    if !installed.contains(&new_version) {
        return Err(format!("Version {} is not installed for {}", new_version, instance.service_type.display_name()));
    }

    // Update the instance version
    drop(binary_manager); // Release lock before calling config_store again
    config_store.update_instance_version(uuid, new_version)?;

    Ok(())
}

// ============================================================================
// Instance ENV Commands
// ============================================================================

/// Get environment variables for connecting to an instance
#[tauri::command]
pub fn get_instance_env(id: String, state: State<'_, AppState>) -> Result<String, String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;
    let config_store = lock!(state.config_store)?;
    let instance = config_store.get_instance(uuid)?;

    Ok(generate_env_for_service(&instance))
}

pub fn generate_env_for_service(instance: &Instance) -> String {
    match instance.service_type {
        ServiceType::Redis => generate_redis_env(instance),
        ServiceType::Valkey => generate_valkey_env(instance),
        ServiceType::Meilisearch => generate_meilisearch_env(instance),
        ServiceType::Memcached => generate_memcached_env(instance),
        ServiceType::Mailpit => generate_mailpit_env(instance),
        ServiceType::MinIO => generate_minio_env(instance),
        ServiceType::MongoDB => generate_mongodb_env(instance),
        ServiceType::Beanstalkd => generate_beanstalkd_env(instance),
        ServiceType::PostgreSQL => generate_postgresql_env(instance),
        ServiceType::MariaDB => generate_mariadb_env(instance),
        ServiceType::MySQL => generate_mysql_env(instance),
        ServiceType::Typesense => generate_typesense_env(instance),
        ServiceType::FrankenPHP => generate_frankenphp_env(instance),
        ServiceType::FrankenPhpPark => "# FrankenPHP Park serves parked directories - configure via Parks section".to_string(),
        ServiceType::Frpc => "# frpc is a tunneling service - no ENV needed".to_string(),
        ServiceType::NodeRed => generate_nodered_env(instance),
        ServiceType::Caddy => "# Caddy is an internal service - no ENV needed".to_string(),
        ServiceType::Centrifugo => generate_centrifugo_env(instance),
        ServiceType::Gitea => generate_gitea_env(instance),
    }
}

fn generate_redis_env(instance: &Instance) -> String {
    let password = instance.config.get("password")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut env = format!(
        "# Laravel\n\
         REDIS_HOST=127.0.0.1\n\
         REDIS_PORT={}\n",
        instance.port
    );

    if !password.is_empty() {
        env.push_str(&format!("REDIS_PASSWORD={}\n", password));
    } else {
        env.push_str("REDIS_PASSWORD=null\n");
    }

    env.push_str(&format!(
        "\n# WordPress (Redis Object Cache)\n\
         WP_REDIS_HOST=127.0.0.1\n\
         WP_REDIS_PORT={}\n",
        instance.port
    ));

    if !password.is_empty() {
        env.push_str(&format!("WP_REDIS_PASSWORD={}\n", password));
    }

    env
}

fn generate_valkey_env(instance: &Instance) -> String {
    let password = instance.config.get("password")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut env = format!(
        "# Laravel (Valkey is Redis-compatible)\n\
         REDIS_HOST=127.0.0.1\n\
         REDIS_PORT={}\n",
        instance.port
    );

    if !password.is_empty() {
        env.push_str(&format!("REDIS_PASSWORD={}\n", password));
    } else {
        env.push_str("REDIS_PASSWORD=null\n");
    }

    env.push_str(&format!(
        "\n# WordPress (Redis Object Cache - Valkey compatible)\n\
         WP_REDIS_HOST=127.0.0.1\n\
         WP_REDIS_PORT={}\n",
        instance.port
    ));

    if !password.is_empty() {
        env.push_str(&format!("WP_REDIS_PASSWORD={}\n", password));
    }

    env
}

fn generate_meilisearch_env(instance: &Instance) -> String {
    let master_key = instance.config.get("master_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    format!(
        "# Laravel Scout\n\
         SCOUT_DRIVER=meilisearch\n\
         MEILISEARCH_HOST=http://127.0.0.1:{}\n\
         MEILISEARCH_KEY={}\n",
        instance.port,
        master_key
    )
}

fn generate_memcached_env(instance: &Instance) -> String {
    format!(
        "# Laravel\n\
         CACHE_DRIVER=memcached\n\
         MEMCACHED_HOST=127.0.0.1\n\
         MEMCACHED_PORT={}\n",
        instance.port
    )
}

fn generate_mailpit_env(instance: &Instance) -> String {
    let smtp_port = instance.config.get("smtp_port")
        .and_then(|v| v.as_str())
        .unwrap_or("1025");

    format!(
        "# Laravel\n\
         MAIL_MAILER=smtp\n\
         MAIL_HOST=127.0.0.1\n\
         MAIL_PORT={}\n\
         MAIL_USERNAME=null\n\
         MAIL_PASSWORD=null\n\
         MAIL_ENCRYPTION=null\n\
         \n\
         # Mailpit Web UI: http://127.0.0.1:{}\n",
        smtp_port,
        instance.port
    )
}

fn generate_minio_env(instance: &Instance) -> String {
    let root_user = instance.config.get("root_user")
        .and_then(|v| v.as_str())
        .unwrap_or("minioadmin");
    let root_password = instance.config.get("root_password")
        .and_then(|v| v.as_str())
        .unwrap_or("minioadmin");

    format!(
        "# Laravel (S3 driver)\n\
         AWS_ACCESS_KEY_ID={}\n\
         AWS_SECRET_ACCESS_KEY={}\n\
         AWS_DEFAULT_REGION=us-east-1\n\
         AWS_BUCKET=your-bucket\n\
         AWS_ENDPOINT=http://127.0.0.1:{}\n\
         AWS_USE_PATH_STYLE_ENDPOINT=true\n\
         \n\
         # WordPress (S3 Offload Media)\n\
         S3_UPLOADS_BUCKET=your-bucket\n\
         S3_UPLOADS_REGION=us-east-1\n\
         S3_UPLOADS_KEY={}\n\
         S3_UPLOADS_SECRET={}\n\
         S3_UPLOADS_ENDPOINT=http://127.0.0.1:{}\n",
        root_user, root_password, instance.port,
        root_user, root_password, instance.port
    )
}

fn generate_mongodb_env(instance: &Instance) -> String {
    format!(
        "# Laravel (mongodb/laravel-mongodb)\n\
         DB_CONNECTION=mongodb\n\
         MONGODB_HOST=127.0.0.1\n\
         MONGODB_PORT={}\n\
         MONGODB_DATABASE=your-database\n\
         \n\
         # Connection URI\n\
         MONGODB_URI=mongodb://127.0.0.1:{}/your-database\n",
        instance.port, instance.port
    )
}

fn generate_beanstalkd_env(instance: &Instance) -> String {
    format!(
        "# Laravel\n\
         QUEUE_CONNECTION=beanstalkd\n\
         BEANSTALKD_HOST=127.0.0.1\n\
         BEANSTALKD_PORT={}\n",
        instance.port
    )
}

fn generate_postgresql_env(instance: &Instance) -> String {
    format!(
        "# Laravel\n\
         DB_CONNECTION=pgsql\n\
         DB_HOST=127.0.0.1\n\
         DB_PORT={}\n\
         DB_DATABASE=your-database\n\
         DB_USERNAME=your-username\n\
         DB_PASSWORD=your-password\n\
         \n\
         # Connection URI\n\
         DATABASE_URL=postgres://your-username:your-password@127.0.0.1:{}/your-database\n",
        instance.port, instance.port
    )
}

fn generate_mariadb_env(instance: &Instance) -> String {
    format!(
        "# Laravel\n\
         DB_CONNECTION=mysql\n\
         DB_HOST=127.0.0.1\n\
         DB_PORT={}\n\
         DB_DATABASE=your-database\n\
         DB_USERNAME=root\n\
         DB_PASSWORD=\n\
         \n\
         # WordPress\n\
         DB_NAME=your-database\n\
         DB_USER=root\n\
         DB_PASS=\n\
         WP_HOME=http://localhost\n\
         WP_SITEURL=http://localhost\n",
        instance.port
    )
}

fn generate_mysql_env(instance: &Instance) -> String {
    format!(
        "# Laravel\n\
         DB_CONNECTION=mysql\n\
         DB_HOST=127.0.0.1\n\
         DB_PORT={}\n\
         DB_DATABASE=your-database\n\
         DB_USERNAME=root\n\
         DB_PASSWORD=\n\
         \n\
         # WordPress\n\
         DB_NAME=your-database\n\
         DB_USER=root\n\
         DB_PASS=\n\
         WP_HOME=http://localhost\n\
         WP_SITEURL=http://localhost\n",
        instance.port
    )
}

fn generate_typesense_env(instance: &Instance) -> String {
    let api_key = instance.config.get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    format!(
        "# Laravel Scout\n\
         SCOUT_DRIVER=typesense\n\
         TYPESENSE_HOST=127.0.0.1\n\
         TYPESENSE_PORT={}\n\
         TYPESENSE_PROTOCOL=http\n\
         TYPESENSE_API_KEY={}\n",
        instance.port, api_key
    )
}

fn generate_frankenphp_env(instance: &Instance) -> String {
    let document_root = instance.config.get("document_root")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    format!(
        "# FrankenPHP Server\n\
         # Web URL: http://127.0.0.1:{}\n\
         # Document Root: {}\n\
         \n\
         APP_URL=http://127.0.0.1:{}\n",
        instance.port, document_root, instance.port
    )
}

fn generate_nodered_env(instance: &Instance) -> String {
    let flow_file = instance.config.get("flow_file")
        .and_then(|v| v.as_str())
        .unwrap_or("flows.json");

    format!(
        "# Node-RED\n\
         # Admin UI: http://127.0.0.1:{}\n\
         # API Root: http://127.0.0.1:{}/api\n\
         # Flow File: {}\n\
         \n\
         NODE_RED_URL=http://127.0.0.1:{}\n\
         NODE_RED_PORT={}\n",
        instance.port, instance.port, flow_file, instance.port, instance.port
    )
}

fn generate_centrifugo_env(instance: &Instance) -> String {
    let api_key = instance.config.get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let token_secret = instance.config.get("token_hmac_secret")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    format!(
        "# Centrifugo\n\
         # WebSocket: ws://127.0.0.1:{}/connection/websocket\n\
         # Admin UI: http://127.0.0.1:{}/\n\
         \n\
         CENTRIFUGO_URL=http://127.0.0.1:{}\n\
         CENTRIFUGO_API_KEY={}\n\
         CENTRIFUGO_TOKEN_HMAC_SECRET={}\n",
        instance.port, instance.port, instance.port, api_key, token_secret
    )
}

/// Reorder instances in the config (for drag-and-drop)
#[tauri::command]
pub async fn reorder_instances(
    instance_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let instance_uuids: Vec<Uuid> = instance_ids
        .iter()
        .map(|id| Uuid::parse_str(id).map_err(|_| format!("Invalid instance ID: {}", id)))
        .collect::<Result<Vec<_>, _>>()?;

    let config_store = lock!(state.config_store)?;
    config_store.reorder_instances(instance_uuids)?;
    Ok(())
}

fn generate_gitea_env(instance: &Instance) -> String {
    format!(
        "# Gitea\n\
         # Web UI: http://127.0.0.1:{}\n\
         \n\
         GITEA_URL=http://127.0.0.1:{}\n",
        instance.port, instance.port
    )
}

// ============================================================================
// Instance Info Command
// ============================================================================

/// Get instance information
#[tauri::command]
pub async fn get_instance_info(
    id: String,
    state: State<'_, AppState>,
) -> Result<InstanceInfo, String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid instance ID")?;

    let (instance, running, pid, tld) = {
        let config_store = lock!(state.config_store)?;
        let process_manager = lock!(state.process_manager)?;

        let config = config_store.load()?;
        let instance = config_store.get_instance(uuid)?;
        let status = process_manager.get_status(&instance);

        (instance, status.running, status.pid, config.tld.clone())
    };

    Ok(generate_info_for_service(&instance, running, pid, &tld))
}

fn generate_info_for_service(instance: &Instance, running: bool, pid: Option<u32>, tld: &str) -> InstanceInfo {
    let mut categories = vec![create_basic_info_category(instance, running, pid, tld)];

    // Add service-specific categories
    match instance.service_type {
        ServiceType::FrankenPHP => categories.push(create_frankenphp_category(instance)),
        ServiceType::MySQL | ServiceType::MariaDB => categories.push(create_mysql_category(instance)),
        ServiceType::PostgreSQL => categories.push(create_postgresql_category(instance)),
        ServiceType::Redis => categories.push(create_redis_category(instance)),
        ServiceType::Valkey => categories.push(create_valkey_category(instance)),
        ServiceType::Memcached => categories.push(create_memcached_category(instance)),
        ServiceType::Meilisearch => categories.push(create_meilisearch_category(instance)),
        ServiceType::Typesense => categories.push(create_typesense_category(instance)),
        ServiceType::MinIO => categories.push(create_minio_category(instance)),
        ServiceType::Mailpit => categories.push(create_mailpit_category(instance)),
        ServiceType::NodeRed => categories.push(create_nodered_category(instance)),
        ServiceType::Gitea => categories.push(create_gitea_category(instance)),
        ServiceType::Centrifugo => categories.push(create_centrifugo_category(instance)),
        _ => {}
    }

    InstanceInfo {
        id: instance.id.to_string(),
        name: instance.name.clone(),
        service_type: instance.service_type.as_str().to_string(),
        version: instance.version.clone(),
        port: instance.port,
        running,
        pid,
        categories,
    }
}

fn create_basic_info_category(instance: &Instance, running: bool, pid: Option<u32>, tld: &str) -> InfoCategory {
    let mut items = vec![
        InfoItem {
            label: "Instance ID".to_string(),
            value: instance.id.to_string(),
            copyable: true,
        },
        InfoItem {
            label: "Service Type".to_string(),
            value: instance.service_type.display_name().to_string(),
            copyable: false,
        },
        InfoItem {
            label: "Version".to_string(),
            value: instance.version.clone(),
            copyable: false,
        },
        InfoItem {
            label: "Port".to_string(),
            value: instance.port.to_string(),
            copyable: true,
        },
        InfoItem {
            label: "Status".to_string(),
            value: if running { "Running".to_string() } else { "Stopped".to_string() },
            copyable: false,
        },
    ];

    if let Some(pid) = pid {
        items.push(InfoItem {
            label: "Process ID".to_string(),
            value: pid.to_string(),
            copyable: true,
        });
    }

    if instance.domain_enabled {
        items.push(InfoItem {
            label: "Domain".to_string(),
            value: instance.full_domain(tld),
            copyable: true,
        });
    }

    InfoCategory {
        title: "Basic Information".to_string(),
        items,
    }
}

fn create_frankenphp_category(instance: &Instance) -> InfoCategory {
    let mut items = Vec::new();

    // Get document root from config
    if let Some(doc_root) = instance.config.get("document_root").and_then(|v| v.as_str()) {
        items.push(InfoItem {
            label: "Document Root".to_string(),
            value: doc_root.to_string(),
            copyable: true,
        });
    }

    // Try to get PHP version and modules
    match get_php_info(instance) {
        Ok((version, modules)) => {
            items.push(InfoItem {
                label: "PHP Version".to_string(),
                value: version,
                copyable: false,
            });

            items.push(InfoItem {
                label: "Loaded Modules".to_string(),
                value: format!("{} modules", modules.len()),
                copyable: false,
            });

            // Add all modules as a single item
            if !modules.is_empty() {
                items.push(InfoItem {
                    label: "Module List".to_string(),
                    value: modules.join(", "),
                    copyable: true,
                });
            }
        }
        Err(e) => {
            items.push(InfoItem {
                label: "PHP Information".to_string(),
                value: format!("Unable to detect: {}", e),
                copyable: false,
            });
        }
    }

    InfoCategory {
        title: "PHP Configuration".to_string(),
        items,
    }
}

fn get_php_info(instance: &Instance) -> Result<(String, Vec<String>), String> {
    use crate::config::get_versioned_binary_path;
    use std::process::Command;

    let binary_path = get_versioned_binary_path(ServiceType::FrankenPHP, &instance.version)?;

    // Get PHP version using FrankenPHP's version command
    let version_output = Command::new(&binary_path)
        .arg("version")
        .output()
        .map_err(|e| format!("Failed to execute FrankenPHP: {}", e))?;

    if !version_output.status.success() {
        return Err("FrankenPHP version command failed".to_string());
    }

    let version_str = String::from_utf8_lossy(&version_output.stdout);
    let version = parse_frankenphp_version(&version_str)?;

    // Get PHP modules using a temporary script
    let modules = get_php_modules_from_frankenphp(&binary_path)?;

    Ok((version, modules))
}

fn parse_frankenphp_version(output: &str) -> Result<String, String> {
    // Parse version from output like: "FrankenPHP v1.11.0 PHP 8.4.16 Caddy v2.10.2 ..."
    if let Some(start) = output.find("PHP ") {
        let rest = &output[start + 4..];
        if let Some(end) = rest.find(' ') {
            return Ok(rest[..end].to_string());
        }
    }
    Err("Could not parse PHP version from FrankenPHP".to_string())
}

fn get_php_modules_from_frankenphp(binary_path: &std::path::Path) -> Result<Vec<String>, String> {
    use std::process::Command;

    // Create temporary PHP script to get modules
    let temp_dir = std::env::temp_dir();
    let script_path = temp_dir.join("burd_get_modules.php");

    let script_content = r#"<?php
$extensions = get_loaded_extensions();
sort($extensions);
foreach ($extensions as $ext) {
    echo $ext . "\n";
}
"#;

    std::fs::write(&script_path, script_content)
        .map_err(|e| format!("Failed to create temp script: {}", e))?;

    // Execute the script with FrankenPHP
    let modules_output = Command::new(binary_path)
        .arg("php-cli")
        .arg(&script_path)
        .output()
        .map_err(|e| format!("Failed to execute PHP script: {}", e))?;

    // Clean up temp script
    let _ = std::fs::remove_file(&script_path);

    if !modules_output.status.success() {
        let stderr = String::from_utf8_lossy(&modules_output.stderr);
        return Err(format!("Failed to get PHP modules: {}", stderr));
    }

    let modules_str = String::from_utf8_lossy(&modules_output.stdout);
    let modules: Vec<String> = modules_str
        .lines()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    Ok(modules)
}


fn create_mysql_category(instance: &Instance) -> InfoCategory {
    let items = vec![
        InfoItem {
            label: "Connection".to_string(),
            value: format!("127.0.0.1:{}", instance.port),
            copyable: true,
        },
        InfoItem {
            label: "Username".to_string(),
            value: "root".to_string(),
            copyable: false,
        },
        InfoItem {
            label: "Password".to_string(),
            value: "(empty)".to_string(),
            copyable: false,
        },
    ];

    InfoCategory {
        title: format!("{} Configuration", instance.service_type.display_name()),
        items,
    }
}

fn create_postgresql_category(instance: &Instance) -> InfoCategory {
    let items = vec![
        InfoItem {
            label: "Connection".to_string(),
            value: format!("127.0.0.1:{}", instance.port),
            copyable: true,
        },
        InfoItem {
            label: "Default Database".to_string(),
            value: "postgres".to_string(),
            copyable: false,
        },
    ];

    InfoCategory {
        title: "PostgreSQL Configuration".to_string(),
        items,
    }
}

fn create_redis_category(instance: &Instance) -> InfoCategory {
    let password = instance.config.get("password")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut items = vec![
        InfoItem {
            label: "Connection".to_string(),
            value: format!("127.0.0.1:{}", instance.port),
            copyable: true,
        },
    ];

    if !password.is_empty() {
        items.push(InfoItem {
            label: "Password".to_string(),
            value: password.to_string(),
            copyable: true,
        });
    }

    InfoCategory {
        title: "Redis Configuration".to_string(),
        items,
    }
}

fn create_valkey_category(instance: &Instance) -> InfoCategory {
    let password = instance.config.get("password")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut items = vec![
        InfoItem {
            label: "Connection".to_string(),
            value: format!("127.0.0.1:{}", instance.port),
            copyable: true,
        },
    ];

    if !password.is_empty() {
        items.push(InfoItem {
            label: "Password".to_string(),
            value: password.to_string(),
            copyable: true,
        });
    }

    InfoCategory {
        title: "Valkey Configuration".to_string(),
        items,
    }
}

fn create_memcached_category(instance: &Instance) -> InfoCategory {
    let items = vec![
        InfoItem {
            label: "Connection".to_string(),
            value: format!("127.0.0.1:{}", instance.port),
            copyable: true,
        },
    ];

    InfoCategory {
        title: "Memcached Configuration".to_string(),
        items,
    }
}

fn create_meilisearch_category(instance: &Instance) -> InfoCategory {
    let master_key = instance.config.get("master_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut items = vec![
        InfoItem {
            label: "API Endpoint".to_string(),
            value: format!("http://127.0.0.1:{}", instance.port),
            copyable: true,
        },
    ];

    if !master_key.is_empty() {
        items.push(InfoItem {
            label: "Master Key".to_string(),
            value: master_key.to_string(),
            copyable: true,
        });
    }

    InfoCategory {
        title: "Meilisearch Configuration".to_string(),
        items,
    }
}

fn create_typesense_category(instance: &Instance) -> InfoCategory {
    let api_key = instance.config.get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut items = vec![
        InfoItem {
            label: "API Endpoint".to_string(),
            value: format!("http://127.0.0.1:{}", instance.port),
            copyable: true,
        },
    ];

    if !api_key.is_empty() {
        items.push(InfoItem {
            label: "API Key".to_string(),
            value: api_key.to_string(),
            copyable: true,
        });
    }

    InfoCategory {
        title: "Typesense Configuration".to_string(),
        items,
    }
}

fn create_minio_category(instance: &Instance) -> InfoCategory {
    let root_user = instance.config.get("root_user")
        .and_then(|v| v.as_str())
        .unwrap_or("minioadmin");
    let root_password = instance.config.get("root_password")
        .and_then(|v| v.as_str())
        .unwrap_or("minioadmin");

    let items = vec![
        InfoItem {
            label: "Console URL".to_string(),
            value: format!("http://127.0.0.1:{}", instance.port),
            copyable: true,
        },
        InfoItem {
            label: "Access Key".to_string(),
            value: root_user.to_string(),
            copyable: true,
        },
        InfoItem {
            label: "Secret Key".to_string(),
            value: root_password.to_string(),
            copyable: true,
        },
    ];

    InfoCategory {
        title: "MinIO Configuration".to_string(),
        items,
    }
}

fn create_mailpit_category(instance: &Instance) -> InfoCategory {
    let smtp_port = instance.config.get("smtp_port")
        .and_then(|v| v.as_str())
        .unwrap_or("1025");

    let items = vec![
        InfoItem {
            label: "Web UI".to_string(),
            value: format!("http://127.0.0.1:{}", instance.port),
            copyable: true,
        },
        InfoItem {
            label: "SMTP Port".to_string(),
            value: smtp_port.to_string(),
            copyable: true,
        },
    ];

    InfoCategory {
        title: "Mailpit Configuration".to_string(),
        items,
    }
}

fn create_nodered_category(instance: &Instance) -> InfoCategory {
    let flow_file = instance.config.get("flow_file")
        .and_then(|v| v.as_str())
        .unwrap_or("flows.json");

    let items = vec![
        InfoItem {
            label: "Admin UI".to_string(),
            value: format!("http://127.0.0.1:{}", instance.port),
            copyable: true,
        },
        InfoItem {
            label: "Flow File".to_string(),
            value: flow_file.to_string(),
            copyable: false,
        },
    ];

    InfoCategory {
        title: "Node-RED Configuration".to_string(),
        items,
    }
}

fn create_gitea_category(instance: &Instance) -> InfoCategory {
    let items = vec![
        InfoItem {
            label: "Web UI".to_string(),
            value: format!("http://127.0.0.1:{}", instance.port),
            copyable: true,
        },
    ];

    InfoCategory {
        title: "Gitea Configuration".to_string(),
        items,
    }
}

fn create_centrifugo_category(instance: &Instance) -> InfoCategory {
    let api_key = instance.config.get("api_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let token_secret = instance.config.get("token_hmac_secret")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let mut items = vec![
        InfoItem {
            label: "Admin UI".to_string(),
            value: format!("http://127.0.0.1:{}", instance.port),
            copyable: true,
        },
        InfoItem {
            label: "WebSocket".to_string(),
            value: format!("ws://127.0.0.1:{}/connection/websocket", instance.port),
            copyable: true,
        },
    ];

    if !api_key.is_empty() {
        items.push(InfoItem {
            label: "API Key".to_string(),
            value: api_key.to_string(),
            copyable: true,
        });
    }

    if !token_secret.is_empty() {
        items.push(InfoItem {
            label: "Token HMAC Secret".to_string(),
            value: token_secret.to_string(),
            copyable: true,
        });
    }

    InfoCategory {
        title: "Centrifugo Configuration".to_string(),
        items,
    }
}
