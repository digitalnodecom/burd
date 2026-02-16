//! Log Commands
//!
//! Tauri commands for log aggregation and streaming.

use crate::logs::{
    get_caddy_log_path, get_last_lines, get_log_sources, parse_caddy_json,
    read_new_lines, LogEntry, LogFileState, LogSourceInfo,
};
use std::time::Duration;
use tauri::ipc::Channel;

/// Get available log sources
#[tauri::command]
pub fn get_available_log_sources() -> Vec<LogSourceInfo> {
    get_log_sources()
}

/// Get recent logs from specified sources
#[tauri::command]
pub async fn get_recent_logs(
    sources: Vec<String>,
    limit: Option<usize>,
) -> Result<Vec<LogEntry>, String> {
    let limit = limit.unwrap_or(500);
    let mut all_logs: Vec<LogEntry> = Vec::new();

    for source in sources {
        if source.as_str() == "caddy" {
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
        }
        // Add more sources as needed
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
) -> Result<(), String> {
    let mut file_state = LogFileState::new();

    // For Caddy, start at end of file to only get new logs
    let caddy_path = get_caddy_log_path();
    if caddy_path.exists() {
        if let Ok(metadata) = std::fs::metadata(&caddy_path) {
            file_state.set_position(
                caddy_path.to_str().unwrap_or(""),
                metadata.len(),
            );
        }
    }

    // Poll for new logs every 100ms
    loop {
        for source in &sources {
            match source.as_str() {
                "caddy" => {
                    let path = get_caddy_log_path();
                    if path.exists() {
                        let path_str = path.to_str().unwrap_or("");
                        if let Ok(lines) = read_new_lines(path_str, &mut file_state) {
                            for line in lines {
                                if let Some(entry) = parse_caddy_json(&line) {
                                    // Send to frontend via channel
                                    if on_log.send(entry).is_err() {
                                        // Channel closed, exit the loop
                                        return Ok(());
                                    }
                                }
                            }
                        }
                    }
                }
                "frankenphp" => {
                    // FrankenPHP stdout streaming would be handled differently
                    // (via process manager stdout capture)
                }
                _ => {}
            }
        }

        // Sleep before next poll
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

/// Clear log file (for testing/maintenance)
#[tauri::command]
pub async fn clear_logs(source: String) -> Result<(), String> {
    match source.as_str() {
        "caddy" => {
            let path = get_caddy_log_path();
            if path.exists() {
                std::fs::write(&path, "")
                    .map_err(|e| format!("Failed to clear caddy logs: {}", e))?;
            }
            Ok(())
        }
        _ => Err(format!("Cannot clear logs for source: {}", source)),
    }
}
