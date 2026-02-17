//! PostgreSQL Service Definition
//!
//! Uses bundled PostgreSQL binary with per-instance data directories.

use crate::config::{get_service_bin_dir, Instance, ServiceType};
use crate::services::{DownloadMethod, HealthCheck, ServiceDefinition, VersionSource};
use std::fs;
use std::path::{Path, PathBuf};

pub struct PostgreSQLService;

impl PostgreSQLService {
    /// Get the PostgreSQL basedir from the bundled binary
    /// Returns the versioned directory (e.g., ~/.burd/bin/postgresql/17.7/)
    pub fn get_basedir() -> Result<PathBuf, String> {
        let service_dir = get_service_bin_dir(ServiceType::PostgreSQL)?;

        if !service_dir.exists() {
            return Err(
                "PostgreSQL not installed. Download it from the Services page.".to_string(),
            );
        }

        // Find the first version directory that has the binary
        for entry in fs::read_dir(&service_dir)
            .map_err(|e| format!("Failed to read PostgreSQL directory: {}", e))?
        {
            let entry = entry.map_err(|e| format!("Failed to read entry: {}", e))?;
            let path = entry.path();

            if path.is_dir() {
                // Check if this version has postgres
                if path.join("bin/postgres").exists() {
                    return Ok(path);
                }
            }
        }

        Err("PostgreSQL binary not found. Download it from the Services page.".to_string())
    }

    /// Get the basedir for a specific version
    #[allow(dead_code)]
    pub fn get_basedir_for_version(version: &str) -> Result<PathBuf, String> {
        let service_dir = get_service_bin_dir(ServiceType::PostgreSQL)?;
        let version_dir = service_dir.join(version);

        if version_dir.exists() && version_dir.join("bin/postgres").exists() {
            Ok(version_dir)
        } else {
            // Fallback to any available version
            Self::get_basedir()
        }
    }

    /// Get the path to the postgres binary
    pub fn get_binary_path() -> Result<PathBuf, String> {
        Self::get_basedir().map(|p| p.join("bin/postgres"))
    }

    /// Get the path to initdb
    pub fn get_initdb_path() -> Result<PathBuf, String> {
        Self::get_basedir().map(|p| p.join("bin/initdb"))
    }

    /// Generate postgresql.conf configuration file for an instance
    pub fn generate_config(instance: &Instance, data_dir: &Path) -> Result<(), String> {
        let config_path = data_dir.join("postgresql.conf");
        let socket_dir = data_dir.to_string_lossy();

        // Append our custom settings to postgresql.conf
        let custom_config = format!(
            r#"
# Burd instance configuration
port = {}
listen_addresses = '127.0.0.1'
unix_socket_directories = '{}'
log_destination = 'stderr'
logging_collector = on
log_directory = 'log'
# Use GMT to avoid timezone data lookup issues with bundled binaries
timezone = 'GMT'
log_timezone = 'GMT'
"#,
            instance.port, socket_dir,
        );

        // Read existing config if present (initdb creates one)
        let existing = fs::read_to_string(&config_path).unwrap_or_default();

        // Append our settings
        let full_config = format!("{}\n{}", existing, custom_config);

        fs::write(&config_path, full_config)
            .map_err(|e| format!("Failed to write postgresql.conf: {}", e))?;

        // Create log directory
        let log_dir = data_dir.join("log");
        if !log_dir.exists() {
            fs::create_dir_all(&log_dir)
                .map_err(|e| format!("Failed to create log directory: {}", e))?;
        }

        Ok(())
    }
}

impl ServiceDefinition for PostgreSQLService {
    fn service_type(&self) -> ServiceType {
        ServiceType::PostgreSQL
    }

    fn display_name(&self) -> &'static str {
        "PostgreSQL"
    }

    fn default_port(&self) -> u16 {
        5432
    }

    fn binary_name(&self) -> &'static str {
        "postgres"
    }

    fn version_source(&self) -> VersionSource {
        // Static versions - managed by binary downloads
        VersionSource::Static(vec!["17.7"])
    }

    fn download_method(&self, version: &str, _arch: &str) -> DownloadMethod {
        // Direct download from S3
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        };
        DownloadMethod::Direct {
            url: format!(
                "https://burdbin.s3.fr-par.scw.cloud/postgresql/{}/postgresql-{}-{}.tar.gz",
                version, version, arch
            ),
            is_archive: true,
            checksum: None, // TODO: Add SHA256 checksums for binary verification
        }
    }

    fn health_check(&self) -> HealthCheck {
        HealthCheck::Tcp
    }

    fn env_vars(&self, _instance: &Instance, _domain: Option<&str>) -> Vec<(String, String)> {
        let mut vars = vec![("TZ".to_string(), "GMT".to_string())];

        // Set PGSHAREDIR and PKGLIBDIR to override hardcoded Homebrew paths in binary
        if let Ok(basedir) = Self::get_basedir() {
            let share_dir = basedir.join("share");
            let lib_dir = basedir.join("lib");
            vars.push((
                "PGSHAREDIR".to_string(),
                share_dir.to_string_lossy().to_string(),
            ));
            vars.push((
                "PKGLIBDIR".to_string(),
                lib_dir.to_string_lossy().to_string(),
            ));
        }

        vars
    }

    fn start_args(&self, _instance: &Instance, data_dir: &Path) -> Vec<String> {
        vec!["-D".to_string(), data_dir.to_string_lossy().to_string()]
    }

    fn needs_init(&self) -> bool {
        true
    }

    fn init_command(&self, data_dir: &Path) -> Option<(String, Vec<String>)> {
        let basedir = Self::get_basedir().ok()?;
        let initdb = basedir.join("bin/initdb");
        // Share files are at ../share relative to bin/, which is basedir/share
        let share_dir = basedir.join("share");

        Some((
            initdb.to_string_lossy().to_string(),
            vec![
                "-D".to_string(),
                data_dir.to_string_lossy().to_string(),
                "-L".to_string(),
                share_dir.to_string_lossy().to_string(),
                "--auth=trust".to_string(),
                "--no-locale".to_string(),
                "--encoding=UTF8".to_string(),
            ],
        ))
    }
}
