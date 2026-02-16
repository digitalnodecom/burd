//! Instance API handlers

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::api::{
    state::ApiState,
    types::{ApiResponse, CreateInstanceRequest},
};
use crate::commands::{parse_service_type, generate_env_for_service};
use crate::process::ProcessManager;
use crate::service_config::ServiceRegistry;
use crate::services::{get_service, HealthCheck};

/// Instance with health status (API response type)
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
}

/// Check health for a service
async fn check_health_for_service(port: u16, service_type: crate::config::ServiceType) -> bool {
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

/// GET /instances - List all instances
pub async fn list(State(state): State<ApiState>) -> Json<ApiResponse<Vec<InstanceWithHealth>>> {
    // Collect instance data while holding lock
    let instances_data = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let process_manager = match state.inner.process_manager.lock() {
            Ok(pm) => pm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        let tld = config.tld.clone();
        let instances: Vec<_> = config
            .instances
            .into_iter()
            .map(|instance| {
                let status = process_manager.get_status(&instance);
                (instance, status.running, status.pid, tld.clone())
            })
            .collect();

        instances
    };

    // Build response with health checks
    let mut results = Vec::new();
    for (instance, running, pid, tld) in instances_data {
        let healthy = if running {
            Some(check_health_for_service(instance.port, instance.service_type).await)
        } else {
            None
        };

        let service = get_service(instance.service_type);
        let has_config = !instance.config.is_null() && instance.config != serde_json::json!({});
        // Only show domain if instance has explicit custom domain set
        let domain = if instance.domain.is_some() {
            instance.full_domain(&tld)
        } else {
            String::new()
        };
        let domain_enabled = instance.domain_enabled;
        let pm = match service.process_manager() {
            crate::services::ProcessManager::Binary => "binary".to_string(),
            crate::services::ProcessManager::Pm2 => "pm2".to_string(),
        };

        results.push(InstanceWithHealth {
            id: instance.id.to_string(),
            name: instance.name,
            port: instance.port,
            service_type: service.display_name().to_string(),
            version: instance.version,
            running,
            pid,
            healthy,
            has_config,
            domain,
            domain_enabled,
            process_manager: pm,
        });
    }

    Json(ApiResponse::ok(results))
}

/// GET /instances/:id - Get a specific instance
pub async fn get(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<InstanceWithHealth>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid instance ID")),
    };

    let instance_data = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let process_manager = match state.inner.process_manager.lock() {
            Ok(pm) => pm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        let instance = match config.instances.into_iter().find(|i| i.id == uuid) {
            Some(i) => i,
            None => return Json(ApiResponse::err("Instance not found")),
        };

        let status = process_manager.get_status(&instance);
        (instance, status.running, status.pid, config.tld)
    };

    let (instance, running, pid, tld) = instance_data;
    let healthy = if running {
        Some(check_health_for_service(instance.port, instance.service_type).await)
    } else {
        None
    };

    let service = get_service(instance.service_type);
    let has_config = !instance.config.is_null() && instance.config != serde_json::json!({});

    // Only show domain if instance has explicit custom domain set
    let domain = if instance.domain.is_some() {
        instance.full_domain(&tld)
    } else {
        String::new()
    };
    Json(ApiResponse::ok(InstanceWithHealth {
        id: instance.id.to_string(),
        name: instance.name.clone(),
        port: instance.port,
        service_type: service.display_name().to_string(),
        version: instance.version.clone(),
        running,
        pid,
        healthy,
        has_config,
        domain,
        domain_enabled: instance.domain_enabled,
        process_manager: match service.process_manager() {
            crate::services::ProcessManager::Binary => "binary".to_string(),
            crate::services::ProcessManager::Pm2 => "pm2".to_string(),
        },
    }))
}

