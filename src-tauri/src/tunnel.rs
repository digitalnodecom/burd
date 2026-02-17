//! Tunnel Module
//!
//! Manages frp (Fast Reverse Proxy) tunnels for exposing local services to the internet.
//! Uses frpc client to connect to a user's self-hosted frp server.

use crate::config::{get_app_dir, Instance};
use crate::error::LockExt;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use uuid::Uuid;

// ============================================================================
// Data Models
// ============================================================================

/// frp Server connection configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrpServer {
    pub id: Uuid,
    /// Display name for the server
    pub name: String,
    /// frp server host (e.g., "tunnel.example.com")
    pub server_addr: String,
    /// frp server port (default: 7000)
    pub server_port: u16,
    /// Authentication token
    pub token: String,
    /// Base domain for subdomains (e.g., "tunnel.example.com")
    /// URLs will be: {subdomain}.{subdomain_host}
    pub subdomain_host: String,
    /// Whether this is the default server
    #[serde(default)]
    pub is_default: bool,
    /// When this server config was created
    pub created_at: DateTime<Utc>,
}

impl FrpServer {
    pub fn new(
        name: String,
        server_addr: String,
        server_port: u16,
        token: String,
        subdomain_host: String,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            server_addr,
            server_port,
            token,
            subdomain_host,
            is_default: false,
            created_at: Utc::now(),
        }
    }
}

/// Tunnel target - what to expose
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "value")]
pub enum TunnelTarget {
    /// Expose a Burd instance by ID
    Instance(Uuid),
    /// Expose a raw port
    Port(u16),
}

impl TunnelTarget {
    /// Resolve the target to a port number
    pub fn resolve_port(&self, instances: &[Instance]) -> Option<u16> {
        match self {
            TunnelTarget::Port(port) => Some(*port),
            TunnelTarget::Instance(instance_id) => instances
                .iter()
                .find(|i| i.id == *instance_id)
                .map(|i| i.port),
        }
    }

    /// Get a display name for the target
    pub fn display_name(&self, instances: &[Instance]) -> String {
        match self {
            TunnelTarget::Port(port) => format!("Port {}", port),
            TunnelTarget::Instance(instance_id) => instances
                .iter()
                .find(|i| i.id == *instance_id)
                .map(|i| format!("{} (port {})", i.name, i.port))
                .unwrap_or_else(|| format!("Instance {}", instance_id)),
        }
    }
}

/// Subdomain configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SubdomainConfig {
    /// Auto-generated random subdomain
    Random {
        /// The generated subdomain (filled in when tunnel starts)
        #[serde(default)]
        generated: Option<String>,
    },
    /// User-specified custom subdomain
    Custom { subdomain: String },
}

impl SubdomainConfig {
    /// Get the effective subdomain (generates one if Random and not yet generated)
    pub fn get_subdomain(&self) -> String {
        match self {
            SubdomainConfig::Custom { subdomain } => subdomain.clone(),
            SubdomainConfig::Random { generated } => {
                generated.clone().unwrap_or_else(generate_random_subdomain)
            }
        }
    }

    /// Check if this is a random subdomain config
    pub fn is_random(&self) -> bool {
        matches!(self, SubdomainConfig::Random { .. })
    }
}

/// A tunnel configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tunnel {
    pub id: Uuid,
    /// Display name for the tunnel
    pub name: String,
    /// Which frp server to use
    pub server_id: Uuid,
    /// What to expose
    pub target: TunnelTarget,
    /// Subdomain configuration
    pub subdomain: SubdomainConfig,
    /// Protocol type (http, https, tcp)
    #[serde(default = "default_protocol")]
    pub protocol: String,
    /// Whether this tunnel auto-starts when Burd launches
    #[serde(default)]
    pub auto_start: bool,
    /// When this tunnel was created
    pub created_at: DateTime<Utc>,
}

fn default_protocol() -> String {
    "http".to_string()
}

