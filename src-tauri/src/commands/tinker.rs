//! Tinker console commands
//!
//! Provides Tauri commands for the PHP Tinker console feature.
//! Lists projects from FrankenPHP instances and executes PHP code.

use crate::config::ServiceType;
use crate::tinker::{
    self, detect_project_type, execute_tinker as do_execute, ProjectType, TinkerExecution,
};
use serde::Serialize;
use tauri::State;

use super::AppState;

// ============================================================================
// Types
// ============================================================================

/// Project info returned to frontend
#[derive(Debug, Clone, Serialize)]
pub struct TinkerProjectInfo {
    pub id: String,
    pub path: String,
    pub project_type: ProjectType,
    pub name: String,
    pub instance_name: String,
}

// ============================================================================
// Commands
// ============================================================================

/// List all projects available for tinker
/// Extracts projects from FrankenPHP instances that have a document_root configured
#[tauri::command]
pub fn list_tinker_projects(state: State<'_, AppState>) -> Result<Vec<TinkerProjectInfo>, String> {
    let config_store = state
        .config_store
        .lock()
        .map_err(|_| "Failed to lock config store")?;
    let config = config_store.load().map_err(|e| e.to_string())?;

    let mut projects = Vec::new();

    for instance in &config.instances {
        // Only FrankenPHP instances have document_root
        if instance.service_type != ServiceType::FrankenPHP {
            continue;
        }

        // Get document_root from instance config
        let doc_root = match instance.config.get("document_root") {
            Some(serde_json::Value::String(path)) => path.clone(),
            _ => continue, // Skip instances without document_root
        };

        // Skip if the document_root doesn't exist
        if !std::path::Path::new(&doc_root).exists() {
            continue;
        }

        let project_type = detect_project_type(&doc_root);

        projects.push(TinkerProjectInfo {
            id: instance.id.to_string(),
            path: doc_root,
            project_type,
            name: instance.name.clone(),
            instance_name: instance.name.clone(),
        });
    }

    // Sort by name
    projects.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(projects)
}

/// Execute PHP code in a project
#[tauri::command]
pub async fn execute_tinker(
    project_path: String,
    project_type: ProjectType,
    code: String,
    timeout_ms: Option<u64>,
    php_version: Option<String>,
) -> Result<TinkerExecution, String> {
    // Execute in a blocking task since it uses std::process::Command
    tokio::task::spawn_blocking(move || {
        do_execute(&project_path, project_type, &code, timeout_ms, php_version.as_deref())
    })
    .await
    .map_err(|e| format!("Task join error: {}", e))?
}

/// Get tinker execution history
#[tauri::command]
pub fn get_tinker_history() -> Result<Vec<TinkerExecution>, String> {
    tinker::load_history()
}

/// Clear all tinker history
#[tauri::command]
pub fn clear_tinker_history() -> Result<(), String> {
    tinker::clear_history()
}

/// Delete a specific history item
#[tauri::command]
pub fn delete_tinker_history_item(id: String) -> Result<(), String> {
    tinker::delete_history_item(&id)
}

/// Get the current PHP binary info that Tinker will use
#[tauri::command]
pub fn get_tinker_php_info() -> Result<TinkerPhpInfo, String> {
    use crate::pvm::{get_pvm_status, list_installed_versions};
    use crate::tinker::get_php_binary;

    let pvm_status = get_pvm_status();
    let installed = list_installed_versions().unwrap_or_default();

    // Get the actual PHP that Tinker will use
    let (version, source, path) = if let Ok(php_path) = get_php_binary() {
        let path_str = php_path.to_string_lossy().to_string();
        // Determine source and version from path
        if path_str.contains("Burd") {
            // Extract version from path like .../Burd/bin/php/8.4.16/php
            let version = php_path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string());
            (version, Some("Burd".to_string()), Some(path_str))
        } else {
            // Fallback - run php -v to get version
            let version = std::process::Command::new(&php_path)
                .arg("-v")
                .output()
                .ok()
                .and_then(|o| {
                    let output = String::from_utf8_lossy(&o.stdout);
                    regex::Regex::new(r"PHP (\d+\.\d+\.\d+)")
                        .ok()
                        .and_then(|re| re.captures(&output))
                        .and_then(|c| c.get(1))
                        .map(|m| m.as_str().to_string())
                });
            let source = crate::pvm::get_current_php().map(|p| p.source);
            (version, source, Some(path_str))
        }
    } else {
        (None, None, None)
    };

    Ok(TinkerPhpInfo {
        version,
        source,
        path,
        pvm_default: pvm_status.default_version,
        installed_versions: installed.into_iter().map(|v| v.version).collect(),
    })
}

/// PHP info for the tinker UI
#[derive(Debug, Serialize)]
pub struct TinkerPhpInfo {
    pub version: Option<String>,
    pub source: Option<String>,
    pub path: Option<String>,
    pub pvm_default: Option<String>,
    pub installed_versions: Vec<String>,
}
