//! PHP Tinker Console
//!
//! Provides a Tinkerwell/TweakPHP-like PHP console for executing PHP code
//! against Laravel, WordPress, or generic PHP projects. Projects are auto-detected
//! from FrankenPHP instances with document_root configured.

use crate::config::get_app_dir;
use crate::pvm::{get_current_php, get_default_link};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;
use uuid::Uuid;

/// Project type detected from directory structure
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ProjectType {
    Laravel,   // Has artisan file
    WordPress, // Has wp-load.php (standard WP)
    Bedrock,   // Has config/application.php (Bedrock WP)
    Generic,   // Fallback
}

impl std::fmt::Display for ProjectType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectType::Laravel => write!(f, "Laravel"),
            ProjectType::WordPress => write!(f, "WordPress"),
            ProjectType::Bedrock => write!(f, "Bedrock"),
            ProjectType::Generic => write!(f, "PHP"),
        }
    }
}

/// A project that can run tinker commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TinkerProject {
    pub id: String,
    pub path: String,
    pub project_type: ProjectType,
    pub name: String,
}

/// Result of a tinker execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TinkerExecution {
    pub id: String,
    pub project_path: String,
    pub project_type: ProjectType,
    pub code: String,
    pub output: String,
    pub error: Option<String>,
    pub executed_at: DateTime<Utc>,
    pub duration_ms: u64,
}

/// History storage format
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TinkerHistory {
    version: u32,
    executions: Vec<TinkerExecution>,
}

impl Default for TinkerHistory {
    fn default() -> Self {
        Self {
            version: 1,
            executions: Vec::new(),
        }
    }
}

// === Project Detection ===

/// Detect the project type from a directory path
pub fn detect_project_type(path: &str) -> ProjectType {
    let path = Path::new(path);

    // Laravel: check for artisan file
    if path.join("artisan").exists() {
        return ProjectType::Laravel;
    }

    // Bedrock: check for config/application.php (must check before standard WP)
    // Check both at path level and parent level (for when document_root is web/)
    if path.join("config").join("application.php").exists() {
        return ProjectType::Bedrock;
    }
    // Check parent directory for Bedrock (when document_root points to web/)
    if let Some(parent) = path.parent() {
        if parent.join("config").join("application.php").exists() {
            return ProjectType::Bedrock;
        }
    }

    // WordPress: check for wp-load.php (standard WP installation)
    if path.join("wp-load.php").exists() {
        return ProjectType::WordPress;
    }

    // Fallback to generic PHP
    ProjectType::Generic
}

// === PHP Binary Resolution ===

/// Get the PHP binary path for a specific version
pub fn get_php_binary_for_version(version: &str) -> Result<PathBuf, String> {
    use crate::pvm::get_version_dir;

    let version_dir = get_version_dir(version)?;
    let php_path = version_dir.join("php");

    if php_path.exists() {
        Ok(php_path)
    } else {
        Err(format!("PHP {} binary not found", version))
    }
}

/// Get the PHP binary path, preferring Burd's PVM default
pub fn get_php_binary() -> Result<PathBuf, String> {
    // 1. Try Burd's default PHP (from PVM)
    if let Ok(default_link) = get_default_link() {
        let php_path = default_link.join("php");
        if php_path.exists() {
            return Ok(php_path);
        }
    }

    // 2. Try to use current PHP from PATH
    if let Some(current) = get_current_php() {
        return Ok(PathBuf::from(current.path));
    }

    // 3. Fallback to which php
    let which_output = Command::new("which")
        .arg("php")
        .output()
        .map_err(|e| format!("Failed to find PHP: {}", e))?;

    if which_output.status.success() {
        let path = String::from_utf8_lossy(&which_output.stdout)
            .trim()
            .to_string();
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }

    Err("No PHP binary found. Please install PHP using the PHP section.".to_string())
}

// === Tinker Temp Directory ===

/// Get the tinker temp directory for temporary script files
fn get_tinker_temp_dir() -> Result<PathBuf, String> {
    let dir = get_app_dir()?.join("tinker").join("tmp");
    fs::create_dir_all(&dir)
        .map_err(|e| format!("Failed to create tinker temp directory: {}", e))?;
    Ok(dir)
}

/// Create a temporary PHP script file
fn create_temp_script(content: &str) -> Result<PathBuf, String> {
    let temp_dir = get_tinker_temp_dir()?;
    let filename = format!("tinker_{}.php", Uuid::new_v4());
    let path = temp_dir.join(filename);

    let mut file =
        fs::File::create(&path).map_err(|e| format!("Failed to create temp script: {}", e))?;
    file.write_all(content.as_bytes())
        .map_err(|e| format!("Failed to write temp script: {}", e))?;

    Ok(path)
}

