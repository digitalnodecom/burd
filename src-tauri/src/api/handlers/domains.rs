//! Domain API handlers

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::api::{
    state::ApiState,
    types::{ApiResponse, CreateDomainRequest, ToggleSslRequest, UpdateDomainRequest},
};
use crate::config::DomainTarget;

/// Domain response
#[derive(Debug, Serialize)]
pub struct DomainInfo {
    pub id: String,
    pub subdomain: String,
    pub full_domain: String,
    pub target_type: String,
    pub target_value: String,
    pub ssl_enabled: bool,
}

/// GET /domains - List all domains
pub async fn list(State(state): State<ApiState>) -> Json<ApiResponse<Vec<DomainInfo>>> {
    let result = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        let tld = config.tld.clone();
        let domains: Vec<DomainInfo> = config
            .domains
            .into_iter()
            .map(|d| {
                let (target_type, target_value) = match &d.target {
                    DomainTarget::Instance(id) => ("instance".to_string(), id.to_string()),
                    DomainTarget::Port(p) => ("port".to_string(), p.to_string()),
                    DomainTarget::StaticFiles { path, .. } => ("static".to_string(), path.clone()),
                };

                DomainInfo {
                    id: d.id.to_string(),
                    subdomain: d.subdomain.clone(),
                    full_domain: d.full_domain(&tld),
                    target_type,
                    target_value,
                    ssl_enabled: d.ssl_enabled,
                }
            })
            .collect();

        domains
    };

    Json(ApiResponse::ok(result))
}

/// POST /domains - Create a new domain
pub async fn create(
    State(state): State<ApiState>,
    Json(req): Json<CreateDomainRequest>,
) -> Json<ApiResponse<DomainInfo>> {
    let result = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        let tld = config.tld.clone();

        // Check if subdomain already exists
        if let Ok(Some(_)) = config_store.find_domain_by_subdomain(&req.subdomain) {
            return Json(ApiResponse::err(format!(
                "Domain '{}' already exists",
                req.subdomain
            )));
        }

        // Create domain based on target type
        let domain = match req.target_type.as_str() {
            "instance" => {
                let instance_id = match Uuid::parse_str(&req.target_value) {
                    Ok(id) => id,
                    Err(_) => return Json(ApiResponse::err("Invalid instance ID")),
                };

                match config_store.create_domain_for_instance(
                    req.subdomain.clone(),
                    instance_id,
                    req.ssl_enabled,
                ) {
                    Ok(d) => d,
                    Err(e) => return Json(ApiResponse::err(e)),
                }
            }
            "port" => {
                let port: u16 = match req.target_value.parse() {
                    Ok(p) => p,
                    Err(_) => return Json(ApiResponse::err("Invalid port number")),
                };

                match config_store.create_domain_for_port(
                    req.subdomain.clone(),
                    port,
                    req.ssl_enabled,
                ) {
                    Ok(d) => d,
                    Err(e) => return Json(ApiResponse::err(e)),
                }
            }
            "static" => {
                let browse = req.static_browse.unwrap_or(false);
                match config_store.create_domain_for_static_files(
                    req.subdomain.clone(),
                    req.target_value.clone(),
                    browse,
                    req.ssl_enabled,
                ) {
                    Ok(d) => d,
                    Err(e) => return Json(ApiResponse::err(e)),
                }
            }
            _ => {
                return Json(ApiResponse::err(
                    "Invalid target_type. Use 'instance', 'port', or 'static'",
                ))
            }
        };

        let (target_type, target_value) = match &domain.target {
            DomainTarget::Instance(id) => ("instance".to_string(), id.to_string()),
            DomainTarget::Port(p) => ("port".to_string(), p.to_string()),
            DomainTarget::StaticFiles { path, .. } => ("static".to_string(), path.clone()),
        };

        let full_domain = domain.full_domain(&tld);
        DomainInfo {
            id: domain.id.to_string(),
            subdomain: domain.subdomain,
            full_domain,
            target_type,
            target_value,
            ssl_enabled: domain.ssl_enabled,
        }
    };

    Json(ApiResponse::ok(result))
}

