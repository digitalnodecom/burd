use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct MeilisearchService;

impl ServiceDefinition for MeilisearchService {
    fn service_type(&self) -> ServiceType {
        ServiceType::Meilisearch
    }

    fn display_name(&self) -> &'static str {
        "Meilisearch"
    }

    fn default_port(&self) -> u16 {
        7700
    }

    fn binary_name(&self) -> &'static str {
        "meilisearch"
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::GitHubReleases("https://api.github.com/repos/meilisearch/meilisearch/releases")
    }

    fn download_method(&self, _version: &str, arch: &str) -> DownloadMethod {
        let asset_pattern = if arch == "aarch64" {
            "meilisearch-macos-apple-silicon"
        } else {
            "meilisearch-macos-amd64"
        };
        // Use specific version release URL
        DownloadMethod::GitHubRelease {
            api_url: "https://api.github.com/repos/meilisearch/meilisearch/releases/tags/",
            asset_pattern: asset_pattern.to_string(),
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
            "--db-path".to_string(),
            data_dir.to_string_lossy().to_string(),
            "--http-addr".to_string(),
            format!("127.0.0.1:{}", instance.port),
            "--env".to_string(),
            "development".to_string(),
        ];

        // Add master key if configured
        if let Some(master_key) = instance.get_master_key() {
            if !master_key.is_empty() {
                args.push("--master-key".to_string());
                args.push(master_key);
            }
        }

        args
    }
}
