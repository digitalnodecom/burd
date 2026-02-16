//! Park commands
//!
//! Tauri commands for managing parked directories and projects.

use serde::Serialize;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use uuid::Uuid;

use crate::commands::AppState;
use crate::config::ParkedDirectory;
use crate::park::{self, DiscoveredProject, SyncResult};
use crate::park_watcher::ParkWatcherState;
use crate::validation;

/// Lock a mutex and return an error string if it fails
macro_rules! lock {
    ($mutex:expr) => {
        $mutex
            .lock()
            .map_err(|e| format!("Failed to acquire lock: {}", e))
    };
}

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
pub struct ParkedDirectoryInfo {
    pub id: String,
    pub path: String,
    pub ssl_enabled: bool,
    pub project_count: usize,
    pub conflicts: Vec<String>,
    pub created_at: String,
}

impl From<ParkedDirectory> for ParkedDirectoryInfo {
    fn from(pd: ParkedDirectory) -> Self {
        Self {
            id: pd.id.to_string(),
            path: pd.path,
            ssl_enabled: pd.ssl_enabled,
            project_count: 0, // Will be populated separately
            conflicts: Vec::new(),
            created_at: pd.created_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ParkedProjectInfo {
    pub name: String,
    pub path: String,
    pub project_type: String,
    pub domain: String,
    pub document_root: String,
    pub status: String, // "active", "conflict", "isolated", "error"
    pub isolated: bool,
    pub instance_id: Option<String>,
}

impl ParkedProjectInfo {
    fn from_discovered(project: &DiscoveredProject, tld: &str, status: &str) -> Self {
        let subdomain = park::generate_subdomain(&project.name);
        Self {
            name: project.name.clone(),
            path: project.path.to_string_lossy().to_string(),
            project_type: project.project_type.as_str().to_string(),
            domain: format!("{}.{}", subdomain, tld),
            document_root: project.document_root.to_string_lossy().to_string(),
            status: status.to_string(),
            isolated: false,
            instance_id: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SyncResultInfo {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub conflicts: Vec<String>,
    pub unchanged: usize,
    pub errors: Vec<String>,
}

impl From<SyncResult> for SyncResultInfo {
    fn from(r: SyncResult) -> Self {
        Self {
            added: r.added,
            removed: r.removed,
            conflicts: r.conflicts,
            unchanged: r.unchanged,
            errors: r.errors,
        }
    }
}

// ============================================================================
// Commands
// ============================================================================

/// Check if park feature is enabled (FrankenPHP Park instance exists)
#[tauri::command]
pub async fn is_park_enabled(state: State<'_, AppState>) -> Result<bool, String> {
    let config_store = lock!(state.config_store)?;
    config_store.is_park_enabled()
}

/// List all parked directories with project counts
#[tauri::command]
pub async fn list_parked_directories(
    state: State<'_, AppState>,
) -> Result<Vec<ParkedDirectoryInfo>, String> {
    let config_store = lock!(state.config_store)?;
    let parked_dirs = config_store.list_parked_directories()?;
    let config = config_store.load()?;

    let mut result = Vec::new();
    for pd in parked_dirs {
        let projects = park::scan_directory(std::path::Path::new(&pd.path)).unwrap_or_default();
        let project_count = projects.len();

        // Find conflicts
        let conflicts: Vec<String> = projects
            .iter()
            .filter_map(|p| {
                let subdomain = park::generate_subdomain(&p.name);
                let is_conflict = config.domains.iter().any(|d| {
                    d.subdomain == subdomain && d.parked_dir_id() != Some(pd.id)
                });
                if is_conflict {
                    Some(subdomain)
                } else {
                    None
                }
            })
            .collect();

        result.push(ParkedDirectoryInfo {
            id: pd.id.to_string(),
            path: pd.path,
            ssl_enabled: pd.ssl_enabled,
            project_count,
            conflicts,
            created_at: pd.created_at.to_rfc3339(),
        });
    }

    Ok(result)
}

/// Park a directory
#[tauri::command]
pub async fn park_directory(
    path: String,
    ssl_enabled: bool,
    app_handle: AppHandle,
    state: State<'_, AppState>,
    watcher_state: State<'_, ParkWatcherState>,
) -> Result<ParkedDirectoryInfo, String> {
    // Validate the directory path to prevent path traversal attacks
    validation::validate_directory_path(&path)
        .map_err(|e| format!("Invalid directory path: {}", e))?;

    // Phase 1: Verify park is enabled and create parked directory (config_store only)
    let (parked_dir, tld) = {
        let config_store = lock!(state.config_store)?;
        if !config_store.is_park_enabled()? {
            return Err("FrankenPHP Park is not enabled. Please create a FrankenPHP Park instance first.".to_string());
        }
        let parked_dir = config_store.create_parked_directory(path.clone(), ssl_enabled)?;
        let config = config_store.load()?;
        (parked_dir, config.tld.clone())
    };
    // config_store lock released here

    // Phase 2: Acquire proxy lock (async)
    let proxy = state.proxy_server.lock().await;

    // Phase 3: Re-acquire config_store and sync domains
    let sync_result = {
        let config_store = lock!(state.config_store)?;
        park::sync_parked_domains(&parked_dir, &config_store, &proxy, &tld)?
    };

    // Phase 4: Start file system watcher for this directory
    let _ = watcher_state.start_watching(
        parked_dir.id,
        PathBuf::from(&parked_dir.path),
        app_handle,
    );

    // Return info
    let projects = park::scan_directory(std::path::Path::new(&path)).unwrap_or_default();
    Ok(ParkedDirectoryInfo {
        id: parked_dir.id.to_string(),
        path: parked_dir.path,
        ssl_enabled: parked_dir.ssl_enabled,
        project_count: projects.len(),
        conflicts: sync_result.conflicts,
        created_at: parked_dir.created_at.to_rfc3339(),
    })
}

/// Unpark a directory (remove from parked directories and cleanup domains)
#[tauri::command]
pub async fn unpark_directory(
    id: String,
    state: State<'_, AppState>,
    watcher_state: State<'_, ParkWatcherState>,
) -> Result<(), String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("Invalid UUID: {}", e))?;

    // Phase 1: Stop the file system watcher for this directory
    let _ = watcher_state.stop_watching(uuid);

    // Phase 2: Delete domains and parked directory from config (config_store only)
    let (removed_domains, tld, park_instance) = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;
        let removed_domains = config_store.delete_domains_for_parked_directory(uuid)?;
        config_store.delete_parked_directory(uuid)?;
        let park_instance = config_store.get_park_instance()?;
        (removed_domains, config.tld.clone(), park_instance)
    };
    // config_store lock released here

    // Phase 3: Unregister routes from proxy (async)
    {
        let proxy = state.proxy_server.lock().await;
        for domain in removed_domains {
            let full_domain = domain.full_domain(&tld);
            let _ = proxy.unregister_route(&full_domain);
        }
    }

    // Phase 4: Regenerate park caddyfile
    if let Some(instance) = park_instance {
        let config_store = lock!(state.config_store)?;
        let _ = park::regenerate_park_caddyfile(&config_store, &instance, &tld);
    }

    Ok(())
}

/// Refresh/rescan a parked directory
#[tauri::command]
pub async fn refresh_parked_directory(
    id: String,
    state: State<'_, AppState>,
) -> Result<SyncResultInfo, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("Invalid UUID: {}", e))?;

    // Phase 1: Get parked directory and TLD (config_store only)
    let (parked_dir, tld) = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;
        let parked_dir = config_store.get_parked_directory(uuid)?;
        (parked_dir, config.tld.clone())
    };
    // config_store lock released here

