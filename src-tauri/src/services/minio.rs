use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct MinIOService;

impl ServiceDefinition for MinIOService {
    fn service_type(&self) -> ServiceType {
        ServiceType::MinIO
    }

    fn display_name(&self) -> &'static str {
        "MinIO"
    }

    fn default_port(&self) -> u16 {
        9000
    }

    fn binary_name(&self) -> &'static str {
        "minio"
    }

    fn version_source(&self) -> VersionSource {
        // MinIO uses date-based releases, provide static list of recent ones
        VersionSource::Static(vec![
            "RELEASE.2024-12-18T13-15-44Z",
            "RELEASE.2024-11-07T00-52-20Z",
            "RELEASE.2024-10-02T17-50-41Z",
        ])
    }

    fn download_method(&self, version: &str, arch: &str) -> DownloadMethod {
        let arch_suffix = if arch == "aarch64" { "arm64" } else { "amd64" };
        // Direct download from MinIO CDN
        // For specific: https://dl.min.io/server/minio/release/darwin-arm64/archive/minio.RELEASE.2024-12-18T13-15-44Z
        let url = format!(
            "https://dl.min.io/server/minio/release/darwin-{}/archive/minio.{}",
            arch_suffix, version
        );
        DownloadMethod::Direct {
            url,
            is_archive: false,
            checksum: None, // TODO: Add SHA256 checksums for binary verification
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Http {
            path: "/minio/health/live".to_string(),
        }
    }

    fn start_args(&self, instance: &Instance, data_dir: &Path) -> Vec<String> {
        let console_port = instance.port + 1;
        vec![
            "server".to_string(),
            data_dir.to_string_lossy().to_string(),
            "--address".to_string(),
            format!("127.0.0.1:{}", instance.port),
            "--console-address".to_string(),
            format!("127.0.0.1:{}", console_port),
        ]
    }

    fn env_vars(&self, instance: &Instance, _domain: Option<&str>) -> Vec<(String, String)> {
        let root_user = instance
            .config
            .get("root_user")
            .and_then(|v| v.as_str())
            .unwrap_or("minioadmin")
            .to_string();

        let root_password = instance
            .config
            .get("root_password")
            .and_then(|v| v.as_str())
            .unwrap_or("minioadmin")
            .to_string();

        vec![
            ("MINIO_ROOT_USER".to_string(), root_user),
            ("MINIO_ROOT_PASSWORD".to_string(), root_password),
        ]
    }
}
