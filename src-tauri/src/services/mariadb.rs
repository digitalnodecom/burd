//! MariaDB Service Definition
//!
//! Uses bundled MariaDB binary with per-instance configuration files.

use crate::config::{get_service_bin_dir, Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::fs;
use std::path::{Path, PathBuf};

pub struct MariaDBService;

impl MariaDBService {
    /// Get the MariaDB basedir from the bundled binary
    /// Returns the versioned directory (e.g., ~/.burd/bin/mariadb/12.1.2/)
    pub fn get_basedir() -> Result<PathBuf, String> {
        let service_dir = get_service_bin_dir(ServiceType::MariaDB)?;

        if !service_dir.exists() {
            return Err("MariaDB not installed. Download it from the Services page.".to_string());
        }

        // Find the first version directory that has the binary
        for entry in fs::read_dir(&service_dir)
            .map_err(|e| format!("Failed to read MariaDB directory: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                // Check if this version has mariadbd
                if path.join("bin/mariadbd").exists() {
                    return Ok(path);
                }
            }
        }

        Err("MariaDB binary not found. Download it from the Services page.".to_string())
    }

    /// Get the basedir for a specific version
    pub fn get_basedir_for_version(version: &str) -> Result<PathBuf, String> {
        let service_dir = get_service_bin_dir(ServiceType::MariaDB)?;
        let version_dir = service_dir.join(version);

        if version_dir.exists() && version_dir.join("bin/mariadbd").exists() {
            Ok(version_dir)
        } else {
            // Fallback to any available version
            Self::get_basedir()
        }
    }

    /// Get the path to the mariadbd binary
    pub fn get_binary_path() -> Result<PathBuf, String> {
        Self::get_basedir().map(|p| p.join("bin/mariadbd"))
    }

    /// Get the path to mariadb-install-db
    pub fn get_install_db_path() -> Result<PathBuf, String> {
        Self::get_basedir().map(|p| p.join("bin/mariadb-install-db"))
    }

    /// Generate my.cnf configuration file for an instance
    pub fn generate_config(instance: &Instance, data_dir: &Path) -> Result<(), String> {
        let basedir = Self::get_basedir_for_version(&instance.version)?;
        let config_path = data_dir.join("my.cnf");
        let socket_path = format!("/tmp/mariadb-{}.sock", instance.id);
        let error_log = data_dir.join("error.log");
        // Bundled binary has libs directly in lib/, not lib/plugin/
        let plugin_dir = basedir.join("lib");
        // Character sets and language files in share/mysql/
        let lc_messages_dir = basedir.join("share/mysql");

        let config_content = format!(
            r#"[mysqld]
datadir="{}"
basedir="{}"
socket="{}"
port={}
log-error="{}"
plugin-dir="{}"
lc-messages-dir="{}"
bind-address=127.0.0.1
disable_log_bin
skip-grant-tables
"#,
            data_dir.to_string_lossy(),
            basedir.to_string_lossy(),
            socket_path,
            instance.port,
            error_log.to_string_lossy(),
            plugin_dir.to_string_lossy(),
            lc_messages_dir.to_string_lossy(),
        );

        fs::write(&config_path, config_content)
            .map_err(|e| format!("Failed to write my.cnf: {}", e))?;

        Ok(())
    }
}

impl ServiceDefinition for MariaDBService {
    fn service_type(&self) -> ServiceType {
        ServiceType::MariaDB
    }

    fn display_name(&self) -> &'static str {
        "MariaDB"
    }

    fn default_port(&self) -> u16 {
        3330
    }

    fn binary_name(&self) -> &'static str {
        "mariadbd"
    }

    fn version_source(&self) -> VersionSource {
        // Static versions - managed by binary downloads
        VersionSource::Static(vec!["12.1.2"])
    }

    fn download_method(&self, version: &str, _arch: &str) -> DownloadMethod {
        // Direct download from S3
        let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "x64" };
        DownloadMethod::Direct {
            url: format!(
                "https://burdbin.s3.fr-par.scw.cloud/mariadb/{}/mariadb-{}-{}.tar.gz",
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
        let install_db = basedir.join("bin/mariadb-install-db");

        Some((
            install_db.to_string_lossy().to_string(),
            vec![
                format!("--basedir={}", basedir.to_string_lossy()),
                format!("--datadir={}", data_dir.to_string_lossy()),
                "--auth-root-authentication-method=normal".to_string(),
            ],
        ))
    }
}