    // Phase 2: Acquire proxy lock (async)
    let proxy = state.proxy_server.lock().await;

    // Phase 3: Re-acquire config_store and sync
    let result = {
        let config_store = lock!(state.config_store)?;
        park::sync_parked_domains(&parked_dir, &config_store, &proxy, &tld)?
    };

    Ok(result.into())
}

/// Refresh all parked directories
#[tauri::command]
pub async fn refresh_all_parked_directories(
    state: State<'_, AppState>,
) -> Result<Vec<SyncResultInfo>, String> {
    // Phase 1: Get all parked directories and TLD (config_store only)
    let (parked_dirs, tld) = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;
        let parked_dirs = config_store.list_parked_directories()?;
        (parked_dirs, config.tld.clone())
    };
    // config_store lock released here

    // Phase 2: Acquire proxy lock (async)
    let proxy = state.proxy_server.lock().await;

    // Phase 3: Re-acquire config_store and sync all directories
    let mut results = Vec::new();
    {
        let config_store = lock!(state.config_store)?;
        for parked_dir in parked_dirs {
            let result = park::sync_parked_domains(&parked_dir, &config_store, &proxy, &tld)?;
            results.push(result.into());
        }
    }

    Ok(results)
}

/// Get projects in a parked directory
#[tauri::command]
pub async fn get_parked_projects(
    id: String,
    state: State<'_, AppState>,
) -> Result<Vec<ParkedProjectInfo>, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("Invalid UUID: {}", e))?;

    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;

    let parked_dir = config_store.get_parked_directory(uuid)?;
    let projects = park::scan_directory(std::path::Path::new(&parked_dir.path))?;

    let mut result = Vec::new();
    for project in projects {
        let subdomain = park::generate_subdomain(&project.name);

        // Determine status
        let status = if let Some(domain) = config.domains.iter().find(|d| d.subdomain == subdomain) {
            if domain.is_isolated() {
                "isolated"
            } else if domain.parked_dir_id() == Some(parked_dir.id) {
                "active"
            } else {
                "conflict"
            }
        } else {
            // Check if there's a conflict with another domain
            if config.domains.iter().any(|d| d.subdomain == subdomain) {
                "conflict"
            } else {
                "pending"
            }
        };

        let mut info = ParkedProjectInfo::from_discovered(&project, &config.tld, status);

        // Check if isolated
        if let Some(domain) = config.domains.iter().find(|d| d.subdomain == subdomain && d.is_isolated()) {
            info.isolated = true;
            if let crate::config::DomainTarget::Instance(instance_id) = &domain.target {
                info.instance_id = Some(instance_id.to_string());
            }
        }

        result.push(info);
    }

    Ok(result)
}

/// Update SSL setting for a parked directory
#[tauri::command]
pub async fn update_parked_directory_ssl(
    id: String,
    ssl_enabled: bool,
    state: State<'_, AppState>,
) -> Result<ParkedDirectoryInfo, String> {
    let uuid = Uuid::parse_str(&id).map_err(|e| format!("Invalid UUID: {}", e))?;

    let config_store = lock!(state.config_store)?;
    let parked_dir = config_store.update_parked_directory_ssl(uuid, ssl_enabled)?;

    // Update SSL for all domains from this parked directory
    let domains = config_store.get_domains_for_parked_directory(uuid)?;

    for domain in domains {
        let _ = config_store.update_domain_ssl(domain.id, ssl_enabled);
    }

    let projects = park::scan_directory(std::path::Path::new(&parked_dir.path)).unwrap_or_default();

    Ok(ParkedDirectoryInfo {
        id: parked_dir.id.to_string(),
        path: parked_dir.path,
        ssl_enabled: parked_dir.ssl_enabled,
        project_count: projects.len(),
        conflicts: Vec::new(),
        created_at: parked_dir.created_at.to_rfc3339(),
    })
}