/// POST /instances - Create a new instance
pub async fn create(
    State(state): State<ApiState>,
    Json(req): Json<CreateInstanceRequest>,
) -> Json<ApiResponse<InstanceWithHealth>> {
    // Validate port
    if req.port < 1024 {
        return Json(ApiResponse::err("Port must be at least 1024"));
    }

    // Parse service type
    let svc_type = match parse_service_type(&req.service_type) {
        Ok(t) => t,
        Err(e) => return Json(ApiResponse::err(e)),
    };

    // Validate version
    if req.version.is_empty() {
        return Json(ApiResponse::err("Version is required"));
    }

    // Check version is installed
    {
        let binary_manager = match state.inner.binary_manager.lock() {
            Ok(bm) => bm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire binary manager lock")),
        };

        let installed = match binary_manager.get_installed_versions_sync(svc_type) {
            Ok(v) => v,
            Err(e) => return Json(ApiResponse::err(e)),
        };

        if !installed.contains(&req.version) {
            return Json(ApiResponse::err(format!(
                "Version {} is not installed for {}",
                req.version, req.service_type
            )));
        }
    }

    // Check auto_create_domain and max_instances
    let registry = ServiceRegistry::load();
    let service_def = registry.get_service(&req.service_type.to_lowercase());
    let auto_create_domain = service_def.map(|s| s.auto_create_domain).unwrap_or(false);

    let service_config = req.config.unwrap_or_else(|| serde_json::json!({}));

    let result = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        // Check max_instances limit
        if let Some(sd) = service_def {
            if let Some(max) = sd.max_instances {
                let existing_count = config
                    .instances
                    .iter()
                    .filter(|i| i.service_type == svc_type)
                    .count();
                if existing_count >= max {
                    return Json(ApiResponse::err(format!(
                        "{} is limited to {} instance(s)",
                        sd.display_name, max
                    )));
                }
            }
        }

        let tld = config.tld.clone();
        let instance = match config_store.create_instance(
            req.name,
            req.port,
            svc_type,
            req.version,
            service_config,
            req.custom_domain.clone(),
        ) {
            Ok(i) => i,
            Err(e) => return Json(ApiResponse::err(e)),
        };

        // Auto-domain creation disabled - users must create domains manually via UI
        // This prevents automatic domain creation on instance creation

        (instance, tld)
    };

    let (instance, tld) = result;
    let service = get_service(instance.service_type);
    let has_config = !instance.config.is_null() && instance.config != serde_json::json!({});
    // Only show domain if instance has explicit custom domain set
    let domain = if instance.domain.is_some() {
        instance.full_domain(&tld)
    } else {
        String::new()
    };

    Json(ApiResponse::ok(InstanceWithHealth {
        id: instance.id.to_string(),
        name: instance.name,
        port: instance.port,
        service_type: service.display_name().to_string(),
        version: instance.version,
        running: false,
        pid: None,
        healthy: None,
        has_config,
        domain,
        domain_enabled: instance.domain_enabled,
        process_manager: match service.process_manager() {
            crate::services::ProcessManager::Binary => "binary".to_string(),
            crate::services::ProcessManager::Pm2 => "pm2".to_string(),
        },
    }))
}

/// POST /instances/:id/start - Start an instance
pub async fn start(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<u32>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid instance ID")),
    };

    let result = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let process_manager = match state.inner.process_manager.lock() {
            Ok(pm) => pm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        let instance = match config_store.get_instance(uuid) {
            Ok(i) => i,
            Err(e) => return Json(ApiResponse::err(e)),
        };

        // Validate that the version is installed
        if instance.version.is_empty() {
            return Json(ApiResponse::err(format!(
                "Instance '{}' has no version set",
                instance.name
            )));
        }

        let version_exists = config.binaries
            .get(&instance.service_type)
            .map(|versions| versions.contains_key(&instance.version))
            .unwrap_or(false);

        if !version_exists {
            return Json(ApiResponse::err(format!(
                "Version {} is not installed for {}",
                instance.version,
                instance.service_type.display_name()
            )));
        }

        let tld = config.tld.clone();
        let ssl_enabled = config
            .domains
            .iter()
            .filter(|d| d.routes_to_instance(&uuid))
            .any(|d| d.ssl_enabled);

        let pid = match process_manager.start(&instance, Some(&tld), ssl_enabled) {
            Ok(p) => p,
            Err(e) => return Json(ApiResponse::err(e)),
        };

        // Collect domains for proxy registration
        let domains: Vec<_> = config
            .domains
            .iter()
            .filter(|d| d.routes_to_instance(&uuid))
            .cloned()
            .collect();

        (pid, instance.port, domains, tld)
    };

    let (pid, port, domains, tld) = result;

    // Register proxy routes
    {
        let proxy = state.inner.proxy_server.lock().await;
        for domain in &domains {
            let _ = proxy.register_route(
                &domain.full_domain(&tld),
                port,
                &domain.id.to_string(),
                domain.ssl_enabled,
            );
        }
    }

    Json(ApiResponse::ok(pid))
}

