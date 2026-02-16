//! Generic key-value service implementation
//!
//! Provides a shared implementation for Redis-compatible key-value stores.
//! This eliminates code duplication between Redis and Valkey services which
//! share 100% identical start_args() implementation.

use crate::config::{Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::path::Path;

/// Generic configuration for Redis-compatible key-value stores
pub struct KeyValueService {
    _service_type: ServiceType,
    display_name: &'static str,
    _default_port: u16,
    binary_name: &'static str,
    version_source: VersionSource,
    download_config: KeyValueDownloadConfig,
}

/// Download configuration for key-value services
struct KeyValueDownloadConfig {
    s3_bucket_prefix: &'static str,
    fallback_url_template: &'static str,
}

impl KeyValueService {
    /// Create a Redis service instance
    pub fn redis() -> Self {
        Self {
            _service_type: ServiceType::Redis,
            display_name: "Redis",
            _default_port: 6379,
            binary_name: "redis-server",
            version_source: VersionSource::Static(vec!["8.4.0"]),
            download_config: KeyValueDownloadConfig {
                s3_bucket_prefix: "redis",
                fallback_url_template: "https://download.redis.io/releases/redis-{}.tar.gz",
            },
        }
    }

    /// Create a Valkey service instance
    pub fn valkey() -> Self {
        Self {
            _service_type: ServiceType::Valkey,
            display_name: "Valkey",
            _default_port: 6380,
            binary_name: "valkey-server",
            version_source: VersionSource::Static(vec!["9.0.1"]),
            download_config: KeyValueDownloadConfig {
                s3_bucket_prefix: "valkey",
                fallback_url_template: "https://github.com/valkey-io/valkey/releases/download/{0}/valkey-{0}.tar.gz",
            },
        }
    }
}

impl ServiceDefinition for KeyValueService {
    fn service_type(&self) -> ServiceType {
        self._service_type
    }

    fn display_name(&self) -> &'static str {
        self.display_name
    }

    fn default_port(&self) -> u16 {
        self._default_port
    }

    fn binary_name(&self) -> &'static str {
        self.binary_name
    }

    fn version_source(&self) -> VersionSource {
        self.version_source.clone()
    }

    fn download_method(&self, version: &str, arch: &str) -> DownloadMethod {
        if arch == "aarch64" {
            DownloadMethod::Direct {
                url: format!(
                    "https://burdbin.s3.fr-par.scw.cloud/{}/{}/{}-{}-arm64.tar.gz",
                    self.download_config.s3_bucket_prefix,
                    version,
                    self.download_config.s3_bucket_prefix,
                    version
                ),
                is_archive: true,
                checksum: None, // TODO: Add SHA256 checksums for binary verification
            }
        } else {
            // Fallback to source build for other architectures
            DownloadMethod::Direct {
                url: self.download_config.fallback_url_template.replace("{}", version)
                    .replace("{0}", version), // Support both {0} and {} templates
                is_archive: true,
                checksum: None, // TODO: Add SHA256 checksums for binary verification
            }
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Tcp
    }

    /// Shared start_args implementation for Redis-compatible services
    ///
    /// This is the implementation that was previously duplicated 100% between
    /// redis.rs (lines 53-72) and valkey.rs (lines 53-72).
    ///
    /// Configuration:
    /// - Binds to 127.0.0.1 (localhost only)
    /// - Uses instance port
    /// - Sets data directory
    /// - Optionally adds password protection via --requirepass
    fn start_args(&self, instance: &Instance, data_dir: &Path) -> Vec<String> {
        let mut args = vec![
            "--port".to_string(),
            instance.port.to_string(),
            "--dir".to_string(),
            data_dir.to_string_lossy().to_string(),
            "--bind".to_string(),
            "127.0.0.1".to_string(),
        ];

        // Add password if configured
        if let Some(password) = instance.config.get("password").and_then(|v| v.as_str()) {
            if !password.is_empty() {
                args.push("--requirepass".to_string());
                args.push(password.to_string());
            }
        }

        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::InstanceBuilder;

    #[test]
    fn test_redis_configuration() {
        let service = KeyValueService::redis();
        assert_eq!(service.service_type(), ServiceType::Redis);
        assert_eq!(service.display_name(), "Redis");
        assert_eq!(service.default_port(), 6379);
        assert_eq!(service.binary_name(), "redis-server");
    }

    #[test]
    fn test_valkey_configuration() {
        let service = KeyValueService::valkey();
        assert_eq!(service.service_type(), ServiceType::Valkey);
        assert_eq!(service.display_name(), "Valkey");
        assert_eq!(service.default_port(), 6380);
        assert_eq!(service.binary_name(), "valkey-server");
    }

    #[test]
    fn test_start_args_without_password() {
        let service = KeyValueService::redis();
        let instance = InstanceBuilder::new()
            .port(6379)
            .config(serde_json::json!({}))
            .build();
        let data_dir = Path::new("/tmp/test");

        let args = service.start_args(&instance, data_dir);

        assert_eq!(args.len(), 6);
        assert_eq!(args[0], "--port");
        assert_eq!(args[1], "6379");
        assert_eq!(args[2], "--dir");
        assert_eq!(args[3], "/tmp/test");
        assert_eq!(args[4], "--bind");
        assert_eq!(args[5], "127.0.0.1");
    }

    #[test]
    fn test_start_args_with_password() {
        let service = KeyValueService::redis();
        let instance = InstanceBuilder::new()
            .port(6379)
            .config(serde_json::json!({ "password": "secret123" }))
            .build();
        let data_dir = Path::new("/tmp/test");

        let args = service.start_args(&instance, data_dir);

        assert_eq!(args.len(), 8);
        assert!(args.contains(&"--requirepass".to_string()));
        assert!(args.contains(&"secret123".to_string()));
    }

    #[test]
    fn test_start_args_with_empty_password() {
        let service = KeyValueService::redis();
        let instance = InstanceBuilder::new()
            .port(6379)
            .config(serde_json::json!({ "password": "" }))
            .build();
        let data_dir = Path::new("/tmp/test");

        let args = service.start_args(&instance, data_dir);

        // Empty password should not add --requirepass
        assert_eq!(args.len(), 6);
        assert!(!args.contains(&"--requirepass".to_string()));
    }

    #[test]
    fn test_download_method_arm64() {
        let service = KeyValueService::redis();
        let method = service.download_method("8.4.0", "aarch64");

        match method {
            DownloadMethod::Direct { url, is_archive, .. } => {
                assert!(url.contains("burdbin.s3.fr-par.scw.cloud"));
                assert!(url.contains("redis/8.4.0/redis-8.4.0-arm64.tar.gz"));
                assert!(is_archive);
            }
            _ => panic!("Expected Direct download method"),
        }
    }

    #[test]
    fn test_download_method_x86() {
        let service = KeyValueService::redis();
        let method = service.download_method("8.4.0", "x86_64");

        match method {
            DownloadMethod::Direct { url, is_archive, .. } => {
                assert!(url.contains("download.redis.io"));
                assert!(url.contains("redis-8.4.0.tar.gz"));
                assert!(is_archive);
            }
            _ => panic!("Expected Direct download method"),
        }
    }

    #[test]
    fn test_health_check() {
        let redis = KeyValueService::redis();
        let valkey = KeyValueService::valkey();

        assert!(matches!(redis.health_check(), HealthCheck::Tcp));
        assert!(matches!(valkey.health_check(), HealthCheck::Tcp));
    }
}
