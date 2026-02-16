//! HTTP Reverse Proxy for routing custom TLD domains to service ports
//!
//! This module provides an HTTP reverse proxy that routes requests based on
//! the Host header to the appropriate backend service port.
//!
//! When the privileged proxy daemon (Caddy) is installed, this module syncs
//! routes to a Caddyfile that Caddy watches for changes.

use crate::caddy;
use crate::domain::DEFAULT_PROXY_PORT;
use crate::launchd;
use axum::{
    body::Body,
    extract::State,
    http::{header::HeaderName, HeaderValue, Request, StatusCode, Uri},
    response::Response,
    routing::any,
    Router,
};
use hyper_util::client::legacy::connect::HttpConnector;
use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use tokio::sync::oneshot;

type HttpClient = Client<HttpConnector, Body>;

/// Type of route for the proxy
#[derive(Debug, Clone)]
pub enum ProxyRouteType {
    /// Reverse proxy to a local port
    ReverseProxy { port: u16 },
    /// Serve static files (handled by Caddy, not in-memory proxy)
    FileServer { path: String, browse: bool },
}

/// Route mapping from domain to backend
#[derive(Debug, Clone)]
pub struct RouteEntry {
    pub domain: String,
    pub route_type: ProxyRouteType,
    pub instance_id: String,
    /// Whether SSL/HTTPS is enabled for this route
    pub ssl_enabled: bool,
}

impl RouteEntry {
    /// Get the port for reverse proxy routes, None for file server routes
    pub fn port(&self) -> Option<u16> {
        match &self.route_type {
            ProxyRouteType::ReverseProxy { port } => Some(*port),
            ProxyRouteType::FileServer { .. } => None,
        }
    }
}

/// Shared state for the proxy
#[derive(Clone)]
struct ProxyState {
    /// Map of domain (without TLD) to backend port
    routes: Arc<RwLock<HashMap<String, RouteEntry>>>,
    /// HTTP client for proxying requests
    client: HttpClient,
    /// The TLD to look for (e.g., "burd")
    tld: String,
}

/// Reverse proxy server
pub struct ProxyServer {
    port: u16,
    tld: String,
    routes: Arc<RwLock<HashMap<String, RouteEntry>>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    running: bool,
}

impl ProxyServer {
    pub fn new(port: u16, tld: String) -> Self {
        Self {
            port,
            tld,
            routes: Arc::new(RwLock::new(HashMap::new())),
            shutdown_tx: None,
            running: false,
        }
    }

    /// Get the TLD this proxy is configured for
    pub fn tld(&self) -> &str {
        &self.tld
    }

