//! Custom Driver System
//!
//! Config-based drivers for project type detection and document root determination.
//! Similar to Laravel Valet's driver system but using TOML configuration files.
//!
//! Driver locations (checked in priority order):
//! 1. Local driver: `{project}/.burd/driver.toml` - Project-specific, highest priority
//! 2. Global drivers: `~/.config/burd/drivers/*.toml` - User-defined, sorted by priority
//! 3. Built-in drivers: Hardcoded detection in `park.rs`

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// A loaded custom driver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomDriver {
    /// Driver metadata
    pub driver: DriverMeta,
    /// Detection rules
    #[serde(default)]
    pub detection: DetectionRules,
    /// Document root configuration
    pub document_root: DocumentRootConfig,
}

/// Driver metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriverMeta {
    /// Display name for this driver
    pub name: String,
    /// Priority (lower = checked first, default = 100)
    #[serde(default = "default_priority")]
    pub priority: i32,
    /// Whether projects of this type require PHP
    #[serde(default = "default_requires_php")]
    pub requires_php: bool,
}

fn default_priority() -> i32 {
    100
}

fn default_requires_php() -> bool {
    true
}

/// Detection rules for matching a project
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DetectionRules {
    /// Files that must exist (relative to project root)
    #[serde(default)]
    pub files_exist: Vec<String>,
    /// Directories that must exist
    #[serde(default)]
    pub dirs_exist: Vec<String>,
    /// Files that must NOT exist
    #[serde(default)]
    pub files_not_exist: Vec<String>,
    /// Directories that must NOT exist
    #[serde(default)]
    pub dirs_not_exist: Vec<String>,
    /// File content patterns (file path -> regex pattern)
    #[serde(default)]
    pub file_contains: HashMap<String, String>,
    /// File content negative patterns (file must NOT contain)
    #[serde(default)]
    pub file_not_contains: HashMap<String, String>,
}

/// Document root configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentRootConfig {
    /// Primary document root path (relative to project root)
    pub path: String,
    /// Fallback paths if primary doesn't exist
    #[serde(default)]
    pub fallbacks: Vec<String>,
}

/// Result of driver detection
#[derive(Debug, Clone)]
pub struct DriverMatch {
    /// The driver name
    pub name: String,
    /// Whether PHP is required
    pub requires_php: bool,
    /// Resolved document root path
    pub document_root: PathBuf,
    /// Whether this was a custom driver (vs built-in)
    #[allow(dead_code)]
    pub is_custom: bool,
}

/// Driver loader and cache
pub struct DriverLoader {
    /// Cached global drivers (sorted by priority)
    global_drivers: Vec<CustomDriver>,
    /// Whether global drivers have been loaded
    loaded: bool,
}

impl DriverLoader {
    pub fn new() -> Self {
        Self {
            global_drivers: Vec::new(),
            loaded: false,
        }
    }