/// Clean up old temporary files (older than 1 hour)
pub fn cleanup_temp_files() -> Result<(), String> {
    let temp_dir = get_tinker_temp_dir()?;

    if !temp_dir.exists() {
        return Ok(());
    }

    let now = std::time::SystemTime::now();
    let hour_ago = now - std::time::Duration::from_secs(3600);

    if let Ok(entries) = fs::read_dir(&temp_dir) {
        for entry in entries.flatten() {
            if let Ok(metadata) = entry.metadata() {
                if let Ok(modified) = metadata.modified() {
                    if modified < hour_ago {
                        let _ = fs::remove_file(entry.path());
                    }
                }
            }
        }
    }

    Ok(())
}

// === Code Execution ===

/// Execute PHP code for a Laravel project using artisan tinker
fn execute_laravel(
    php: &Path,
    project_path: &str,
    code: &str,
) -> Result<(String, Option<String>), String> {
    // Laravel artisan tinker --execute expects the code as an argument
    // We need to escape the code properly for shell
    let output = Command::new(php)
        .current_dir(project_path)
        .args(["artisan", "tinker", "--execute", code])
        .output()
        .map_err(|e| format!("Failed to execute Laravel tinker: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok((
            stdout,
            if stderr.is_empty() {
                None
            } else {
                Some(stderr)
            },
        ))
    } else {
        // Include both stdout and stderr in error case
        let error_msg = if stderr.is_empty() {
            stdout.clone()
        } else {
            format!("{}\n{}", stdout, stderr)
        };
        Ok((stdout, Some(error_msg)))
    }
}

/// Execute PHP code for a WordPress project
fn execute_wordpress(
    php: &Path,
    project_path: &str,
    code: &str,
) -> Result<(String, Option<String>), String> {
    // Create a wrapper script that loads WordPress
    let script = format!(
        r#"<?php
define('WP_USE_THEMES', false);
require_once '{}';

// Execute user code
{}
"#,
        Path::new(project_path).join("wp-load.php").display(),
        code
    );

    let temp_script = create_temp_script(&script)?;

    let output = Command::new(php)
        .current_dir(project_path)
        .arg(&temp_script)
        .output()
        .map_err(|e| {
            let _ = fs::remove_file(&temp_script);
            format!("Failed to execute WordPress code: {}", e)
        })?;

    // Clean up temp file
    let _ = fs::remove_file(&temp_script);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok((
            stdout,
            if stderr.is_empty() {
                None
            } else {
                Some(stderr)
            },
        ))
    } else {
        let error_msg = if stderr.is_empty() {
            stdout.clone()
        } else {
            format!("{}\n{}", stdout, stderr)
        };
        Ok((stdout, Some(error_msg)))
    }
}

/// Execute PHP code for a Bedrock WordPress project
fn execute_bedrock(
    php: &Path,
    project_path: &str,
    code: &str,
) -> Result<(String, Option<String>), String> {
    let path = Path::new(project_path);

    // Determine the correct paths based on whether we're at project root or web/
    // Case 1: project_path is /var/www/bedrock (has config/ subdirectory)
    // Case 2: project_path is /var/www/bedrock/web (parent has config/)
    let (web_dir, wp_load_path) = if path.join("config").join("application.php").exists() {
        // We're at project root, web/ is a subdirectory
        let web = path.join("web");
        let wp_load = web.join("wp").join("wp-load.php");
        (web, wp_load)
    } else if let Some(parent) = path.parent() {
        if parent.join("config").join("application.php").exists() {
            // We're in web/, wp/ is a subdirectory
            let wp_load = path.join("wp").join("wp-load.php");
            (path.to_path_buf(), wp_load)
        } else {
            return Err("Could not locate Bedrock project structure".to_string());
        }
    } else {
        return Err("Could not locate Bedrock project structure".to_string());
    };

    if !wp_load_path.exists() {
        return Err(format!(
            "Bedrock wp-load.php not found at: {}",
            wp_load_path.display()
        ));
    }

    // Create a wrapper script that loads Bedrock
    // Run from web/ directory so wp-config.php can find its relative paths
    let script = format!(
        r#"<?php
define('WP_USE_THEMES', false);
require_once '{}';

// Execute user code
{}
"#,
        wp_load_path.display(),
        code
    );

    let temp_script = create_temp_script(&script)?;

    // Run from the web/ directory for proper Bedrock bootstrapping
    let output = Command::new(php)
        .current_dir(&web_dir)
        .arg(&temp_script)
        .output()
        .map_err(|e| {
            let _ = fs::remove_file(&temp_script);
            format!("Failed to execute Bedrock code: {}", e)
        })?;

    // Clean up temp file
    let _ = fs::remove_file(&temp_script);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok((
            stdout,
            if stderr.is_empty() {
                None
            } else {
                Some(stderr)
            },
        ))
    } else {
        let error_msg = if stderr.is_empty() {
            stdout.clone()
        } else {
            format!("{}\n{}", stdout, stderr)
        };
        Ok((stdout, Some(error_msg)))
    }
}