    /// Start the proxy server
    pub async fn start(&mut self) -> Result<(), String> {
        if self.running {
            return Ok(());
        }

        let addr = SocketAddr::from(([127, 0, 0, 1], self.port));

        // Create HTTP client for proxying
        let client: HttpClient = Client::builder(TokioExecutor::new())
            .build_http();

        let state = ProxyState {
            routes: Arc::clone(&self.routes),
            client,
            tld: self.tld.clone(),
        };

        let app = Router::new()
            .route("/", any(proxy_handler))
            .route("/*path", any(proxy_handler))
            .with_state(state);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| format!("Failed to bind proxy to {}: {}", addr, e))?;

        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        // Spawn the server
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .ok();
        });

        self.shutdown_tx = Some(shutdown_tx);
        self.running = true;

        Ok(())
    }

    /// Stop the proxy server
    pub fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        self.running = false;
    }

    /// Check if the proxy is running
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Get the port
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Register a reverse proxy route from domain to port
    pub fn register_route(&self, domain: &str, port: u16, instance_id: &str, ssl_enabled: bool) -> Result<(), String> {
        // Extract just the subdomain part (without TLD)
        let subdomain = domain
            .strip_suffix(&format!(".{}", self.tld))
            .unwrap_or(domain)
            .to_string();

        {
            let mut routes = self.routes.write()
                .map_err(|_| "Failed to acquire routes lock")?;

            routes.insert(subdomain.clone(), RouteEntry {
                domain: domain.to_string(),
                route_type: ProxyRouteType::ReverseProxy { port },
                instance_id: instance_id.to_string(),
                ssl_enabled,
            });
        }

        // Sync to daemon (ignore errors - daemon might not be installed)
        let _ = self.sync_to_daemon();

        Ok(())
    }

    /// Register a static file server route
    pub fn register_static_route(&self, domain: &str, path: &str, browse: bool, instance_id: &str, ssl_enabled: bool) -> Result<(), String> {
        // Extract just the subdomain part (without TLD)
        let subdomain = domain
            .strip_suffix(&format!(".{}", self.tld))
            .unwrap_or(domain)
            .to_string();

        {
            let mut routes = self.routes.write()
                .map_err(|_| "Failed to acquire routes lock")?;

            routes.insert(subdomain.clone(), RouteEntry {
                domain: domain.to_string(),
                route_type: ProxyRouteType::FileServer {
                    path: path.to_string(),
                    browse
                },
                instance_id: instance_id.to_string(),
                ssl_enabled,
            });
        }

        // Sync to daemon (ignore errors - daemon might not be installed)
        let _ = self.sync_to_daemon();

        Ok(())
    }

    /// Unregister a route
    pub fn unregister_route(&self, domain: &str) -> Result<(), String> {
        let subdomain = domain
            .strip_suffix(&format!(".{}", self.tld))
            .unwrap_or(domain)
            .to_string();

        {
            let mut routes = self.routes.write()
                .map_err(|_| "Failed to acquire routes lock")?;

            routes.remove(&subdomain);
        }

        // Sync to daemon (ignore errors - daemon might not be installed)
        let _ = self.sync_to_daemon();

        Ok(())
    }

    /// Get all registered routes
    pub fn list_routes(&self) -> Vec<RouteEntry> {
        self.routes
            .read()
            .map(|routes| routes.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Sync routes to the Caddyfile for the privileged proxy daemon
    ///
    /// This should be called whenever routes change so Caddy
    /// (running on ports 80/443) can pick up the changes.
    pub fn sync_to_daemon(&self) -> Result<(), String> {
        // Only sync if daemon is installed
        if !launchd::is_installed() {
            return Ok(());
        }

        let routes: Vec<caddy::RouteEntry> = self.routes
            .read()
            .map_err(|_| "Failed to read routes")?
            .values()
            .map(|r| match &r.route_type {
                ProxyRouteType::ReverseProxy { port } => {
                    caddy::RouteEntry::reverse_proxy(
                        r.domain.clone(),
                        *port,
                        r.instance_id.clone(),
                        r.ssl_enabled,
                    )
                }
                ProxyRouteType::FileServer { path, browse } => {
                    caddy::RouteEntry::file_server(
                        r.domain.clone(),
                        path.clone(),
                        *browse,
                        r.instance_id.clone(),
                        r.ssl_enabled,
                    )
                }
            })
            .collect();

        // Write Caddyfile - Caddy will auto-reload when file changes
        caddy::write_caddyfile(&self.tld, &routes)?;

        Ok(())
    }
}

impl Default for ProxyServer {
    fn default() -> Self {
        Self::new(DEFAULT_PROXY_PORT, crate::domain::DEFAULT_TLD.to_string())
    }
}

impl Drop for ProxyServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Handle incoming proxy requests
async fn proxy_handler(
    State(state): State<ProxyState>,
    req: Request<Body>,
) -> Response {
    // Extract host from request (clone it since we'll move req later)
    let host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .to_string();

    // Parse host to get subdomain (remove port if present)
    let host_without_port = host.split(':').next().unwrap_or(&host);

    // Extract subdomain (part before .tld)
    let subdomain = host_without_port
        .strip_suffix(&format!(".{}", state.tld))
        .unwrap_or(host_without_port);

    // Look up the route
    let route = {
        let routes = match state.routes.read() {
            Ok(r) => r,
            Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Route lookup failed"),
        };
        routes.get(subdomain).cloned()
    };

    let route = match route {
        Some(r) => r,
        None => return error_response(
            StatusCode::NOT_FOUND,
            &format!("No service registered for domain: {}", host_without_port)
        ),
    };

    // Handle based on route type
    let port = match &route.route_type {
        ProxyRouteType::ReverseProxy { port } => *port,
        ProxyRouteType::FileServer { .. } => {
            // File server routes are handled by Caddy daemon, not this in-memory proxy
            // This only happens if someone accesses the internal proxy port directly
            return error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "This domain serves static files via the Caddy daemon (port 80). Please use http://domain.burd instead."
            );
        }
    };

    // Build the proxied request URL
    let path_and_query = req.uri().path_and_query()
        .map(|pq| pq.as_str())
        .unwrap_or("/");

    let target_uri = format!("http://127.0.0.1:{}{}", port, path_and_query);

    let uri: Uri = match target_uri.parse() {
        Ok(u) => u,
        Err(_) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, "Invalid target URI"),
    };

    // Create the proxied request
    let (mut parts, body) = req.into_parts();
    parts.uri = uri;

    // Add standard reverse proxy headers so backends know the original request info
    if let Ok(host_value) = HeaderValue::from_str(&host) {
        parts.headers.insert(
            HeaderName::from_static("x-forwarded-host"),
            host_value,
        );
    }
    parts.headers.insert(
        HeaderName::from_static("x-forwarded-proto"),
        HeaderValue::from_static("http"),
    );

    // Keep the original Host header - don't remove it!
    // The backend needs to know which domain was requested

    let proxy_req = Request::from_parts(parts, body);

    // Forward the request
    match state.client.request(proxy_req).await {
        Ok(resp) => {
            let (parts, body) = resp.into_parts();
            Response::from_parts(parts, Body::new(body))
        }
        Err(e) => error_response(
            StatusCode::BAD_GATEWAY,
            &format!("Failed to connect to backend service: {}", e)
        ),
    }
}

fn error_response(status: StatusCode, message: &str) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(Body::from(message.to_string()))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_registration() {
        let proxy = ProxyServer::new(18080, "burd".to_string());

        proxy.register_route("my-api.burd", 7700, "test-id", false).unwrap();

        let routes = proxy.list_routes();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].port(), Some(7700));
        assert!(!routes[0].ssl_enabled);

        proxy.unregister_route("my-api.burd").unwrap();
        assert!(proxy.list_routes().is_empty());
    }

    #[test]
    fn test_route_registration_with_ssl() {
        let proxy = ProxyServer::new(18080, "burd".to_string());

        proxy.register_route("my-api.burd", 7700, "test-id", true).unwrap();

        let routes = proxy.list_routes();
        assert_eq!(routes.len(), 1);
        assert!(routes[0].ssl_enabled);

        proxy.unregister_route("my-api.burd").unwrap();
    }

    #[test]
    fn test_static_route_registration() {
        let proxy = ProxyServer::new(18080, "burd".to_string());

        proxy.register_static_route("static.burd", "/var/www/html", true, "test-static", false).unwrap();

        let routes = proxy.list_routes();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].port(), None); // File server routes don't have a port
        assert!(!routes[0].ssl_enabled);
        match &routes[0].route_type {
            ProxyRouteType::FileServer { path, browse } => {
                assert_eq!(path, "/var/www/html");
                assert!(*browse);
            }
            _ => panic!("Expected FileServer route"),
        }

        proxy.unregister_route("static.burd").unwrap();
        assert!(proxy.list_routes().is_empty());
    }
}
