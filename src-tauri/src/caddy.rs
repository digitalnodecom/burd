//! Caddy Server Integration
//!
//! This module manages the Caddy web server for HTTPS reverse proxy.
//! Caddy provides automatic local HTTPS with its built-in CA.

use crate::config::get_app_dir;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

/// Get the data directory for Burd app
fn get_data_dir() -> Result<PathBuf, String> {
    get_app_dir()
}

/// Get the path to the main Caddyfile (in user space)
pub fn get_caddyfile_path() -> PathBuf {
    get_data_dir().unwrap_or_else(|_| PathBuf::from("/tmp")).join("Caddyfile")
}

/// Get the path to the domains directory (in user space)
pub fn get_domains_dir() -> PathBuf {
    get_data_dir().unwrap_or_else(|_| PathBuf::from("/tmp")).join("domains")
}

/// Get the path to the logs directory (in user space)
pub fn get_logs_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Library/Logs/Burd")
}

/// Get the path where we install Caddy binary for the daemon (in user space)
pub fn get_caddy_daemon_bin() -> PathBuf {
    get_data_dir().unwrap_or_else(|_| PathBuf::from("/tmp")).join("bin").join("caddy")
}

/// Type of route for Caddyfile generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RouteType {
    /// Reverse proxy to a local port
    ReverseProxy { port: u16 },
    /// Serve static files from a directory
    FileServer { path: String, browse: bool },
}

/// Route entry for Caddyfile generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteEntry {
    pub domain: String,
    pub route_type: RouteType,
    pub instance_id: String,
    /// Whether SSL/HTTPS is enabled for this route
    #[serde(default)]
    pub ssl_enabled: bool,
}

