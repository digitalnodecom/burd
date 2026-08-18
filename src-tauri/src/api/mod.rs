//! HTTP API for external control of Burd
//!
//! Provides a REST API on localhost:19840 for programmatic control.
//! Used by the MCP CLI to expose Burd functionality to Claude and other AI agents.

pub mod handlers;
pub mod state;
pub mod types;

use axum::{
    extract::Request,
    http::StatusCode,
    middleware::{self, Next},
    response::Response,
    routing::{delete, get, post, put},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;

use crate::commands::AppState;
use state::ApiState;

/// Default port for the API server
pub const API_PORT: u16 = 19840;

/// Is `host` (the value of a Host or the authority of an Origin header) a
/// loopback name? Port is ignored; only the hostname is checked. Handles
/// bracketed IPv6 (`[::1]:port`) as well as `host:port` and bare forms.
fn is_loopback_host(host: &str) -> bool {
    let hostname = if let Some(rest) = host.strip_prefix('[') {
        // [ipv6] or [ipv6]:port
        rest.split(']').next().unwrap_or(rest)
    } else if host.matches(':').count() == 1 {
        // host:port (IPv4 or name) — strip the single port colon
        host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host)
    } else {
        // bare IPv6 (multiple colons, no brackets) or a plain hostname
        host
    };
    matches!(hostname, "127.0.0.1" | "localhost" | "::1")
}

/// Guard every request against DNS-rebinding and cross-site (CSRF) access.
///
/// The API has no auth and binds to loopback, so its whole security model rests
/// on "only local, first-party callers reach it." Two browser-driven ways to
/// defeat that, both closed here:
///
/// * **DNS rebinding** — an attacker page on `evil.com` rebinds the name to
///   `127.0.0.1`; the browser then treats `evil.com:19840` as same-origin and
///   CORS stops applying. We reject any request whose `Host` is not loopback,
///   so the rebound name is turned away.
/// * **CSRF** — a plain cross-site `POST` (no preflight) carries an `Origin`.
///   We reject any request whose `Origin` is present and non-loopback.
///
/// The first-party CLI/MCP client sends neither header against a loopback Host,
/// so it passes untouched.
async fn local_only_guard(req: Request, next: Next) -> Result<Response, StatusCode> {
    let headers = req.headers();

    if let Some(host) = headers
        .get(axum::http::header::HOST)
        .and_then(|h| h.to_str().ok())
    {
        if !is_loopback_host(host) {
            return Err(StatusCode::FORBIDDEN);
        }
    }

    if let Some(origin) = headers
        .get(axum::http::header::ORIGIN)
        .and_then(|h| h.to_str().ok())
    {
        let authority = origin.split_once("://").map(|(_, a)| a).unwrap_or(origin);
        if !is_loopback_host(authority) {
            return Err(StatusCode::FORBIDDEN);
        }
    }

    Ok(next.run(req).await)
}

/// Create the API router with all routes
pub fn create_router(app_state: Arc<AppState>) -> Router {
    create_router_with_state(ApiState::new(app_state))
}

