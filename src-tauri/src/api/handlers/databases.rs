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
use crate::config::{Instance, ServiceType};
use crate::db_manager::{create_manager_for_instance, sanitize_db_name};

/// Collect the running MariaDB/PostgreSQL instances (the ones with a manager).
/// Shared by the database and user listing endpoints.
fn running_db_instances(state: &ApiState) -> Result<Vec<Instance>, String> {
    let config_store = state
        .inner
        .config_store
        .lock()
        .map_err(|_| "Failed to acquire config lock".to_string())?;
    let process_manager = state
        .inner
        .process_manager
        .lock()
        .map_err(|_| "Failed to acquire process manager lock".to_string())?;
    let config = config_store
        .load()
        .map_err(|e| format!("Failed to load config: {}", e))?;
    Ok(config
        .instances
        .into_iter()
        .filter(|i| {
            (i.service_type == ServiceType::MariaDB || i.service_type == ServiceType::PostgreSQL)
                && process_manager.get_status(i).running
        })
        .collect())
}

/// Human-readable service label, or None for non-DB types.
fn db_service_label(service_type: ServiceType) -> Option<&'static str> {
    match service_type {
        ServiceType::MariaDB => Some("MariaDB"),
        ServiceType::PostgreSQL => Some("PostgreSQL"),
        _ => None,
    }
}

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
    let instances = match running_db_instances(&state) {
        Ok(i) => i,
        Err(e) => return Json(ApiResponse::err(e)),
    };

    let mut all_databases = Vec::new();

    for instance in instances {
        let manager = match create_manager_for_instance(&instance) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let Some(service_type) = db_service_label(instance.service_type) else {
            continue;
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

/// Database user/role response
#[derive(Debug, Serialize)]
pub struct DbUserInfo {
    pub name: String,
    pub host: Option<String>,
    pub is_superuser: bool,
    pub can_login: bool,
    pub instance_id: String,
    pub instance_name: String,
    pub service_type: String,
}

/// GET /database-users - List users/roles across all running DB instances
pub async fn list_users(State(state): State<ApiState>) -> Json<ApiResponse<Vec<DbUserInfo>>> {
    let instances = match running_db_instances(&state) {
        Ok(i) => i,
        Err(e) => return Json(ApiResponse::err(e)),
    };

    let mut all_users = Vec::new();

    for instance in instances {
        let manager = match create_manager_for_instance(&instance) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let Some(service_type) = db_service_label(instance.service_type) else {
            continue;
        };

        if let Ok(users) = manager.list_users() {
            for u in users {
                all_users.push(DbUserInfo {
                    name: u.name,
                    host: u.host,
                    is_superuser: u.is_superuser,
                    can_login: u.can_login,
                    instance_id: instance.id.to_string(),
                    instance_name: instance.name.clone(),
                    service_type: service_type.to_string(),
                });
            }
        }
    }

    Json(ApiResponse::ok(all_users))
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

// === PostgreSQL extensions ===

/// Find a running PostgreSQL instance and build its manager.
fn postgres_manager(
    state: &ApiState,
) -> Result<Box<dyn crate::db_manager::DatabaseManager>, String> {
    let config_store = state
        .inner
        .config_store
        .lock()
        .map_err(|_| "Failed to acquire config lock".to_string())?;
    let process_manager = state
        .inner
        .process_manager
        .lock()
        .map_err(|_| "Failed to acquire process manager lock".to_string())?;
    let config = config_store.load().map_err(|e| e.to_string())?;
    let instance = config
        .instances
        .iter()
        .find(|i| {
            i.service_type == ServiceType::PostgreSQL && process_manager.get_status(i).running
        })
        .cloned()
        .ok_or_else(|| "No running PostgreSQL instance found".to_string())?;
    create_manager_for_instance(&instance)
}

/// GET /databases/{name}/extensions - list extensions and their install state.
pub async fn list_extensions(
    State(state): State<ApiState>,
    Path(name): Path<String>,
) -> Json<ApiResponse<Vec<crate::db_manager::ExtensionInfo>>> {
    let db = match sanitize_db_name(&name) {
        Ok(n) => n,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let manager = match postgres_manager(&state) {
        Ok(m) => m,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    match manager.list_extensions(&db) {
        Ok(exts) => Json(ApiResponse::ok(exts)),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

/// POST /databases/{name}/extensions/{extension} - enable an extension.
pub async fn enable_extension(
    State(state): State<ApiState>,
    Path((name, extension)): Path<(String, String)>,
) -> Json<ApiResponse<()>> {
    let db = match sanitize_db_name(&name) {
        Ok(n) => n,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let manager = match postgres_manager(&state) {
        Ok(m) => m,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    match manager.enable_extension(&db, &extension) {
        Ok(()) => Json(ApiResponse::success()),
        Err(e) => Json(ApiResponse::err(e)),
    }
}

/// DELETE /databases/{name}/extensions/{extension} - disable an extension.
pub async fn disable_extension(
    State(state): State<ApiState>,
    Path((name, extension)): Path<(String, String)>,
) -> Json<ApiResponse<()>> {
    let db = match sanitize_db_name(&name) {
        Ok(n) => n,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    let manager = match postgres_manager(&state) {
        Ok(m) => m,
        Err(e) => return Json(ApiResponse::err(e)),
    };
    match manager.disable_extension(&db, &extension) {
        Ok(()) => Json(ApiResponse::success()),
        Err(e) => Json(ApiResponse::err(e)),
    }
}
