use crate::config::{Instance, ServiceType};
use crate::services::key_value_service::KeyValueService;
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

/// Valkey service implementation
///
/// Delegates to the shared KeyValueService implementation to avoid code duplication
/// with Redis (Valkey is Redis-compatible).
pub struct ValkeyService {
    inner: KeyValueService,
}

impl ValkeyService {
    pub fn new() -> Self {
        Self {
            inner: KeyValueService::valkey(),
        }
    }
}

impl Default for ValkeyService {
    fn default() -> Self {
        Self::new()
    }
}

impl ServiceDefinition for ValkeyService {
    fn service_type(&self) -> ServiceType {
        self.inner.service_type()
    }

    fn display_name(&self) -> &'static str {
        self.inner.display_name()
    }

    fn default_port(&self) -> u16 {
        self.inner.default_port()
    }

    fn binary_name(&self) -> &'static str {
        self.inner.binary_name()
    }

    fn version_source(&self) -> VersionSource {
        self.inner.version_source()
    }

    fn download_method(&self, version: &str, arch: &str) -> DownloadMethod {
        self.inner.download_method(version, arch)
    }

    fn health_check(&self) -> HealthCheck {
        self.inner.health_check()
    }

    fn start_args(&self, instance: &Instance, data_dir: &Path) -> Vec<String> {
        self.inner.start_args(instance, data_dir)
    }
}