impl Tunnel {
    pub fn new(
        name: String,
        server_id: Uuid,
        target: TunnelTarget,
        subdomain: SubdomainConfig,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            server_id,
            target,
            subdomain,
            protocol: default_protocol(),
            auto_start: false,
            created_at: Utc::now(),
        }
    }

    /// Get the effective subdomain for this tunnel
    pub fn get_subdomain(&self) -> String {
        self.subdomain.get_subdomain()
    }

    /// Get the public URL for this tunnel
    pub fn get_public_url(&self, server: &FrpServer) -> String {
        let subdomain = self.get_subdomain();
        let protocol = if self.protocol == "https" {
            "https"
        } else {
            "http"
        };
        format!("{}://{}.{}", protocol, subdomain, server.subdomain_host)
    }
}

/// Runtime state for a tunnel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelState {
    pub running: bool,
    /// The actual public URL when running
    pub public_url: Option<String>,
    /// Error message if failed to start
    pub error: Option<String>,
}

impl TunnelState {
    pub fn stopped() -> Self {
        Self {
            running: false,
            public_url: None,
            error: None,
        }
    }

    pub fn running_with_url(public_url: String) -> Self {
        Self {
            running: true,
            public_url: Some(public_url),
            error: None,
        }
    }

    pub fn with_error(error: String) -> Self {
        Self {
            running: false,
            public_url: None,
            error: Some(error),
        }
    }
}

/// Combined tunnel info with runtime state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TunnelWithState {
    pub tunnel: Tunnel,
    pub state: TunnelState,
    /// Server name for display
    pub server_name: Option<String>,
    /// Target name (instance name if applicable)
    pub target_name: Option<String>,
    /// Resolved target port
    pub target_port: Option<u16>,
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Generate a random 8-character subdomain
pub fn generate_random_subdomain() -> String {
    use rand::distr::Alphanumeric;
    use rand::Rng;

    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Get the tunnels directory
pub fn get_tunnels_dir() -> Result<PathBuf, String> {
    get_app_dir().map(|p| p.join("tunnels"))
}

/// Get the frpc config file path
pub fn get_frpc_config_path() -> Result<PathBuf, String> {
    get_tunnels_dir().map(|p| p.join("frpc.toml"))
}

/// Get the frpc binary path from versioned directory
pub fn get_frpc_binary_path() -> Result<PathBuf, String> {
    let service_dir = crate::config::get_service_bin_dir(crate::config::ServiceType::Frpc)?;
    if service_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&service_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                if entry.path().is_dir() {
                    let binary = entry.path().join("frpc");
                    if binary.exists() {
                        return Ok(binary);
                    }
                }
            }
        }
    }

    Err("frpc binary not found. Download it from the Services page.".to_string())
}

/// Get the frpc pid file path
pub fn get_frpc_pid_path() -> Result<PathBuf, String> {
    get_tunnels_dir().map(|p| p.join("frpc.pid"))
}

/// Get the frpc log file path
pub fn get_frpc_log_path() -> Result<PathBuf, String> {
    get_tunnels_dir().map(|p| p.join("frpc.log"))
}

// ============================================================================
// frpc Config Generation
// ============================================================================

/// Admin UI configuration for frpc webServer
#[derive(Debug, Clone)]
pub struct FrpcAdminConfig {
    pub port: u16,
    pub user: String,
    pub password: String,
}

impl Default for FrpcAdminConfig {
    fn default() -> Self {
        Self {
            port: 7400,
            user: "admin".to_string(),
            password: "admin".to_string(),
        }
    }
}

