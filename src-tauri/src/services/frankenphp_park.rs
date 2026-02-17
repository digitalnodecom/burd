//! FrankenPHP Park Service
//!
//! A singleton FrankenPHP instance that serves all parked directories.
//! Uses a dynamically generated Caddyfile for virtual host routing.

use crate::config::{get_instance_dir, Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct FrankenPHPParkService;

impl ServiceDefinition for FrankenPHPParkService {
    fn service_type(&self) -> ServiceType {
        ServiceType::FrankenPhpPark
    }

    fn display_name(&self) -> &'static str {
        "PHP Park"
    }

    fn default_port(&self) -> u16 {
        8888
    }

    fn binary_name(&self) -> &'static str {
        "frankenphp" // Uses same binary as regular FrankenPHP
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::GitHubReleases("https://api.github.com/repos/dunglas/frankenphp/releases")
    }

    fn download_method(&self, _version: &str, arch: &str) -> DownloadMethod {
        let asset_pattern = if arch == "aarch64" {
            "frankenphp-mac-arm64"
        } else {
            "frankenphp-mac-x86_64"
        };
        DownloadMethod::GitHubRelease {
            api_url: "https://api.github.com/repos/dunglas/frankenphp/releases/tags/",
            asset_pattern: asset_pattern.to_string(),
            checksum: None, // TODO: Add SHA256 checksums for binary verification
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Tcp
    }

    fn start_args(&self, instance: &Instance, _data_dir: &Path) -> Vec<String> {
        // Get the instance's Caddyfile path
        let config_file = get_instance_dir(&instance.id)
            .map(|p| p.join("Caddyfile"))
            .unwrap_or_else(|_| std::path::PathBuf::from("Caddyfile"));

        // Create initial Caddyfile if it doesn't exist
        if !config_file.exists() {
            let initial_caddyfile = format!(
                r#"{{
    frankenphp
    order php_server before file_server
}}

:{} {{
    # Map X-Forwarded-Proto header to HTTPS env value
    map {{header.X-Forwarded-Proto}} {{https_env}} {{
        https on
        default ""
    }}

    handle {{
        respond "FrankenPHP Park is running. Add a parked directory to serve your projects." 200
    }}
}}
"#,
                instance.port
            );
            let _ = std::fs::write(&config_file, initial_caddyfile);
        }

        vec![
            "run".to_string(),
            "--config".to_string(),
            config_file.to_string_lossy().to_string(),
            "--watch".to_string(), // Auto-reload on Caddyfile changes
        ]
    }

    fn env_vars(&self, instance: &Instance, _domain: Option<&str>) -> Vec<(String, String)> {
        let mut vars = vec![];

        // PHP memory limit
        if let Some(v) = instance
            .config
            .get("php_memory_limit")
            .and_then(|v| v.as_str())
        {
            if !v.is_empty() {
                vars.push(("PHP_MEMORY_LIMIT".to_string(), v.to_string()));
            }
        }

        // PHP upload max filesize
        if let Some(v) = instance
            .config
            .get("php_upload_max_filesize")
            .and_then(|v| v.as_str())
        {
            if !v.is_empty() {
                vars.push(("PHP_UPLOAD_MAX_FILESIZE".to_string(), v.to_string()));
            }
        }

        // PHP post max size
        if let Some(v) = instance
            .config
            .get("php_post_max_size")
            .and_then(|v| v.as_str())
        {
            if !v.is_empty() {
                vars.push(("PHP_POST_MAX_SIZE".to_string(), v.to_string()));
            }
        }

        // PHP max execution time
        if let Some(v) = instance
            .config
            .get("php_max_execution_time")
            .and_then(|v| v.as_str())
        {
            if !v.is_empty() {
                vars.push(("PHP_MAX_EXECUTION_TIME".to_string(), v.to_string()));
            }
        }

        vars
    }
}