/// POST /instances/:id/stop - Stop an instance
pub async fn stop(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid instance ID")),
    };

    // Get domains before stopping
    let domains = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        config
            .domains
            .iter()
            .filter(|d| d.routes_to_instance(&uuid))
            .cloned()
            .collect::<Vec<_>>()
    };

    // Stop the process
    {
        let process_manager = match state.inner.process_manager.lock() {
            Ok(pm) => pm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };

        if let Err(e) = process_manager.stop(&uuid) {
            return Json(ApiResponse::err(e));
        }
    }

    // Unregister proxy routes
    {
        let proxy = state.inner.proxy_server.lock().await;
        let tld = proxy.tld();
        for domain in &domains {
            let _ = proxy.unregister_route(&domain.full_domain(tld));
        }
    }

    Json(ApiResponse::success())
}

/// POST /instances/:id/restart - Restart an instance
pub async fn restart(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid instance ID")),
    };

    let instance = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        match config_store.get_instance(uuid) {
            Ok(i) => i,
            Err(e) => return Json(ApiResponse::err(e)),
        }
    };

    // Check if PM2-managed
    let service = crate::services::get_service(instance.service_type);
    if service.process_manager() == crate::services::ProcessManager::Pm2 {
        let process_manager = match state.inner.process_manager.lock() {
            Ok(pm) => pm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };
        if let Err(e) = process_manager.restart_pm2(&instance) {
            return Json(ApiResponse::err(e));
        }
    } else {
        // For binary services, stop then start
        {
            let process_manager = match state.inner.process_manager.lock() {
                Ok(pm) => pm,
                Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
            };
            let _ = process_manager.stop(&uuid);
        }

        // Small delay
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // Start again
        let start_result = start(State(state.clone()), Path(id)).await;
        if let Json(ApiResponse { success: false, error: Some(e), .. }) = &start_result {
            return Json(ApiResponse::err(e.clone()));
        }
    }

    Json(ApiResponse::success())
}

/// DELETE /instances/:id - Delete an instance
pub async fn remove(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid instance ID")),
    };

    // Get domains before deleting
    let domains = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        config
            .domains
            .iter()
            .filter(|d| d.routes_to_instance(&uuid))
            .cloned()
            .collect::<Vec<_>>()
    };

    // Stop if running
    {
        let process_manager = match state.inner.process_manager.lock() {
            Ok(pm) => pm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };
        let _ = process_manager.stop(&uuid);
    }

    // Delete from config
    {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        if let Err(e) = config_store.delete_instance(uuid) {
            return Json(ApiResponse::err(e));
        }
    }

    // Unregister proxy routes
    {
        let proxy = state.inner.proxy_server.lock().await;
        let tld = proxy.tld();
        for domain in &domains {
            let _ = proxy.unregister_route(&domain.full_domain(tld));
        }
    }

    Json(ApiResponse::success())
}

/// GET /instances/:id/logs - Get instance logs
pub async fn logs(
    State(_state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<String>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid instance ID")),
    };

    // Check if this is a PM2-managed instance
    let result = if ProcessManager::is_pm2_managed(&uuid) {
        ProcessManager::read_logs_pm2(&uuid, 100)
    } else {
        ProcessManager::read_logs(&uuid)
    };

    match result {
        Ok(logs) => Json(ApiResponse::ok(logs)),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

/// GET /instances/:id/env - Get instance environment variables
pub async fn env(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<String>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid instance ID")),
    };

    let result = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        let instance = match config_store.get_instance(uuid) {
            Ok(i) => i,
            Err(e) => return Json(ApiResponse::err(e)),
        };

        // Generate environment variables based on service type
        generate_env_for_service(&instance)
    };

    Json(ApiResponse::ok(result))
}