/// Generate frpc TOML configuration
pub fn generate_frpc_config(
    server: &FrpServer,
    tunnels: &[Tunnel],
    instances: &[Instance],
    admin_config: Option<&FrpcAdminConfig>,
) -> String {
    let mut config = String::new();

    // Server connection
    config.push_str(&format!("serverAddr = \"{}\"\n", server.server_addr));
    config.push_str(&format!("serverPort = {}\n", server.server_port));
    config.push_str("loginFailExit = false\n"); // Keep retrying if server is unavailable
    config.push('\n');

    // Authentication
    config.push_str("[auth]\n");
    config.push_str("method = \"token\"\n");
    config.push_str(&format!("token = \"{}\"\n", server.token));
    config.push('\n');

    // Web server for admin UI
    let admin = admin_config.cloned().unwrap_or_default();
    config.push_str("[webServer]\n");
    config.push_str("addr = \"127.0.0.1\"\n");
    config.push_str(&format!("port = {}\n", admin.port));
    config.push_str(&format!("user = \"{}\"\n", admin.user));
    config.push_str(&format!("password = \"{}\"\n", admin.password));
    config.push('\n');

    // Proxies
    for tunnel in tunnels {
        if tunnel.server_id != server.id {
            continue;
        }

        let port = match tunnel.target.resolve_port(instances) {
            Some(p) => p,
            None => continue, // Skip if we can't resolve the port
        };

        let subdomain = tunnel.get_subdomain();
        let proxy_name = format!(
            "tunnel-{}",
            tunnel.id.to_string().split('-').next().unwrap_or("unknown")
        );

        config.push_str("[[proxies]]\n");
        config.push_str(&format!("name = \"{}\"\n", proxy_name));
        config.push_str(&format!("type = \"{}\"\n", tunnel.protocol));
        config.push_str("localIP = \"127.0.0.1\"\n");
        config.push_str(&format!("localPort = {}\n", port));
        config.push_str(&format!("subdomain = \"{}\"\n", subdomain));
        config.push('\n');
    }

    config
}

// ============================================================================
// FrpcManager - Process Management
// ============================================================================

/// Manages the frpc process and tunnel configurations
pub struct FrpcManager {
    /// Currently running frpc process
    process: Mutex<Option<Child>>,
    /// Directory for tunnel files
    tunnels_dir: PathBuf,
}

impl FrpcManager {
    pub fn new() -> Result<Self, String> {
        let tunnels_dir = get_tunnels_dir()?;
        Ok(Self {
            process: Mutex::new(None),
            tunnels_dir,
        })
    }

    /// Get IDs of running tunnels based on PID file and config
    pub fn get_running_tunnel_ids(&self) -> Vec<Uuid> {
        if !self.is_running() {
            return vec![];
        }

        // Read the config file to get configured tunnel IDs
        let config_path = match get_frpc_config_path() {
            Ok(p) => p,
            Err(_) => return vec![],
        };

        let config_content = match std::fs::read_to_string(&config_path) {
            Ok(c) => c,
            Err(_) => return vec![],
        };

        // Parse tunnel IDs from config (format: name = "tunnel-{uuid}")
        let mut tunnel_ids = vec![];
        for line in config_content.lines() {
            let line = line.trim();
            if line.starts_with("name = \"tunnel-") {
                // Extract UUID from: name = "tunnel-8d3a49c3-..."
                let uuid_part = line
                    .trim_start_matches("name = \"tunnel-")
                    .trim_end_matches('"');
                if let Ok(uuid) = Uuid::parse_str(uuid_part) {
                    tunnel_ids.push(uuid);
                }
            }
        }

        tunnel_ids
    }

    /// Get overall frpc status
    pub fn get_status(&self) -> TunnelState {
        if self.is_running() {
            TunnelState {
                running: true,
                public_url: None,
                error: None,
            }
        } else {
            TunnelState::stopped()
        }
    }

    /// Read logs from the log file
    pub fn read_logs() -> Result<String, String> {
        let log_path = get_frpc_log_path()?;
        if !log_path.exists() {
            return Ok(String::new());
        }

        fs::read_to_string(&log_path).map_err(|e| format!("Failed to read log file: {}", e))
    }

    /// Check if frpc binary is installed
    pub fn is_binary_installed(&self) -> bool {
        get_frpc_binary_path().map(|p| p.exists()).unwrap_or(false)
    }

    /// Check if frpc process is running
    pub fn is_running(&self) -> bool {
        let mut process = match self.process.lock_or_err() {
            Ok(guard) => guard,
            Err(_) => return false, // If lock is poisoned, assume not running
        };
        if let Some(ref mut child) = *process {
            match child.try_wait() {
                Ok(Some(_)) => {
                    // Process has exited
                    *process = None;
                    false
                }
                Ok(None) => true,
                Err(_) => false,
            }
        } else {
            // Check PID file
            self.check_pid_file()
        }
    }

