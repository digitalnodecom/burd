//! Stack API handlers

use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api::handlers::instances as instance_handlers;
use crate::api::{state::ApiState, types::ApiResponse};

#[derive(Serialize)]
pub struct StackSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub instance_count: usize,
    pub running_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct StackInstanceSummary {
    pub id: String,
    pub name: String,
    pub service_type: String,
    pub running: bool,
}

#[derive(Serialize)]
pub struct StackDetail {
    #[serde(flatten)]
    pub stack: StackSummary,
    pub instances: Vec<StackInstanceSummary>,
}

#[derive(Deserialize)]
pub struct CreateStackRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub instance_ids: Vec<String>,
}

#[derive(Deserialize)]
pub struct UpdateStackRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

#[derive(Deserialize, Default)]
pub struct DeleteStackQuery {
    #[serde(default)]
    pub delete_instances: bool,
}

fn build_summary(state: &ApiState, stack_id: Uuid) -> Result<StackSummary, String> {
    let cs = state
        .inner
        .config_store
        .lock()
        .map_err(|_| "Failed to acquire config lock".to_string())?;
    let cfg = cs.load().map_err(|e| e.to_string())?;
    let stack = cfg
        .stacks
        .iter()
        .find(|s| s.id == stack_id)
        .ok_or_else(|| format!("Stack {} not found", stack_id))?;
    let pm = state
        .inner
        .process_manager
        .lock()
        .map_err(|_| "Failed to acquire process manager lock".to_string())?;
    let mut count = 0usize;
    let mut running = 0usize;
    for i in &cfg.instances {
        if i.stack_id == Some(stack_id) {
            count += 1;
            if pm.is_running(&i.id) {
                running += 1;
            }
        }
    }
    Ok(StackSummary {
        id: stack.id.to_string(),
        name: stack.name.clone(),
        description: stack.description.clone(),
        instance_count: count,
        running_count: running,
        created_at: stack.created_at.to_rfc3339(),
        updated_at: stack.updated_at.to_rfc3339(),
    })
}

pub async fn list(State(state): State<ApiState>) -> Json<ApiResponse<Vec<StackSummary>>> {
    let stack_ids: Vec<Uuid> = match state.inner.config_store.lock() {
        Ok(cs) => match cs.load() {
            Ok(cfg) => cfg.stacks.iter().map(|s| s.id).collect(),
            Err(e) => return Json(ApiResponse::err(e.to_string())),
        },
        Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
    };
    let mut out = Vec::new();
    for id in stack_ids {
        match build_summary(&state, id) {
            Ok(s) => out.push(s),
            Err(e) => return Json(ApiResponse::err(e)),
        }
    }
    Json(ApiResponse::ok(out))
}

pub async fn get(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<StackDetail>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid stack ID")),
    };
    let summary = match build_summary(&state, uuid) {
        Ok(s) => s,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let instances = {
        let cs = match state.inner.config_store.lock() {
            Ok(c) => c,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let cfg = match cs.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(e.to_string())),
        };
        let pm = match state.inner.process_manager.lock() {
            Ok(p) => p,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };
        cfg.instances
            .iter()
            .filter(|i| i.stack_id == Some(uuid))
            .map(|i| StackInstanceSummary {
                id: i.id.to_string(),
                name: i.name.clone(),
                service_type: i.service_type.as_str().to_string(),
                running: pm.is_running(&i.id),
            })
            .collect::<Vec<_>>()
    };
    Json(ApiResponse::ok(StackDetail {
        stack: summary,
        instances,
    }))
}

pub async fn create(
    State(state): State<ApiState>,
    Json(req): Json<CreateStackRequest>,
) -> Json<ApiResponse<StackSummary>> {
    let instance_uuids: Vec<Uuid> = match req
        .instance_ids
        .iter()
        .map(|s| Uuid::parse_str(s))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(e) => return Json(ApiResponse::err(format!("Invalid instance ID: {}", e))),
    };
    let stack_id = {
        let cs = match state.inner.config_store.lock() {
            Ok(c) => c,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        match cs.create_stack(req.name, req.description, instance_uuids) {
            Ok(s) => s.id,
            Err(e) => return Json(ApiResponse::err(e)),
        }
    };
    match build_summary(&state, stack_id) {
        Ok(s) => Json(ApiResponse::ok(s)),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

pub async fn update(
    State(state): State<ApiState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateStackRequest>,
) -> Json<ApiResponse<StackSummary>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid stack ID")),
    };
    {
        let cs = match state.inner.config_store.lock() {
            Ok(c) => c,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        if let Err(e) = cs.update_stack(uuid, req.name, req.description) {
            return Json(ApiResponse::err(e));
        }
    }
    match build_summary(&state, uuid) {
        Ok(s) => Json(ApiResponse::ok(s)),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

pub async fn remove(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<()>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid stack ID")),
    };
    let cs = match state.inner.config_store.lock() {
        Ok(c) => c,
        Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
    };
    // Default: keep instances; agent can call delete_instance separately.
    match cs.delete_stack(uuid, false) {
        Ok(_) => Json(ApiResponse::success()),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

fn instance_ids_in_stack(state: &ApiState, stack_id: Uuid) -> Result<Vec<Uuid>, String> {
    let cs = state
        .inner
        .config_store
        .lock()
        .map_err(|_| "Failed to acquire config lock".to_string())?;
    let cfg = cs.load().map_err(|e| e.to_string())?;
    Ok(cfg
        .instances
        .iter()
        .filter(|i| i.stack_id == Some(stack_id))
        .map(|i| i.id)
        .collect())
}

#[derive(Serialize)]
pub struct StackActionResult {
    pub started: Vec<String>,
    pub stopped: Vec<String>,
    pub errors: Vec<String>,
}

pub async fn start_stack(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<StackActionResult>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid stack ID")),
    };
    let ids = match instance_ids_in_stack(&state, uuid) {
        Ok(v) => v,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let mut result = StackActionResult {
        started: vec![],
        stopped: vec![],
        errors: vec![],
    };
    for iid in ids {
        let resp = instance_handlers::start(
            axum::extract::State(state.clone()),
            axum::extract::Path(iid.to_string()),
        )
        .await;
        let Json(api) = resp;
        if api.success {
            result.started.push(iid.to_string());
        } else {
            result
                .errors
                .push(format!("{}: {}", iid, api.error.unwrap_or_default()));
        }
    }
    Json(ApiResponse::ok(result))
}

pub async fn stop_stack(
    State(state): State<ApiState>,
    Path(id): Path<String>,
) -> Json<ApiResponse<StackActionResult>> {
    let uuid = match Uuid::parse_str(&id) {
        Ok(u) => u,
        Err(_) => return Json(ApiResponse::err("Invalid stack ID")),
    };
    let ids = match instance_ids_in_stack(&state, uuid) {
        Ok(v) => v,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let mut result = StackActionResult {
        started: vec![],
        stopped: vec![],
        errors: vec![],
    };
    for iid in ids {
        let resp = instance_handlers::stop(
            axum::extract::State(state.clone()),
            axum::extract::Path(iid.to_string()),
        )
        .await;
        let Json(api) = resp;
        if api.success {
            result.stopped.push(iid.to_string());
        } else {
            result
                .errors
                .push(format!("{}: {}", iid, api.error.unwrap_or_default()));
        }
    }
    Json(ApiResponse::ok(result))
}