/// Execute generic PHP code
fn execute_generic(
    php: &Path,
    project_path: &str,
    code: &str,
) -> Result<(String, Option<String>), String> {
    // Check if code starts with <?php, if not wrap it
    let script_content = if code.trim().starts_with("<?php") || code.trim().starts_with("<?") {
        code.to_string()
    } else {
        format!("<?php\n{}", code)
    };

    let temp_script = create_temp_script(&script_content)?;

    let output = Command::new(php)
        .current_dir(project_path)
        .arg(&temp_script)
        .output()
        .map_err(|e| {
            let _ = fs::remove_file(&temp_script);
            format!("Failed to execute PHP code: {}", e)
        })?;

    // Clean up temp file
    let _ = fs::remove_file(&temp_script);

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if output.status.success() {
        Ok((
            stdout,
            if stderr.is_empty() {
                None
            } else {
                Some(stderr)
            },
        ))
    } else {
        let error_msg = if stderr.is_empty() {
            stdout.clone()
        } else {
            stderr
        };
        Ok((stdout, Some(error_msg)))
    }
}

/// Execute tinker code against a project
pub fn execute_tinker(
    project_path: &str,
    project_type: ProjectType,
    code: &str,
    timeout_ms: Option<u64>,
    php_version: Option<&str>,
) -> Result<TinkerExecution, String> {
    // Get PHP binary - use specified version or default
    let php = match php_version {
        Some(version) => get_php_binary_for_version(version)?,
        None => get_php_binary()?,
    };
    let start = Instant::now();

    // Clean up old temp files before executing
    let _ = cleanup_temp_files();

    let (output, error) = match project_type {
        ProjectType::Laravel => execute_laravel(&php, project_path, code)?,
        ProjectType::WordPress => execute_wordpress(&php, project_path, code)?,
        ProjectType::Bedrock => execute_bedrock(&php, project_path, code)?,
        ProjectType::Generic => execute_generic(&php, project_path, code)?,
    };

    let duration_ms = start.elapsed().as_millis() as u64;

    // Check timeout (default 30 seconds)
    let timeout = timeout_ms.unwrap_or(30000);
    if duration_ms > timeout {
        return Err(format!("Execution timed out after {}ms", timeout));
    }

    let execution = TinkerExecution {
        id: Uuid::new_v4().to_string(),
        project_path: project_path.to_string(),
        project_type,
        code: code.to_string(),
        output,
        error,
        executed_at: Utc::now(),
        duration_ms,
    };

    // Save to history
    let _ = save_to_history(&execution);

    Ok(execution)
}

// === History Management ===

/// Get the history file path
fn get_history_path() -> Result<PathBuf, String> {
    get_app_dir().map(|p| p.join("tinker_history.json"))
}

/// Load history from disk
pub fn load_history() -> Result<Vec<TinkerExecution>, String> {
    let path = get_history_path()?;

    if !path.exists() {
        return Ok(Vec::new());
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read history file: {}", e))?;

    let history: TinkerHistory = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse history file: {}", e))?;

    Ok(history.executions)
}

/// Save an execution to history
fn save_to_history(execution: &TinkerExecution) -> Result<(), String> {
    let path = get_history_path()?;

    let mut history = if path.exists() {
        let content = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        TinkerHistory::default()
    };

    // Add new execution at the beginning
    history.executions.insert(0, execution.clone());

    // Keep only the last 100 executions
    if history.executions.len() > 100 {
        history.executions.truncate(100);
    }

    let content = serde_json::to_string_pretty(&history)
        .map_err(|e| format!("Failed to serialize history: {}", e))?;

    fs::write(&path, content).map_err(|e| format!("Failed to write history file: {}", e))?;

    Ok(())
}

/// Clear all history
pub fn clear_history() -> Result<(), String> {
    let path = get_history_path()?;

    if path.exists() {
        fs::remove_file(&path).map_err(|e| format!("Failed to delete history file: {}", e))?;
    }

    Ok(())
}

/// Delete a specific history item
pub fn delete_history_item(id: &str) -> Result<(), String> {
    let path = get_history_path()?;

    if !path.exists() {
        return Ok(());
    }

    let content =
        fs::read_to_string(&path).map_err(|e| format!("Failed to read history file: {}", e))?;

    let mut history: TinkerHistory = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse history file: {}", e))?;

    // Remove the item with matching id
    history.executions.retain(|e| e.id != id);

    let content = serde_json::to_string_pretty(&history)
        .map_err(|e| format!("Failed to serialize history: {}", e))?;

    fs::write(&path, content).map_err(|e| format!("Failed to write history file: {}", e))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_project_type() {
        // These tests would need actual directories, so just test the logic
        assert_eq!(ProjectType::Laravel.to_string(), "Laravel");
        assert_eq!(ProjectType::WordPress.to_string(), "WordPress");
        assert_eq!(ProjectType::Generic.to_string(), "PHP");
    }
}
