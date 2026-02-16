//! Log Aggregation Module
//!
//! This module provides functionality for aggregating logs from multiple sources:
//! - Caddy reverse proxy (JSON access logs)
//! - FrankenPHP instances (stdout/stderr)
//! - Application logs (Laravel logs, etc.)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use uuid::Uuid;

use crate::caddy::get_logs_dir;

/// A single log entry from any source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique ID for frontend keying
    pub id: String,
    /// Source of the log: "caddy", "frankenphp", "php-app", etc.
    pub source: String,
    /// Instance ID if applicable (for FrankenPHP instances)
    pub instance_id: Option<String>,
    /// Unix timestamp in milliseconds
    pub timestamp: i64,
    /// Log level: "DEBUG", "INFO", "WARN", "ERROR"
    pub level: String,
    /// The log message
    pub message: String,
    /// Domain if applicable (extracted from request)
    pub domain: Option<String>,
    /// Request ID for correlation
    pub request_id: Option<String>,
    /// HTTP method if this is an access log
    pub method: Option<String>,
    /// HTTP path if this is an access log
    pub path: Option<String>,
    /// HTTP status code if this is an access log
    pub status: Option<u16>,
    /// Response time in milliseconds
    pub duration_ms: Option<f64>,
    /// Additional context (varies by source)
    pub context: serde_json::Value,
}

impl LogEntry {
    /// Create a new log entry with a generated ID
    #[allow(dead_code)]
    pub fn new(source: &str, level: &str, message: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            source: source.to_string(),
            instance_id: None,
            timestamp: Utc::now().timestamp_millis(),
            level: level.to_string(),
            message: message.to_string(),
            domain: None,
            request_id: None,
            method: None,
            path: None,
            status: None,
            duration_ms: None,
            context: serde_json::Value::Null,
        }
    }
}

/// Information about a log source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogSourceInfo {
    pub id: String,
    pub name: String,
    pub log_type: String, // "file", "stdout", "combined"
    pub path: Option<String>,
    pub color: String,
}

/// State for tracking file positions for tailing
#[derive(Default)]
pub struct LogFileState {
    /// Last read position for each file
    positions: HashMap<String, u64>,
}

impl LogFileState {
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    pub fn get_position(&self, path: &str) -> u64 {
        *self.positions.get(path).unwrap_or(&0)
    }

    pub fn set_position(&mut self, path: &str, pos: u64) {
        self.positions.insert(path.to_string(), pos);
    }
}

/// Parse a Caddy JSON log line into a LogEntry
pub fn parse_caddy_json(line: &str) -> Option<LogEntry> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;

    // Caddy JSON format:
    // {
    //   "level": "info",
    //   "ts": 1704067200.123,
    //   "logger": "http.log.access",
    //   "msg": "handled request",
    //   "request": { "remote_ip": "...", "method": "GET", "host": "...", "uri": "/..." },
    //   "resp": { "status": 200, "latency": 0.045 }
    // }

    let ts = json["ts"].as_f64().unwrap_or(0.0);
    let timestamp = (ts * 1000.0) as i64;

    let level = json["level"]
        .as_str()
        .unwrap_or("info")
        .to_uppercase();

    let msg = json["msg"].as_str().unwrap_or("").to_string();

    // Extract request details
    let request = &json["request"];
    let method = request["method"].as_str().map(|s| s.to_string());
    let host = request["host"].as_str().map(|s| s.to_string());
    let uri = request["uri"].as_str().unwrap_or("");
    let path = Some(uri.to_string());

    // Extract response details
    let resp = &json["resp"];
    let status = resp["status"].as_u64().map(|s| s as u16);
    let latency = resp["duration"].as_f64().or_else(|| resp["latency"].as_f64());
    let duration_ms = latency.map(|l| l * 1000.0);

    // Extract request ID if present
    let request_id = request["headers"]["X-Request-Id"]
        .as_array()
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Build message
    let message = if method.is_some() && status.is_some() {
        format!(
            "{} {} {} {:.2}ms",
            method.as_deref().unwrap_or("-"),
            uri,
            status.unwrap_or(0),
            duration_ms.unwrap_or(0.0)
        )
    } else {
        msg
    };

    Some(LogEntry {
        id: Uuid::new_v4().to_string(),
        source: "caddy".to_string(),
        instance_id: None,
        timestamp,
        level,
        message,
        domain: host,
        request_id,
        method,
        path,
        status,
        duration_ms,
        context: json.clone(),
    })
}

/// Parse a Laravel/Monolog JSON log line
#[allow(dead_code)]
pub fn parse_laravel_json(line: &str, instance_id: Option<&str>) -> Option<LogEntry> {
    let json: serde_json::Value = serde_json::from_str(line).ok()?;

    // Monolog JSON format:
    // {
    //   "message": "...",
    //   "context": {...},
    //   "level": 200,
    //   "level_name": "INFO",
    //   "channel": "app",
    //   "datetime": "2025-01-04T12:00:00.000000+00:00"
    // }

    let message = json["message"].as_str().unwrap_or("").to_string();
    let level = json["level_name"]
        .as_str()
        .unwrap_or("INFO")
        .to_uppercase();

    let datetime = json["datetime"].as_str().unwrap_or("");
    let timestamp = DateTime::parse_from_rfc3339(datetime)
        .map(|dt| dt.timestamp_millis())
        .unwrap_or_else(|_| Utc::now().timestamp_millis());

    let context = &json["context"];
    let request_id = context["request_id"]
        .as_str()
        .map(|s| s.to_string());

    Some(LogEntry {
        id: Uuid::new_v4().to_string(),
        source: "php-app".to_string(),
        instance_id: instance_id.map(|s| s.to_string()),
        timestamp,
        level,
        message,
        domain: None,
        request_id,
        method: None,
        path: None,
        status: None,
        duration_ms: None,
        context: json.clone(),
    })
}

