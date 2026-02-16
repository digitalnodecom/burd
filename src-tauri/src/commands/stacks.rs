//! Stack related commands
//!
//! Handles stack management for grouping instances and team sharing.

use crate::config::{
    Stack, StackExport, StackService, StackDomain, StackRequirements,
    StackImportPreview, MissingVersion, ImportConflict, ConflictResolution, ImportResult,
    Instance, Domain, DomainTarget,
};
use crate::error::LockExt;
use crate::lock;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use super::AppState;

// ============================================================================
// Types
// ============================================================================

/// Stack info for frontend
#[derive(Debug, Serialize)]
pub struct StackInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub instance_count: usize,
    pub created_at: String,
    pub updated_at: String,
}

/// Create stack request payload
#[derive(Debug, Deserialize)]
pub struct CreateStackRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub instance_ids: Vec<String>,
}

/// Update stack request payload
#[derive(Debug, Deserialize)]
pub struct UpdateStackRequest {
    pub name: Option<String>,
    pub description: Option<Option<String>>,
}

/// Export stack request payload
#[derive(Debug, Deserialize)]
pub struct ExportStackRequest {
    pub stack_id: String,
    #[serde(default = "default_true")]
    pub include_domains: bool,
    #[serde(default)]
    pub created_by: Option<String>,
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Stack CRUD Commands
// ============================================================================

/// List all stacks with their instance counts
#[tauri::command]
pub async fn list_stacks(state: State<'_, AppState>) -> Result<Vec<StackInfo>, String> {
    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;

    let stacks = config.stacks.iter().map(|stack| {
        let instance_count = config.instances.iter()
            .filter(|i| i.stack_id == Some(stack.id))
            .count();

        StackInfo {
            id: stack.id.to_string(),
            name: stack.name.clone(),
            description: stack.description.clone(),
            instance_count,
            created_at: stack.created_at.to_rfc3339(),
            updated_at: stack.updated_at.to_rfc3339(),
        }
    }).collect();

    Ok(stacks)
}

/// Get a specific stack
#[tauri::command]
pub async fn get_stack(id: String, state: State<'_, AppState>) -> Result<StackInfo, String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid stack ID")?;
    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;

    let stack = config.stacks.iter()
        .find(|s| s.id == uuid)
        .ok_or_else(|| format!("Stack {} not found", id))?;

    let instance_count = config.instances.iter()
        .filter(|i| i.stack_id == Some(uuid))
        .count();

    Ok(StackInfo {
        id: stack.id.to_string(),
        name: stack.name.clone(),
        description: stack.description.clone(),
        instance_count,
        created_at: stack.created_at.to_rfc3339(),
        updated_at: stack.updated_at.to_rfc3339(),
    })
}

/// Create a new stack from selected instances
#[tauri::command]
pub async fn create_stack(
    request: CreateStackRequest,
    state: State<'_, AppState>,
) -> Result<StackInfo, String> {
    let instance_ids: Vec<Uuid> = request.instance_ids
        .iter()
        .map(|id| Uuid::parse_str(id).map_err(|_| format!("Invalid instance ID: {}", id)))
        .collect::<Result<Vec<_>, _>>()?;

    let config_store = lock!(state.config_store)?;
    let stack = config_store.create_stack(request.name, request.description, instance_ids.clone())?;

    Ok(StackInfo {
        id: stack.id.to_string(),
        name: stack.name,
        description: stack.description,
        instance_count: instance_ids.len(),
        created_at: stack.created_at.to_rfc3339(),
        updated_at: stack.updated_at.to_rfc3339(),
    })
}

/// Update a stack's name and/or description
#[tauri::command]
pub async fn update_stack(
    id: String,
    request: UpdateStackRequest,
    state: State<'_, AppState>,
) -> Result<StackInfo, String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid stack ID")?;
    let config_store = lock!(state.config_store)?;

    let stack = config_store.update_stack(uuid, request.name, request.description)?;
    let instances = config_store.get_instances_in_stack(uuid)?;

