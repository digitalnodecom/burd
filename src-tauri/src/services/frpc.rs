use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

pub struct FrpcService;

impl ServiceDefinition for FrpcService {
    fn service_type(&self) -> ServiceType {
        ServiceType::Frpc
    }

    fn display_name(&self) -> &'static str {
        "frp Client"
    }

    fn binary_name(&self) -> &'static str {
        "frpc"
    }

    fn default_port(&self) -> u16 {
        0 // frpc doesn't have a default port
    }

    fn version_source(&self) -> VersionSource {
        VersionSource::Static(vec!["0.65.0"])
    }

    fn download_method(&self, version: &str, _arch: &str) -> DownloadMethod {
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        };
        DownloadMethod::Direct {
            url: format!(
                "https://burdbin.s3.fr-par.scw.cloud/frpc/{}/frpc-{}-{}.tar.gz",
                version, version, arch
            ),
            is_archive: true,
            checksum: None, // TODO: Add SHA256 checksums for binary verification
        }
    }

    fn start_args(&self, _instance: &Instance, _data_dir: &Path) -> Vec<String> {
        vec![] // frpc is not started via the normal service mechanism
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Tcp // Not used for frpc
    }
}
