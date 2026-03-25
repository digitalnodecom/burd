use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct BunService;

impl ServiceDefinition for BunService {
    fn service_type(&self) -> ServiceType {
        ServiceType::Bun
    }

    fn display_name(&self) -> &'static str {
        "Bun"
    }

    fn default_port(&self) -> u16 {
        3000
    }

    fn binary_name(&self) -> &'static str {
        "bun"
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::GitHubReleases("https://api.github.com/repos/oven-sh/bun/releases")
    }

    fn download_method(&self, _version: &str, arch: &str) -> DownloadMethod {
        let asset_pattern = if arch == "aarch64" {
            "bun-darwin-aarch64.zip"
        } else {
            "bun-darwin-x64.zip"
        };
        DownloadMethod::GitHubRelease {
            api_url: "https://api.github.com/repos/oven-sh/bun/releases/tags/",
            asset_pattern: asset_pattern.to_string(),
            checksum: None,
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Tcp
    }

    fn start_args(&self, instance: &Instance, _data_dir: &Path) -> Vec<String> {
        // Get the dev command from config, default to "dev"
        let script = instance
            .config
            .get("script")
            .and_then(|v| v.as_str())
            .unwrap_or("dev");

        // Pass --port for Vite/Next/Nuxt (bun passes args after script name directly)
        vec![
            "run".to_string(),
            script.to_string(),
            "--port".to_string(),
            instance.port.to_string(),
        ]
    }

    fn env_vars(&self, instance: &Instance, _domain: Option<&str>) -> Vec<(String, String)> {
        // Set PORT env var - most JS frameworks read this
        vec![("PORT".to_string(), instance.port.to_string())]
    }

    fn needs_init(&self) -> bool {
        false
    }

    fn init_command(&self, _data_dir: &Path) -> Option<(String, Vec<String>)> {
        None
    }
}
