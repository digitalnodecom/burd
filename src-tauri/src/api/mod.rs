//! HTTP API for external control of Burd
//!
//! Provides a REST API on localhost:19840 for programmatic control.
//! Used by the MCP CLI to expose Burd functionality to Claude and other AI agents.

pub mod handlers;
pub mod state;
pub mod types;

use axum::{
    routing::{get, post, put, delete},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::commands::AppState;
use state::ApiState;

/// Default port for the API server
pub const API_PORT: u16 = 19840;

/// Create the API router with all routes
pub fn create_router(app_state: Arc<AppState>) -> Router {
    let api_state = ApiState::new(app_state);

    Router::new()
        // Status
        .route("/status", get(handlers::status::get_status))

        // Instances
        .route("/instances", get(handlers::instances::list))
        .route("/instances", post(handlers::instances::create))
        .route("/instances/{id}", get(handlers::instances::get))
        .route("/instances/{id}", delete(handlers::instances::remove))
        .route("/instances/{id}/start", post(handlers::instances::start))
        .route("/instances/{id}/stop", post(handlers::instances::stop))
        .route("/instances/{id}/restart", post(handlers::instances::restart))
        .route("/instances/{id}/logs", get(handlers::instances::logs))
        .route("/instances/{id}/env", get(handlers::instances::env))

        // Domains
        .route("/domains", get(handlers::domains::list))
        .route("/domains", post(handlers::domains::create))
        .route("/domains/{id}", put(handlers::domains::update))
        .route("/domains/{id}", delete(handlers::domains::remove))
        .route("/domains/{id}/ssl", post(handlers::domains::toggle_ssl))

        // Databases
        .route("/databases", get(handlers::databases::list))
        .route("/databases", post(handlers::databases::create))
        .route("/databases/{name}", delete(handlers::databases::drop))

        // Services
        .route("/services", get(handlers::services::list))
        .route("/services/{service_type}/versions", get(handlers::services::get_versions))

        .with_state(api_state)
}

/// Start the API server on localhost:19840
pub async fn start_server(app_state: Arc<AppState>) -> Result<(), String> {
    let router = create_router(app_state);
    let addr = SocketAddr::from(([127, 0, 0, 1], API_PORT));

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|e| format!("Failed to bind API server to {}: {}", addr, e))?;

    println!("MCP API server listening on http://{}", addr);

    // Run server (this will block until shutdown)
    axum::serve(listener, router)
        .await
        .map_err(|e| format!("API server error: {}", e))?;

    Ok(())
}
