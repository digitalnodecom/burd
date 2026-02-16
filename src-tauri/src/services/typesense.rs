use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct TypesenseService;

impl ServiceDefinition for TypesenseService {
    fn service_type(&self) -> ServiceType {
        ServiceType::Typesense
    }

    fn display_name(&self) -> &'static str {
        "Typesense"
    }

    fn default_port(&self) -> u16 {
        8108
    }

    fn binary_name(&self) -> &'static str {
        "typesense-server"
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::GitHubReleases("https://api.github.com/repos/typesense/typesense/releases")
    }

    fn download_method(&self, version: &str, arch: &str) -> DownloadMethod {
        let arch_suffix = if arch == "aarch64" { "arm64" } else { "amd64" };
        // Direct download from Typesense CDN
        // Example: https://dl.typesense.org/releases/27.1/typesense-server-27.1-darwin-arm64.tar.gz
        let clean_version = version.trim_start_matches('v');
        let url = format!(
            "https://dl.typesense.org/releases/{}/typesense-server-{}-darwin-{}.tar.gz",
            clean_version, clean_version, arch_suffix
        );
        DownloadMethod::Direct {
            url,
            is_archive: true,
                checksum: None, // TODO: Add SHA256 checksums for binary verification
            }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Http {
            path: "/health".to_string(),
        }
    }

    fn start_args(&self, instance: &Instance, data_dir: &Path) -> Vec<String> {
        let mut args = vec![
            "--data-dir".to_string(),
            data_dir.to_string_lossy().to_string(),
            "--api-port".to_string(),
            instance.port.to_string(),
            "--enable-cors".to_string(),
        ];

        // API key is required for Typesense
        if let Some(api_key) = instance.config.get("api_key").and_then(|v| v.as_str()) {
            if !api_key.is_empty() {
                args.push("--api-key".to_string());
                args.push(api_key.to_string());
            }
        } else {
            // Default API key if not set
            args.push("--api-key".to_string());
            args.push("xyz".to_string());
        }

        args
    }
}
