//! PHP CLI version manager (PVM) API handlers
//!
//! Exposes Burd's PHP CLI version manager over the HTTP API so AI agents (via
//! MCP) can install, switch, and remove command-line PHP versions — the same
//! functionality the desktop GUI drives through Tauri commands.

use axum::{
    extract::{Path, State},
    Json,
};

use crate::api::{state::ApiState, types::ApiResponse};
use crate::pvm::{self, PHPVersion, PvmStatus, RemotePHPVersion, ShellIntegrationStatus};
use crate::validation::validate_version;

/// GET /php/status - Overall PVM status (installed count, default, current, shell).
pub async fn status(State(_state): State<ApiState>) -> Json<ApiResponse<PvmStatus>> {
    match tokio::task::spawn_blocking(pvm::get_pvm_status).await {
        Ok(s) => Json(ApiResponse::ok(s)),
        Err(e) => Json(ApiResponse::err(format!("Task error: {}", e))),
    }
}

/// GET /php/versions - Installed CLI PHP versions.
pub async fn list_versions(State(_state): State<ApiState>) -> Json<ApiResponse<Vec<PHPVersion>>> {
    match tokio::task::spawn_blocking(pvm::list_installed_versions).await {
        Ok(Ok(v)) => Json(ApiResponse::ok(v)),
        Ok(Err(e)) => Json(ApiResponse::err(e)),
        Err(e) => Json(ApiResponse::err(format!("Task error: {}", e))),
    }
}

/// GET /php/versions/available - Remote/installable CLI PHP versions.
pub async fn available_versions(
    State(_state): State<ApiState>,
) -> Json<ApiResponse<Vec<RemotePHPVersion>>> {
    match pvm::list_remote_versions().await {
        Ok(v) => Json(ApiResponse::ok(v)),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

/// POST /php/versions/{version} - Download and install a CLI PHP version.
pub async fn install_version(
    State(state): State<ApiState>,
    Path(version): Path<String>,
) -> Json<ApiResponse<()>> {
    // `version` is joined into the PVM bin path and interpolated into the
    // download URL — reject anything that isn't a real version string.
    if let Err(e) = validate_version(&version) {
        return Json(ApiResponse::err(e.to_string()));
    }
    let app_handle = match state.app_handle.clone() {
        Some(h) => h,
        None => {
            return Json(ApiResponse::err(
                "App handle not available (running detached?)",
            ))
        }
    };
    match pvm::download_version(&version, &app_handle).await {
        Ok(()) => Json(ApiResponse::success()),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

/// DELETE /php/versions/{version} - Uninstall a CLI PHP version.
pub async fn uninstall_version(
    State(_state): State<ApiState>,
    Path(version): Path<String>,
) -> Json<ApiResponse<()>> {
    // `version` is joined into the bin dir and handed to remove_dir_all — guard
    // the traversal before it reaches the filesystem.
    if let Err(e) = validate_version(&version) {
        return Json(ApiResponse::err(e.to_string()));
    }
    match tokio::task::spawn_blocking(move || pvm::delete_version(&version)).await {
        Ok(Ok(())) => Json(ApiResponse::success()),
        Ok(Err(e)) => Json(ApiResponse::err(e)),
        Err(e) => Json(ApiResponse::err(format!("Task error: {}", e))),
    }
}

/// POST /php/default/{version} - Switch the active CLI PHP version.
pub async fn set_default(
    State(_state): State<ApiState>,
    Path(version): Path<String>,
) -> Json<ApiResponse<()>> {
    if let Err(e) = validate_version(&version) {
        return Json(ApiResponse::err(e.to_string()));
    }
    match tokio::task::spawn_blocking(move || pvm::set_default_version(&version)).await {
        Ok(Ok(())) => Json(ApiResponse::success()),
        Ok(Err(e)) => Json(ApiResponse::err(e)),
        Err(e) => Json(ApiResponse::err(format!("Task error: {}", e))),
    }
}

/// GET /php/shell - Shell integration status (PATH configured, conflicts).
pub async fn shell_status(
    State(_state): State<ApiState>,
) -> Json<ApiResponse<ShellIntegrationStatus>> {
    match tokio::task::spawn_blocking(pvm::get_shell_integration_status).await {
        Ok(s) => Json(ApiResponse::ok(s)),
        Err(e) => Json(ApiResponse::err(format!("Task error: {}", e))),
    }
}

/// POST /php/shell - Configure shell integration (adds Burd PHP to PATH).
pub async fn configure_shell(State(_state): State<ApiState>) -> Json<ApiResponse<()>> {
    match tokio::task::spawn_blocking(pvm::configure_shell_integration).await {
        Ok(Ok(())) => Json(ApiResponse::success()),
        Ok(Err(e)) => Json(ApiResponse::err(e)),
        Err(e) => Json(ApiResponse::err(format!("Task error: {}", e))),
    }
}
