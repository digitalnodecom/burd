//! MySQL Service Definition
//!
//! Uses bundled MySQL binary with per-instance configuration files.

use crate::config::{get_service_bin_dir, Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::fs;
use std::path::{Path, PathBuf};

pub struct MySQLService;

impl MySQLService {
    /// Get the MySQL basedir from the bundled binary
    /// Returns the versioned directory (e.g., ~/.burd/bin/mysql/9.5.0/)
    pub fn get_basedir() -> Result<PathBuf, String> {
        let service_dir = get_service_bin_dir(ServiceType::MySQL)?;

        if !service_dir.exists() {
            return Err("MySQL not installed. Download it from the Services page.".to_string());
        }

        // Find the first version directory that has the binary
        for entry in fs::read_dir(&service_dir)
            .map_err(|e| format!("Failed to read MySQL directory: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                // Check if this version has mysqld
                if path.join("bin/mysqld").exists() {
                    return Ok(path);
                }
            }
        }

        Err("MySQL binary not found. Download it from the Services page.".to_string())
    }

    /// Get the basedir for a specific version
    #[allow(dead_code)]
    pub fn get_basedir_for_version(version: &str) -> Result<PathBuf, String> {
        let service_dir = get_service_bin_dir(ServiceType::MySQL)?;
        let version_dir = service_dir.join(version);

        if version_dir.exists() && version_dir.join("bin/mysqld").exists() {
            Ok(version_dir)
        } else {
            // Fallback to any available version
            Self::get_basedir()
        }
    }

    /// Get the path to the mysqld binary
    #[allow(dead_code)]
    pub fn get_binary_path() -> Result<PathBuf, String> {
        Self::get_basedir().map(|p| p.join("bin/mysqld"))
    }

    /// Generate my.cnf configuration file for an instance
    #[allow(dead_code)]
    pub fn generate_config(instance: &Instance, data_dir: &Path) -> Result<(), String> {
        let basedir = Self::get_basedir_for_version(&instance.version)?;
        let config_path = data_dir.join("my.cnf");
        let socket_path = format!("/tmp/mysql-{}.sock", instance.id);
        let error_log = data_dir.join("error.log");
        let plugin_dir = basedir.join("lib/plugin");

        let config_content = format!(
            r#"[mysqld]
datadir="{}"
basedir="{}"
socket="{}"
port={}
log-error="{}"
plugin-dir="{}"
bind-address=127.0.0.1
skip-log-bin
mysqlx=0
"#,
            data_dir.to_string_lossy(),
            basedir.to_string_lossy(),
            socket_path,
            instance.port,
            error_log.to_string_lossy(),
            plugin_dir.to_string_lossy(),
        );

        fs::write(&config_path, config_content)
            .map_err(|e| format!("Failed to write my.cnf: {}", e))?;

        Ok(())
    }
}

impl ServiceDefinition for MySQLService {
    fn service_type(&self) -> ServiceType {
        ServiceType::MySQL
    }

    fn display_name(&self) -> &'static str {
        "MySQL"
    }

    fn default_port(&self) -> u16 {
        3306
    }

    fn binary_name(&self) -> &'static str {
        "mysqld"
    }

    fn version_source(&self) -> VersionSource {
        // Static versions - managed by binary downloads
        VersionSource::Static(vec!["9.5.0"])
    }

    fn download_method(&self, version: &str, _arch: &str) -> DownloadMethod {
        // Direct download from S3
        let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
        DownloadMethod::Direct {
            url: format!(
                "https://burdbin.s3.fr-par.scw.cloud/mysql/{}/mysql-{}-{}.tar.gz",
                version, version, arch
            ),
            is_archive: true,
                checksum: None, // TODO: Add SHA256 checksums for binary verification
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Tcp
    }

    fn start_args(&self, _instance: &Instance, data_dir: &Path) -> Vec<String> {
        let config_path = data_dir.join("my.cnf");
        vec![format!("--defaults-file={}", config_path.to_string_lossy())]
    }

    fn needs_init(&self) -> bool {
        true
    }

    fn init_command(&self, data_dir: &Path) -> Option<(String, Vec<String>)> {
        let basedir = Self::get_basedir().ok()?;
        let mysqld = basedir.join("bin/mysqld");

        Some((
            mysqld.to_string_lossy().to_string(),
            vec![
                format!("--basedir={}", basedir.to_string_lossy()),
                format!("--datadir={}", data_dir.to_string_lossy()),
                "--initialize-insecure".to_string(),
            ],
        ))
    }
}
