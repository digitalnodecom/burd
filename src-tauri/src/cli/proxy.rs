//! Proxy CLI commands
//!
//! Commands for creating port-based proxy domains from the command line.

use crate::caddy;
use crate::config::{ConfigStore, DomainTarget};

/// Create a proxy domain to a local port
///
/// Creates a domain that proxies to localhost on the specified port.
/// Unlike 'link', this doesn't create a FrankenPHP instance.
pub fn run_proxy(name: String, port: u16) -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Strip TLD suffix if present (e.g., "myapi.burd" -> "myapi")
    let tld_suffix = format!(".{}", config.tld);
    let subdomain = slug::slugify(name.strip_suffix(&tld_suffix).unwrap_or(&name));

    // Check if domain already exists
    if config.domains.iter().any(|d| d.subdomain == subdomain) {
        return Err(format!(
            "Domain '{}.{}' already exists.\n\
             Use 'burd unproxy {}' to remove it first, or choose a different name.",
            subdomain, config.tld, subdomain
        ));
    }

    // Create domain with DomainTarget::Port (SSL enabled by default)
    config_store.create_domain_for_port(subdomain.clone(), port, true)?;
    let _ = crate::commands::auto_trust_ca_if_needed()?;

    // Regenerate Caddyfile
    regenerate_caddyfile(&config_store)?;

    println!();
    println!("Created proxy {}.{} -> localhost:{}", subdomain, config.tld, port);
    println!();
    if config.proxy_installed {
        println!("  https://{}.{}", subdomain, config.tld);
    } else {
        println!("  https://{}.{}:{}", subdomain, config.tld, config.proxy_port);
    }
    println!();

    Ok(())
}

/// Remove a proxied domain
///
/// Removes a domain created by 'burd proxy'.
pub fn run_unproxy(name: String) -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Strip TLD suffix if present
    let tld_suffix = format!(".{}", config.tld);
    let subdomain = slug::slugify(name.strip_suffix(&tld_suffix).unwrap_or(&name));

    // Find the domain
    let domain = config
        .domains
        .iter()
        .find(|d| d.subdomain == subdomain)
        .ok_or_else(|| {
            format!(
                "Domain '{}.{}' not found.\n\
                 Use 'burd proxies' to see available proxy domains.",
                subdomain, config.tld
            )
        })?;

    // Check if it's a port-based proxy (not an instance-backed domain)
    if !matches!(domain.target, DomainTarget::Port(_)) {
        return Err(format!(
            "'{}.{}' is not a proxy domain.\n\
             Use 'burd unlink' to remove linked sites.",
            subdomain, config.tld
        ));
    }

    let full_domain = format!("{}.{}", subdomain, config.tld);

    // Remove domain
    config_store.delete_domain(domain.id)?;

    // Delete Caddy domain file
    if let Err(e) = caddy::delete_domain_file(&full_domain) {
        eprintln!("Warning: Failed to delete domain file: {}", e);
    }

    // Regenerate Caddyfile
    regenerate_caddyfile(&config_store)?;

    println!();
    println!("Removed proxy {}", full_domain);
    println!();

    Ok(())
}

/// List all proxied domains
///
/// Shows domains created via 'burd proxy' (port-based proxies).
pub fn run_proxies() -> Result<(), String> {
    let config_store = ConfigStore::new()?;
    let config = config_store.load()?;

    // Find all port-based domains
    let proxy_domains: Vec<_> = config
        .domains
        .iter()
        .filter_map(|d| {
            if let DomainTarget::Port(port) = d.target {
                Some((d, port))
            } else {
                None
            }
        })
        .collect();

    if proxy_domains.is_empty() {
        println!("No proxy domains found.");
        println!();
        println!("Create one with: burd proxy <name> <port>");
        return Ok(());
    }

    println!("Proxy Domains:");
    println!();

    for (domain, port) in proxy_domains {
        let full_domain = format!("{}.{}", domain.subdomain, config.tld);
        let ssl_status = if domain.ssl_enabled { "SSL" } else { "HTTP" };

        println!("  {} -> localhost:{} ({})", full_domain, port, ssl_status);
    }

    println!();
    Ok(())
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
                DomainTarget::Instance(inst_id) => {
                    let instance = config.instances.iter().find(|i| i.id == *inst_id)?;
                    Some(caddy::RouteEntry::reverse_proxy(
                        full_domain,
                        instance.port,
                        domain.id.to_string(),
                        domain.ssl_enabled,
                    ))
                }
                DomainTarget::Port(port) => Some(caddy::RouteEntry::reverse_proxy(
                    full_domain,
                    *port,
                    domain.id.to_string(),
                    domain.ssl_enabled,
                )),
                DomainTarget::StaticFiles { path, browse } => {
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