    /// Check if process is running via PID file
    fn check_pid_file(&self) -> bool {
        let pid_path = match get_frpc_pid_path() {
            Ok(p) => p,
            Err(_) => return false,
        };

        if !pid_path.exists() {
            return false;
        }

        let pid_str = match fs::read_to_string(&pid_path) {
            Ok(s) => s,
            Err(_) => return false,
        };

        let pid: i32 = match pid_str.trim().parse() {
            Ok(p) => p,
            Err(_) => return false,
        };

        // Check if process is running using kill -0
        unsafe { libc::kill(pid, 0) == 0 }
    }

    /// Start the frpc process with the given configuration
    pub async fn start(
        &mut self,
        tunnels: &[Tunnel],
        servers: &[FrpServer],
        instances: &[Instance],
        admin_config: Option<&FrpcAdminConfig>,
    ) -> Result<(), String> {
        // Check if already running
        if self.is_running() {
            return Err("frpc is already running".to_string());
        }

        // Need at least one server
        if servers.is_empty() {
            return Err("No frp servers configured. Please add a server first.".to_string());
        }

        // Get the default server (or first one)
        let server = servers
            .iter()
            .find(|s| s.is_default)
            .or_else(|| servers.first())
            .ok_or("No frp server available")?;

        // Get binary path
        let binary_path = get_frpc_binary_path()?;
        if !binary_path.exists() {
            return Err("frpc binary not installed. Please download it first.".to_string());
        }

        // Ensure tunnels directory exists
        fs::create_dir_all(&self.tunnels_dir)
            .map_err(|e| format!("Failed to create tunnels directory: {}", e))?;

        // Generate and write config
        let config_path = get_frpc_config_path()?;
        let config_content = generate_frpc_config(server, tunnels, instances, admin_config);
        fs::write(&config_path, &config_content)
            .map_err(|e| format!("Failed to write frpc config: {}", e))?;

        // Get log path
        let log_path = get_frpc_log_path()?;

        // Start frpc process
        let log_file =
            fs::File::create(&log_path).map_err(|e| format!("Failed to create log file: {}", e))?;

        let config_path_str = config_path
            .to_str()
            .ok_or_else(|| "Invalid config path encoding".to_string())?;
        let log_file_clone = log_file
            .try_clone()
            .map_err(|e| format!("Failed to clone log file handle: {}", e))?;

        let child = Command::new(&binary_path)
            .args(["-c", config_path_str])
            .stdout(Stdio::from(log_file_clone))
            .stderr(Stdio::from(log_file))
            .spawn()
            .map_err(|e| format!("Failed to start frpc: {}", e))?;

        // Save PID
        let pid = child.id();
        let pid_path = get_frpc_pid_path()?;
        fs::write(&pid_path, pid.to_string())
            .map_err(|e| format!("Failed to write PID file: {}", e))?;

        // Store process handle
        let mut process = self.process.lock_or_err().map_err(|e| e.to_string())?;
        *process = Some(child);

        Ok(())
    }

    /// Stop the frpc process
    pub fn stop(&mut self) -> Result<(), String> {
        let mut process = self.process.lock_or_err().map_err(|e| e.to_string())?;

        if let Some(ref mut child) = *process {
            child
                .kill()
                .map_err(|e| format!("Failed to kill frpc: {}", e))?;
            let _ = child.wait();
        } else {
            // Try to kill via PID file
            self.kill_by_pid()?;
        }

        *process = None;

        // Clean up PID file
        if let Ok(pid_path) = get_frpc_pid_path() {
            let _ = fs::remove_file(pid_path);
        }

        Ok(())
    }

    /// Kill frpc process using PID file
    fn kill_by_pid(&self) -> Result<(), String> {
        let pid_path = get_frpc_pid_path()?;
        if !pid_path.exists() {
            return Ok(());
        }

        let pid_str =
            fs::read_to_string(&pid_path).map_err(|e| format!("Failed to read PID file: {}", e))?;

        let pid: i32 = pid_str
            .trim()
            .parse()
            .map_err(|e| format!("Invalid PID: {}", e))?;

        unsafe {
            if libc::kill(pid, libc::SIGTERM) != 0 {
                // Process might already be dead
            }
        }

        Ok(())
    }

