//! Service API handlers

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::api::{state::ApiState, types::ApiResponse};
use crate::commands::parse_service_type;
use crate::service_config::ServiceRegistry;

/// Service info response
#[derive(Debug, Serialize)]
pub struct ServiceInfo {
    pub id: String,
    pub name: String,
    pub binary_name: String,
    pub default_port: u16,
    pub max_instances: Option<usize>,
    pub internal: bool,
    pub auto_create_domain: bool,
}

/// Service versions response
#[derive(Debug, Serialize)]
pub struct ServiceVersions {
    pub service_type: String,
    pub installed: Vec<String>,
}

/// GET /services - List all available service types
pub async fn list(State(_state): State<ApiState>) -> Json<ApiResponse<Vec<ServiceInfo>>> {
    let registry = ServiceRegistry::load();

    let services: Vec<ServiceInfo> = registry
        .all_services()
        .iter()
        .filter(|(_, config)| !config.internal) // Filter out internal services
        .map(|(id, config)| ServiceInfo {
            id: id.to_string(),
            name: config.display_name.clone(),
            binary_name: config.binary_name.clone(),
            default_port: config.default_port,
            max_instances: config.max_instances,
            internal: config.internal,
            auto_create_domain: config.auto_create_domain,
        })
        .collect();

    Json(ApiResponse::ok(services))
}

/// GET /services/:service_type/versions - Get installed versions for a service
pub async fn get_versions(
    State(state): State<ApiState>,
    Path(service_type): Path<String>,
) -> Json<ApiResponse<ServiceVersions>> {
    // Parse and validate service type
    let svc_type = match parse_service_type(&service_type) {
        Ok(t) => t,
        Err(e) => return Json(ApiResponse::err(e)),
    };

    let installed = {
        let binary_manager = match state.inner.binary_manager.lock() {
            Ok(bm) => bm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire binary manager lock")),
        };

        match binary_manager.get_installed_versions_sync(svc_type) {
            Ok(v) => v,
            Err(e) => return Json(ApiResponse::err(e)),
        }
    };

    Json(ApiResponse::ok(ServiceVersions {
        service_type,
        installed,
    }))
}
