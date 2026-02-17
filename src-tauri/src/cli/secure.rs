//! Secure/Unsecure CLI commands
//!
//! Commands for enabling/disabling HTTPS for domains from the command line.

use crate::caddy;
use crate::config::ConfigStore;
use std::env;

/// Enable HTTPS for a domain
///
/// If no domain is specified, uses the current directory's domain.
pub fn run_secure(name: Option<String>) -> Result<(), String> {
    set_domain_ssl(name, true)
}

/// Disable HTTPS for a domain
///
/// If no domain is specified, uses the current directory's domain.
pub fn run_unsecure(name: Option<String>) -> Result<(), String> {
    set_domain_ssl(name, false)
}

/// Set SSL status for a domain
fn set_domain_ssl(name: Option<String>, ssl_enabled: bool) -> Result<(), String> {
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

    // Check if already in desired state
    if domain.ssl_enabled == ssl_enabled {
        if ssl_enabled {
            println!("SSL is already enabled for {}.{}", subdomain, config.tld);
        } else {
            println!("SSL is already disabled for {}.{}", subdomain, config.tld);
        }
        return Ok(());
    }

    // Update SSL setting
    config_store.update_domain_ssl(domain.id, ssl_enabled)?;

    // Regenerate Caddyfile
    regenerate_caddyfile(&config_store)?;

    if ssl_enabled {
        println!("Enabled SSL for {}.{}", subdomain, config.tld);
        println!();
        println!("  https://{}.{}", subdomain, config.tld);
    } else {
        println!("Disabled SSL for {}.{}", subdomain, config.tld);
        println!();
        println!("  http://{}.{}", subdomain, config.tld);
    }

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

/// Regenerate Caddyfile with current config
fn regenerate_caddyfile(config_store: &ConfigStore) -> Result<(), String> {
    let config = config_store.load()?;

    // Build routes from domains
    let routes: Vec<caddy::RouteEntry> = config
        .domains
        .iter()
        .filter_map(|domain| {
            let full_domain = format!("{}.{}", domain.subdomain, config.tld);
            match &domain.target {
                crate::config::DomainTarget::Instance(inst_id) => {
                    let instance = config.instances.iter().find(|i| i.id == *inst_id)?;
                    Some(caddy::RouteEntry::reverse_proxy(
                        full_domain,
                        instance.port,
                        domain.id.to_string(),
                        domain.ssl_enabled,
                    ))
                }
                crate::config::DomainTarget::Port(port) => Some(caddy::RouteEntry::reverse_proxy(
                    full_domain,
                    *port,
                    domain.id.to_string(),
                    domain.ssl_enabled,
                )),
                crate::config::DomainTarget::StaticFiles { path, browse } => {
                    Some(caddy::RouteEntry::file_server(
                        full_domain,
                        path.clone(),
                        *browse,
                        domain.id.to_string(),
                        domain.ssl_enabled,
                    ))
                }
            }
        })
        .collect();

    caddy::write_caddyfile(&config.tld, &routes)?;

    Ok(())
}
