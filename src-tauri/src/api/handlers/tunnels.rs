//! Tunnel API handlers (list / start / stop / status)

use axum::{extract::State, Json};
use serde::Serialize;

use crate::api::{state::ApiState, types::ApiResponse};
use crate::config::{ServiceType, SubdomainConfig, TunnelTarget};
use crate::tunnel::{FrpcAdminConfig, FrpcManager};

#[derive(Serialize)]
pub struct TunnelSummary {
    pub id: String,
    pub name: String,
    pub server_id: String,
    pub server_name: Option<String>,
    pub target_type: String,
    pub target_value: String,
    pub running: bool,
    pub public_url: Option<String>,
}

pub async fn list(State(state): State<ApiState>) -> Json<ApiResponse<Vec<TunnelSummary>>> {
    let (tunnels, servers) = {
        let cs = match state.inner.config_store.lock() {
            Ok(c) => c,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let cfg = match cs.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(e.to_string())),
        };
        (cfg.tunnels.clone(), cfg.frp_servers.clone())
    };
    let manager = match FrpcManager::new() {
        Ok(m) => m,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let running = manager.get_running_tunnel_ids();
    let summaries = tunnels
        .iter()
        .map(|t| {
            let server = servers.iter().find(|s| s.id == t.server_id);
            let is_running = running.contains(&t.id);
            let public_url = if is_running {
                server.map(|s| {
                    let sub = match &t.subdomain {
                        SubdomainConfig::Random { generated } => {
                            generated.clone().unwrap_or_default()
                        }
                        SubdomainConfig::Custom { subdomain } => subdomain.clone(),
                    };
                    format!("https://{}.{}", sub, s.subdomain_host)
                })
            } else {
                None
            };
            let (target_type, target_value) = match &t.target {
                TunnelTarget::Instance(id) => ("instance".to_string(), id.to_string()),
                TunnelTarget::Port(p) => ("port".to_string(), p.to_string()),
            };
            TunnelSummary {
                id: t.id.to_string(),
                name: t.name.clone(),
                server_id: t.server_id.to_string(),
                server_name: server.map(|s| s.name.clone()),
                target_type,
                target_value,
                running: is_running,
                public_url,
            }
        })
        .collect();
    Json(ApiResponse::ok(summaries))
}

pub async fn start(State(state): State<ApiState>) -> Json<ApiResponse<()>> {
    let (tunnels, servers, instances, admin_config) = {
        let cs = match state.inner.config_store.lock() {
            Ok(c) => c,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let cfg = match cs.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(e.to_string())),
        };
        let admin = cfg
            .instances
            .iter()
            .find(|i| i.service_type == ServiceType::Frpc)
            .map(|i| {
                let user = i
                    .config
                    .get("admin_user")
                    .and_then(|v| v.as_str())
                    .unwrap_or("admin")
                    .to_string();
                let password = i
                    .config
                    .get("admin_password")
                    .and_then(|v| v.as_str())
                    .unwrap_or("admin")
                    .to_string();
                FrpcAdminConfig {
                    port: i.port,
                    user,
                    password,
                }
            });
        (
            cfg.tunnels.clone(),
            cfg.frp_servers.clone(),
            cfg.instances.clone(),
            admin,
        )
    };

    let mut manager = match FrpcManager::new() {
        Ok(m) => m,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    match manager
        .start(&tunnels, &servers, &instances, admin_config.as_ref())
        .await
    {
        Ok(()) => Json(ApiResponse::success()),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

pub async fn stop() -> Json<ApiResponse<()>> {
    let mut manager = match FrpcManager::new() {
        Ok(m) => m,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    match manager.stop() {
        Ok(()) => Json(ApiResponse::success()),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

pub async fn status() -> Json<ApiResponse<serde_json::Value>> {
    let manager = match FrpcManager::new() {
        Ok(m) => m,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    Json(ApiResponse::ok(
        serde_json::to_value(manager.get_status()).unwrap_or_default(),
    ))
}
