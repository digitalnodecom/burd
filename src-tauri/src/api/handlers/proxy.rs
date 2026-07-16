//! Proxy API handlers

use axum::{extract::State, Json};

use crate::api::{state::ApiState, types::ApiResponse};
use crate::commands::{get_proxy_port_conflicts, get_proxy_status, restart_proxy_daemon};

pub async fn status(State(state): State<ApiState>) -> Json<ApiResponse<serde_json::Value>> {
    // Re-implement the cached-health read because get_proxy_status takes
    // tauri::State which we don't have in the HTTP context.
    let daemon = crate::launchd::get_status();
    let proxy_healthy = if daemon.installed && daemon.running {
        match state.inner.proxy_healthy.load(std::sync::atomic::Ordering::Relaxed) {
            1 => Some(true),
            2 => Some(false),
            _ => None,
        }
    } else {
        None
    };
    let caddy_installed = {
        let bm = match state.inner.binary_manager.lock() {
            Ok(b) => b,
            Err(_) => return Json(ApiResponse::err("Failed to acquire binary manager lock")),
        };
        bm.get_installed_versions_sync(crate::config::ServiceType::Caddy)
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    };

    Json(ApiResponse::ok(serde_json::json!({
        "daemon_installed": daemon.installed,
        "daemon_running": daemon.running,
        "daemon_pid": daemon.pid,
        "caddy_installed": caddy_installed,
        "proxy_healthy": proxy_healthy,
    })))
}

pub async fn restart() -> Json<ApiResponse<()>> {
    match restart_proxy_daemon() {
        Ok(()) => Json(ApiResponse::success()),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

pub async fn conflicts() -> Json<ApiResponse<serde_json::Value>> {
    match get_proxy_port_conflicts().await {
        Ok(c) => Json(ApiResponse::ok(serde_json::to_value(c).unwrap_or_default())),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

// Suppress unused-import warning when get_proxy_status isn't directly called.
#[allow(dead_code)]
fn _unused() {
    let _ = get_proxy_status;
}
