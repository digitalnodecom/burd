use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct MemcachedService;

impl ServiceDefinition for MemcachedService {
    fn service_type(&self) -> ServiceType {
        ServiceType::Memcached
    }

    fn display_name(&self) -> &'static str {
        "Memcached"
    }

    fn default_port(&self) -> u16 {
        11211
    }

    fn binary_name(&self) -> &'static str {
        "memcached"
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::Static(vec!["1.6.40"])
    }

    fn download_method(&self, version: &str, arch: &str) -> DownloadMethod {
        if arch == "aarch64" {
            DownloadMethod::Direct {
                url: format!(
                    "https://burdbin.s3.fr-par.scw.cloud/memcached/{}/memcached-{}-arm64.tar.gz",
                    version, version
                ),
                is_archive: true,
                checksum: None, // TODO: Add SHA256 checksums for binary verification
            }
        } else {
            // Fallback to source
            DownloadMethod::Direct {
                url: format!(
                    "https://memcached.org/files/memcached-{}.tar.gz",
                    version
                ),
                is_archive: true,
                checksum: None, // TODO: Add SHA256 checksums for binary verification
            }
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Tcp
    }

    fn start_args(&self, instance: &Instance, _data_dir: &Path) -> Vec<String> {
        let memory = instance.config.get("memory")
            .and_then(|v| v.as_str())
            .unwrap_or("64");

        vec![
            "-l".to_string(),
            "127.0.0.1".to_string(),
            "-p".to_string(),
            instance.port.to_string(),
            "-m".to_string(),
            memory.to_string(),
        ]
    }
}
