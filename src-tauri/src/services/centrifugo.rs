use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct CentrifugoService;

impl ServiceDefinition for CentrifugoService {
    fn service_type(&self) -> ServiceType {
        ServiceType::Centrifugo
    }

    fn display_name(&self) -> &'static str {
        "Centrifugo"
    }

    fn default_port(&self) -> u16 {
        8000
    }

    fn binary_name(&self) -> &'static str {
        "centrifugo"
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::GitHubReleases("https://api.github.com/repos/centrifugal/centrifugo/releases")
    }

    fn download_method(&self, _version: &str, arch: &str) -> DownloadMethod {
        let asset_pattern = if arch == "aarch64" {
            "centrifugo_*_darwin_arm64.tar.gz"
        } else {
            "centrifugo_*_darwin_amd64.tar.gz"
        };
        DownloadMethod::GitHubRelease {
            api_url: "https://api.github.com/repos/centrifugal/centrifugo/releases/tags/",
            asset_pattern: asset_pattern.to_string(),
            checksum: None, // TODO: Add SHA256 checksums for binary verification
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Http {
            path: "/health".to_string(),
        }
    }

    fn start_args(&self, instance: &Instance, _data_dir: &Path) -> Vec<String> {
        let mut args = vec![
            "--port".to_string(),
            instance.port.to_string(),
            "--address".to_string(),
            "127.0.0.1".to_string(),
            "--health".to_string(),
        ];

        // Add API key if configured
        if let Some(api_key) = instance.config.get("api_key").and_then(|v| v.as_str()) {
            if !api_key.is_empty() {
                args.push("--api_key".to_string());
                args.push(api_key.to_string());
            }
        }

        // Add token HMAC secret if configured
        if let Some(secret) = instance.config.get("token_hmac_secret").and_then(|v| v.as_str()) {
            if !secret.is_empty() {
                args.push("--token_hmac_secret_key".to_string());
                args.push(secret.to_string());
            }
        }

        // Enable admin UI if configured
        if let Some(admin) = instance.config.get("admin").and_then(|v| v.as_str()) {
            if admin == "true" {
                args.push("--admin".to_string());
            }
        }

        // Add admin password if configured
        if let Some(password) = instance.config.get("admin_password").and_then(|v| v.as_str()) {
            if !password.is_empty() {
                args.push("--admin_password".to_string());
                args.push(password.to_string());
            }
        }

        args
    }
}
