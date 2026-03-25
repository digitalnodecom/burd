//! Process Manager
//!
//! Manages the lifecycle of service instances (start, stop, restart).
//! Handles PID tracking, process health checks, and inter-process communication.

use crate::config::{
    get_app_dir, get_binary_path, get_instance_dir, get_pids_dir, get_versioned_binary_path,
    Instance, ServiceType, SubdomainConfig,
};
use crate::services::get_service;
use crate::tunnel::{
    generate_frpc_config, get_frpc_binary_path, get_frpc_config_path, get_frpc_log_path,
    get_frpc_pid_path, get_tunnels_dir, FrpcAdminConfig,
};
use serde::Serialize;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct InstanceStatus {
    pub id: Uuid,
    pub name: String,
    pub port: u16,
    pub service_type: String,
    pub version: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub healthy: Option<bool>,
}

pub struct ProcessManager;

impl Default for ProcessManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessManager {
    pub fn new() -> Self {
        Self
    }

    fn get_pid_file(&self, id: &Uuid) -> Result<PathBuf, String> {
        let pids_dir = get_pids_dir()?;
        fs::create_dir_all(&pids_dir)
            .map_err(|e| format!("Failed to create pids directory: {}", e))?;
        Ok(pids_dir.join(format!("{}.pid", id)))
    }

    fn get_log_dir() -> Result<PathBuf, String> {
        let log_dir = get_app_dir()?.join("logs");
        fs::create_dir_all(&log_dir)
            .map_err(|e| format!("Failed to create logs directory: {}", e))?;
        Ok(log_dir)
    }

    pub fn get_log_path(id: &Uuid) -> Result<PathBuf, String> {
        Ok(Self::get_log_dir()?.join(format!("{}.log", id)))
    }

    pub fn read_logs(id: &Uuid) -> Result<String, String> {
        let log_path = Self::get_log_path(id)?;
        if !log_path.exists() {
            return Ok("No logs available yet.".to_string());
        }
        let content =
            fs::read_to_string(&log_path).map_err(|e| format!("Failed to read log file: {}", e))?;
        // Return last 100 lines
        let lines: Vec<&str> = content.lines().collect();
        let start = if lines.len() > 100 {
            lines.len() - 100
        } else {
            0
        };
        Ok(lines[start..].join("\n"))
    }

