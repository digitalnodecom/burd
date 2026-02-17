//! Database API handlers

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::api::{
    state::ApiState,
    types::{ApiResponse, CreateDatabaseRequest},
};
use crate::config::ServiceType;
use crate::db_manager::{create_manager_for_instance, sanitize_db_name};

/// Database info response
#[derive(Debug, Serialize)]
pub struct DatabaseInfo {
    pub name: String,
    pub instance_id: String,
    pub instance_name: String,
    pub service_type: String,
    pub size: Option<u64>,
    pub tables: Option<u32>,
}

/// GET /databases - List all databases across all DB instances
pub async fn list(State(state): State<ApiState>) -> Json<ApiResponse<Vec<DatabaseInfo>>> {
    let instances = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let process_manager = match state.inner.process_manager.lock() {
            Ok(pm) => pm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        // Get all running database instances
        config
            .instances
            .into_iter()
            .filter(|i| {
                (i.service_type == ServiceType::MariaDB
                    || i.service_type == ServiceType::PostgreSQL)
                    && process_manager.get_status(i).running
            })
            .collect::<Vec<_>>()
    };

    let mut all_databases = Vec::new();

    for instance in instances {
        let manager = match create_manager_for_instance(&instance) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let service_type = match instance.service_type {
            ServiceType::MariaDB => "MariaDB",
            ServiceType::PostgreSQL => "PostgreSQL",
            _ => continue,
        };

        if let Ok(databases) = manager.list_databases() {
            for db in databases {
                all_databases.push(DatabaseInfo {
                    name: db.name,
                    instance_id: instance.id.to_string(),
                    instance_name: instance.name.clone(),
                    service_type: service_type.to_string(),
                    size: db.size,
                    tables: db.tables,
                });
            }
        }
    }

    Json(ApiResponse::ok(all_databases))
}

/// POST /databases - Create a new database
pub async fn create(
    State(state): State<ApiState>,
    Json(req): Json<CreateDatabaseRequest>,
) -> Json<ApiResponse<DatabaseInfo>> {
    // Sanitize database name
    let db_name = match sanitize_db_name(&req.name) {
        Ok(n) => n,
        Err(e) => return Json(ApiResponse::err(e)),
    };

    // Find the target instance
    let instance = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let process_manager = match state.inner.process_manager.lock() {
            Ok(pm) => pm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        // If instance_id is provided, use that specific instance
        if let Some(ref id_str) = req.instance_id {
            let uuid = match Uuid::parse_str(id_str) {
                Ok(u) => u,
                Err(_) => return Json(ApiResponse::err("Invalid instance ID")),
            };

            match config.instances.iter().find(|i| i.id == uuid).cloned() {
                Some(i) => {
                    if !process_manager.get_status(&i).running {
                        return Json(ApiResponse::err("Instance is not running"));
                    }
                    i
                }
                None => return Json(ApiResponse::err("Instance not found")),
            }
        } else {
            // Find first running database instance
            match config
                .instances
                .iter()
                .find(|i| {
                    (i.service_type == ServiceType::MariaDB
                        || i.service_type == ServiceType::PostgreSQL)
                        && process_manager.get_status(i).running
                })
                .cloned()
            {
                Some(i) => i,
                None => {
                    return Json(ApiResponse::err(
                        "No running database instance found. Please start a MariaDB or PostgreSQL instance first.",
                    ))
                }
            }
        }
    };

    // Create database manager and execute
    let manager = match create_manager_for_instance(&instance) {
        Ok(m) => m,
        Err(e) => return Json(ApiResponse::err(e)),
    };

    // Check if database already exists
    match manager.database_exists(&db_name) {
        Ok(true) => {
            return Json(ApiResponse::err(format!(
                "Database '{}' already exists",
                db_name
            )))
        }
        Err(e) => return Json(ApiResponse::err(e)),
        _ => {}
    }

    // Create the database
    if let Err(e) = manager.create_database(&db_name) {
        return Json(ApiResponse::err(e));
    }

    let service_type = match instance.service_type {
        ServiceType::MariaDB => "MariaDB",
        ServiceType::PostgreSQL => "PostgreSQL",
        _ => "Unknown",
    };

    Json(ApiResponse::ok(DatabaseInfo {
        name: db_name,
        instance_id: instance.id.to_string(),
        instance_name: instance.name,
        service_type: service_type.to_string(),
        size: None,
        tables: Some(0),
    }))
}

/// DELETE /databases/:name - Drop a database
pub async fn drop(
    State(state): State<ApiState>,
    Path(name): Path<String>,
) -> Json<ApiResponse<()>> {
    // Sanitize database name
    let db_name = match sanitize_db_name(&name) {
        Ok(n) => n,
        Err(e) => return Json(ApiResponse::err(e)),
    };

    // Find running database instances and try to drop from each
    let instances = {
        let config_store = match state.inner.config_store.lock() {
            Ok(cs) => cs,
            Err(_) => return Json(ApiResponse::err("Failed to acquire config lock")),
        };
        let process_manager = match state.inner.process_manager.lock() {
            Ok(pm) => pm,
            Err(_) => return Json(ApiResponse::err("Failed to acquire process manager lock")),
        };

        let config = match config_store.load() {
            Ok(c) => c,
            Err(e) => return Json(ApiResponse::err(format!("Failed to load config: {}", e))),
        };

        config
            .instances
            .into_iter()
            .filter(|i| {
                (i.service_type == ServiceType::MariaDB
                    || i.service_type == ServiceType::PostgreSQL)
                    && process_manager.get_status(i).running
            })
            .collect::<Vec<_>>()
    };

    // Try to find and drop the database
    for instance in instances {
        let manager = match create_manager_for_instance(&instance) {
            Ok(m) => m,
            Err(_) => continue,
        };

        // Check if this instance has the database
        match manager.database_exists(&db_name) {
            Ok(true) => {
                // Found it, drop it
                if let Err(e) = manager.drop_database(&db_name) {
                    return Json(ApiResponse::err(e));
                }
                return Json(ApiResponse::success());
            }
            _ => continue,
        }
    }

    Json(ApiResponse::err(format!(
        "Database '{}' not found in any running database instance",
        db_name
    )))
}