    /// Load global drivers from ~/.config/burd/drivers/
    pub fn load_global_drivers(&mut self) -> Result<(), String> {
        if self.loaded {
            return Ok(());
        }

        let drivers_dir = Self::get_global_drivers_dir()?;
        if !drivers_dir.exists() {
            self.loaded = true;
            return Ok(());
        }

        let entries = fs::read_dir(&drivers_dir)
            .map_err(|e| format!("Failed to read drivers directory: {}", e))?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                if let Ok(driver) = Self::load_driver_file(&path) {
                    self.global_drivers.push(driver);
                }
            }
        }

        // Sort by priority (lower first)
        self.global_drivers.sort_by_key(|d| d.driver.priority);
        self.loaded = true;

        Ok(())
    }

    /// Load a local driver from project's .burd/driver.toml
    pub fn load_local_driver(project_path: &Path) -> Option<CustomDriver> {
        let driver_path = project_path.join(".burd").join("driver.toml");
        if !driver_path.exists() {
            return None;
        }

        Self::load_driver_file(&driver_path).ok().map(|mut driver| {
            // Local drivers always have highest priority
            driver.driver.priority = i32::MIN;
            driver
        })
    }

    fn load_driver_file(path: &Path) -> Result<CustomDriver, String> {
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read driver file: {}", e))?;
        toml::from_str(&content).map_err(|e| format!("Failed to parse driver TOML: {}", e))
    }

    fn get_global_drivers_dir() -> Result<PathBuf, String> {
        dirs::config_dir()
            .map(|p| p.join("burd").join("drivers"))
            .ok_or_else(|| "Could not determine config directory".to_string())
    }

    /// Detect project type using custom drivers first, then built-in
    /// Returns None if no custom driver matches (caller should use built-in detection)
    pub fn detect_custom(&mut self, project_path: &Path) -> Option<DriverMatch> {
        // Ensure global drivers are loaded
        let _ = self.load_global_drivers();

        // 1. Check local driver first (highest priority)
        if let Some(local_driver) = Self::load_local_driver(project_path) {
            // Local drivers always match (presence of file is detection)
            if let Some(doc_root) =
                Self::resolve_document_root(project_path, &local_driver.document_root)
            {
                return Some(DriverMatch {
                    name: local_driver.driver.name,
                    requires_php: local_driver.driver.requires_php,
                    document_root: doc_root,
                    is_custom: true,
                });
            }
        }

        // 2. Check global custom drivers (sorted by priority)
        for driver in &self.global_drivers {
            if Self::matches_detection_rules(project_path, &driver.detection) {
                if let Some(doc_root) =
                    Self::resolve_document_root(project_path, &driver.document_root)
                {
                    return Some(DriverMatch {
                        name: driver.driver.name.clone(),
                        requires_php: driver.driver.requires_php,
                        document_root: doc_root,
                        is_custom: true,
                    });
                }
            }
        }

        // No custom driver matched
        None
    }

    fn matches_detection_rules(project_path: &Path, rules: &DetectionRules) -> bool {
        // Check files_exist
        for file in &rules.files_exist {
            if !project_path.join(file).exists() {
                return false;
            }
        }

        // Check dirs_exist
        for dir in &rules.dirs_exist {
            if !project_path.join(dir).is_dir() {
                return false;
            }
        }

        // Check files_not_exist
        for file in &rules.files_not_exist {
            if project_path.join(file).exists() {
                return false;
            }
        }

        // Check dirs_not_exist
        for dir in &rules.dirs_not_exist {
            if project_path.join(dir).is_dir() {
                return false;
            }
        }

        // Check file_contains patterns
        for (file, pattern) in &rules.file_contains {
            let file_path = project_path.join(file);
            if !file_path.exists() {
                return false;
            }
            if let Ok(content) = fs::read_to_string(&file_path) {
                if let Ok(re) = Regex::new(pattern) {
                    if !re.is_match(&content) {
                        return false;
                    }
                } else if !content.contains(pattern) {
                    // Fall back to literal match if regex fails
                    return false;
                }
            } else {
                return false;
            }
        }

        // Check file_not_contains patterns
        for (file, pattern) in &rules.file_not_contains {
            let file_path = project_path.join(file);
            if file_path.exists() {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    if let Ok(re) = Regex::new(pattern) {
                        if re.is_match(&content) {
                            return false;
                        }
                    } else if content.contains(pattern) {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn resolve_document_root(project_path: &Path, config: &DocumentRootConfig) -> Option<PathBuf> {
        // Handle empty path or "." as project root
        if config.path.is_empty() || config.path == "." {
            return Some(project_path.to_path_buf());
        }

        // Try primary path
        let primary = project_path.join(&config.path);
        if primary.exists() {
            return Some(primary);
        }

        // Try fallbacks
        for fallback in &config.fallbacks {
            let path = if fallback.is_empty() || fallback == "." {
                project_path.to_path_buf()
            } else {
                project_path.join(fallback)
            };
            if path.exists() {
                return Some(path);
            }
        }

        // Default to project root if nothing else works
        Some(project_path.to_path_buf())
    }

    /// Reload drivers (clear cache)
    #[allow(dead_code)]
    pub fn reload(&mut self) {
        self.global_drivers.clear();
        self.loaded = false;
    }

    /// Get list of loaded global drivers
    #[allow(dead_code)]
    pub fn list_global_drivers(&mut self) -> Vec<&CustomDriver> {
        let _ = self.load_global_drivers();
        self.global_drivers.iter().collect()
    }
}

impl Default for DriverLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn test_driver_loading() {
        let toml = r#"
[driver]
name = "TestFramework"
priority = 50
requires_php = true

[detection]
files_exist = ["config.php"]
dirs_exist = ["app"]

[document_root]
path = "public"
fallbacks = ["www", "."]
"#;

        let driver: CustomDriver = toml::from_str(toml).unwrap();
        assert_eq!(driver.driver.name, "TestFramework");
        assert_eq!(driver.driver.priority, 50);
        assert!(driver.driver.requires_php);
        assert_eq!(driver.detection.files_exist, vec!["config.php"]);
        assert_eq!(driver.document_root.path, "public");
    }

    #[test]
    fn test_detection_rules() {
        let dir = tempdir().unwrap();
        let project = dir.path();

        // Create test files
        fs::write(project.join("config.php"), "<?php").unwrap();
        fs::create_dir(project.join("app")).unwrap();

        let rules = DetectionRules {
            files_exist: vec!["config.php".to_string()],
            dirs_exist: vec!["app".to_string()],
            ..Default::default()
        };

        assert!(DriverLoader::matches_detection_rules(project, &rules));

        // Test negative rule
        let rules_fail = DetectionRules {
            files_exist: vec!["nonexistent.php".to_string()],
            ..Default::default()
        };

        assert!(!DriverLoader::matches_detection_rules(project, &rules_fail));
    }

    #[test]
    fn test_document_root_resolution() {
        let dir = tempdir().unwrap();
        let project = dir.path();

        // Create public directory
        fs::create_dir(project.join("public")).unwrap();

        let config = DocumentRootConfig {
            path: "public".to_string(),
            fallbacks: vec![],
        };

        let result = DriverLoader::resolve_document_root(project, &config);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("public"));

        // Test fallback
        let config_fallback = DocumentRootConfig {
            path: "nonexistent".to_string(),
            fallbacks: vec!["public".to_string()],
        };

        let result = DriverLoader::resolve_document_root(project, &config_fallback);
        assert!(result.is_some());
        assert!(result.unwrap().ends_with("public"));
    }
}