    fn read_pid(&self, id: &Uuid) -> Option<u32> {
        let pid_file = self.get_pid_file(id).ok()?;
        if !pid_file.exists() {
            return None;
        }
        fs::read_to_string(&pid_file)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    fn write_pid(&self, id: &Uuid, pid: u32) -> Result<(), String> {
        let pid_file = self.get_pid_file(id)?;
        fs::write(&pid_file, pid.to_string())
            .map_err(|e| format!("Failed to write PID file: {}", e))
    }

    fn remove_pid(&self, id: &Uuid) -> Result<(), String> {
        let pid_file = self.get_pid_file(id)?;
        if pid_file.exists() {
            fs::remove_file(&pid_file).map_err(|e| format!("Failed to remove PID file: {}", e))?;
        }
        Ok(())
    }

    fn is_process_running(&self, pid: u32) -> bool {
        // Use kill -0 to check if process exists
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    }

    pub fn is_running(&self, id: &Uuid) -> bool {
        self.read_pid(id)
            .map(|pid| self.is_process_running(pid))
            .unwrap_or(false)
    }

    /// Start an instance with optional TLD for domain resolution
    /// If TLD is provided and domain_enabled is true, the full domain will be passed to the service
    /// If ssl_enabled is true, HTTPS=on env var will be set for PHP services
    pub fn start(
        &self,
        instance: &Instance,
        tld: Option<&str>,
        ssl_enabled: bool,
    ) -> Result<u32, String> {
        // Check if already running
        if self.is_running(&instance.id) {
            return Err("Instance is already running".to_string());
        }

        // Check if port is already in use
        {
            use std::net::TcpStream;
            use std::time::Duration;
            if let Ok(addr) = format!("127.0.0.1:{}", instance.port).parse() {
                if TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok() {
                    return Err(format!(
                        "Port {} is already in use. Choose a different port or stop the process using it.",
                        instance.port
                    ));
                }
            }
        }

        // Handle frpc specially - it needs to generate tunnel config
        if instance.service_type == ServiceType::Frpc {
            return self.start_frpc(instance);
        }

        let service = get_service(instance.service_type);

        // Get binary path using instance's version
        // For Homebrew-based services (MariaDB, PostgreSQL), use their specific paths
        let binary_path = if instance.service_type == ServiceType::MariaDB {
            use crate::services::mariadb::MariaDBService;
            MariaDBService::get_binary_path()?
        } else if instance.service_type == ServiceType::PostgreSQL {
            use crate::services::postgresql::PostgreSQLService;
            PostgreSQLService::get_binary_path()?
        } else if instance.version.is_empty() || instance.version == "legacy" {
            // Legacy instance without version or using legacy flat binary - try flat binary path
            let legacy_path = get_binary_path(instance.service_type)?;
            if legacy_path.exists() {
                legacy_path
            } else {
                return Err(format!(
                    "{} binary not found. No version specified for this instance.",
                    service.display_name()
                ));
            }
        } else {
            // Use versioned path
            get_versioned_binary_path(instance.service_type, &instance.version)?
        };

        if !binary_path.exists() {
            return Err(format!(
                "{} version {} not found. Please download it first.",
                service.display_name(),
                if instance.version.is_empty() {
                    "unknown".to_string()
                } else {
                    instance.version.clone()
                }
            ));
        }

        let data_dir = get_instance_dir(&instance.id)?;
        fs::create_dir_all(&data_dir)
            .map_err(|e| format!("Failed to create data directory: {}", e))?;

        // Generate config files for Homebrew-based services before initialization
        if instance.service_type == ServiceType::MariaDB {
            use crate::services::mariadb::MariaDBService;
            MariaDBService::generate_config(instance, &data_dir)?;
        } else if instance.service_type == ServiceType::PostgreSQL {
            // PostgreSQL config is generated after initdb, so we skip it here
            // and call it after initialization below
        }

        // Run initialization if needed (first start)
        if service.needs_init() {
            let init_marker = data_dir.join(".initialized");
            if !init_marker.exists() {
                if let Some((init_cmd, init_args)) = service.init_command(&data_dir) {
                    // Get init binary path
                    let init_binary = if instance.service_type == ServiceType::MariaDB {
                        use crate::services::mariadb::MariaDBService;
                        MariaDBService::get_install_db_path()?
                    } else if instance.service_type == ServiceType::PostgreSQL {
                        use crate::services::postgresql::PostgreSQLService;
                        PostgreSQLService::get_initdb_path()?
                    } else if init_cmd.starts_with('/') {
                        PathBuf::from(&init_cmd)
                    } else {
                        // Assume it's relative to the binary directory (versioned or flat)
                        binary_path
                            .parent()
                            .map(|p| p.join(&init_cmd))
                            .unwrap_or_else(|| PathBuf::from(&init_cmd))
                    };

                    let mut cmd = Command::new(&init_binary);
                    cmd.args(&init_args).current_dir(&data_dir);

                    // PostgreSQL needs TZ=GMT, PGSHAREDIR, and PKGLIBDIR
                    if instance.service_type == ServiceType::PostgreSQL {
                        use crate::services::postgresql::PostgreSQLService;
                        cmd.env("TZ", "GMT");
                        if let Ok(basedir) = PostgreSQLService::get_basedir() {
                            let share_dir = basedir.join("share");
                            let lib_dir = basedir.join("lib");
                            cmd.env("PGSHAREDIR", share_dir.to_string_lossy().to_string());
                            cmd.env("PKGLIBDIR", lib_dir.to_string_lossy().to_string());
                        }
                    }

                    let output = cmd
                        .output()
                        .map_err(|e| format!("Failed to run init command: {}", e))?;

                    if !output.status.success() {
                        return Err(format!(
                            "Init command failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        ));
                    }

                    // Generate PostgreSQL config after initdb creates the data directory
                    if instance.service_type == ServiceType::PostgreSQL {
                        use crate::services::postgresql::PostgreSQLService;
                        PostgreSQLService::generate_config(instance, &data_dir)?;
                    }

                    // Mark as initialized
                    fs::write(&init_marker, "").ok();
                }
            }
        }

        // Create log file for output
        let log_path = Self::get_log_path(&instance.id)?;
        let log_file =
            File::create(&log_path).map_err(|e| format!("Failed to create log file: {}", e))?;
        let log_file_err = log_file
            .try_clone()
            .map_err(|e| format!("Failed to clone log file handle: {}", e))?;

        // Log the startup info
        use std::io::Write;
        let mut debug_log = log_file
            .try_clone()
            .map_err(|e| format!("Failed to clone log for debug: {}", e))?;
        writeln!(debug_log, "=== Burd Debug Info ===").ok();
        writeln!(debug_log, "Service: {}", service.display_name()).ok();
        writeln!(
            debug_log,
            "Version: {}",
            if instance.version.is_empty() {
                "unknown"
            } else {
                &instance.version
            }
        )
        .ok();
        writeln!(debug_log, "Binary path: {:?}", binary_path).ok();
        writeln!(debug_log, "Data dir: {:?}", data_dir).ok();
        writeln!(debug_log, "Port: {}", instance.port).ok();
        let effective_working_dir = if instance.service_type == ServiceType::Bun {
            instance.config.get("working_directory").and_then(|v| v.as_str()).unwrap_or("/").to_string()
        } else {
            data_dir.to_string_lossy().to_string()
        };
        writeln!(debug_log, "Working dir: {}", effective_working_dir).ok();
        writeln!(debug_log, "========================").ok();
        debug_log.flush().ok();

        // Get service-specific start arguments
        let args = service.start_args(instance, &data_dir);

        let mut cmd = Command::new(&binary_path);

        // Set working directory
        // FrankenPHP needs to run from / to avoid path issues
        // Bun instances run in the project's working directory
        let bun_working_dir = if instance.service_type == ServiceType::Bun {
            instance
                .config
                .get("working_directory")
                .and_then(|v| v.as_str())
                .map(PathBuf::from)
        } else {
            None
        };

        let working_dir = if let Some(ref bun_dir) = bun_working_dir {
            bun_dir.as_path()
        } else if matches!(
            instance.service_type,
            ServiceType::FrankenPHP | ServiceType::FrankenPhpPark
        ) {
            Path::new("/")
        } else {
            &data_dir
        };

        cmd.args(&args)
            .current_dir(working_dir)
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_err));

        // Calculate full domain if TLD is provided and domain routing is enabled
        let full_domain = if instance.domain_enabled {
            tld.map(|t| instance.full_domain(t))
        } else {
            None
        };

        // Set service-specific environment variables
        for (key, value) in service.env_vars(instance, full_domain.as_deref()) {
            cmd.env(key, value);
        }

        // Set HTTPS=on for PHP services when SSL is enabled
        // This allows Laravel/PHP to detect HTTPS without TrustProxies configuration
        if ssl_enabled
            && matches!(
                instance.service_type,
                ServiceType::FrankenPHP | ServiceType::FrankenPhpPark
            )
        {
            cmd.env("HTTPS", "on");
        }

        let child: Child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start {}: {}", service.display_name(), e))?;

        let pid = child.id();
        self.write_pid(&instance.id, pid)?;

        // Forget the child to prevent it from becoming a zombie when dropped
        // The process will run independently and we track it via PID file
        std::mem::forget(child);

        // Wait briefly and verify it started
        std::thread::sleep(Duration::from_millis(500));

        if !self.is_process_running(pid) {
            self.remove_pid(&instance.id)?;
            return Err(format!(
                "{} process exited immediately. Check port availability.",
                service.display_name()
            ));
        }

        Ok(pid)
    }

    /// Start an frpc instance - generates tunnel config and starts frpc
    fn start_frpc(&self, instance: &Instance) -> Result<u32, String> {
        // Get frpc binary path
        let binary_path = get_frpc_binary_path()?;
        if !binary_path.exists() {
            return Err(
                "frpc binary not found. Please install it via the Services tab.".to_string(),
            );
        }

        // Load app config to get tunnels, servers, and instances
        let config_store = crate::config::ConfigStore::new()
            .map_err(|e| format!("Failed to load config: {}", e))?;
        let mut app_config = config_store.load()?;

        // Need at least one server configured
        if app_config.frp_servers.is_empty() {
            return Err(
                "No frp servers configured. Please add a server in the Tunnels section first."
                    .to_string(),
            );
        }

        // Get the default server (or first one)
        let server = app_config
            .frp_servers
            .iter()
            .find(|s| s.is_default)
            .or_else(|| app_config.frp_servers.first())
            .ok_or("No frp server available")?
            .clone();

        // Generate random subdomains for any tunnels that need them
        let mut needs_save = false;
        for tunnel in &mut app_config.tunnels {
            if let SubdomainConfig::Random { generated: None } = &tunnel.subdomain {
                tunnel.subdomain = SubdomainConfig::Random {
                    generated: Some(crate::tunnel::generate_random_subdomain()),
                };
                needs_save = true;
            }
        }
        if needs_save {
            let _ = config_store.save(&app_config);
        }

        // Build admin config from instance settings
        let admin_user = instance
            .config
            .get("admin_user")
            .and_then(|v| v.as_str())
            .unwrap_or("admin")
            .to_string();
        let admin_password = instance
            .config
            .get("admin_password")
            .and_then(|v| v.as_str())
            .unwrap_or("admin")
            .to_string();

        let admin_config = FrpcAdminConfig {
            port: instance.port,
            user: admin_user,
            password: admin_password,
        };

        // Ensure tunnels directory exists
        let tunnels_dir = get_tunnels_dir()?;
        fs::create_dir_all(&tunnels_dir)
            .map_err(|e| format!("Failed to create tunnels directory: {}", e))?;

        // Generate and write frpc config
        let config_path = get_frpc_config_path()?;
        let config_content = generate_frpc_config(
            &server,
            &app_config.tunnels,
            &app_config.instances,
            Some(&admin_config),
        );
        fs::write(&config_path, &config_content)
            .map_err(|e| format!("Failed to write frpc config: {}", e))?;

        // Create log file
        let log_path = get_frpc_log_path()?;
        let log_file =
            File::create(&log_path).map_err(|e| format!("Failed to create log file: {}", e))?;
        let log_file_err = log_file
            .try_clone()
            .map_err(|e| format!("Failed to clone log file handle: {}", e))?;

        // Log startup info
        use std::io::Write;
        let mut debug_log = log_file
            .try_clone()
            .map_err(|e| format!("Failed to clone log for debug: {}", e))?;
        writeln!(debug_log, "=== Burd frpc Debug Info ===").ok();
        writeln!(debug_log, "Binary path: {:?}", binary_path).ok();
        writeln!(debug_log, "Config path: {:?}", config_path).ok();
        writeln!(debug_log, "Admin port: {}", instance.port).ok();
        writeln!(
            debug_log,
            "Server: {} ({}:{})",
            server.name, server.server_addr, server.server_port
        )
        .ok();
        writeln!(debug_log, "Tunnels: {}", app_config.tunnels.len()).ok();
        writeln!(debug_log, "============================").ok();
        debug_log.flush().ok();

        // Start frpc with config file
        let config_path_str = config_path
            .to_str()
            .ok_or_else(|| "Invalid config path encoding".to_string())?;
        let child: Child = Command::new(&binary_path)
            .args(["-c", config_path_str])
            .stdout(Stdio::from(log_file))
            .stderr(Stdio::from(log_file_err))
            .spawn()
            .map_err(|e| format!("Failed to start frpc: {}", e))?;

        let pid = child.id();
        self.write_pid(&instance.id, pid)?;

        // Also write to the legacy frpc.pid location for compatibility
        if let Ok(frpc_pid_path) = get_frpc_pid_path() {
            let _ = fs::write(&frpc_pid_path, pid.to_string());
        }

        // Wait briefly and verify it started
        std::thread::sleep(Duration::from_millis(500));

        if !self.is_process_running(pid) {
            self.remove_pid(&instance.id)?;
            return Err("frpc process exited immediately. Check the logs for details.".to_string());
        }

        Ok(pid)
    }

    pub fn stop(&self, id: &Uuid) -> Result<(), String> {
        let pid = self
            .read_pid(id)
            .ok_or_else(|| "Instance is not running (no PID file)".to_string())?;

        if !self.is_process_running(pid) {
            self.remove_pid(id)?;
            return Ok(());
        }

        // Try graceful shutdown (SIGTERM)
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();

        // Wait up to 5 seconds for graceful shutdown
        for _ in 0..50 {
            std::thread::sleep(Duration::from_millis(100));
            if !self.is_process_running(pid) {
                self.remove_pid(id)?;
                return Ok(());
            }
        }

        // Force kill (SIGKILL)
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status();

        std::thread::sleep(Duration::from_millis(200));
        self.remove_pid(id)?;

        Ok(())
    }

    pub fn get_status(&self, instance: &Instance) -> InstanceStatus {
        let service = get_service(instance.service_type);

        let pid = self.read_pid(&instance.id);
        let running = pid.map(|p| self.is_process_running(p)).unwrap_or(false);

        // Clean up stale PID file
        if !running && pid.is_some() {
            let _ = self.remove_pid(&instance.id);
        }

        InstanceStatus {
            id: instance.id,
            name: instance.name.clone(),
            port: instance.port,
            service_type: service.display_name().to_string(),
            version: instance.version.clone(),
            running,
            pid: if running { pid } else { None },
            healthy: None, // Will be filled by health check
        }
    }
}
