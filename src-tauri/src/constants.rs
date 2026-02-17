//! Application-wide constants
//!
//! Centralized location for magic values, paths, and configuration defaults.

// =============================================================================
// Network Constants
// =============================================================================

/// Localhost IP address
pub const LOCALHOST_IP: &str = "127.0.0.1";

/// Localhost hostname
pub const LOCALHOST: &str = "localhost";

// =============================================================================
// System Paths (macOS)
// =============================================================================

/// Path to privileged helper tools directory
pub const PRIVILEGED_HELPER_DIR: &str = "/Library/PrivilegedHelperTools";

/// Path to launch daemons directory
pub const LAUNCH_DAEMONS_DIR: &str = "/Library/LaunchDaemons";

/// Path to Burd system logs directory
pub const SYSTEM_LOGS_DIR: &str = "/Library/Logs/Burd";

// =============================================================================
// App Identifiers
// =============================================================================

/// Helper tool bundle identifier
pub const HELPER_IDENTIFIER: &str = "com.burd.helper";

/// Proxy daemon bundle identifier
pub const PROXY_IDENTIFIER: &str = "com.burd.proxy";

// =============================================================================
// Derived Paths
// =============================================================================

/// Full path to the installed helper binary
pub const HELPER_INSTALL_PATH: &str = "/Library/PrivilegedHelperTools/com.burd.helper";

/// Full path to the helper plist
pub const HELPER_PLIST_PATH: &str = "/Library/LaunchDaemons/com.burd.helper.plist";

/// Full path to the proxy daemon plist
pub const PROXY_PLIST_PATH: &str = "/Library/LaunchDaemons/com.burd.proxy.plist";

/// Unix socket path for helper communication
pub const HELPER_SOCKET_PATH: &str = "/var/run/com.burd.helper.sock";

// =============================================================================
// CLI Installation
// =============================================================================

/// Default CLI installation path
pub const CLI_INSTALL_PATH: &str = "/usr/local/bin/burd";

// =============================================================================
// Helper Binary
// =============================================================================

/// Helper binary name
pub const HELPER_BINARY_NAME: &str = "burd-helper";

// =============================================================================
// Tunnel/FRP Defaults
// =============================================================================

/// Default FRP server port
pub const DEFAULT_FRP_SERVER_PORT: u16 = 7000;

/// Default FRP admin port
pub const DEFAULT_FRP_ADMIN_PORT: u16 = 7400;
