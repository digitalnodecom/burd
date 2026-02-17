//! Status API handler

use axum::{extract::State, Json};

use crate::api::{
    state::ApiState,
    types::{ApiResponse, StatusResponse},
};
use crate::launchd;

/// GET /status - Get overall system status
pub async fn get_status(State(state): State<ApiState>) -> Json<ApiResponse<StatusResponse>> {
    // Get config and count instances
    let (tld, instance_count, running_instances, dns_running) = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        let process_manager = match state.inner.process_manager.lock() {
            Ok(pm) => pm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };

        let running = config
            .instances
            .iter()
            .filter(|inst| process_manager.get_status(inst).running)
            .count();

        let dns_server = match state.inner.dns_server.lock() {
            Ok(dns) => dns,
            Err(_) => return Json(ApiResponse::err("Failed to acquire DNS server lock")),
        };

        (
            config.tld.clone(),
            config.instances.len(),
            running,
            dns_server.is_running(),
        )
    };

    // Check if proxy daemon is installed
    let proxy_installed = launchd::is_installed();

    Json(ApiResponse::ok(StatusResponse {
        app_running: true,
        dns_running,
        proxy_installed,
        tld,
        instance_count,
        running_instances,
    }))
}
