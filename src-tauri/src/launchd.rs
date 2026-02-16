//! launchd Plist Management for Caddy Proxy
//!
//! This module manages the macOS launchd daemon that runs Caddy
//! as a reverse proxy on privileged ports 80 and 443.

use crate::constants::{PROXY_IDENTIFIER, PROXY_PLIST_PATH};
use crate::helper_client::{HelperClient, HelperRequest};
use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Status of the launchd daemon
#[derive(Debug, Clone, Serialize)]
pub struct LaunchdStatus {
    pub installed: bool,
    pub running: bool,
    pub pid: Option<u32>,
}

/// Get the user's app directory explicitly using home_dir
/// This ensures we always use user paths even if the code runs with elevated privileges
fn get_user_app_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join("Library/Application Support/Burd"))
        .unwrap_or_else(|| PathBuf::from("/tmp/Burd"))
}

/// Get the user's logs directory
fn get_user_logs_dir() -> PathBuf {
    dirs::home_dir()
        .map(|h| h.join("Library/Logs/Burd"))
        .unwrap_or_else(|| PathBuf::from("/tmp/Burd/logs"))
}

/// Get the path where Caddy stores its data (PKI, etc.) when running as daemon
pub fn get_caddy_data_dir() -> PathBuf {
    get_user_app_dir().join("caddy-data")
}

/// Generate the launchd plist content for Caddy
/// Uses explicit user home paths to ensure daemon reads from user space
fn generate_plist() -> String {
    // Use user-space paths explicitly (not dirs::data_dir which varies by user)
    let user_app_dir = get_user_app_dir();
    let caddy_bin = user_app_dir.join("bin/caddy");
    let caddyfile = user_app_dir.join("Caddyfile");
    let logs_dir = get_user_logs_dir();
    let caddy_data = get_caddy_data_dir();
    let working_dir = user_app_dir;

    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>

    <key>ProgramArguments</key>
    <array>
        <string>{caddy}</string>
        <string>run</string>
        <string>--config</string>
        <string>{caddyfile}</string>
        <string>--adapter</string>
        <string>caddyfile</string>
        <string>--watch</string>
    </array>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <true/>

    <key>Umask</key>
    <integer>18</integer>

    <key>StandardOutPath</key>
    <string>{logs_dir}/caddy.log</string>

    <key>StandardErrorPath</key>
    <string>{logs_dir}/caddy.error.log</string>

    <key>WorkingDirectory</key>
    <string>{working_dir}</string>

    <key>EnvironmentVariables</key>
    <dict>
        <key>XDG_DATA_HOME</key>
        <string>{caddy_data}</string>
    </dict>
</dict>
</plist>
"#,
        label = PROXY_IDENTIFIER,
        caddy = caddy_bin.display(),
        caddyfile = caddyfile.display(),
        logs_dir = logs_dir.display(),
        working_dir = working_dir.display(),
        caddy_data = caddy_data.display()
    )
}