/// Common CSS styles for error pages
const ERROR_PAGE_STYLES: &str = r#"*{margin:0;padding:0;box-sizing:border-box}
body{font-family:-apple-system,BlinkMacSystemFont,"Segoe UI",Roboto,sans-serif;min-height:100vh;display:flex;align-items:center;justify-content:center;background:#f5f5f7;color:#1d1d1f}
.container{text-align:center;padding:40px;max-width:500px}
h1{font-size:120px;font-weight:700;color:#d1d1d6;line-height:1}
h2{font-size:24px;font-weight:600;margin:20px 0 12px}
p{color:#86868b;line-height:1.5;margin-bottom:8px}
.domain{font-family:ui-monospace,monospace;background:#e5e5e5;padding:2px 8px;border-radius:4px;color:#1d1d1f}
.hint{margin-top:24px;padding:16px;background:#fff;border-radius:12px;border:1px solid #e5e5e5}
.hint p{margin:0;font-size:14px}
@media(prefers-color-scheme:dark){
body{background:#1c1c1e;color:#f5f5f7}
h1{color:#3a3a3c}
p{color:#98989d}
.domain{background:#2c2c2e;color:#f5f5f7}
.hint{background:#2c2c2e;border-color:#3a3a3c}
}"#;

/// Generate a styled error page HTML
fn get_error_html(code: u16, title: &str, body_content: &str) -> String {
    format!(r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>{code} - {title}</title>
<style>{styles}</style>
</head>
<body>
<div class="container">
<h1>{code}</h1>
<h2>{title}</h2>
{body_content}
</div>
</body>
</html>"#, code = code, title = title, styles = ERROR_PAGE_STYLES, body_content = body_content)
}

/// Generate a styled 502 error page HTML for when the backend service is not running
fn get_502_error_html(domain: &str, port: u16) -> String {
    let body = format!(
        r#"<p>Could not connect to <span class="domain">localhost:{port}</span></p>
<p>The backend for <span class="domain">{domain}</span> is not responding.</p>
<div class="hint">
<p>Start the instance in <strong>Burd</strong> to access this site.</p>
</div>"#,
        domain = domain, port = port
    );
    get_error_html(502, "Service Not Running", &body)
}

/// Generate a styled 503 error page HTML for when the service is temporarily unavailable
fn get_503_error_html(domain: &str, port: u16) -> String {
    let body = format!(
        r#"<p>The service at <span class="domain">localhost:{port}</span> is temporarily unavailable.</p>
<p>The backend for <span class="domain">{domain}</span> may be overloaded.</p>
<div class="hint">
<p>Please try again in a few moments.</p>
</div>"#,
        domain = domain, port = port
    );
    get_error_html(503, "Service Unavailable", &body)
}

/// Generate a styled 504 error page HTML for when the backend times out
fn get_504_error_html(domain: &str, port: u16) -> String {
    let body = format!(
        r#"<p>Request to <span class="domain">localhost:{port}</span> timed out.</p>
<p>The backend for <span class="domain">{domain}</span> took too long to respond.</p>
<div class="hint">
<p>The server may be processing a heavy request. Try again later.</p>
</div>"#,
        domain = domain, port = port
    );
    get_error_html(504, "Gateway Timeout", &body)
}

/// Generate a styled 404 error page HTML for when a file is not found
fn get_404_error_html(domain: &str) -> String {
    let body = format!(
        r#"<p>The requested page on <span class="domain">{domain}</span> could not be found.</p>
<div class="hint">
<p>Check that the URL is correct or return to the homepage.</p>
</div>"#,
        domain = domain
    );
    get_error_html(404, "Page Not Found", &body)
}

impl RouteEntry {
    /// Create a reverse proxy route entry
    pub fn reverse_proxy(domain: String, port: u16, instance_id: String, ssl_enabled: bool) -> Self {
        Self {
            domain,
            route_type: RouteType::ReverseProxy { port },
            instance_id,
            ssl_enabled,
        }
    }

    /// Create a file server route entry
    pub fn file_server(domain: String, path: String, browse: bool, instance_id: String, ssl_enabled: bool) -> Self {
        Self {
            domain,
            route_type: RouteType::FileServer { path, browse },
            instance_id,
            ssl_enabled,
        }
    }
}

/// Routes configuration (for Caddyfile generation)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RoutesConfig {
    pub tld: String,
    pub routes: HashMap<String, RouteEntry>,
}

/// Get the path to the Caddy binary in the app's data directory
/// This is for the app's bundled/downloaded copy of Caddy
pub fn get_caddy_binary_path() -> Result<PathBuf, String> {
    let data_dir = get_data_dir()?;
    let caddy_dir = data_dir.join("binaries").join("caddy");

    // Find the latest version directory
    if caddy_dir.exists() {
        let mut versions: Vec<_> = fs::read_dir(&caddy_dir)
            .map_err(|e| format!("Failed to read caddy directory: {}", e))?
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect();

        versions.sort_by_key(|b| std::cmp::Reverse(b.file_name()));

        if let Some(latest) = versions.first() {
            let binary = latest.path().join("caddy");
            if binary.exists() {
                return Ok(binary);
            }
        }
    }

    Err("Caddy binary not found. Please download Caddy first.".to_string())
}

/// Check if Caddy is installed (binary exists)
pub fn is_caddy_installed() -> bool {
    get_caddy_binary_path().is_ok()
}

/// Get the Caddy version
pub fn get_caddy_version() -> Result<String, String> {
    let binary = get_caddy_binary_path()?;

    let output = Command::new(&binary)
        .arg("version")
        .output()
        .map_err(|e| format!("Failed to run caddy: {}", e))?;

    if !output.status.success() {
        return Err("Failed to get Caddy version".to_string());
    }

    let version = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("unknown")
        .to_string();

    Ok(version)
}

/// Generate main Caddyfile content (global config + import + catch-all)
pub fn generate_main_caddyfile(tld: &str) -> String {
    format!(r#"{{
    # Disable admin API in daemon mode for security
    admin off
    # Configure internal CA with custom name
    pki {{
        ca local {{
            name "Burd CA Self Signed CN"
        }}
    }}
    # Enable JSON access logging for log aggregation
    log {{
        output file "{logs_dir}/caddy-access.json" {{
            mode 0644
            roll_size 50mb
            roll_keep 5
        }}
        format json
    }}
}}

# Import all domain configurations
import domains/*.caddy

# Default catch-all for *.{tld} (must be last) - HTTP only
# Note: No HTTPS catch-all to avoid generating a wildcard certificate
http://*.{tld} {{
    respond "No service configured for this domain" 404
}}
"#, tld = tld, logs_dir = get_logs_dir().display())
}

/// Generate content for a single domain config file
/// When ssl_enabled is true, generates both HTTP and HTTPS blocks
pub fn generate_domain_config(route: &RouteEntry) -> String {
    match &route.route_type {
        RouteType::ReverseProxy { port } => {
            // Generate error pages for common proxy errors
            let error_502 = get_502_error_html(&route.domain, *port).replace('`', "\\`");
            let error_503 = get_503_error_html(&route.domain, *port).replace('`', "\\`");
            let error_504 = get_504_error_html(&route.domain, *port).replace('`', "\\`");

            if route.ssl_enabled {
                // Generate both HTTP and HTTPS blocks
                format!(r#"# Route: {instance_id}
http://{domain} {{
    reverse_proxy localhost:{port} {{
        header_up X-Forwarded-Proto http
        header_up X-Forwarded-Port 80
    }}
    handle_errors {{
        @502 expression `{{http.error.status_code}} == 502`
        @503 expression `{{http.error.status_code}} == 503`
        @504 expression `{{http.error.status_code}} == 504`
        header @502 Content-Type text/html
        header @503 Content-Type text/html
        header @504 Content-Type text/html
        respond @502 `{error_502}` 502
        respond @503 `{error_503}` 503
        respond @504 `{error_504}` 504
    }}
}}

https://{domain} {{
    tls internal
    reverse_proxy localhost:{port} {{
        header_up X-Forwarded-Proto https
        header_up X-Forwarded-Port 443
    }}
    handle_errors {{
        @502 expression `{{http.error.status_code}} == 502`
        @503 expression `{{http.error.status_code}} == 503`
        @504 expression `{{http.error.status_code}} == 504`
        header @502 Content-Type text/html
        header @503 Content-Type text/html
        header @504 Content-Type text/html
        respond @502 `{error_502}` 502
        respond @503 `{error_503}` 503
        respond @504 `{error_504}` 504
    }}
}}
"#,
                    domain = route.domain,
                    port = port,
                    instance_id = route.instance_id,
                    error_502 = error_502,
                    error_503 = error_503,
                    error_504 = error_504
                )
            } else {
                // HTTP only
                format!(r#"# Route: {instance_id}
http://{domain} {{
    reverse_proxy localhost:{port} {{
        header_up X-Forwarded-Proto http
        header_up X-Forwarded-Port 80
    }}
    handle_errors {{
        @502 expression `{{http.error.status_code}} == 502`
        @503 expression `{{http.error.status_code}} == 503`
        @504 expression `{{http.error.status_code}} == 504`
        header @502 Content-Type text/html
        header @503 Content-Type text/html
        header @504 Content-Type text/html
        respond @502 `{error_502}` 502
        respond @503 `{error_503}` 503
        respond @504 `{error_504}` 504
    }}
}}
"#,
                    domain = route.domain,
                    port = port,
                    instance_id = route.instance_id,
                    error_502 = error_502,
                    error_503 = error_503,
                    error_504 = error_504
                )
            }
        }
        RouteType::FileServer { path, browse } => {
            let browse_directive = if *browse { "\n        browse" } else { "" };
            let error_404 = get_404_error_html(&route.domain).replace('`', "\\`");

            if route.ssl_enabled {
                // Generate both HTTP and HTTPS blocks
                format!(r#"# Route: {instance_id} (Static Files)
http://{domain} {{
    root * "{path}"
    file_server {{{browse_directive}
    }}
    handle_errors {{
        @404 expression `{{http.error.status_code}} == 404`
        header @404 Content-Type text/html
        respond @404 `{error_404}` 404
    }}
}}

https://{domain} {{
    tls internal
    root * "{path}"
    file_server {{{browse_directive}
    }}
    handle_errors {{
        @404 expression `{{http.error.status_code}} == 404`
        header @404 Content-Type text/html
        respond @404 `{error_404}` 404
    }}
}}
"#,
                    domain = route.domain,
                    path = path,
                    browse_directive = browse_directive,
                    instance_id = route.instance_id,
                    error_404 = error_404
                )
            } else {
                // HTTP only
                format!(r#"# Route: {instance_id} (Static Files)
http://{domain} {{
    root * "{path}"
    file_server {{{browse_directive}
    }}
    handle_errors {{
        @404 expression `{{http.error.status_code}} == 404`
        header @404 Content-Type text/html
        respond @404 `{error_404}` 404
    }}
}}
"#,
                    domain = route.domain,
                    path = path,
                    browse_directive = browse_directive,
                    instance_id = route.instance_id,
                    error_404 = error_404
                )
            }
        }
    }
}

/// Get the filename for a domain config file
pub fn get_domain_filename(domain: &str) -> String {
    format!("{}.caddy", domain)
}

/// Get the full path for a domain config file
pub fn get_domain_filepath(domain: &str) -> PathBuf {
    get_domains_dir().join(get_domain_filename(domain))
}

/// Write a file to the Burd config directory (user space, no privileges needed)
fn write_file(path: &PathBuf, content: &str) -> Result<(), String> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {:?}: {}", parent, e))?;
    }

    fs::write(path, content)
        .map_err(|e| format!("Failed to write file {:?}: {}", path, e))
}

/// Delete a file from the Burd config directory (user space, no privileges needed)
fn delete_file(path: &PathBuf) -> Result<(), String> {
    // If file doesn't exist, nothing to do
    if !path.exists() {
        return Ok(());
    }

    fs::remove_file(path)
        .map_err(|e| format!("Failed to delete file {:?}: {}", path, e))
}

/// Write the main Caddyfile (global config + import + catch-all)
pub fn write_main_caddyfile(tld: &str) -> Result<(), String> {
    let content = generate_main_caddyfile(tld);
    write_file(&get_caddyfile_path(), &content)
}

/// Write a single domain config file
pub fn write_domain_file(route: &RouteEntry) -> Result<(), String> {
    let content = generate_domain_config(route);
    let path = get_domain_filepath(&route.domain);
    write_file(&path, &content)?;

    // Touch main Caddyfile to trigger Caddy's --watch reload
    touch_caddyfile()
}

/// Write raw content to a domain config file (for custom edits)
pub fn write_domain_config_raw(path: &PathBuf, content: &str) -> Result<(), String> {
    write_file(path, content)
}

/// Write all domain files and the main Caddyfile
/// This replaces the old write_caddyfile function for full sync
pub fn write_caddyfile(tld: &str, routes: &[RouteEntry]) -> Result<(), String> {
    // First, ensure domains directory exists and write main Caddyfile
    write_main_caddyfile(tld)?;

    // Write each domain file
    for route in routes {
        write_domain_file(route)?;
    }

    // Clean up orphaned domain files (files that exist but aren't in routes)
    cleanup_orphaned_domain_files(routes)?;

    Ok(())
}

/// Remove domain files that are no longer in the routes list
fn cleanup_orphaned_domain_files(routes: &[RouteEntry]) -> Result<(), String> {
    let domains_dir = get_domains_dir();

    // If domains directory doesn't exist, nothing to clean
    if !domains_dir.exists() {
        return Ok(());
    }

    // Safety: don't delete anything if routes is empty
    // An empty routes list likely indicates initialization issues, not intentional removal
    if routes.is_empty() {
        return Ok(());
    }

    // Get set of current domain filenames
    let current_domains: std::collections::HashSet<String> = routes
        .iter()
        .map(|r| get_domain_filename(&r.domain))
        .collect();

    // Read existing files in domains directory
    let entries = fs::read_dir(&domains_dir)
        .map_err(|e| format!("Failed to read domains directory: {}", e))?;

    for entry in entries.flatten() {
        let filename = entry.file_name().to_string_lossy().to_string();
        if filename.ends_with(".caddy") && !current_domains.contains(&filename) {
            // This file is orphaned, delete it
            delete_file(&entry.path())?;
        }
    }

    Ok(())
}

/// Delete a specific domain config file
/// Used by CLI commands that don't have access to the proxy server
pub fn delete_domain_file(domain: &str) -> Result<(), String> {
    let path = get_domain_filepath(domain);

    if path.exists() {
        delete_file(&path)?;
        // Touch main Caddyfile to trigger Caddy's --watch reload
        touch_caddyfile()?;
    }

    Ok(())
}

/// Read the current Caddyfile content
pub fn read_caddyfile() -> Result<String, String> {
    let caddyfile_path = get_caddyfile_path();

    if !caddyfile_path.exists() {
        return Ok(String::new());
    }

    fs::read_to_string(&caddyfile_path)
        .map_err(|e| format!("Failed to read Caddyfile: {}", e))
}

/// Touch the main Caddyfile to trigger Caddy's --watch reload
///
/// This is needed because Caddy's --watch flag only watches the main config file,
/// not imported files like domains/*.caddy. By touching the main Caddyfile after
/// writing domain files, we trigger Caddy to reload and pick up all changes.
pub fn touch_caddyfile() -> Result<(), String> {
    let caddyfile_path = get_caddyfile_path();

    if !caddyfile_path.exists() {
        return Ok(()); // Nothing to touch
    }

    // Re-read and re-write the file to update its timestamp
    let content = fs::read_to_string(&caddyfile_path)
        .map_err(|e| format!("Failed to read Caddyfile: {}", e))?;
    write_file(&caddyfile_path, &content)?;

    Ok(())
}

/// Copy Caddy binary to the daemon location (user space)
/// This is needed because the launchd daemon needs a fixed path to the binary
pub fn install_caddy_for_daemon() -> Result<(), String> {
    let source = get_caddy_binary_path()?;
    let dest = get_caddy_daemon_bin();

    // Ensure parent directory exists
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create directory {:?}: {}", parent, e))?;
    }

    // Copy the binary
    fs::copy(&source, &dest)
        .map_err(|e| format!("Failed to copy Caddy binary: {}", e))?;

    // Make it executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dest, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("Failed to set permissions: {}", e))?;
    }

    Ok(())
}

/// Check if Caddy is installed for the daemon
pub fn is_daemon_caddy_installed() -> bool {
    get_caddy_daemon_bin().exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_main_caddyfile() {
        let caddyfile = generate_main_caddyfile("burd");

        assert!(caddyfile.contains("admin off"));
        assert!(caddyfile.contains("Burd CA Self Signed CN"));
        assert!(caddyfile.contains("import domains/*.caddy"));
        assert!(caddyfile.contains("http://*.burd"));
        // Note: No HTTPS catch-all to avoid generating a wildcard certificate
        assert!(!caddyfile.contains("https://*.burd"));
        // Import should come before catch-all
        let import_pos = caddyfile.find("import domains").unwrap();
        let catchall_pos = caddyfile.find("*.burd").unwrap();
        assert!(import_pos < catchall_pos, "Import should come before catch-all");
    }

    #[test]
    fn test_generate_domain_config_reverse_proxy_no_ssl() {
        let route = RouteEntry::reverse_proxy("api.burd".to_string(), 7700, "test-1".to_string(), false);
        let config = generate_domain_config(&route);

        assert!(config.contains("http://api.burd"));
        assert!(config.contains("localhost:7700"));
        assert!(config.contains("reverse_proxy"));
        assert!(!config.contains("https://"));
        assert!(!config.contains("tls internal"));
        // Custom error pages for 502, 503, 504
        assert!(config.contains("handle_errors"));
        assert!(config.contains("@502"));
        assert!(config.contains("@503"));
        assert!(config.contains("@504"));
        assert!(config.contains("Service Not Running"));
        assert!(config.contains("Service Unavailable"));
        assert!(config.contains("Gateway Timeout"));
    }

    #[test]
    fn test_generate_domain_config_reverse_proxy_with_ssl() {
        let route = RouteEntry::reverse_proxy("api.burd".to_string(), 7700, "test-1".to_string(), true);
        let config = generate_domain_config(&route);

        assert!(config.contains("http://api.burd"));
        assert!(config.contains("https://api.burd"));
        assert!(config.contains("tls internal"));
        assert!(config.contains("localhost:7700"));
        // Custom error pages for both HTTP and HTTPS (2 handle_errors blocks)
        assert_eq!(config.matches("handle_errors").count(), 2);
        // Each block has 502, 503, 504 error handling
        assert!(config.contains("@502"));
        assert!(config.contains("@503"));
        assert!(config.contains("@504"));
    }

    #[test]
    fn test_generate_domain_config_file_server() {
        let route = RouteEntry::file_server(
            "static.burd".to_string(),
            "/var/www/html".to_string(),
            true,
            "test-static".to_string(),
            false
        );
        let config = generate_domain_config(&route);

        assert!(config.contains("http://static.burd"));
        assert!(config.contains("file_server"));
        assert!(config.contains("root * \"/var/www/html\""));
        assert!(config.contains("browse"));
        assert!(!config.contains("https://"));
        // FileServer has 404 error handling
        assert!(config.contains("handle_errors"));
        assert!(config.contains("@404"));
        assert!(config.contains("Page Not Found"));
    }

    #[test]
    fn test_generate_domain_config_file_server_with_ssl() {
        let route = RouteEntry::file_server(
            "static.burd".to_string(),
            "/var/www/html".to_string(),
            true,
            "test-static".to_string(),
            true
        );
        let config = generate_domain_config(&route);

        assert!(config.contains("http://static.burd"));
        assert!(config.contains("https://static.burd"));
        assert!(config.contains("tls internal"));
        assert!(config.contains("file_server"));
    }

    #[test]
    fn test_generate_domain_config_file_server_no_browse() {
        let route = RouteEntry::file_server(
            "static.burd".to_string(),
            "/var/www/html".to_string(),
            false,
            "test-static".to_string(),
            false
        );
        let config = generate_domain_config(&route);

        assert!(config.contains("file_server"));
        assert!(!config.contains("browse"));
    }

    #[test]
    fn test_get_domain_filename() {
        assert_eq!(get_domain_filename("api.burd"), "api.burd.caddy");
        assert_eq!(get_domain_filename("my-app.burd"), "my-app.burd.caddy");
    }

    #[test]
    fn test_get_domain_filepath() {
        let path = get_domain_filepath("api.burd");
        let path_str = path.to_str().unwrap();
        // Path should be in user space and end with the domain file
        assert!(path_str.ends_with("Burd/domains/api.burd.caddy"));
    }
}