    /// Reload frpc configuration (for adding/removing tunnels without restart)
    pub fn reload(
        &self,
        tunnels: &[Tunnel],
        servers: &[FrpServer],
        instances: &[Instance],
        admin_config: Option<&FrpcAdminConfig>,
    ) -> Result<(), String> {
        if !self.is_running() {
            return Err("frpc is not running".to_string());
        }

        // Get the default server (or first one)
        let server = servers
            .iter()
            .find(|s| s.is_default)
            .or_else(|| servers.first())
            .ok_or("No frp server available")?;

        // Generate and write new config
        let config_path = get_frpc_config_path()?;
        let config_content = generate_frpc_config(server, tunnels, instances, admin_config);
        fs::write(&config_path, &config_content)
            .map_err(|e| format!("Failed to write frpc config: {}", e))?;

        // Send SIGHUP to reload config
        let pid_path = get_frpc_pid_path()?;
        let pid_str =
            fs::read_to_string(&pid_path).map_err(|e| format!("Failed to read PID file: {}", e))?;

        let pid: i32 = pid_str
            .trim()
            .parse()
            .map_err(|e| format!("Invalid PID: {}", e))?;

        unsafe {
            if libc::kill(pid, libc::SIGHUP) != 0 {
                return Err("Failed to send SIGHUP to frpc".to_string());
            }
        }

        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Generate a random 32-character alphanumeric token for testing
    fn generate_token() -> String {
        use rand::distr::Alphanumeric;
        use rand::Rng;

        rand::rng()
            .sample_iter(&Alphanumeric)
            .take(32)
            .map(char::from)
            .collect()
    }

    #[test]
    fn test_generate_random_subdomain() {
        let subdomain = generate_random_subdomain();
        assert_eq!(subdomain.len(), 8);
        assert!(subdomain.chars().all(|c| c.is_ascii_alphanumeric()));
        assert!(subdomain
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
    }

    #[test]
    fn test_generate_token() {
        let token = generate_token();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_subdomain_config() {
        let custom = SubdomainConfig::Custom {
            subdomain: "my-api".to_string(),
        };
        assert_eq!(custom.get_subdomain(), "my-api");
        assert!(!custom.is_random());

        let random = SubdomainConfig::Random {
            generated: Some("abc12345".to_string()),
        };
        assert_eq!(random.get_subdomain(), "abc12345");
        assert!(random.is_random());
    }

    #[test]
    fn test_tunnel_public_url() {
        let server = FrpServer::new(
            "Test".to_string(),
            "tunnel.example.com".to_string(),
            7000,
            "token".to_string(),
            "tunnel.example.com".to_string(),
        );

        let tunnel = Tunnel::new(
            "My API".to_string(),
            server.id,
            TunnelTarget::Port(8080),
            SubdomainConfig::Custom {
                subdomain: "my-api".to_string(),
            },
        );

        assert_eq!(
            tunnel.get_public_url(&server),
            "http://my-api.tunnel.example.com"
        );
    }

    #[test]
    fn test_generate_frpc_config() {
        let server = FrpServer::new(
            "Test".to_string(),
            "tunnel.example.com".to_string(),
            7000,
            "secret-token".to_string(),
            "tunnel.example.com".to_string(),
        );

        let tunnel = Tunnel::new(
            "My API".to_string(),
            server.id,
            TunnelTarget::Port(8080),
            SubdomainConfig::Custom {
                subdomain: "my-api".to_string(),
            },
        );

        let config = generate_frpc_config(&server, &[tunnel], &[], None);

        assert!(config.contains("serverAddr = \"tunnel.example.com\""));
        assert!(config.contains("serverPort = 7000"));
        assert!(config.contains("token = \"secret-token\""));
        assert!(config.contains("[[proxies]]"));
        assert!(config.contains("localPort = 8080"));
        assert!(config.contains("subdomain = \"my-api\""));
    }
}
