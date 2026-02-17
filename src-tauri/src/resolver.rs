//! macOS Resolver Integration
//!
//! Manages the `/etc/resolver/<tld>` file that tells macOS to route
//! custom TLD domain queries to our local DNS server.

use crate::domain::DEFAULT_DNS_PORT;
use crate::helper_client::{HelperClient, HelperRequest};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Path to the resolver directory on macOS
const RESOLVER_DIR: &str = "/etc/resolver";

/// Get the path to the resolver file for a TLD
pub fn resolver_file_path(tld: &str) -> PathBuf {
    PathBuf::from(RESOLVER_DIR).join(tld)
}

/// Check if a resolver file is installed for the given TLD
pub fn is_installed(tld: &str) -> bool {
    resolver_file_path(tld).exists()
}

/// Get the content that should be in the resolver file
fn resolver_content(tld: &str, dns_port: u16) -> String {
    format!(
        "# Burd DNS resolver for .{} domains\n\
         # This file was created by the Burd application\n\
         nameserver 127.0.0.1\n\
         port {}\n",
        tld, dns_port
    )
}

/// Install the resolver file using osascript for privilege escalation
///
/// This will prompt the user for their password via the standard macOS dialog.
pub fn install(tld: &str, dns_port: u16) -> Result<(), String> {
    let path = resolver_file_path(tld);

    // Check if already installed with correct port
    if let Ok(content) = fs::read_to_string(&path) {
        if content.contains(&format!("port {}", dns_port)) {
            return Ok(()); // Already correctly configured
        }
    }

    // Try using helper if available
    if HelperClient::is_running() {
        let response = HelperClient::send_request(HelperRequest::InstallResolver {
            tld: tld.to_string(),
            dns_port,
        })?;
        if response.success {
            return Ok(());
        }
        // Fall through to osascript if helper failed
    }

    // Fall back to osascript
    let content = resolver_content(tld, dns_port);

    // Create the resolver directory if it doesn't exist and write the file
    // We use osascript to prompt for admin privileges
    let script = format!(
        r#"do shell script "mkdir -p {} && cat > {} << 'EOF'
{}EOF" with administrator privileges"#,
        RESOLVER_DIR,
        path.display(),
        content
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
        return Err(format!("Failed to install resolver: {}", stderr));
    }

    Ok(())
}

/// Uninstall the resolver file
pub fn uninstall(tld: &str) -> Result<(), String> {
    if !is_installed(tld) {
        return Ok(()); // Already uninstalled
    }

    // Try using helper if available
    if HelperClient::is_running() {
        let response = HelperClient::send_request(HelperRequest::UninstallResolver {
            tld: tld.to_string(),
        })?;
        if response.success {
            return Ok(());
        }
        // Fall through to osascript if helper failed
    }

    // Fall back to osascript
    let script = format!(
        r#"do shell script "rm -f {}" with administrator privileges"#,
        resolver_file_path(tld).display()
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
        return Err(format!("Failed to uninstall resolver: {}", stderr));
    }

    Ok(())
}

/// Get the current resolver configuration if installed
pub fn get_current_config(tld: &str) -> Option<ResolverConfig> {
    let content = fs::read_to_string(resolver_file_path(tld)).ok()?;

    let mut port = DEFAULT_DNS_PORT;

    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("port ") {
            if let Ok(p) = line.strip_prefix("port ").unwrap_or("").trim().parse() {
                port = p;
            }
        }
    }

    Some(ResolverConfig { port })
}

/// Resolver configuration
#[derive(Debug, Clone)]
pub struct ResolverConfig {
    pub port: u16,
}

/// Flush the DNS cache on macOS
///
/// This ensures the system picks up changes to the resolver configuration.
pub fn flush_dns_cache() -> Result<(), String> {
    // macOS uses dscacheutil and mDNSResponder
    Command::new("dscacheutil")
        .arg("-flushcache")
        .output()
        .map_err(|e| format!("Failed to flush DNS cache: {}", e))?;

    // Also try to restart mDNSResponder (may require privileges)
    let _ = Command::new("killall")
        .args(["-HUP", "mDNSResponder"])
        .output();

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolver_file_path() {
        let path = resolver_file_path("burd");
        assert_eq!(path.to_string_lossy(), "/etc/resolver/burd");
    }

    #[test]
    fn test_resolver_content() {
        let content = resolver_content("burd", 5354);
        assert!(content.contains("nameserver 127.0.0.1"));
        assert!(content.contains("port 5354"));
        assert!(content.contains(".burd"));
    }
}