    Ok(StackInfo {
        id: stack.id.to_string(),
        name: stack.name,
        description: stack.description,
        instance_count: instances.len(),
        created_at: stack.created_at.to_rfc3339(),
        updated_at: stack.updated_at.to_rfc3339(),
    })
}

/// Delete a stack
#[tauri::command]
pub async fn delete_stack(
    id: String,
    delete_instances: bool,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let uuid = Uuid::parse_str(&id).map_err(|_| "Invalid stack ID")?;
    let config_store = lock!(state.config_store)?;

    let deleted_instance_ids = config_store.delete_stack(uuid, delete_instances)?;

    Ok(deleted_instance_ids.iter().map(|id| id.to_string()).collect())
}

/// Add instances to a stack
#[tauri::command]
pub async fn add_instances_to_stack(
    stack_id: String,
    instance_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let stack_uuid = Uuid::parse_str(&stack_id).map_err(|_| "Invalid stack ID")?;
    let instance_uuids: Vec<Uuid> = instance_ids
        .iter()
        .map(|id| Uuid::parse_str(id).map_err(|_| format!("Invalid instance ID: {}", id)))
        .collect::<Result<Vec<_>, _>>()?;

    let config_store = lock!(state.config_store)?;
    config_store.add_instances_to_stack(stack_uuid, instance_uuids)
}

/// Remove instances from their stack (move to standalone)
#[tauri::command]
pub async fn remove_instances_from_stack(
    instance_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let instance_uuids: Vec<Uuid> = instance_ids
        .iter()
        .map(|id| Uuid::parse_str(id).map_err(|_| format!("Invalid instance ID: {}", id)))
        .collect::<Result<Vec<_>, _>>()?;

    let config_store = lock!(state.config_store)?;
    config_store.remove_instances_from_stack(instance_uuids)
}

/// Move an instance to a different stack (or to standalone)
#[tauri::command]
pub async fn move_instance_to_stack(
    instance_id: String,
    stack_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let instance_uuid = Uuid::parse_str(&instance_id).map_err(|_| "Invalid instance ID")?;
    let stack_uuid = match stack_id {
        Some(sid) => Some(Uuid::parse_str(&sid).map_err(|_| "Invalid stack ID")?),
        None => None,
    };

    let config_store = lock!(state.config_store)?;
    config_store.move_instance_to_stack(instance_uuid, stack_uuid)?;
    Ok(())
}

// ============================================================================
// Export Commands
// ============================================================================

/// Fields that should be stripped from config as secrets
const SECRET_FIELDS: &[&str] = &["password", "master_key", "api_key", "token", "secret"];

/// Strip secret fields from a config value
fn strip_secrets(config: &serde_json::Value) -> serde_json::Value {
    match config {
        serde_json::Value::Object(map) => {
            let mut new_map = serde_json::Map::new();
            for (key, value) in map {
                let key_lower = key.to_lowercase();
                if SECRET_FIELDS.iter().any(|s| key_lower.contains(s)) {
                    // Skip secret fields
                    continue;
                }
                new_map.insert(key.clone(), strip_secrets(value));
            }
            serde_json::Value::Object(new_map)
        }
        serde_json::Value::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(strip_secrets).collect())
        }
        other => other.clone(),
    }
}

