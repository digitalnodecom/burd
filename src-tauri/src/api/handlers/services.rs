//! Service API handlers

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;

use crate::api::{state::ApiState, types::ApiResponse};
use crate::binary::VersionInfo;
use crate::commands::parse_service_type;
use crate::service_config::ServiceRegistry;
use crate::validation::validate_version;

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

/// GET /services/:service_type/available - List all downloadable versions (from upstream catalog)
pub async fn get_available(
    State(state): State<ApiState>,
    Path(service_type): Path<String>,
) -> Json<ApiResponse<Vec<VersionInfo>>> {
    let svc_type = match parse_service_type(&service_type) {
        Ok(t) => t,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let bm = match state.inner.binary_manager.lock() {
        Ok(b) => b.clone(),
        Err(_) => return Json(ApiResponse::err("Failed to acquire binary manager lock")),
    };
    match bm.get_available_versions(svc_type).await {
        Ok(v) => Json(ApiResponse::ok(v)),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

/// POST /services/:service_type/versions/:version - Download and install a binary version.
pub async fn download_version(
    State(state): State<ApiState>,
    Path((service_type, version)): Path<(String, String)>,
) -> Json<ApiResponse<()>> {
    // Reject anything that isn't a real version string before it reaches the
    // filesystem/URL layer. `version` is otherwise joined into a bin path and
    // interpolated into the upstream release URL, so a traversal value like
    // `..%2F..%2F..` would escape both.
    if let Err(e) = validate_version(&version) {
        return Json(ApiResponse::err(e.to_string()));
    }
    let svc_type = match parse_service_type(&service_type) {
        Ok(t) => t,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let app_handle = match state.app_handle.clone() {
        Some(h) => h,
        None => return Json(ApiResponse::err("App handle not available (running detached?)")),
    };
    let bm = match state.inner.binary_manager.lock() {
        Ok(b) => b.clone(),
        Err(_) => return Json(ApiResponse::err("Failed to acquire binary manager lock")),
    };
    let info = match bm.download(svc_type, &version, app_handle).await {
        Ok(i) => i,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let cs = match state.inner.config_store.lock() {
        Ok(c) => c,
        Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
    };
    if let Err(e) = cs.update_binary_info(svc_type, info) {
        return Json(ApiResponse::err(e));
    }
    Json(ApiResponse::success())
}

/// DELETE /services/:service_type/versions/:version - Delete an installed version.
pub async fn delete_version(
    State(state): State<ApiState>,
    Path((service_type, version)): Path<(String, String)>,
) -> Json<ApiResponse<()>> {
    // Guard the traversal: `version` is joined into the bin dir and handed to
    // remove_dir_all, so `..%2F..` would delete arbitrary directories.
    if let Err(e) = validate_version(&version) {
        return Json(ApiResponse::err(e.to_string()));
    }
    let svc_type = match parse_service_type(&service_type) {
        Ok(t) => t,
        Err(e) => return Json(ApiResponse::err(e)),
    };

    {
        let cs = match state.inner.config_store.lock() {
            Ok(c) => c,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let cfg = match cs.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(e.to_string())),
        };
        for instance in &cfg.instances {
            if instance.service_type == svc_type && instance.version == version {
                return Json(ApiResponse::err(format!(
                    "Cannot delete version {} - instance '{}' is using it",
                    version, instance.name
                )));
            }
        }
    }

    {
        let bm = match state.inner.binary_manager.lock() {
            Ok(b) => b,
            Err(_) => return Json(ApiResponse::err("Failed to acquire binary manager lock")),
        };
        if let Err(e) = bm.delete_version(svc_type, &version) {
            return Json(ApiResponse::err(e));
        }
    }

    let cs = match state.inner.config_store.lock() {
        Ok(c) => c,
        Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
    };
    if let Err(e) = cs.remove_binary_version(svc_type, &version) {
        return Json(ApiResponse::err(e));
    }
    Json(ApiResponse::success())
}
