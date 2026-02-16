//! Helper Client
//!
//! Communicates with the privileged helper tool via Unix domain socket.

use crate::constants::{
    HELPER_BINARY_NAME, HELPER_INSTALL_PATH, HELPER_PLIST_PATH, HELPER_SOCKET_PATH,
    PRIVILEGED_HELPER_DIR, LAUNCH_DAEMONS_DIR, SYSTEM_LOGS_DIR,
};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

/// Request types the helper can handle
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum HelperRequest {
    /// Install resolver file for a TLD
    InstallResolver { tld: String, dns_port: u16 },
    /// Uninstall resolver file for a TLD
    UninstallResolver { tld: String },
    /// Install proxy daemon plist
    InstallProxyDaemon { plist_content: String },
    /// Uninstall proxy daemon
    UninstallProxyDaemon,
    /// Start proxy daemon
    StartProxyDaemon,
    /// Restart proxy daemon
    RestartProxyDaemon,
    /// Check if helper is running (ping)
    Ping,
    /// Get certificate info (exists, name, expiry) - for reading certs in restricted paths
    GetCertInfo { cert_path: String },
    /// Fix permissions on Caddy data directory to be user-readable
    FixCaddyPermissions { path: String },
    /// Setup /opt/burd directory with user ownership
    SetupOptBurd { username: String },
}

/// Response from the helper
#[derive(Debug, Serialize, Deserialize)]
pub struct HelperResponse {
    pub success: bool,
    pub message: String,
}

/// Helper client for communicating with the privileged helper
pub struct HelperClient;

impl HelperClient {
    /// Check if the helper is installed
    pub fn is_installed() -> bool {
        Path::new(HELPER_INSTALL_PATH).exists() && Path::new(HELPER_PLIST_PATH).exists()
    }

    /// Check if the helper is running (can connect to socket)
    pub fn is_running() -> bool {
        if let Ok(response) = Self::send_request(HelperRequest::Ping) {
            response.success
        } else {
            false
        }
    }

    /// Install the helper (requires admin password - one time only)
    pub fn install() -> Result<(), String> {
        // Get path to helper binary in app bundle or build directory
        let helper_source = Self::find_helper_binary()?;

        // Create the installation script
        let script = format!(
            r#"do shell script "
mkdir -p '{privileged_helper_dir}'
mkdir -p '{launch_daemons_dir}'
mkdir -p '{system_logs_dir}'
cp '{source}' '{install_path}'
chmod 755 '{install_path}'
chown root:wheel '{install_path}'

cat > '{plist_path}' << 'PLIST'
<?xml version=\"1.0\" encoding=\"UTF-8\"?>
<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">
<plist version=\"1.0\">
<dict>
    <key>Label</key>
    <string>com.burd.helper</string>
    <key>ProgramArguments</key>
    <array>
        <string>{install_path}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>{system_logs_dir}/helper.log</string>
    <key>StandardErrorPath</key>
    <string>{system_logs_dir}/helper.error.log</string>
</dict>
</plist>
PLIST

chmod 644 '{plist_path}'
launchctl load -w '{plist_path}'
" with administrator privileges"#,
            source = helper_source,
            install_path = HELPER_INSTALL_PATH,
            plist_path = HELPER_PLIST_PATH,
            privileged_helper_dir = PRIVILEGED_HELPER_DIR,
            launch_daemons_dir = LAUNCH_DAEMONS_DIR,
            system_logs_dir = SYSTEM_LOGS_DIR
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        if output.status.success() {
            // Wait a moment for the helper to start
            std::thread::sleep(Duration::from_millis(500));
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("User canceled") || stderr.contains("(-128)") {
                Err("User cancelled the installation".to_string())
            } else {
                Err(format!("Failed to install helper: {}", stderr))
            }
        }
    }

    /// Uninstall the helper
    pub fn uninstall() -> Result<(), String> {
        let script = format!(
            r#"do shell script "
launchctl bootout system/com.burd.helper 2>/dev/null || launchctl unload '{}' 2>/dev/null || true
rm -f '{}'
rm -f '{}'
rm -f /var/run/com.burd.helper.sock
" with administrator privileges"#,
            HELPER_PLIST_PATH,
            HELPER_PLIST_PATH,
            HELPER_INSTALL_PATH
        );

        let output = Command::new("osascript")
            .arg("-e")
            .arg(&script)
            .output()
            .map_err(|e| format!("Failed to run osascript: {}", e))?;

        if output.status.success() {
            Ok(())
        } else {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("User canceled") || stderr.contains("(-128)") {
                Err("User cancelled the uninstallation".to_string())
            } else {
                Err(format!("Failed to uninstall helper: {}", stderr))
            }
        }
    }

    /// Send a request to the helper
    pub fn send_request(request: HelperRequest) -> Result<HelperResponse, String> {
        let stream = UnixStream::connect(HELPER_SOCKET_PATH)
            .map_err(|e| format!("Failed to connect to helper: {}", e))?;

        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| format!("Failed to set timeout: {}", e))?;

        stream
            .set_write_timeout(Some(Duration::from_secs(10)))
            .map_err(|e| format!("Failed to set timeout: {}", e))?;

        Self::send_request_on_stream(stream, request)
    }

    fn send_request_on_stream(
        mut stream: UnixStream,
        request: HelperRequest,
    ) -> Result<HelperResponse, String> {
        // Serialize and send request
        let request_json = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;

        writeln!(stream, "{}", request_json)
            .map_err(|e| format!("Failed to send request: {}", e))?;

        stream
            .flush()
            .map_err(|e| format!("Failed to flush: {}", e))?;

        // Read response
        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader
            .read_line(&mut response_line)
            .map_err(|e| format!("Failed to read response: {}", e))?;

        serde_json::from_str(&response_line)
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Find the helper binary in the app bundle or build directory
    fn find_helper_binary() -> Result<String, String> {
        // First, check if we're in an app bundle
        if let Ok(exe) = std::env::current_exe() {
            // Check in Contents/MacOS/
            if let Some(macos_dir) = exe.parent() {
                let helper_in_bundle = macos_dir.join(HELPER_BINARY_NAME);
                if helper_in_bundle.exists() {
                    return helper_in_bundle
                        .to_str()
                        .map(|s| s.to_string())
                        .ok_or_else(|| "Invalid path".to_string());
                }

                // Check in Contents/Helpers/
                if let Some(contents_dir) = macos_dir.parent() {
                    let helper_in_helpers = contents_dir.join("Helpers").join(HELPER_BINARY_NAME);
                    if helper_in_helpers.exists() {
                        return helper_in_helpers
                            .to_str()
                            .map(|s| s.to_string())
                            .ok_or_else(|| "Invalid path".to_string());
                    }
                }
            }
        }

        // Check in cargo target directory (for development)
        let cargo_paths = [
            "target/debug/burd-helper",
            "target/release/burd-helper",
            "../target/debug/burd-helper",
            "../target/release/burd-helper",
        ];

        for path in cargo_paths {
            if Path::new(path).exists() {
                return std::fs::canonicalize(path)
                    .map_err(|e| e.to_string())?
                    .to_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| "Invalid path".to_string());
            }
        }

        Err("Helper binary not found. Please build it first.".to_string())
    }
}
