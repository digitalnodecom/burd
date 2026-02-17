use crate::config::{get_instance_dir, Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct FrankenPHPService;

impl ServiceDefinition for FrankenPHPService {
    fn service_type(&self) -> ServiceType {
        ServiceType::FrankenPHP
    }

    fn display_name(&self) -> &'static str {
        "PHP"
    }

    fn default_port(&self) -> u16 {
        8000
    }

    fn binary_name(&self) -> &'static str {
        "frankenphp"
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
        let data_dir =
            get_instance_dir(&instance.id).unwrap_or_else(|_| std::path::PathBuf::from("."));

        let config_file = data_dir.join("Caddyfile");

        // Get document root from config, default to current directory
        let doc_root = instance
            .config
            .get("document_root")
            .and_then(|v| v.as_str())
            .unwrap_or(".");

        // Generate Caddyfile that maps X-Forwarded-Proto header to HTTPS env var
        // This allows PHP to correctly detect HTTPS when behind Caddy reverse proxy
        // Includes Laravel-compatible routing with try_files
        let caddyfile = format!(
            r#"{{
    frankenphp
    order php_server before file_server
}}

:{port} {{
    # Map X-Forwarded-Proto header to HTTPS env value
    map {{header.X-Forwarded-Proto}} {{https_env}} {{
        https on
        default ""
    }}

    root * "{doc_root}"

    # Enable compression (zstd, brotli, gzip)
    encode zstd br gzip

    # Laravel/PHP framework routing (FrankenPHP official config)
    php_server {{
        try_files {{path}} index.php
        env HTTPS {{https_env}}
    }}
}}
"#,
            port = instance.port,
            doc_root = doc_root
        );

        let _ = std::fs::write(&config_file, caddyfile);

        vec![
            "run".to_string(),
            "--config".to_string(),
            config_file.to_string_lossy().to_string(),
        ]
    }

    fn env_vars(&self, instance: &Instance, domain: Option<&str>) -> Vec<(String, String)> {
        let mut vars = vec![];

        // Set SERVER_NAME to the domain if available (important for PHP apps like Laravel)
        if let Some(d) = domain {
            vars.push(("SERVER_NAME".to_string(), d.to_string()));
        }

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
