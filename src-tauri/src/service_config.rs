//! Service configuration loaded from services.json
//!
//! This module provides a centralized way to define services, their versions,
//! and platform-specific download URLs without modifying Rust code.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

/// Global service registry loaded from services.json
static SERVICE_REGISTRY: OnceLock<ServiceRegistry> = OnceLock::new();

/// Root structure of services.json
#[derive(Debug, Deserialize)]
pub struct ServiceRegistry {
    pub services: HashMap<String, ServiceConfig>,
}

/// Configuration for a single service
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    pub display_name: String,
    pub binary_name: String,
    pub default_port: u16,
    /// Maximum number of instances allowed (None = unlimited)
    #[serde(default)]
    pub max_instances: Option<usize>,
    /// Whether this is an internal service (not shown in UI)
    #[serde(default)]
    pub internal: bool,
    /// Whether to auto-create a domain when an instance is created
    #[serde(default)]
    pub auto_create_domain: bool,
    pub health_check: HealthCheckConfig,
    #[serde(default)]
    pub config_fields: Vec<ConfigField>,
    #[serde(default)]
    pub start_args: Vec<String>,
    #[serde(default)]
    pub start_args_conditional: Vec<ConditionalArgs>,
    #[serde(default)]
    pub env_vars: Vec<EnvVar>,
    #[serde(default)]
    pub computed_values: HashMap<String, String>,
    pub versions: VersionConfig,
    pub platforms: HashMap<String, PlatformConfig>,
    /// Labels for specific versions (e.g., "v1.3.2" -> "PHP 8.3")
    /// Use "default" key for versions not explicitly listed
    #[serde(default)]
    pub version_labels: HashMap<String, String>,
}

/// Health check configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HealthCheckConfig {
    Http { path: String },
    Tcp,
    None,
}

/// UI config field definition
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigField {
    pub key: String,
    pub label: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
    pub default: Option<String>,
}

/// Conditional arguments based on config values
#[derive(Debug, Clone, Deserialize)]
pub struct ConditionalArgs {
    pub if_config: String,
    pub args: Vec<String>,
}

/// Environment variable configuration
#[derive(Debug, Clone, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub value: String,
}

/// Version source configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum VersionConfig {
    GithubReleases { github_repo: String },
    Static { versions: Vec<String> },
}

/// Platform-specific configuration
#[derive(Debug, Clone, Deserialize)]
pub struct PlatformConfig {
    pub download: DownloadConfig,
    #[serde(default)]
    pub is_archive: bool,
    pub binary_name: Option<String>,
    #[serde(default)]
    pub requires_build: bool,
    #[serde(default)]
    pub build_commands: Vec<String>,
}

/// Download configuration
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DownloadConfig {
    GithubAsset {
        asset_pattern: String,
    },
    Direct {
        url_template: String,
        url_template_versioned: Option<String>,
    },
    /// Homebrew-based installation (macOS only)
    Homebrew {
        formula: String,
    },
    /// NPM-based installation (installed per-instance, not globally)
    Npm {
        package: String,
    },
}

impl ServiceRegistry {
    /// Load the service registry from the embedded JSON
    pub fn load() -> &'static ServiceRegistry {
        SERVICE_REGISTRY.get_or_init(|| {
            let json = include_str!("../services.json");
            serde_json::from_str(json).expect("Failed to parse services.json")
        })
    }

    /// Get a service configuration by ID
    pub fn get_service(&self, id: &str) -> Option<&ServiceConfig> {
        self.services.get(id)
    }

    /// Get all service IDs
    pub fn service_ids(&self) -> Vec<&String> {
        self.services.keys().collect()
    }

    /// Get all services
    pub fn all_services(&self) -> Vec<(&String, &ServiceConfig)> {
        self.services.iter().collect()
    }
}

impl ServiceConfig {
    /// Get the platform configuration for the current OS/arch
    pub fn current_platform(&self) -> Option<&PlatformConfig> {
        let platform_key = get_current_platform();
        self.platforms.get(&platform_key)
    }

    /// Get the binary name for the current platform
    pub fn binary_name_for_platform(&self) -> String {
        self.current_platform()
            .and_then(|p| p.binary_name.clone())
            .unwrap_or_else(|| self.binary_name.clone())
    }

    /// Build start arguments for an instance
    pub fn build_start_args(
        &self,
        port: u16,
        data_dir: &str,
        config: &serde_json::Value,
    ) -> Vec<String> {
        let mut args = Vec::new();
        let mut vars = HashMap::new();

        // Basic variables
        vars.insert("port".to_string(), port.to_string());
        vars.insert("data_dir".to_string(), data_dir.to_string());

        // Computed values
        for (key, expr) in &self.computed_values {
            if expr.contains('+') {
                // Simple addition expression like "{port} + 1"
                let parts: Vec<&str> = expr.split('+').map(|s| s.trim()).collect();
                if parts.len() == 2 {
                    let base = parts[0].trim_matches(|c| c == '{' || c == '}');
                    if let Some(base_val) = vars.get(base) {
                        if let (Ok(a), Ok(b)) = (base_val.parse::<i32>(), parts[1].parse::<i32>()) {
                            vars.insert(key.clone(), (a + b).to_string());
                        }
                    }
                }
            }
        }

        // Config values with defaults
        for field in &self.config_fields {
            let value = config
                .get(&field.key)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| field.default.clone())
                .unwrap_or_default();
            vars.insert(field.key.clone(), value);
        }

