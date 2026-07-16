//! Park API handlers (read-only — write ops require ParkWatcherState which is Tauri-only)

use axum::{extract::State, Json};
use serde::Serialize;

use crate::api::{state::ApiState, types::ApiResponse};
use crate::park;

#[derive(Serialize)]
pub struct ParkedDirInfo {
    pub id: String,
    pub path: String,
    pub ssl_enabled: bool,
    pub project_count: usize,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct ParkedProjectInfo {
    pub name: String,
    pub path: String,
    pub project_type: String,
    pub domain: String,
}

pub async fn list(State(state): State<ApiState>) -> Json<ApiResponse<Vec<ParkedDirInfo>>> {
    let cs = match state.inner.config_store.lock() {
        Ok(c) => c,
        Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
    };
    let dirs = match cs.list_parked_directories() {
        Ok(d) => d,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let mut out = Vec::new();
    for pd in dirs {
        let projects = park::scan_directory(std::path::Path::new(&pd.path)).unwrap_or_default();
        out.push(ParkedDirInfo {
            id: pd.id.to_string(),
            path: pd.path,
            ssl_enabled: pd.ssl_enabled,
            project_count: projects.len(),
            created_at: pd.created_at.to_rfc3339(),
        });
    }
    Json(ApiResponse::ok(out))
}

pub async fn list_projects(
    State(state): State<ApiState>,
) -> Json<ApiResponse<Vec<ParkedProjectInfo>>> {
    let (dirs, tld) = {
        let cs = match state.inner.config_store.lock() {
            Ok(c) => c,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let cfg = match cs.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(e.to_string())),
        };
        let dirs = match cs.list_parked_directories() {
            Ok(d) => d,
            Err(e) => return Json(ApiResponse::err(e)),
        };
        (dirs, cfg.tld.clone())
    };
    let mut out = Vec::new();
    for pd in dirs {
        let projects = park::scan_directory(std::path::Path::new(&pd.path)).unwrap_or_default();
        for p in projects {
            let subdomain = park::generate_subdomain(&p.name);
            out.push(ParkedProjectInfo {
                name: p.name,
                path: p.path.to_string_lossy().to_string(),
                project_type: p.project_type.as_str().to_string(),
                domain: format!("{}.{}", subdomain, tld),
            });
        }
    }
    Json(ApiResponse::ok(out))
}
