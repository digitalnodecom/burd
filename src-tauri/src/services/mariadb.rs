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

    /// True when the bundled mariadb-install-db symlink resolves to a real file.
    /// Some MariaDB archives ship the symlink without the underlying `scripts/`
    /// directory (the script is a Perl wrapper that's omitted on macOS bundles).
    pub fn install_db_script_usable() -> bool {
        match Self::get_install_db_path() {
            Ok(p) => fs::metadata(&p).is_ok(), // follows symlinks
            Err(_) => false,
        }
    }

    /// Initialize a fresh MariaDB data directory.
    ///
    /// Tries `mariadb-install-db` first (the upstream way). If that script is
    /// missing — which happens with several official tarballs that ship a
    /// dangling `bin/mariadb-install-db -> ../scripts/...` symlink — we fall
    /// back to running `mariadbd --bootstrap` ourselves, piping in the system
    /// table SQL templates that *are* present in `share/mysql/`.
    pub fn initialize_data_dir(data_dir: &Path) -> Result<(), String> {
        use std::io::Write;
        use std::process::{Command, Stdio};

        let basedir = Self::get_basedir()?;

        // Preferred path: invoke mariadb-install-db
        if Self::install_db_script_usable() {
            let install_db = basedir.join("bin/mariadb-install-db");
            let output = Command::new(&install_db)
                .args([
                    format!("--basedir={}", basedir.to_string_lossy()),
                    format!("--datadir={}", data_dir.to_string_lossy()),
                    "--auth-root-authentication-method=normal".to_string(),
                ])
                .current_dir(data_dir)
                .output()
                .map_err(|e| format!("Failed to run mariadb-install-db: {}", e))?;
            if !output.status.success() {
                return Err(format!(
                    "mariadb-install-db failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            return Ok(());
        }

        // Fallback path: feed the system-table SQL templates directly to
        // `mariadbd --bootstrap`. This is what the install-db script does
        // internally on every other platform.
        let mariadbd = basedir.join("bin/mariadbd");
        if !mariadbd.exists() {
            return Err(format!(
                "MariaDB binary missing at {:?}. Reinstall via Services page.",
                mariadbd
            ));
        }

        let share = basedir.join("share/mysql");
        // Order matters — schema before data, then optional schemas.
        let scripts = [
            "mariadb_system_tables.sql",
            "mariadb_system_tables_data.sql",
            "mariadb_performance_tables.sql",
            "mariadb_sys_schema.sql",
            "fill_help_tables.sql",
            "maria_add_gis_sp_bootstrap.sql",
        ];
        let mut sql = String::from(
            "CREATE DATABASE IF NOT EXISTS mysql;\nCREATE DATABASE IF NOT EXISTS test;\nUSE mysql;\nSET sql_mode='';\n",
        );
        for name in &scripts {
            let p = share.join(name);
            if !p.exists() {
                return Err(format!(
                    "MariaDB bundle is missing both bin/mariadb-install-db and the SQL template share/mysql/{} — the binary archive is incomplete. Re-download MariaDB from the Services page.",
                    name
                ));
            }
            let body = fs::read_to_string(&p)
                .map_err(|e| format!("Failed to read {:?}: {}", p, e))?;
            sql.push_str(&body);
            sql.push('\n');
        }

        // Make sure the data dir exists; bootstrap doesn't create it.
        fs::create_dir_all(data_dir)
            .map_err(|e| format!("Failed to create data directory: {}", e))?;

        // If a previous init attempt left InnoDB tablespace files behind,
        // mariadbd --bootstrap will refuse with "tablespace already exists".
        // Since we know init hasn't finished (no .initialized marker), it's
        // safe to wipe these stale artifacts.
        for stale in [
            "ibdata1",
            "ib_logfile0",
            "ib_logfile1",
            "ib_buffer_pool",
            "ibtmp1",
            "undo001",
            "undo002",
            "undo003",
            "aria_log_control",
            "aria_log.00000001",
        ] {
            let p = data_dir.join(stale);
            if p.exists() {
                let _ = fs::remove_file(&p);
            }
        }

        let plugin_dir = basedir.join("lib");
        let lc_messages_dir = basedir.join("share/mysql");

        let mut child = Command::new(&mariadbd)
            .args([
                "--bootstrap".to_string(),
                "--skip-grant-tables".to_string(),
                "--default-storage-engine=Aria".to_string(),
                format!("--basedir={}", basedir.to_string_lossy()),
                format!("--datadir={}", data_dir.to_string_lossy()),
                format!("--plugin-dir={}", plugin_dir.to_string_lossy()),
                format!("--lc-messages-dir={}", lc_messages_dir.to_string_lossy()),
                "--loose-innodb".to_string(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("Failed to spawn mariadbd --bootstrap: {}", e))?;

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(sql.as_bytes())
                .map_err(|e| format!("Failed to write bootstrap SQL: {}", e))?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| format!("Failed to wait on mariadbd --bootstrap: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "MariaDB bootstrap failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
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

        // Create conf.d directory for user custom configs
        let conf_d = data_dir.join("conf.d");
        if !conf_d.exists() {
            fs::create_dir_all(&conf_d)
                .map_err(|e| format!("Failed to create conf.d directory: {}", e))?;
        }

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

# User custom configuration (files in conf.d/ survive restarts)
!includedir {}
"#,
            data_dir.to_string_lossy(),
            basedir.to_string_lossy(),
            socket_path,
            instance.port,
            error_log.to_string_lossy(),
            plugin_dir.to_string_lossy(),
            lc_messages_dir.to_string_lossy(),
            conf_d.to_string_lossy(),
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
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        };
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