/// Create the API router from a pre-built ApiState (allows passing AppHandle).
pub fn create_router_with_state(api_state: ApiState) -> Router {
    Router::new()
        // Status
        .route("/status", get(handlers::status::get_status))
        // Instances
        .route("/instances", get(handlers::instances::list))
        .route("/instances", post(handlers::instances::create))
        .route("/instances/{id}", get(handlers::instances::get))
        .route("/instances/{id}", put(handlers::instances::update))
        .route("/instances/{id}", delete(handlers::instances::remove))
        .route("/instances/{id}/start", post(handlers::instances::start))
        .route("/instances/{id}/stop", post(handlers::instances::stop))
        .route(
            "/instances/{id}/restart",
            post(handlers::instances::restart),
        )
        .route("/instances/{id}/logs", get(handlers::instances::logs))
        .route("/instances/{id}/env", get(handlers::instances::env))
        .route("/instances/{id}/open", post(handlers::instances::open))
        .route(
            "/instances/{id}/validate",
            get(handlers::instances::validate),
        )
        // Domains
        .route("/domains", get(handlers::domains::list))
        .route("/domains", post(handlers::domains::create))
        .route("/domains/{id}", put(handlers::domains::update))
        .route("/domains/{id}", delete(handlers::domains::remove))
        .route("/domains/{id}/ssl", post(handlers::domains::toggle_ssl))
        // Databases
        .route("/databases", get(handlers::databases::list))
        .route("/databases", post(handlers::databases::create))
        .route("/database-users", get(handlers::databases::list_users))
        .route("/databases/{name}", delete(handlers::databases::drop))
        .route(
            "/databases/{name}/extensions",
            get(handlers::databases::list_extensions),
        )
        .route(
            "/databases/{name}/extensions/{extension}",
            post(handlers::databases::enable_extension),
        )
        .route(
            "/databases/{name}/extensions/{extension}",
            delete(handlers::databases::disable_extension),
        )
        // Mail (Mailpit)
        .route("/mail/config", get(handlers::mail::config))
        .route("/mail/unread-count", get(handlers::mail::unread_count))
        .route("/mail/messages", get(handlers::mail::list))
        .route("/mail/messages", delete(handlers::mail::delete_all))
        .route("/mail/messages/read", post(handlers::mail::mark_read))
        .route("/mail/messages/{id}", get(handlers::mail::get))
        .route("/mail/messages/{id}", delete(handlers::mail::delete_one))
        // Services
        .route("/services", get(handlers::services::list))
        .route(
            "/services/{service_type}/versions",
            get(handlers::services::get_versions),
        )
        .route(
            "/services/{service_type}/available",
            get(handlers::services::get_available),
        )
        .route(
            "/services/{service_type}/versions/{version}",
            post(handlers::services::download_version),
        )
        .route(
            "/services/{service_type}/versions/{version}",
            delete(handlers::services::delete_version),
        )
        // PHP CLI (PVM — PHP version manager)
        .route("/php/status", get(handlers::php::status))
        .route("/php/versions", get(handlers::php::list_versions))
        .route(
            "/php/versions/available",
            get(handlers::php::available_versions),
        )
        .route(
            "/php/versions/{version}",
            post(handlers::php::install_version),
        )
        .route(
            "/php/versions/{version}",
            delete(handlers::php::uninstall_version),
        )
        .route("/php/default/{version}", post(handlers::php::set_default))
        .route("/php/shell", get(handlers::php::shell_status))
        .route("/php/shell", post(handlers::php::configure_shell))
        // Proxy
        .route("/proxy/status", get(handlers::proxy::status))
        .route("/proxy/restart", post(handlers::proxy::restart))
        .route("/proxy/conflicts", get(handlers::proxy::conflicts))
        // Parks
        .route("/parks", get(handlers::parks::list))
        .route("/parks/projects", get(handlers::parks::list_projects))
        // Tunnels
        .route("/tunnels", get(handlers::tunnels::list))
        .route("/tunnels/start", post(handlers::tunnels::start))
        .route("/tunnels/stop", post(handlers::tunnels::stop))
        .route("/tunnels/status", get(handlers::tunnels::status))
        // Stacks
        .route("/stacks", get(handlers::stacks::list))
        .route("/stacks", post(handlers::stacks::create))
        .route("/stacks/{id}", get(handlers::stacks::get))
        .route("/stacks/{id}", put(handlers::stacks::update))
        .route("/stacks/{id}", delete(handlers::stacks::remove))
        .route("/stacks/{id}/start", post(handlers::stacks::start_stack))
        .route("/stacks/{id}/stop", post(handlers::stacks::stop_stack))
        // Logs
        .route("/logs/sources", get(handlers::logs::sources))
        .route("/logs", get(handlers::logs::recent))
        .layer(middleware::from_fn(local_only_guard))
        .with_state(api_state)
}

/// Start the API server on localhost:19840
pub async fn start_server(app_state: Arc<AppState>) -> Result<(), String> {
    start_server_with_state(ApiState::new(app_state)).await
}

pub async fn start_server_with_state(api_state: ApiState) -> Result<(), String> {
    let router = create_router_with_state(api_state);
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

#[cfg(test)]
mod guard_tests {
    use super::{is_loopback_host, local_only_guard};
    use axum::{body::Body, http::Request, middleware, routing::get, Router};
    use tower::ServiceExt;

    fn guarded_router() -> Router {
        Router::new()
            .route("/status", get(|| async { "ok" }))
            .layer(middleware::from_fn(local_only_guard))
    }

    async fn status_of(req: Request<Body>) -> u16 {
        guarded_router()
            .oneshot(req)
            .await
            .unwrap()
            .status()
            .as_u16()
    }

    #[tokio::test]
    async fn first_party_loopback_request_passes() {
        // CLI/MCP shape: loopback Host, no Origin.
        let req = Request::builder()
            .uri("/status")
            .header("host", "127.0.0.1:19840")
            .body(Body::empty())
            .unwrap();
        assert_eq!(status_of(req).await, 200);
    }

    #[tokio::test]
    async fn dns_rebinding_host_is_blocked() {
        let req = Request::builder()
            .uri("/status")
            .header("host", "evil.com:19840")
            .body(Body::empty())
            .unwrap();
        assert_eq!(status_of(req).await, 403);
    }

    #[tokio::test]
    async fn cross_site_origin_is_blocked() {
        // CSRF shape: loopback Host but a foreign Origin.
        let req = Request::builder()
            .uri("/status")
            .header("host", "127.0.0.1:19840")
            .header("origin", "https://evil.com")
            .body(Body::empty())
            .unwrap();
        assert_eq!(status_of(req).await, 403);
    }

    #[tokio::test]
    async fn same_origin_loopback_is_allowed() {
        let req = Request::builder()
            .uri("/status")
            .header("host", "127.0.0.1:19840")
            .header("origin", "http://localhost:19840")
            .body(Body::empty())
            .unwrap();
        assert_eq!(status_of(req).await, 200);
    }

    #[test]
    fn accepts_loopback_hosts_any_port() {
        for h in [
            "127.0.0.1",
            "127.0.0.1:19840",
            "localhost",
            "localhost:19840",
            "::1",
            "[::1]:19840",
        ] {
            assert!(is_loopback_host(h), "{} should be loopback", h);
        }
    }

    #[test]
    fn rejects_rebinding_and_external_hosts() {
        for h in [
            "evil.com",
            "evil.com:19840",
            "burd.dev",
            "127.0.0.1.evil.com",
            "notlocalhost",
            "0.0.0.0:19840",
        ] {
            assert!(!is_loopback_host(h), "{} must be rejected", h);
        }
    }
}
