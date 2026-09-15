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
        // MinIO uses date-based releases. Note: this trait impl is only a
        // fallback — services.json is the source of truth for MinIO downloads.
        // MinIO discontinued its dl.min.io binary CDN (now 410), so binaries are
        // mirrored to the burd-binaries GitHub releases.
        VersionSource::Static(vec!["2025-10-15T17-29-55Z"])
    }

    fn download_method(&self, version: &str, arch: &str) -> DownloadMethod {
        let arch_suffix = if arch == "aarch64" { "arm64" } else { "x86_64" };
        let url = format!(
            "https://github.com/digitalnodecom/burd-binaries/releases/download/minio-{version}/minio-{version}-{arch_suffix}.tar.gz"
        );
        let checksum = match arch_suffix {
            "arm64" => Some("81d402f9ef877ab98e0c1560f8a221a8ada4bc6e9479c91e4ef0269376c60e2d"),
            _ => Some("4614d5b5b4609d7374d1437d6c6eb00c71eb673b5ee67905fe85b4b47914cec0"),
        };
        DownloadMethod::Direct {
            url,
            is_archive: true,
            checksum,
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