/// Parse a plain text log line (fallback)
#[allow(dead_code)]
pub fn parse_plain_text(line: &str, source: &str, instance_id: Option<&str>) -> LogEntry {
    // Try to detect log level from common patterns
    let level = if line.contains("[ERROR]") || line.contains("ERROR:") || line.contains(" error ") {
        "ERROR"
    } else if line.contains("[WARN]") || line.contains("WARNING:") || line.contains(" warn ") {
        "WARN"
    } else if line.contains("[DEBUG]") || line.contains("DEBUG:") {
        "DEBUG"
    } else {
        "INFO"
    };

    LogEntry {
        id: Uuid::new_v4().to_string(),
        source: source.to_string(),
        instance_id: instance_id.map(|s| s.to_string()),
        timestamp: Utc::now().timestamp_millis(),
        level: level.to_string(),
        message: line.to_string(),
        domain: None,
        request_id: None,
        method: None,
        path: None,
        status: None,
        duration_ms: None,
        context: serde_json::Value::Null,
    }
}

/// Read new lines from a file since the last position
pub fn read_new_lines(
    path: &str,
    state: &mut LogFileState,
) -> Result<Vec<String>, String> {
    let file = File::open(path)
        .map_err(|e| format!("Failed to open log file {}: {}", path, e))?;

    let metadata = file.metadata()
        .map_err(|e| format!("Failed to get file metadata: {}", e))?;
    let file_size = metadata.len();

    let last_pos = state.get_position(path);

    // If file was truncated (rotated), start from beginning
    let start_pos = if file_size < last_pos {
        0
    } else {
        last_pos
    };

    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start_pos))
        .map_err(|e| format!("Failed to seek: {}", e))?;

    let mut lines = Vec::new();
    let mut current_pos = start_pos;

    loop {
        let mut line = String::new();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(n) => {
                current_pos += n as u64;
                let trimmed = line.trim_end();
                if !trimmed.is_empty() {
                    lines.push(trimmed.to_string());
                }
            }
            Err(_) => break,
        }
    }

    state.set_position(path, current_pos);
    Ok(lines)
}

/// Get the last N lines from a file (for initial load)
pub fn get_last_lines(path: &str, count: usize) -> Result<Vec<String>, String> {
    let file = File::open(path)
        .map_err(|e| format!("Failed to open log file {}: {}", path, e))?;

    let reader = BufReader::new(file);
    let all_lines: Vec<String> = reader
        .lines()
        .map_while(Result::ok)
        .collect();

    let start = if all_lines.len() > count {
        all_lines.len() - count
    } else {
        0
    };

    Ok(all_lines[start..].to_vec())
}

/// Get the path to the Caddy access log
pub fn get_caddy_log_path() -> PathBuf {
    get_logs_dir().join("caddy-access.json")
}

/// Get available log sources
/// Currently only Caddy is implemented. FrankenPHP and PHP app logs
/// will be added in a future update.
pub fn get_log_sources() -> Vec<LogSourceInfo> {
    vec![
        LogSourceInfo {
            id: "caddy".to_string(),
            name: "Caddy (Proxy)".to_string(),
            log_type: "file".to_string(),
            path: Some(get_caddy_log_path().to_string_lossy().to_string()),
            color: "#3B82F6".to_string(), // Blue
        },
        // TODO: Add FrankenPHP stdout/stderr capture
        // TODO: Add PHP app log file reading (Laravel, etc.)
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_caddy_json() {
        let line = r#"{"level":"info","ts":1704067200.123,"logger":"http.log.access","msg":"handled request","request":{"remote_ip":"127.0.0.1","method":"GET","host":"api.test","uri":"/users"},"resp":{"status":200,"duration":0.045}}"#;

        let entry = parse_caddy_json(line).unwrap();
        assert_eq!(entry.source, "caddy");
        assert_eq!(entry.level, "INFO");
        assert_eq!(entry.domain, Some("api.test".to_string()));
        assert_eq!(entry.method, Some("GET".to_string()));
        assert_eq!(entry.status, Some(200));
    }

    #[test]
    fn test_parse_plain_text_error() {
        let line = "[ERROR] Something went wrong";
        let entry = parse_plain_text(line, "test", None);
        assert_eq!(entry.level, "ERROR");
    }

    #[test]
    fn test_log_entry_new() {
        let entry = LogEntry::new("test", "INFO", "Test message");
        assert_eq!(entry.source, "test");
        assert_eq!(entry.level, "INFO");
        assert!(!entry.id.is_empty());
    }
}