/// Export a stack to JSON format for sharing
#[tauri::command]
pub async fn export_stack(
    request: ExportStackRequest,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let stack_uuid = Uuid::parse_str(&request.stack_id).map_err(|_| "Invalid stack ID")?;

    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;

    // Get the stack
    let stack = config.stacks.iter()
        .find(|s| s.id == stack_uuid)
        .ok_or_else(|| format!("Stack {} not found", request.stack_id))?;

    // Get instances in the stack
    let instances: Vec<&Instance> = config.instances.iter()
        .filter(|i| i.stack_id == Some(stack_uuid))
        .collect();

    // Build services list
    let services: Vec<StackService> = instances.iter().map(|instance| {
        StackService {
            ref_id: instance.id.to_string(),
            service_type: instance.service_type,
            version: instance.version.clone(),
            name: instance.name.clone(),
            port: instance.port,
            auto_start: instance.auto_start,
            config: strip_secrets(&instance.config),
        }
    }).collect();

    // Build domains list if requested
    let domains: Vec<StackDomain> = if request.include_domains {
        let instance_ids: Vec<Uuid> = instances.iter().map(|i| i.id).collect();
        config.domains.iter()
            .filter(|d| {
                if let DomainTarget::Instance(instance_id) = &d.target {
                    instance_ids.contains(instance_id)
                } else {
                    false
                }
            })
            .map(|d| {
                let target_ref = match &d.target {
                    DomainTarget::Instance(id) => id.to_string(),
                    _ => String::new(),
                };
                StackDomain {
                    subdomain: d.subdomain.clone(),
                    target_ref,
                    ssl_enabled: d.ssl_enabled,
                }
            })
            .collect()
    } else {
        Vec::new()
    };

    let export = StackExport {
        id: stack.id,
        name: stack.name.clone(),
        description: stack.description.clone(),
        schema_version: 1,
        created_by: request.created_by,
        created_at: stack.created_at,
        updated_at: Utc::now(),
        services,
        domains,
        requirements: StackRequirements {
            min_burd_version: Some(env!("CARGO_PKG_VERSION").to_string()),
        },
    };

    serde_json::to_string_pretty(&export)
        .map_err(|e| format!("Failed to serialize stack: {}", e))
}

// ============================================================================
// Import Commands
// ============================================================================

/// Preview a stack import - validates and detects conflicts
#[tauri::command]
pub async fn preview_stack_import(
    config_json: String,
    state: State<'_, AppState>,
) -> Result<StackImportPreview, String> {
    // Parse the import config
    let import: StackExport = serde_json::from_str(&config_json)
        .map_err(|e| format!("Invalid stack config: {}", e))?;

    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;

    // Check if stack with this ID already exists
    let existing_stack = config.stacks.iter()
        .find(|s| s.id == import.id)
        .cloned();

    // Check for missing versions
    let mut missing_versions: Vec<MissingVersion> = Vec::new();
    for service in &import.services {
        let has_version = config.binaries
            .get(&service.service_type)
            .map(|versions| versions.contains_key(&service.version))
            .unwrap_or(false);

        if !has_version && service.version != "system" {
            missing_versions.push(MissingVersion {
                service_type: service.service_type,
                version: service.version.clone(),
                download_size: None, // Could be fetched from version info
            });
        }
    }

    // Check for conflicts
    let mut conflicts: Vec<ImportConflict> = Vec::new();

    // Check for stack ID conflict
    if let Some(ref stack) = existing_stack {
        conflicts.push(ImportConflict::StackIdExists {
            existing_stack_name: stack.name.clone(),
        });
    }

    // Check for port and name conflicts
    for service in &import.services {
        // Port conflict
        if let Some(existing) = config.instances.iter().find(|i| i.port == service.port) {
            conflicts.push(ImportConflict::PortInUse {
                port: service.port,
                existing_instance_name: existing.name.clone(),
                new_service_ref: service.ref_id.clone(),
            });
        }

        // Name conflict
        if let Some(existing) = config.instances.iter().find(|i| i.name == service.name) {
            conflicts.push(ImportConflict::NameExists {
                name: service.name.clone(),
                existing_id: existing.id,
                new_service_ref: service.ref_id.clone(),
            });
        }
    }

    Ok(StackImportPreview {
        config: import,
        missing_versions,
        conflicts,
        existing_stack,
    })
}