/// Install the launchd plist for Caddy proxy
///
/// This requires admin privileges for the plist installation.
/// Before calling this, ensure:
/// 1. Caddy binary is installed via `caddy::install_caddy_for_daemon()`
/// 2. Initial Caddyfile is written via `caddy::write_caddyfile()`
pub fn install() -> Result<(), String> {
    // Verify Caddy is installed for daemon
    if !crate::caddy::is_daemon_caddy_installed() {
        return Err("Caddy binary not installed. Call caddy::install_caddy_for_daemon() first.".to_string());
    }

    // Verify Caddyfile exists (now in user space)
    let caddyfile_path = get_user_app_dir().join("Caddyfile");
    if !caddyfile_path.exists() {
        return Err("Caddyfile not found. Call caddy::write_caddyfile() first.".to_string());
    }

    // Create user-space directories (no admin needed)
    let logs_dir = get_user_logs_dir();
    let caddy_data = get_caddy_data_dir();
    fs::create_dir_all(&logs_dir)
        .map_err(|e| format!("Failed to create logs directory: {}", e))?;
    fs::create_dir_all(&caddy_data)
        .map_err(|e| format!("Failed to create caddy data directory: {}", e))?;

    let plist_content = generate_plist();

    // Try using helper if available
    if HelperClient::is_running() {
        let response = HelperClient::send_request(HelperRequest::InstallProxyDaemon {
            plist_content: plist_content.clone(),
        })?;
        if response.success {
            return Ok(());
        }
        // Fall through to osascript if helper failed
    }

    // Fall back to osascript
    // Write plist to a temp file first (avoids AppleScript parsing issues)
    let temp_dir = std::env::temp_dir();
    let temp_plist = temp_dir.join("com.burd.proxy.plist");
    fs::write(&temp_plist, &plist_content)
        .map_err(|e| format!("Failed to write temp plist: {}", e))?;

    let temp_plist_str = temp_plist.to_str().ok_or("Invalid temp path")?;

    // Use osascript to install plist with admin privileges
    // Note: Config files are now in user space, only plist needs admin
    let script = format!(
        r#"do shell script "
cp '{}' '{}'
chmod 644 '{}'
launchctl load -w '{}'
" with administrator privileges"#,
        temp_plist_str, PROXY_PLIST_PATH, PROXY_PLIST_PATH, PROXY_PLIST_PATH
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    // Clean up temp file
    let _ = fs::remove_file(&temp_plist);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("(-128)") {
            return Err("User cancelled the authorization".to_string());
        }
        return Err(format!("Failed to install launchd plist: {}", stderr));
    }

    Ok(())
}

/// Uninstall the launchd plist
///
/// This requires admin privileges and will prompt the user.
pub fn uninstall() -> Result<(), String> {
    // Try using helper if available
    if HelperClient::is_running() {
        let response = HelperClient::send_request(HelperRequest::UninstallProxyDaemon)?;
        if response.success {
            return Ok(());
        }
        // Fall through to osascript if helper failed
    }

    // Fall back to osascript
    // Unload and remove plist using osascript for admin privileges
    // Use launchctl bootout for system domain services
    let script = format!(
        r#"do shell script "
launchctl bootout system/{} 2>/dev/null || launchctl unload -w '{}' 2>/dev/null || true
rm -f '{}'
" with administrator privileges"#,
        PROXY_IDENTIFIER, PROXY_PLIST_PATH, PROXY_PLIST_PATH
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to run osascript: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("(-128)") {
            return Err("User cancelled the authorization".to_string());
        }
        return Err(format!("Failed to uninstall launchd plist: {}", stderr));
    }

    Ok(())
}

/// Check if the launchd daemon is installed
pub fn is_installed() -> bool {
    PathBuf::from(PROXY_PLIST_PATH).exists()
}

/// Check if the launchd daemon is running and get its status
pub fn get_status() -> LaunchdStatus {
    let installed = is_installed();

    if !installed {
        return LaunchdStatus {
            installed: false,
            running: false,
            pid: None,
        };
    }

    // Check if daemon is running by trying to connect to port 80
    use std::net::TcpStream;
    use std::time::Duration;

    let running = TcpStream::connect_timeout(
        &"127.0.0.1:80".parse().unwrap(),
        Duration::from_millis(100),
    ).is_ok();

    // Try to get PID using lsof
    let pid = if running {
        get_pid_on_port(80)
    } else {
        None
    };

    LaunchdStatus {
        installed: true,
        running,
        pid,
    }
}

