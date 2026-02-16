use crate::config::{get_instance_dir, Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct GiteaService;

impl ServiceDefinition for GiteaService {
    fn service_type(&self) -> ServiceType {
        ServiceType::Gitea
    }

    fn display_name(&self) -> &'static str {
        "Gitea"
    }

    fn default_port(&self) -> u16 {
        3000
    }

    fn binary_name(&self) -> &'static str {
        "gitea"
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::GitHubReleases("https://api.github.com/repos/go-gitea/gitea/releases")
    }

    fn download_method(&self, _version: &str, arch: &str) -> DownloadMethod {
        let asset_pattern = if arch == "aarch64" {
            "darwin-10.12-arm64"
        } else {
            "darwin-10.12-amd64"
        };
        DownloadMethod::GitHubRelease {
            api_url: "https://api.github.com/repos/go-gitea/gitea/releases/tags/",
            asset_pattern: asset_pattern.to_string(),
            checksum: None, // TODO: Add SHA256 checksums for binary verification
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Http {
            path: "/".to_string(),
        }
    }

    fn start_args(&self, instance: &Instance, _data_dir: &Path) -> Vec<String> {
        let data_dir = get_instance_dir(&instance.id)
            .unwrap_or_else(|_| std::path::PathBuf::from("."));

        // Generate app.ini config file
        let config_file = data_dir.join("custom").join("conf").join("app.ini");

        // Create directories
        if let Some(parent) = config_file.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        // Get config values
        let app_name = instance
            .config
            .get("app_name")
            .and_then(|v| v.as_str())
            .unwrap_or("Gitea");

        let disable_registration = instance
            .config
            .get("disable_registration")
            .and_then(|v| v.as_str())
            .unwrap_or("true");

        let lfs_enabled = instance
            .config
            .get("lfs_enabled")
            .and_then(|v| v.as_str())
            .unwrap_or("false");

        // Generate app.ini content
        let config_content = format!(
            r#"APP_NAME = {app_name}
RUN_MODE = prod

[server]
HTTP_PORT = {port}
HTTP_ADDR = 127.0.0.1
ROOT_URL = http://127.0.0.1:{port}/

[database]
DB_TYPE = sqlite3
PATH = {data_dir}/gitea.db

[repository]
ROOT = {data_dir}/repositories

[security]
INSTALL_LOCK = true

[service]
DISABLE_REGISTRATION = {disable_registration}

[lfs]
START_SERVER = {lfs_enabled}
PATH = {data_dir}/lfs

[log]
ROOT_PATH = {data_dir}/log
"#,
            app_name = app_name,
            port = instance.port,
            data_dir = data_dir.display(),
            disable_registration = disable_registration,
            lfs_enabled = lfs_enabled,
        );

        let _ = std::fs::write(&config_file, config_content);

        vec![
            "web".to_string(),
            "--config".to_string(),
            config_file.to_string_lossy().to_string(),
        ]
    }
}
