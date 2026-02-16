use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct MailpitService;

impl ServiceDefinition for MailpitService {
    fn service_type(&self) -> ServiceType {
        ServiceType::Mailpit
    }

    fn display_name(&self) -> &'static str {
        "Mailpit"
    }

    fn default_port(&self) -> u16 {
        8025
    }

    fn binary_name(&self) -> &'static str {
        "mailpit"
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::Static(vec!["1.28.0"])
    }

    fn download_method(&self, version: &str, arch: &str) -> DownloadMethod {
        if arch == "aarch64" {
            DownloadMethod::Direct {
                url: format!(
                    "https://burdbin.s3.fr-par.scw.cloud/mailpit/{}/mailpit-{}-arm64.tar.gz",
                    version, version
                ),
                is_archive: true,
                    checksum: None, // TODO: Add SHA256 checksums for binary verification
            }
        } else {
            DownloadMethod::Direct {
                url: format!(
                    "https://github.com/axllent/mailpit/releases/download/v{}/mailpit-darwin-amd64.tar.gz",
                    version
                ),
                is_archive: true,
                    checksum: None, // TODO: Add SHA256 checksums for binary verification
            }
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Http {
            path: "/livez".to_string(),
        }
    }

    fn start_args(&self, instance: &Instance, _data_dir: &Path) -> Vec<String> {
        let smtp_port = instance.config.get("smtp_port")
            .and_then(|v| v.as_str())
            .unwrap_or("1025");

        vec![
            "--listen".to_string(),
            format!("127.0.0.1:{}", instance.port),
            "--smtp".to_string(),
            format!("127.0.0.1:{}", smtp_port),
        ]
    }
}
