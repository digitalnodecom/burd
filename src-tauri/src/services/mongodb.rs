use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct MongoDBService;

impl ServiceDefinition for MongoDBService {
    fn service_type(&self) -> ServiceType {
        ServiceType::MongoDB
    }

    fn display_name(&self) -> &'static str {
        "MongoDB"
    }

    fn default_port(&self) -> u16 {
        27017
    }

    fn binary_name(&self) -> &'static str {
        "mongod"
    }

    fn version_source(&self) -> VersionSource {
        // MongoDB doesn't use GitHub releases for binaries, use static list
        VersionSource::Static(vec![
            "8.0.4",
            "8.0.3",
            "7.0.15",
            "7.0.14",
            "6.0.19",
            "6.0.18",
        ])
    }

    fn download_method(&self, version: &str, arch: &str) -> DownloadMethod {
        let arch_suffix = if arch == "aarch64" { "arm64" } else { "x86_64" };
        // Direct download from MongoDB
        // Example: https://fastdl.mongodb.org/osx/mongodb-macos-arm64-8.0.4.tgz
        let url = format!(
            "https://fastdl.mongodb.org/osx/mongodb-macos-{}-{}.tgz",
            arch_suffix, version
        );
        DownloadMethod::Direct {
            url,
            is_archive: true,
                checksum: None, // TODO: Add SHA256 checksums for binary verification
            }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Tcp
    }

    fn start_args(&self, instance: &Instance, data_dir: &Path) -> Vec<String> {
        vec![
            "--dbpath".to_string(),
            data_dir.to_string_lossy().to_string(),
            "--port".to_string(),
            instance.port.to_string(),
            "--bind_ip".to_string(),
            "127.0.0.1".to_string(),
        ]
    }

    fn needs_init(&self) -> bool {
        false // MongoDB auto-initializes on first start
    }
}