/// Get the PID of the process listening on a given port using lsof
fn get_pid_on_port(port: u16) -> Option<u32> {
    let output = Command::new("lsof")
        .args(["-i", &format!(":{}", port), "-t", "-sTCP:LISTEN"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    // lsof -t returns just the PID(s), one per line
    // Take the first one if there are multiple
    stdout
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<u32>().ok())
}

/// Parse the PID from launchctl list output
#[cfg(test)]
fn parse_launchctl_pid(output: &str) -> Option<u32> {
    // launchctl list output format:
    // {
    //     "PID" = 12345;
    //     ...
    // }
    // or just a line like: 12345	0	com.burd.proxy
    for line in output.lines() {
        let line = line.trim();
        if line.starts_with('"') && line.contains("PID") {
            // JSON-ish format
            if let Some(pid_str) = line.split('=').nth(1) {
                let pid_str = pid_str.trim().trim_end_matches(';').trim();
                if let Ok(pid) = pid_str.parse::<u32>() {
                    return Some(pid);
                }
            }
        } else if let Some(first) = line.split_whitespace().next() {
            // Tab-separated format
            if first != "-" {
                if let Ok(pid) = first.parse::<u32>() {
                    return Some(pid);
                }
            }
        }
    }
    None
}

/// Start the launchd daemon
pub fn start() -> Result<(), String> {
    if !is_installed() {
        return Err("Proxy daemon is not installed".to_string());
    }

    // Try using helper if available
    if HelperClient::is_running() {
        let response = HelperClient::send_request(HelperRequest::StartProxyDaemon)?;
        if response.success {
            return Ok(());
        }
        // Fall through to osascript if helper failed
    }

    // Fall back to osascript
    // Use osascript to run launchctl with admin privileges for system domain
    let script = format!(
        r#"do shell script "launchctl kickstart system/{}" with administrator privileges"#,
        PROXY_IDENTIFIER
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to start daemon: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("(-128)") {
            return Err("User cancelled the authorization".to_string());
        }
        return Err(format!("Failed to start daemon: {}", stderr));
    }

    Ok(())
}

/// Restart the launchd daemon
pub fn restart() -> Result<(), String> {
    if !is_installed() {
        return Err("Proxy daemon is not installed".to_string());
    }

    // Try using helper if available
    if HelperClient::is_running() {
        let response = HelperClient::send_request(HelperRequest::RestartProxyDaemon)?;
        if response.success {
            return Ok(());
        }
        // Fall through to osascript if helper failed
    }

    // Fall back to osascript
    // Use kickstart -k to kill and restart in one command
    let script = format!(
        r#"do shell script "launchctl kickstart -k system/{}" with administrator privileges"#,
        PROXY_IDENTIFIER
    );

    let output = Command::new("osascript")
        .arg("-e")
        .arg(&script)
        .output()
        .map_err(|e| format!("Failed to restart daemon: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("User canceled") || stderr.contains("(-128)") {
            return Err("User cancelled the authorization".to_string());
        }
        return Err(format!("Failed to restart daemon: {}", stderr));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plist_generation() {
        let plist = generate_plist();
        assert!(plist.contains("com.burd.proxy"));
        assert!(plist.contains("caddy"));
        assert!(plist.contains("run"));
        assert!(plist.contains("--config"));
        assert!(plist.contains("Caddyfile"));
        assert!(plist.contains("--adapter"));
        assert!(plist.contains("caddyfile"));
        assert!(plist.contains("--watch"));
        // Ensure no old burd flags
        assert!(!plist.contains("--proxy-daemon"));
        assert!(!plist.contains("--tld"));
        // Verify XDG_DATA_HOME is set for consistent PKI storage (in user space)
        assert!(plist.contains("EnvironmentVariables"));
        assert!(plist.contains("XDG_DATA_HOME"));
        assert!(plist.contains("Burd/caddy-data"));
    }

    #[test]
    fn test_parse_launchctl_pid() {
        // Tab-separated format
        assert_eq!(parse_launchctl_pid("12345\t0\tcom.burd.proxy"), Some(12345));
        assert_eq!(parse_launchctl_pid("-\t0\tcom.burd.proxy"), None);

        // JSON-ish format
        assert_eq!(parse_launchctl_pid(r#""PID" = 12345;"#), Some(12345));
    }
}