        // Build base args
        for arg in &self.start_args {
            args.push(substitute_vars(arg, &vars));
        }

        // Add conditional args
        for cond in &self.start_args_conditional {
            let value = vars.get(&cond.if_config).map(|s| s.as_str()).unwrap_or("");
            if !value.is_empty() {
                for arg in &cond.args {
                    args.push(substitute_vars(arg, &vars));
                }
            }
        }

        args
    }

    /// Build environment variables for an instance
    pub fn build_env_vars(&self, config: &serde_json::Value) -> Vec<(String, String)> {
        let mut vars = HashMap::new();

        // Config values with defaults
        for field in &self.config_fields {
            let value = config
                .get(&field.key)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .or_else(|| field.default.clone())
                .unwrap_or_default();
            vars.insert(field.key.clone(), value);
        }

        self.env_vars
            .iter()
            .map(|ev| (ev.name.clone(), substitute_vars(&ev.value, &vars)))
            .collect()
    }

    /// Get the download URL for a specific version and platform
    pub fn get_download_url(&self, version: &str, platform: &str) -> Option<String> {
        let platform_config = self.platforms.get(platform)?;

        match &platform_config.download {
            DownloadConfig::GithubAsset { .. } => {
                // GitHub asset downloads are handled separately via API
                None
            }
            DownloadConfig::Direct {
                url_template,
                url_template_versioned,
            } => {
                // Use versioned template if version is not "latest" and template exists
                let template = if version != "latest" {
                    url_template_versioned.as_ref().unwrap_or(url_template)
                } else {
                    url_template
                };

                let clean_version = version.trim_start_matches('v');
                Some(
                    template
                        .replace("{version}", clean_version)
                        .replace("{VERSION}", version),
                )
            }
            DownloadConfig::Homebrew { .. } => {
                // Homebrew uses brew install, not direct downloads
                None
            }
            DownloadConfig::Npm { .. } => {
                // NPM packages are installed per-instance, not downloaded as binaries
                None
            }
        }
    }
}

/// Get the current platform identifier
pub fn get_current_platform() -> String {
    let os = if cfg!(target_os = "macos") {
        "darwin"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "linux"
    };

    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };

    format!("{}-{}", os, arch)
}

/// Substitute variables in a string
fn substitute_vars(template: &str, vars: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in vars {
        result = result.replace(&format!("{{{}}}", key), value);
    }
    result
}

/// Service info for frontend
#[derive(Debug, Clone, serde::Serialize)]
pub struct ServiceInfo {
    pub id: String,
    pub display_name: String,
    pub default_port: u16,
    pub config_fields: Vec<ConfigFieldInfo>,
    pub available: bool,
    /// Whether this service uses Homebrew for installation
    pub is_homebrew: bool,
    /// Whether this service uses NPM for installation (per-instance)
    pub is_npm: bool,
    /// Whether to auto-create a domain when an instance is created
    pub auto_create_domain: bool,
    /// Maximum number of instances allowed (None = unlimited)
    pub max_instances: Option<usize>,
    /// Process manager type: "binary" or "pm2"
    pub process_manager: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ConfigFieldInfo {
    pub key: String,
    pub label: String,
    pub field_type: String,
    pub required: bool,
    pub default: Option<String>,
}

impl ServiceRegistry {
    /// Get service info for the frontend
    pub fn get_service_info_list(&self) -> Vec<ServiceInfo> {
        let platform = get_current_platform();

        self.services
            .iter()
            .filter_map(|(id, config)| {
                let platform_config = config.platforms.get(&platform);
                // Skip services that require building (like Redis)
                let requires_build = platform_config.map(|p| p.requires_build).unwrap_or(false);

                if requires_build {
                    return None;
                }

                // Check if this service uses Homebrew
                let is_homebrew = platform_config
                    .map(|p| matches!(p.download, DownloadConfig::Homebrew { .. }))
                    .unwrap_or(false);

                // Check if this service uses NPM
                let is_npm = platform_config
                    .map(|p| matches!(p.download, DownloadConfig::Npm { .. }))
                    .unwrap_or(false);

                // Get process manager type - Node-RED uses PM2, others use binary
                let process_manager = if id == "nodered" {
                    "pm2".to_string()
                } else {
                    "binary".to_string()
                };

                Some(ServiceInfo {
                    id: id.clone(),
                    display_name: config.display_name.clone(),
                    default_port: config.default_port,
                    config_fields: config
                        .config_fields
                        .iter()
                        .map(|f| ConfigFieldInfo {
                            key: f.key.clone(),
                            label: f.label.clone(),
                            field_type: f.field_type.clone(),
                            required: f.required,
                            default: f.default.clone(),
                        })
                        .collect(),
                    available: platform_config.is_some(),
                    is_homebrew,
                    is_npm,
                    auto_create_domain: config.auto_create_domain,
                    max_instances: config.max_instances,
                    process_manager,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_services() {
        let registry = ServiceRegistry::load();
        assert!(registry.services.contains_key("meilisearch"));
        assert!(registry.services.contains_key("mongodb"));
    }

    #[test]
    fn test_platform_detection() {
        let platform = get_current_platform();
        assert!(
            platform.contains("darwin")
                || platform.contains("linux")
                || platform.contains("windows")
        );
    }
}
