//! Open CLI command
//!
//! Opens a domain in the default browser.

use crate::config::ConfigStore;
use std::env;
use std::process::Command;

/// Open a site in the default browser
///
/// If no domain is specified, uses the current directory's domain.
pub fn run_open(name: Option<String>) -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Determine subdomain to look for
    let subdomain = match name {
        Some(n) => {
            // Strip TLD suffix if present (e.g., "hello.burd" -> "hello")
            let tld_suffix = format!(".{}", config.tld);
            let stripped = n.strip_suffix(&tld_suffix).unwrap_or(&n);
            slug::slugify(stripped)
        }
        None => {
            // Find domain for current directory
            find_current_directory_subdomain(&config)?
        }
    };

    // Find the domain
    let domain = config
        .domains
        .iter()
        .find(|d| d.subdomain == subdomain)
        .ok_or_else(|| {
            format!(
                "Domain '{}.{}' not found.\nUse 'burd links' to see available domains.",
                subdomain, config.tld
            )
        })?;

    // Build URL based on SSL status and proxy installation
    let url = if config.proxy_installed {
        if domain.ssl_enabled {
            format!("https://{}.{}", subdomain, config.tld)
        } else {
            format!("http://{}.{}", subdomain, config.tld)
        }
    } else {
        format!("http://{}.{}:{}", subdomain, config.tld, config.proxy_port)
    };

    // Open in default browser using macOS `open` command
    Command::new("open")
        .arg(&url)
        .spawn()
        .map_err(|e| format!("Failed to open browser: {}", e))?;

    println!("Opening {} in browser...", url);

    Ok(())
}

/// Find the subdomain for the current directory
fn find_current_directory_subdomain(config: &crate::config::Config) -> Result<String, String> {
    let current_dir =
        env::current_dir().map_err(|e| format!("Failed to get current directory: {}", e))?;
    let document_root = current_dir.to_string_lossy().to_string();

    // Find instance with matching document_root
    let instance = config
        .instances
        .iter()
        .find(|i| {
            i.config
                .get("document_root")
                .and_then(|v| v.as_str())
                .map(|dr| dr == document_root)
                .unwrap_or(false)
        })
        .ok_or_else(|| {
            "No linked domain found for current directory.\n\
                 Use 'burd link' first or specify a domain name."
                .to_string()
        })?;

    // Find domain for this instance
    let domain = config
        .domains
        .iter()
        .find(|d| d.routes_to_instance(&instance.id))
        .ok_or_else(|| "Instance has no associated domain".to_string())?;

    Ok(domain.subdomain.clone())
}
