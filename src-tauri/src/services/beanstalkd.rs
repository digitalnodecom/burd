use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct BeanstalkdService;

impl ServiceDefinition for BeanstalkdService {
    fn service_type(&self) -> ServiceType {
        ServiceType::Beanstalkd
    }

    fn display_name(&self) -> &'static str {
        "Beanstalkd"
    }

    fn default_port(&self) -> u16 {
        11300
    }

    fn binary_name(&self) -> &'static str {
        "beanstalkd"
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::Static(vec!["1.13"])
    }

    fn download_method(&self, version: &str, arch: &str) -> DownloadMethod {
        if arch == "aarch64" {
            DownloadMethod::Direct {
                url: format!(
                    "https://burdbin.s3.fr-par.scw.cloud/beanstalkd/{}/beanstalkd-{}-arm64.tar.gz",
                    version, version
                ),
                is_archive: true,
                    checksum: None, // TODO: Add SHA256 checksums for binary verification
            }
        } else {
            // Fallback to source
            DownloadMethod::Direct {
                url: format!(
                    "https://github.com/beanstalkd/beanstalkd/archive/refs/tags/v{}.tar.gz",
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
        vec![
            "-l".to_string(),
            "127.0.0.1".to_string(),
            "-p".to_string(),
            instance.port.to_string(),
        ]
    }
}
