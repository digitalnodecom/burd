//! Log Commands
//!
//! Tauri commands for log aggregation and streaming.
//! Supports Caddy proxy logs and per-instance process logs.

use crate::error::LockExt;
use crate::lock;
use crate::logs::{
    get_caddy_log_path, get_instance_log_path, get_last_lines, get_log_sources_with_instances,
    parse_caddy_json, parse_plain_text, read_new_lines, LogEntry, LogFileState, LogSourceInfo,
};
use std::time::Duration;
use tauri::ipc::Channel;
use tauri::State;

use super::AppState;

/// Get available log sources (Caddy + per-service-type from instances)
#[tauri::command]
pub fn get_available_log_sources(state: State<'_, AppState>) -> Result<Vec<LogSourceInfo>, String> {
    let config_store = lock!(state.config_store)?;
    let config = config_store.load()?;
    Ok(get_log_sources_with_instances(&config.instances))
}

/// Get recent logs from specified sources
#[tauri::command]
pub async fn get_recent_logs(
    sources: Vec<String>,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> Result<Vec<LogEntry>, String> {
    let limit = limit.unwrap_or(500);
    let mut all_logs: Vec<LogEntry> = Vec::new();

    // Load instances for non-caddy sources
    let instances = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;
        config.instances.clone()
    };

    let sources_empty = sources.is_empty();

    for source in &sources {
        if source == "caddy" {
            let path = get_caddy_log_path();
            if path.exists() {
                if let Ok(lines) = get_last_lines(path.to_str().unwrap_or(""), limit) {
                    for line in lines {
                        if let Some(entry) = parse_caddy_json(&line) {
                            all_logs.push(entry);
                        }
                    }
                }
            }
        } else {
            // Read logs from all instances of this service type
            for instance in &instances {
                let svc_type = instance.service_type.as_str();
                if svc_type == source {
                    if let Ok(log_path) = get_instance_log_path(&instance.id.to_string()) {
                        if log_path.exists() {
                            if let Ok(lines) =
                                get_last_lines(log_path.to_str().unwrap_or(""), limit)
                            {
                                for line in lines {
                                    let trimmed = line.trim();
                                    if !trimmed.is_empty() {
                                        let mut entry = parse_plain_text(
                                            trimmed,
                                            svc_type,
                                            Some(&instance.id.to_string()),
                                        );
                                        entry.domain =
                                            Some(instance.name.clone());
                                        all_logs.push(entry);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // If no sources specified, load from all
    if sources_empty {
        // Caddy
        let path = get_caddy_log_path();
        if path.exists() {
            if let Ok(lines) = get_last_lines(path.to_str().unwrap_or(""), limit) {
                for line in lines {
                    if let Some(entry) = parse_caddy_json(&line) {
                        all_logs.push(entry);
                    }
                }
            }
        }
        // All instances
        for instance in &instances {
            let svc_type = instance.service_type.as_str();
            if svc_type == "caddy" {
                continue;
            }
            if let Ok(log_path) = get_instance_log_path(&instance.id.to_string()) {
                if log_path.exists() {
                    if let Ok(lines) = get_last_lines(log_path.to_str().unwrap_or(""), limit) {
                        for line in lines {
                            let trimmed = line.trim();
                            if !trimmed.is_empty() {
                                let mut entry = parse_plain_text(
                                    trimmed,
                                    svc_type,
                                    Some(&instance.id.to_string()),
                                );
                                entry.domain = Some(instance.name.clone());
                                all_logs.push(entry);
                            }
                        }
                    }
                }
            }
        }
    }

    // Sort by timestamp (newest first)
    all_logs.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    // Limit results
    all_logs.truncate(limit);

    Ok(all_logs)
}

/// Stream logs in real-time via Channel
/// This command runs continuously, sending log entries as they appear
#[tauri::command]
pub async fn stream_logs(
    sources: Vec<String>,
    on_log: Channel<LogEntry>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    // Load instances for non-caddy sources
    let instances = {
        let config_store = lock!(state.config_store)?;
        let config = config_store.load()?;
        config.instances.clone()
    };

    let stream_all = sources.is_empty();
    let stream_caddy = stream_all || sources.contains(&"caddy".to_string());

    let mut file_state = LogFileState::new();

    // Initialize Caddy log position at end of file (only new logs)
    if stream_caddy {
        let caddy_path = get_caddy_log_path();
        if caddy_path.exists() {
            if let Ok(metadata) = std::fs::metadata(&caddy_path) {
                file_state.set_position(caddy_path.to_str().unwrap_or(""), metadata.len());
            }
        }
    }

    // Initialize instance log positions at end of file
    for instance in &instances {
        let svc_type = instance.service_type.as_str();
        if svc_type == "caddy" {
            continue;
        }
        if stream_all || sources.contains(&svc_type.to_string()) {
            if let Ok(log_path) = get_instance_log_path(&instance.id.to_string()) {
                if log_path.exists() {
                    if let Ok(metadata) = std::fs::metadata(&log_path) {
                        file_state
                            .set_position(log_path.to_str().unwrap_or(""), metadata.len());
                    }
                }
            }
        }
    }

    // Poll for new logs every 100ms
    loop {
        // Stream Caddy logs
        if stream_caddy {
            let path = get_caddy_log_path();
            if path.exists() {
                let path_str = path.to_str().unwrap_or("");
                if let Ok(lines) = read_new_lines(path_str, &mut file_state) {
                    for line in lines {
                        if let Some(entry) = parse_caddy_json(&line) {
                            if on_log.send(entry).is_err() {
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }

        // Stream instance logs
        for instance in &instances {
            let svc_type = instance.service_type.as_str();
            if svc_type == "caddy" {
                continue;
            }
            if stream_all || sources.contains(&svc_type.to_string()) {
                if let Ok(log_path) = get_instance_log_path(&instance.id.to_string()) {
                    if log_path.exists() {
                        let path_str = log_path.to_str().unwrap_or("");
                        if let Ok(lines) = read_new_lines(path_str, &mut file_state) {
                            for line in lines {
                                let trimmed = line.trim();
                                if !trimmed.is_empty() {
                                    let mut entry = parse_plain_text(
                                        trimmed,
                                        svc_type,
                                        Some(&instance.id.to_string()),
                                    );
                                    entry.domain = Some(instance.name.clone());
                                    if on_log.send(entry).is_err() {
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sleep before next poll
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Clear log file (for testing/maintenance)
#[tauri::command]
pub async fn clear_logs(source: String, state: State<'_, AppState>) -> Result<(), String> {
    match source.as_str() {
        "caddy" => {
            let path = get_caddy_log_path();
            if path.exists() {
                std::fs::write(&path, "")
                    .map_err(|e| format!("Failed to clear caddy logs: {}", e))?;
            }
            Ok(())
        }
        _ => {
            // Clear logs for all instances of this service type
            let instances = {
                let config_store = lock!(state.config_store)?;
                let config = config_store.load()?;
                config.instances.clone()
            };

            let mut cleared = false;
            for instance in &instances {
                if instance.service_type.as_str() == source {
                    if let Ok(log_path) = get_instance_log_path(&instance.id.to_string()) {
                        if log_path.exists() {
                            std::fs::write(&log_path, "").map_err(|e| {
                                format!("Failed to clear logs for {}: {}", instance.name, e)
                            })?;
                            cleared = true;
                        }
                    }
                }
            }

            if cleared {
                Ok(())
            } else {
                Err(format!("No log files found for source: {}", source))
            }
        }
    }
}