/// Import a stack (after preview and conflict resolution)
#[tauri::command]
pub async fn import_stack(
    config_json: String,
    conflict_resolutions: Vec<ConflictResolution>,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    // Parse the import config
    let import: StackExport = serde_json::from_str(&config_json)
        .map_err(|e| format!("Invalid stack config: {}", e))?;

    let config_store = lock!(state.config_store)?;
    let mut config = config_store.load()?;

    // Build resolution maps
    let mut port_reassignments: std::collections::HashMap<String, u16> = std::collections::HashMap::new();
    let mut name_reassignments: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    let mut skipped_services: Vec<String> = Vec::new();
    let mut update_existing_stack = false;

    for resolution in conflict_resolutions {
        match resolution {
            ConflictResolution::ReassignPort { service_ref, new_port } => {
                port_reassignments.insert(service_ref, new_port);
            }
            ConflictResolution::RenameService { service_ref, new_name } => {
                name_reassignments.insert(service_ref, new_name);
            }
            ConflictResolution::Skip { service_ref } => {
                skipped_services.push(service_ref);
            }
            ConflictResolution::ReplaceExisting { service_ref } => {
                // Remove existing instance with same name
                if let Some(service) = import.services.iter().find(|s| s.ref_id == service_ref) {
                    config.instances.retain(|i| i.name != service.name && i.port != service.port);
                }
            }
            ConflictResolution::UpdateExistingStack => {
                update_existing_stack = true;
            }
        }
    }

    // Create or update the stack
    let stack = if update_existing_stack {
        // Update existing stack
        if let Some(existing) = config.stacks.iter_mut().find(|s| s.id == import.id) {
            existing.name = import.name.clone();
            existing.description = import.description.clone();
            existing.updated_at = Utc::now();
            existing.clone()
        } else {
            return Err("Existing stack not found for update".to_string());
        }
    } else {
        // Create new stack (with imported ID to support future updates)
        let stack = Stack {
            id: import.id,
            name: import.name.clone(),
            description: import.description.clone(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        config.stacks.push(stack.clone());
        stack
    };

    // Track results
    let mut instances_created: Vec<Uuid> = Vec::new();
    let mut instances_updated: Vec<Uuid> = Vec::new();
    let mut instances_skipped: Vec<String> = Vec::new();
    let mut domains_created: Vec<Uuid> = Vec::new();

    // Map of ref_id to new instance_id for domain creation
    let mut ref_to_instance: std::collections::HashMap<String, Uuid> = std::collections::HashMap::new();

    // Create instances
    for service in &import.services {
        if skipped_services.contains(&service.ref_id) {
            instances_skipped.push(service.ref_id.clone());
            continue;
        }

        let port = port_reassignments.get(&service.ref_id).copied().unwrap_or(service.port);
        let name = name_reassignments.get(&service.ref_id).cloned().unwrap_or_else(|| service.name.clone());

        // Check if we're updating an existing instance (same ID in ref_id is valid UUID)
        let existing_instance_id = Uuid::parse_str(&service.ref_id).ok();
        let existing_instance = existing_instance_id
            .and_then(|id| config.instances.iter_mut().find(|i| i.id == id && i.stack_id == Some(stack.id)));

        if let Some(instance) = existing_instance {
            // Update existing instance
            instance.name = name;
            instance.port = port;
            instance.version = service.version.clone();
            instance.config = service.config.clone();
            instance.auto_start = service.auto_start;
            instances_updated.push(instance.id);
            ref_to_instance.insert(service.ref_id.clone(), instance.id);
        } else {
            // Create new instance
            let instance = Instance {
                id: Uuid::new_v4(),
                name,
                port,
                service_type: service.service_type,
                version: service.version.clone(),
                config: service.config.clone(),
                master_key: None,
                auto_start: service.auto_start,
                created_at: Utc::now(),
                domain: None,
                domain_enabled: true,
                stack_id: Some(stack.id),
            };
            instances_created.push(instance.id);
            ref_to_instance.insert(service.ref_id.clone(), instance.id);
            config.instances.push(instance);
        }
    }

    // Create domains
    for domain in &import.domains {
        if let Some(&instance_id) = ref_to_instance.get(&domain.target_ref) {
            // Check if domain already exists
            if !config.domains.iter().any(|d| d.subdomain == domain.subdomain) {
                let new_domain = Domain::for_instance(
                    domain.subdomain.clone(),
                    instance_id,
                    domain.ssl_enabled,
                );
                domains_created.push(new_domain.id);
                config.domains.push(new_domain);
            }
        }
    }

    // Save the config
    config_store.save(&config)?;

    Ok(ImportResult {
        stack,
        instances_created,
        instances_updated,
        instances_skipped,
        domains_created,
    })
}