/// PUT /domains/:id - Update a domain
pub async fn update(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateDomainRequest>,
) -> Json<ApiResponse<DomainInfo>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid domain ID")),
    };

    let result = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        let tld = config.tld.clone();

        // Find the current domain
        let _current_domain = match config.domains.iter().find(|d| d.id == uuid) {
            Some(d) => d.clone(),
            None => return Json(ApiResponse::err("Domain not found")),
        };

        // Build new target if target_type/target_value provided
        let new_target = if let (Some(target_type), Some(target_value)) =
            (&req.target_type, &req.target_value)
        {
            match target_type.as_str() {
                "instance" => {
                    let instance_id = match Uuid::parse_str(target_value) {
                        Ok(id) => id,
                        Err(_) => return Json(ApiResponse::err("Invalid instance ID")),
                    };
                    Some(DomainTarget::Instance(instance_id))
                }
                "port" => {
                    let port: u16 = match target_value.parse() {
                        Ok(p) => p,
                        Err(_) => return Json(ApiResponse::err("Invalid port number")),
                    };
                    Some(DomainTarget::Port(port))
                }
                "static" => Some(DomainTarget::StaticFiles {
                    path: target_value.clone(),
                    browse: false,
                }),
                _ => return Json(ApiResponse::err("Invalid target_type")),
            }
        } else {
            None
        };

        let updated = match config_store.update_domain(uuid, req.subdomain, new_target) {
            Ok(d) => d,
            Err(e) => return Json(ApiResponse::err(e)),
        };

        let (target_type, target_value) = match &updated.target {
            DomainTarget::Instance(id) => ("instance".to_string(), id.to_string()),
            DomainTarget::Port(p) => ("port".to_string(), p.to_string()),
            DomainTarget::StaticFiles { path, .. } => ("static".to_string(), path.clone()),
        };

        let full_domain = updated.full_domain(&tld);
        DomainInfo {
            id: updated.id.to_string(),
            subdomain: updated.subdomain.clone(),
            full_domain,
            target_type,
            target_value,
            ssl_enabled: updated.ssl_enabled,
        }
    };

    Json(ApiResponse::ok(result))
}

/// DELETE /domains/:id - Delete a domain
pub async fn remove(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid domain ID")),
    };

    // Get domain info before deleting
    let domain_info = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        let tld = config.tld.clone();
        config
            .domains
            .iter()
            .find(|d| d.id == uuid)
            .map(|d| d.full_domain(&tld))
    };

    // Delete from config
    {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        if let Err(e) = config_store.delete_domain(uuid) {
            return Json(ApiResponse::err(e));
        }
    }

    // Unregister from proxy
    if let Some(full_domain) = domain_info {
        let proxy = state.inner.proxy_server.lock().await;
        let _ = proxy.unregister_route(&full_domain);
    }

    Json(ApiResponse::success())
}

/// POST /domains/:id/ssl - Toggle SSL for a domain
pub async fn toggle_ssl(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<ToggleSslRequest>,
) -> Json<ApiResponse<DomainInfo>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid domain ID")),
    };

    let result = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        let tld = config.tld.clone();

        let domain = match config_store.update_domain_ssl(uuid, req.ssl_enabled) {
            Ok(d) => d,
            Err(e) => return Json(ApiResponse::err(e)),
        };

        let (target_type, target_value) = match &domain.target {
            DomainTarget::Instance(id) => ("instance".to_string(), id.to_string()),
            DomainTarget::Port(p) => ("port".to_string(), p.to_string()),
            DomainTarget::StaticFiles { path, .. } => ("static".to_string(), path.clone()),
        };

        let full_domain = domain.full_domain(&tld);
        DomainInfo {
            id: domain.id.to_string(),
            subdomain: domain.subdomain,
            full_domain,
            target_type,
            target_value,
            ssl_enabled: domain.ssl_enabled,
        }
    };

    Json(ApiResponse::ok(result))
}
